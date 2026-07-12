use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::error::ConnectorError;

const LOCK_SUFFIX: &str = ".llm-notch.lock";
const STALE_LOCK: Duration = Duration::from_secs(120);

pub struct FileLock {
    path: PathBuf,
    _file: File,
}

impl FileLock {
    pub fn acquire(target: &Path) -> Result<Self, ConnectorError> {
        let lock_path = lock_path_for(target);
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                ConnectorError::Internal(format!("cannot create lock parent: {error}"))
            })?;
        }

        if lock_path.exists() {
            if let Ok(meta) = std::fs::metadata(&lock_path) {
                if let Ok(age) = meta.modified().and_then(|modified| {
                    SystemTime::now()
                        .duration_since(modified)
                        .map_err(|error| std::io::Error::other(error))
                }) {
                    if age > STALE_LOCK {
                        let _ = std::fs::remove_file(&lock_path);
                    }
                }
            }
        }

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    ConnectorError::LockContention
                } else {
                    ConnectorError::Internal(format!("lock create failed: {error}"))
                }
            })?;

        file.set_len(0)
            .map_err(|error| ConnectorError::Internal(format!("lock truncate failed: {error}")))?;
        (&file)
            .write_all(format!("pid={}\n", std::process::id()).as_bytes())
            .map_err(|error| ConnectorError::Internal(format!("lock write failed: {error}")))?;

        Ok(Self {
            path: lock_path,
            _file: file,
        })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn lock_path_for(target: &Path) -> PathBuf {
    let mut path = target.to_path_buf();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "target".into());
    path.set_file_name(format!("{file_name}{LOCK_SUFFIX}"));
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::TempDir;

    #[test]
    fn second_lock_fails() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("hooks.json");
        std::fs::write(&target, b"{}").expect("write");
        let _first = FileLock::acquire(&target).expect("first lock");
        let second = FileLock::acquire(&target);
        assert!(matches!(second, Err(ConnectorError::LockContention)));
    }

    #[test]
    fn lock_released_on_drop() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("hooks.json");
        std::fs::write(&target, b"{}").expect("write");
        {
            let _lock = FileLock::acquire(&target).expect("lock");
        }
        thread::sleep(Duration::from_millis(10));
        let _again = FileLock::acquire(&target).expect("reacquire");
    }
}
