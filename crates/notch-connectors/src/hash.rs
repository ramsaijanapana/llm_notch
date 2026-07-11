use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

pub fn sha256_file(path: &std::path::Path) -> Result<String, std::io::Error> {
    let bytes = std::fs::read(path)?;
    Ok(sha256_hex(&bytes))
}

/// Read file bytes once and hash the same buffer (no separate path re-read for verify).
pub fn read_and_hash(path: &std::path::Path) -> Result<(Vec<u8>, String), std::io::Error> {
    let bytes = std::fs::read(path)?;
    Ok((bytes.clone(), sha256_hex(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_and_hash_uses_same_bytes() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("data.txt");
        std::fs::write(&path, b"payload").expect("write");
        let (bytes, hash) = read_and_hash(&path).expect("read");
        assert_eq!(bytes, b"payload");
        assert_eq!(hash, sha256_hex(b"payload"));
        assert_eq!(hash, sha256_file(&path).expect("file hash"));
    }
}
