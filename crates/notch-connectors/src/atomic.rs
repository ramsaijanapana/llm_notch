use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::ConnectorError;

/// Atomically replace `target` with `content`, using platform-specific replace semantics.
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<(), ConnectorError> {
    let parent = path
        .parent()
        .ok_or_else(|| ConnectorError::Internal("target has no parent".into()))?;
    fs::create_dir_all(parent).map_err(|error| {
        ConnectorError::Internal(format!("create parent failed: {error}"))
    })?;

    let temp_name = format!(
        ".{}.llm-notch.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
    );
    let temp_path = parent.join(temp_name);

    {
        let mut temp = fs::File::create(&temp_path).map_err(|error| {
            ConnectorError::Internal(format!("temp create failed: {error}"))
        })?;
        temp.write_all(content).map_err(|error| {
            ConnectorError::Internal(format!("temp write failed: {error}"))
        })?;
        temp.sync_all().map_err(|error| {
            ConnectorError::Internal(format!("temp sync failed: {error}"))
        })?;
    }

    replace_file(&temp_path, path)?;
    Ok(())
}

fn replace_file(from: &Path, to: &Path) -> Result<(), ConnectorError> {
    if !to.exists() {
        fs::rename(from, to).map_err(|error| {
            ConnectorError::Internal(format!("rename failed: {error}"))
        })?;
        return Ok(());
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{ReplaceFileW, REPLACEFILE_WRITE_THROUGH};

        let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain([0]).collect();
        let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain([0]).collect();

        let ok = unsafe {
            ReplaceFileW(
                PCWSTR(to_wide.as_ptr()),
                PCWSTR(from_wide.as_ptr()),
                PCWSTR::null(),
                REPLACEFILE_WRITE_THROUGH,
                None,
                None,
            )
        };
        if ok.is_err() {
            // Fallback to rename swap when ReplaceFileW fails (e.g. cross-volume).
            let backup = to.with_extension("llm-notch.pre-replace");
            let _ = fs::remove_file(&backup);
            fs::rename(to, &backup).map_err(|error| {
                ConnectorError::Internal(format!("backup rename failed: {error}"))
            })?;
            if fs::rename(from, to).is_err() {
                let _ = fs::rename(&backup, to);
                return Err(ConnectorError::Internal(
                    "atomic replace failed on Windows".into(),
                ));
            }
            let _ = fs::remove_file(&backup);
        } else {
            let _ = fs::remove_file(from);
        }
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        fs::rename(from, to).map_err(|error| {
            ConnectorError::Internal(format!("atomic rename failed: {error}"))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::sha256_hex;
    use tempfile::TempDir;

    #[test]
    fn replaces_existing_file() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("hooks.json");
        fs::write(&target, b"old").expect("seed");
        atomic_write(&target, b"new").expect("write");
        let content = fs::read(&target).expect("read");
        assert_eq!(content, b"new");
        assert_eq!(sha256_hex(&content), sha256_hex(b"new"));
    }

    #[test]
    fn creates_new_file() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("nested/hooks.json");
        atomic_write(&target, b"{}").expect("write");
        assert!(target.exists());
    }
}
