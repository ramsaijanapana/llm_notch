use std::path::{Component, Path, PathBuf};

use crate::error::ConnectorError;

/// Allowlisted scope root for connector file operations.
#[derive(Debug, Clone)]
pub struct ScopeRoot {
    pub canonical: PathBuf,
    pub display_prefix: String,
}

impl ScopeRoot {
    pub fn user_home() -> Result<Self, ConnectorError> {
        let home = directories::UserDirs::new()
            .ok_or_else(|| ConnectorError::Internal("home directory unavailable".into()))?
            .home_dir()
            .to_path_buf();
        let canonical = validate_root(&home)?;
        Ok(Self {
            display_prefix: "~".into(),
            canonical,
        })
    }

    pub fn project(workspace: &Path) -> Result<Self, ConnectorError> {
        let canonical = validate_root(workspace)?;
        Ok(Self {
            display_prefix: canonical.display().to_string(),
            canonical,
        })
    }

    pub fn resolve(&self, relative: &Path) -> Result<PathBuf, ConnectorError> {
        validate_under_root(&self.canonical, relative)
    }

    pub fn display_path(&self, relative: &Path) -> String {
        let rel = relative.to_string_lossy();
        if self.display_prefix == "~" {
            format!("~/{rel}")
        } else {
            format!("{}/{}", self.display_prefix, rel)
        }
    }
}

fn validate_root(path: &Path) -> Result<PathBuf, ConnectorError> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        ConnectorError::Internal(format!(
            "cannot stat scope root {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(ConnectorError::PathEscapesScope(
            "scope root is a symlink".into(),
        ));
    }
    std::fs::canonicalize(path).map_err(|error| {
        ConnectorError::Internal(format!("cannot canonicalize scope root: {error}"))
    })
}

/// Resolve `relative` under `root`, rejecting escapes, symlinks, junctions, and reparse points.
pub fn validate_under_root(root: &Path, relative: &Path) -> Result<PathBuf, ConnectorError> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        ConnectorError::Internal(format!("cannot canonicalize scope root: {error}"))
    })?;

    if relative.is_absolute() {
        return Err(ConnectorError::PathEscapesScope(
            "absolute paths are not allowed".into(),
        ));
    }

    for component in relative.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ConnectorError::PathEscapesScope(
                    "path traversal rejected".into(),
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    let mut current = root.to_path_buf();
    for component in relative.components() {
        if let Component::Normal(part) = component {
            current.push(part);
            lstat_component(&current)?;
        }
    }

    let canonical = std::fs::canonicalize(&current).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            return error;
        }
        std::io::Error::new(error.kind(), format!("canonicalize failed: {error}"))
    });

    match canonical {
        Ok(path) => {
            if !path.starts_with(root) {
                return Err(ConnectorError::PathEscapesScope(
                    "resolved path escapes scope".into(),
                ));
            }
            Ok(path)
        }
        Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
            if !current.starts_with(root) {
                return Err(ConnectorError::PathEscapesScope(
                    "constructed path escapes scope".into(),
                ));
            }
            Ok(current)
        }
        Err(other) => Err(ConnectorError::PathEscapesScope(other.to_string())),
    }
}

fn lstat_component(path: &Path) -> Result<(), ConnectorError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            return error;
        }
        std::io::Error::new(
            error.kind(),
            format!("lstat failed for {}: {error}", path.display()),
        )
    });

    match metadata {
        Ok(meta) => {
            reject_special_file_type(path, &meta)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ConnectorError::PathEscapesScope(error.to_string())),
    }
}

fn reject_special_file_type(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), ConnectorError> {
    if metadata.file_type().is_symlink() {
        return Err(ConnectorError::PathEscapesScope(format!(
            "symlink rejected: {}",
            path.display()
        )));
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if (metadata.file_attributes() & 0x400) != 0 {
            return Err(ConnectorError::PathEscapesScope(format!(
                "reparse point rejected: {}",
                path.display()
            )));
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let file_type = metadata.file_type();
        if file_type.is_fifo() || file_type.is_socket() {
            return Err(ConnectorError::PathEscapesScope(format!(
                "special file rejected: {}",
                path.display()
            )));
        }
    }

    Ok(())
}

/// Reject hardlinks when reading/writing a regular file target.
pub fn reject_hardlink(path: &Path) -> Result<(), ConnectorError> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = std::fs::metadata(path).map_err(|error| {
        ConnectorError::Internal(format!("stat failed for {}: {error}", path.display()))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() > 1 {
            return Err(ConnectorError::PathEscapesScope(format!(
                "hardlink rejected: {}",
                path.display()
            )));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(ConnectorError::PathEscapesScope(format!(
                "reparse point rejected: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Revalidate that `expected` still resolves under `root` via `relative`, rejecting parent swaps.
pub fn revalidate_locked_target(
    root: &Path,
    relative: &Path,
    expected: &Path,
) -> Result<PathBuf, ConnectorError> {
    let resolved = validate_under_root(root, relative)?;
    let expected = std::fs::canonicalize(expected).unwrap_or_else(|_| expected.to_path_buf());
    let resolved = std::fs::canonicalize(&resolved).unwrap_or(resolved);
    if resolved != expected {
        return Err(ConnectorError::PathEscapesScope(
            "target path changed under lock".into(),
        ));
    }
    assert_parent_chain_safe(&resolved)?;
    reject_hardlink(&resolved)?;
    Ok(resolved)
}

/// Reject symlink/junction/reparse components in the parent chain of `path`.
pub fn assert_parent_chain_safe(path: &Path) -> Result<(), ConnectorError> {
    let mut current = path.parent();
    while let Some(parent) = current {
        if parent.as_os_str().is_empty() {
            break;
        }
        if parent.exists() {
            lstat_component(parent)?;
        }
        current = parent.parent();
    }
    Ok(())
}

/// Allocate a sibling backup path with an unpredictable name under the target directory.
pub fn secure_backup_path(target: &Path, timestamp: &str) -> Result<PathBuf, ConnectorError> {
    let parent = target
        .parent()
        .ok_or_else(|| ConnectorError::Internal("target has no parent".into()))?;
    assert_parent_chain_safe(parent)?;
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let backup_path = parent.join(format!(
        "{file_name}.llm-notch.bak.{timestamp}.{}",
        uuid::Uuid::new_v4().simple()
    ));
    if backup_path.exists() {
        reject_hardlink(&backup_path)?;
        return Err(ConnectorError::PathEscapesScope(
            "backup path already exists".into(),
        ));
    }
    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn rejects_parent_dir_traversal() {
        let root = TempDir::new().expect("tempdir");
        let err = validate_under_root(root.path(), Path::new("../escape.txt")).unwrap_err();
        assert!(matches!(err, ConnectorError::PathEscapesScope(_)));
    }

    #[test]
    fn rejects_symlink_component() {
        let root = TempDir::new().expect("tempdir");
        let outside = TempDir::new().expect("outside");
        let link_parent = root.path().join("nested");
        fs::create_dir_all(&link_parent).expect("mkdir");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), link_parent.join("link")).expect("symlink");
            let err =
                validate_under_root(root.path(), Path::new("nested/link/file.json")).unwrap_err();
            assert!(matches!(err, ConnectorError::PathEscapesScope(_)));
        }
    }

    #[test]
    fn allows_missing_leaf_under_root() {
        let root = TempDir::new().expect("tempdir");
        let resolved =
            validate_under_root(root.path(), Path::new(".cursor/hooks.json")).expect("resolve");
        let canonical_root = std::fs::canonicalize(root.path()).expect("canonicalize root");
        assert!(resolved.starts_with(&canonical_root));
    }
}
