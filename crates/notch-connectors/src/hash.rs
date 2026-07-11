use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

pub fn sha256_file(path: &std::path::Path) -> Result<String, std::io::Error> {
    let bytes = std::fs::read(path)?;
    Ok(sha256_hex(&bytes))
}
