use std::fs;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use crate::error::ConnectorError;

/// Atomically replace `target` with `content`, using platform-specific replace semantics.
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<(), ConnectorError> {
    atomic_write_with_revalidate(path, content, || Ok(()))
}

/// Like [`atomic_write`], invoking `revalidate` immediately before temp create and replace.
pub fn atomic_write_with_revalidate<F>(
    path: &Path,
    content: &[u8],
    mut revalidate: F,
) -> Result<(), ConnectorError>
where
    F: FnMut() -> Result<(), ConnectorError>,
{
    let parent = path
        .parent()
        .ok_or_else(|| ConnectorError::Internal("target has no parent".into()))?;
    fs::create_dir_all(parent)
        .map_err(|error| ConnectorError::Internal(format!("create parent failed: {error}")))?;

    let temp_path = parent.join(format!(".{}.llm-notch.tmp", Uuid::new_v4().simple()));

    revalidate()?;
    {
        let mut temp = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| ConnectorError::Internal(format!("temp create failed: {error}")))?;
        temp.write_all(content)
            .map_err(|error| ConnectorError::Internal(format!("temp write failed: {error}")))?;
        temp.sync_all()
            .map_err(|error| ConnectorError::Internal(format!("temp sync failed: {error}")))?;
    }

    revalidate()?;
    durable_replace(&temp_path, path)?;
    Ok(())
}

/// Replace `to` with the contents of `from`, deleting `from` on success.
pub fn durable_replace(from: &Path, to: &Path) -> Result<(), ConnectorError> {
    if !to.exists() {
        fs::rename(from, to)
            .map_err(|error| ConnectorError::Internal(format!("rename failed: {error}")))?;
        return Ok(());
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW};
        use windows::core::PCWSTR;

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
            let backup = to.with_extension(format!("llm-notch.pre-replace.{}", Uuid::new_v4().simple()));
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
        fs::rename(from, to)
            .map_err(|error| ConnectorError::Internal(format!("atomic rename failed: {error}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::sha256_hex;
    use tempfile::TempDir;

    #[test]
    fn atomic_write_invokes_revalidate_before_each_mutation() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("hooks.json");
        fs::write(&target, b"old").expect("seed");
        let mut count = 0_u32;
        atomic_write_with_revalidate(&target, b"new", || {
            count += 1;
            Ok(())
        })
        .expect("write");
        assert_eq!(count, 2, "expected revalidation before temp create and replace");
        assert_eq!(fs::read(&target).expect("read"), b"new");
    }

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

    #[test]
    fn durable_replace_updates_existing_file_multiple_times() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("journal.json");
        fs::write(&target, b"v1").expect("seed");
        for version in [b"v2".as_slice(), b"v3", b"v4"] {
            let temp = dir.path().join(format!("{}.tmp", Uuid::new_v4().simple()));
            fs::write(&temp, version).expect("temp");
            durable_replace(&temp, &target).expect("replace");
            assert_eq!(fs::read(&target).expect("read"), version);
        }
    }
}
