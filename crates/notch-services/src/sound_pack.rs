use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::ZipArchive;

use crate::sound::{MAX_ASSET_BYTES, MAX_THEME_BYTES, SoundError, SoundTheme, builtin_8_bit_theme};

pub const PACK_SCHEMA_VERSION: u16 = 1;
pub const THEME_MANIFEST_NAME: &str = "theme.json";
pub const INTEGRITY_MANIFEST_NAME: &str = "integrity.json";
pub const MAX_PACK_BYTES: u64 = MAX_THEME_BYTES + 256 * 1024;
pub const MAX_PACK_ENTRIES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPackIntegrity {
    pub schema_version: u16,
    pub algorithm: String,
    pub files: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SoundPackSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPackSignature {
    pub algorithm: String,
    pub public_key_id: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPackValidation {
    pub theme: SoundTheme,
    pub trusted: bool,
    pub installed: bool,
    pub message: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoundPackError {
    #[error("sound pack exceeds the maximum allowed size")]
    PackTooLarge,
    #[error("sound pack archive is invalid: {0}")]
    InvalidArchive(String),
    #[error("sound pack integrity manifest is missing or invalid")]
    MissingIntegrity,
    #[error("unsupported integrity algorithm: {0}")]
    UnsupportedIntegrityAlgorithm(String),
    #[error("unsupported sound pack schema version {0}")]
    UnsupportedSchema(u16),
    #[error("unsafe archive entry path: {0}")]
    UnsafeEntryPath(String),
    #[error("reserved theme id `{0}` cannot be imported or replaced")]
    ReservedThemeId(String),
    #[error("reserved asset prefix `{0}` is not allowed in imported packs")]
    ReservedAssetPrefix(String),
    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("missing pack file referenced by integrity manifest: {0}")]
    MissingPackFile(String),
    #[error("unexpected pack file not covered by integrity manifest: {0}")]
    UnexpectedPackFile(String),
    #[error("pack signatures are not supported yet")]
    UnsupportedSignature,
    #[error("sound theme manifest invalid: {0}")]
    InvalidTheme(#[from] SoundError),
    #[error("sound theme already installed at {0}")]
    ThemeAlreadyInstalled(String),
    #[error("cannot replace built-in sound theme `{0}`")]
    BuiltinThemeProtected(String),
    #[error("failed to install sound pack: {0}")]
    InstallFailed(String),
}

pub fn reserved_theme_ids() -> Vec<String> {
    vec![builtin_8_bit_theme().id]
}

pub fn is_reserved_theme_id(theme_id: &str) -> bool {
    let normalized = theme_id.trim().to_ascii_lowercase();
    normalized.starts_with("builtin.") || reserved_theme_ids().iter().any(|id| id == &normalized)
}

pub fn validate_pack_bytes(pack_bytes: &[u8]) -> Result<SoundPackValidation, SoundPackError> {
    if pack_bytes.is_empty() || pack_bytes.len() as u64 > MAX_PACK_BYTES {
        return Err(SoundPackError::PackTooLarge);
    }

    let entries = read_archive_entries(pack_bytes)?;
    let theme = parse_theme_manifest(entries.get(THEME_MANIFEST_NAME).ok_or(
        SoundPackError::InvalidArchive("theme.json is required".into()),
    )?)?;
    theme.validate()?;
    reject_reserved_theme(&theme)?;

    let integrity = parse_integrity_manifest(
        entries
            .get(INTEGRITY_MANIFEST_NAME)
            .ok_or(SoundPackError::MissingIntegrity)?,
    )?;
    if integrity.signature.is_some() {
        return Err(SoundPackError::UnsupportedSignature);
    }

    verify_integrity(&integrity, &entries)?;
    verify_theme_assets_present(&theme, &entries)?;

    Ok(SoundPackValidation {
        theme,
        trusted: true,
        installed: false,
        message: "sound pack passed validation".into(),
    })
}

pub fn install_validated_pack(
    validation: &SoundPackValidation,
    entries: &BTreeMap<String, Vec<u8>>,
    themes_root: &Path,
    reserved_theme_ids: &[String],
) -> Result<SoundPackValidation, SoundPackError> {
    reject_reserved_theme(&validation.theme)?;

    let theme_id = validation.theme.id.clone();
    if reserved_theme_ids
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(&theme_id))
    {
        return Err(SoundPackError::BuiltinThemeProtected(theme_id));
    }

    let target_dir = themes_root.join(&theme_id);
    if target_dir.exists() {
        return Err(SoundPackError::ThemeAlreadyInstalled(
            target_dir.display().to_string(),
        ));
    }

    std::fs::create_dir_all(&target_dir).map_err(|error| {
        SoundPackError::InstallFailed(format!("could not create theme directory: {error}"))
    })?;

    let mut written_paths = BTreeSet::new();
    for (path, bytes) in entries {
        if path == INTEGRITY_MANIFEST_NAME {
            continue;
        }
        let destination = target_dir.join(path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                SoundPackError::InstallFailed(format!(
                    "could not create parent directory for {path}: {error}"
                ))
            })?;
        }
        std::fs::write(&destination, bytes).map_err(|error| {
            SoundPackError::InstallFailed(format!("could not write {path}: {error}"))
        })?;
        written_paths.insert(path.clone());
    }

    if !written_paths.contains(THEME_MANIFEST_NAME) {
        let _ = std::fs::remove_dir_all(&target_dir);
        return Err(SoundPackError::InstallFailed(
            "theme.json was not written".into(),
        ));
    }

    Ok(SoundPackValidation {
        theme: validation.theme.clone(),
        trusted: validation.trusted,
        installed: true,
        message: format!("installed sound theme `{theme_id}`"),
    })
}

pub fn validate_and_install_pack_bytes(
    pack_bytes: &[u8],
    themes_root: &Path,
    reserved_theme_ids: &[String],
) -> Result<SoundPackValidation, SoundPackError> {
    let entries = read_archive_entries(pack_bytes)?;
    let validation = validate_pack_bytes(pack_bytes)?;
    install_validated_pack(&validation, &entries, themes_root, reserved_theme_ids)
}

fn read_archive_entries(pack_bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, SoundPackError> {
    let cursor = Cursor::new(pack_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| SoundPackError::InvalidArchive(error.to_string()))?;
    if archive.len() > MAX_PACK_ENTRIES {
        return Err(SoundPackError::InvalidArchive(format!(
            "too many entries (max {MAX_PACK_ENTRIES})"
        )));
    }

    let mut entries = BTreeMap::new();
    let mut total_uncompressed = 0_u64;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| SoundPackError::InvalidArchive(error.to_string()))?;
        let normalized = normalize_archive_path(file.name())?;
        if file.is_dir() || normalized.is_empty() {
            continue;
        }

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| SoundPackError::InvalidArchive(error.to_string()))?;
        if bytes.is_empty() {
            return Err(SoundPackError::InvalidArchive(format!(
                "archive entry `{normalized}` is empty"
            )));
        }
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err(SoundPackError::PackTooLarge);
        }
        total_uncompressed = total_uncompressed
            .checked_add(bytes.len() as u64)
            .ok_or(SoundPackError::PackTooLarge)?;
        if total_uncompressed > MAX_PACK_BYTES {
            return Err(SoundPackError::PackTooLarge);
        }
        if entries.insert(normalized.clone(), bytes).is_some() {
            return Err(SoundPackError::InvalidArchive(format!(
                "duplicate archive entry `{normalized}`"
            )));
        }
    }

    Ok(entries)
}

fn normalize_archive_path(raw: &str) -> Result<String, SoundPackError> {
    let replaced = raw.replace('\\', "/");
    let has_drive_prefix = replaced
        .as_bytes()
        .get(1)
        .is_some_and(|character| *character == b':');
    if replaced.is_empty() || replaced.starts_with('/') || has_drive_prefix {
        return Err(SoundPackError::UnsafeEntryPath(raw.into()));
    }
    let parts: Vec<&str> = replaced.split('/').collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(SoundPackError::UnsafeEntryPath(raw.into()));
    }
    Ok(parts.join("/"))
}

fn parse_theme_manifest(bytes: &[u8]) -> Result<SoundTheme, SoundPackError> {
    serde_json::from_slice(bytes).map_err(|error| {
        SoundPackError::InvalidTheme(SoundError::InvalidManifest(error.to_string()))
    })
}

fn parse_integrity_manifest(bytes: &[u8]) -> Result<SoundPackIntegrity, SoundPackError> {
    let integrity: SoundPackIntegrity =
        serde_json::from_slice(bytes).map_err(|_| SoundPackError::MissingIntegrity)?;
    if integrity.schema_version != PACK_SCHEMA_VERSION {
        return Err(SoundPackError::UnsupportedSchema(integrity.schema_version));
    }
    if integrity.algorithm != "sha256" {
        return Err(SoundPackError::UnsupportedIntegrityAlgorithm(
            integrity.algorithm.clone(),
        ));
    }
    if integrity.files.is_empty() {
        return Err(SoundPackError::MissingIntegrity);
    }
    Ok(integrity)
}

fn verify_integrity(
    integrity: &SoundPackIntegrity,
    entries: &BTreeMap<String, Vec<u8>>,
) -> Result<(), SoundPackError> {
    let mut declared = BTreeSet::new();
    for (path, expected) in &integrity.files {
        let normalized = normalize_archive_path(path)?;
        declared.insert(normalized.clone());
        let actual_bytes = entries
            .get(&normalized)
            .ok_or_else(|| SoundPackError::MissingPackFile(normalized.clone()))?;
        let actual = sha256_hex(actual_bytes);
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(SoundPackError::HashMismatch {
                path: normalized,
                expected: expected.clone(),
                actual,
            });
        }
    }

    for path in entries.keys() {
        if path == INTEGRITY_MANIFEST_NAME {
            continue;
        }
        if !declared.contains(path) {
            return Err(SoundPackError::UnexpectedPackFile(path.clone()));
        }
    }

    Ok(())
}

fn verify_theme_assets_present(
    theme: &SoundTheme,
    entries: &BTreeMap<String, Vec<u8>>,
) -> Result<(), SoundPackError> {
    for asset in theme.events.values() {
        if asset.path.starts_with("builtin/") {
            return Err(SoundPackError::ReservedAssetPrefix(asset.path.clone()));
        }
        let bytes = entries
            .get(&asset.path)
            .ok_or_else(|| SoundPackError::MissingPackFile(asset.path.clone()))?;
        if bytes.len() as u64 != asset.size_bytes {
            return Err(SoundPackError::InvalidTheme(SoundError::InvalidAssetSize {
                path: asset.path.clone(),
                size: asset.size_bytes,
            }));
        }
    }
    Ok(())
}

fn reject_reserved_theme(theme: &SoundTheme) -> Result<(), SoundPackError> {
    if is_reserved_theme_id(&theme.id) {
        return Err(SoundPackError::ReservedThemeId(theme.id.clone()));
    }
    Ok(())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
pub mod fixtures {
    use std::collections::BTreeMap;
    use std::io::Write;

    use super::*;
    use crate::sound::{SoundAsset, SoundEvent};

    pub fn sample_theme() -> SoundTheme {
        SoundTheme {
            schema_version: 1,
            id: "community.test-pack".into(),
            name: "Test Pack".into(),
            author: "Test Author".into(),
            events: BTreeMap::from([(
                SoundEvent::Completed,
                SoundAsset {
                    path: "sounds/completed.wav".into(),
                    size_bytes: 4,
                    duration_ms: 500,
                },
            )]),
        }
    }

    pub fn pack_entries(theme: &SoundTheme, asset_bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
        let theme_json = serde_json::to_vec(theme).expect("theme json");
        let mut files = BTreeMap::from([
            (THEME_MANIFEST_NAME.to_string(), theme_json),
            (
                theme.events[&SoundEvent::Completed].path.clone(),
                asset_bytes.to_vec(),
            ),
        ]);
        let integrity = SoundPackIntegrity {
            schema_version: PACK_SCHEMA_VERSION,
            algorithm: "sha256".into(),
            files: files
                .iter()
                .map(|(path, bytes)| (path.clone(), sha256_hex(bytes)))
                .collect(),
            signature: None,
        };
        files.insert(
            INTEGRITY_MANIFEST_NAME.to_string(),
            serde_json::to_vec(&integrity).expect("integrity json"),
        );
        files
    }

    pub fn build_zip(entries: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
        use zip::write::SimpleFileOptions;

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for (path, bytes) in entries {
                writer.start_file(path, options).expect("start zip entry");
                writer.write_all(bytes).expect("write zip entry");
            }
            writer.finish().expect("finish zip");
        }
        buffer.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{build_zip, pack_entries, sample_theme};
    use super::*;
    use crate::sound::{SoundEvent, builtin_8_bit_theme};
    use tempfile::TempDir;

    #[test]
    fn zip_roundtrip_preserves_declared_entries() {
        let theme = sample_theme();
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        let parsed = read_archive_entries(&zip).expect("archive entries");
        assert_eq!(
            parsed.keys().collect::<Vec<_>>(),
            entries.keys().collect::<Vec<_>>(),
            "parsed keys should match source entries"
        );
    }

    #[test]
    fn valid_pack_is_accepted() {
        let theme = sample_theme();
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        let validation = validate_pack_bytes(&zip).expect("valid pack");
        assert!(validation.trusted);
        assert!(!validation.installed);
        assert_eq!(validation.theme.id, "community.test-pack");
    }

    #[test]
    fn arbitrary_bytes_are_rejected() {
        assert!(matches!(
            validate_pack_bytes(b"not-a-zip"),
            Err(SoundPackError::InvalidArchive(_))
        ));
    }

    #[test]
    fn missing_integrity_is_rejected() {
        let theme = sample_theme();
        let mut entries = pack_entries(&theme, b"RIFF");
        entries.remove(INTEGRITY_MANIFEST_NAME);
        let zip = build_zip(&entries);
        assert!(matches!(
            validate_pack_bytes(&zip),
            Err(SoundPackError::MissingIntegrity)
        ));
    }

    #[test]
    fn hash_mismatch_is_rejected() {
        let theme = sample_theme();
        let mut entries = pack_entries(&theme, b"RIFF");
        entries.insert("sounds/completed.wav".into(), b"tampered".to_vec());
        let zip = build_zip(&entries);
        assert!(matches!(
            validate_pack_bytes(&zip),
            Err(SoundPackError::HashMismatch { .. })
        ));
    }

    #[test]
    fn reserved_builtin_theme_id_is_rejected() {
        let mut theme = sample_theme();
        theme.id = "builtin.custom".into();
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        assert!(matches!(
            validate_pack_bytes(&zip),
            Err(SoundPackError::ReservedThemeId(_))
        ));
    }

    #[test]
    fn reserved_asset_prefix_is_rejected() {
        let mut theme = sample_theme();
        theme.events.get_mut(&SoundEvent::Completed).unwrap().path =
            "builtin/8-bit/completed.wav".into();
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        assert!(matches!(
            validate_pack_bytes(&zip),
            Err(SoundPackError::ReservedAssetPrefix(_))
        ));
    }

    #[test]
    fn traversal_entries_are_rejected() {
        let theme = sample_theme();
        let mut entries = pack_entries(&theme, b"RIFF");
        entries.insert("../escape.wav".into(), b"RIFF".to_vec());
        let zip = build_zip(&entries);
        assert!(matches!(
            validate_pack_bytes(&zip),
            Err(SoundPackError::UnsafeEntryPath(_))
        ));
    }

    #[test]
    fn install_writes_theme_and_refuses_existing_directory() {
        let theme = sample_theme();
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        let validation = validate_pack_bytes(&zip).unwrap();
        let dir = TempDir::new().expect("tempdir");
        let reserved = reserved_theme_ids();
        let installed = install_validated_pack(
            &validation,
            &read_archive_entries(&zip).unwrap(),
            dir.path(),
            &reserved,
        )
        .expect("install");
        assert!(installed.installed);
        assert!(dir.path().join("community.test-pack/theme.json").is_file());

        assert!(matches!(
            install_validated_pack(
                &validation,
                &read_archive_entries(&zip).unwrap(),
                dir.path(),
                &reserved,
            ),
            Err(SoundPackError::ThemeAlreadyInstalled(_))
        ));
    }

    #[test]
    fn install_refuses_reserved_theme_id() {
        let mut theme = sample_theme();
        theme.id = builtin_8_bit_theme().id;
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        assert!(matches!(
            validate_pack_bytes(&zip),
            Err(SoundPackError::ReservedThemeId(_))
        ));
    }

    #[test]
    fn unsupported_signature_is_rejected() {
        let theme = sample_theme();
        let mut entries = pack_entries(&theme, b"RIFF");
        let mut integrity: SoundPackIntegrity =
            serde_json::from_slice(entries.get(INTEGRITY_MANIFEST_NAME).unwrap()).unwrap();
        integrity.signature = Some(SoundPackSignature {
            algorithm: "ed25519".into(),
            public_key_id: "test".into(),
            value: "abc".into(),
        });
        entries.insert(
            INTEGRITY_MANIFEST_NAME.into(),
            serde_json::to_vec(&integrity).unwrap(),
        );
        let zip = build_zip(&entries);
        assert!(matches!(
            validate_pack_bytes(&zip),
            Err(SoundPackError::UnsupportedSignature)
        ));
    }
}
