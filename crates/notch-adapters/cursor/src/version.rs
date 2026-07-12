//! Cursor version detection from hook payloads and installed config.

/// Profile returned by [`detect_version`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorVersionProfile {
    /// Hooks schema v1 with a semver Cursor app version we recognize.
    Known {
        cursor_version: String,
        hooks_schema_version: u64,
    },
    /// Unrecognized Cursor semver or missing version metadata — observation-only.
    Unknown {
        cursor_version: Option<String>,
        hooks_schema_version: Option<u64>,
    },
}

/// Minimum Cursor semver treated as fully supported for the shipped template.
const KNOWN_MIN_MAJOR: u64 = 0;
const KNOWN_MIN_MINOR: u64 = 45;
const KNOWN_MIN_PATCH: u64 = 0;
/// Major versions above this are observation-only until validated against current docs.
const KNOWN_MAX_MAJOR: u64 = 9;

/// Detect Cursor version profile from hook payload fields and optional config metadata.
///
/// Unknown versions downgrade to observation-only capabilities via [`crate::capabilities`].
pub fn detect_version(
    cursor_version: Option<&str>,
    hooks_json_version: Option<u64>,
) -> CursorVersionProfile {
    let schema = hooks_json_version.unwrap_or(1);
    let Some(raw) = cursor_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return CursorVersionProfile::Unknown {
            cursor_version: None,
            hooks_schema_version: Some(schema),
        };
    };

    if schema != 1 {
        return CursorVersionProfile::Unknown {
            cursor_version: Some(raw.to_string()),
            hooks_schema_version: Some(schema),
        };
    }

    if is_known_cursor_semver(raw) {
        CursorVersionProfile::Known {
            cursor_version: raw.to_string(),
            hooks_schema_version: schema,
        }
    } else {
        CursorVersionProfile::Unknown {
            cursor_version: Some(raw.to_string()),
            hooks_schema_version: Some(schema),
        }
    }
}

fn is_known_cursor_semver(raw: &str) -> bool {
    let mut parts = raw.split('.');
    let major = parts.next().and_then(|part| part.parse::<u64>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u64>().ok());
    let patch = parts
        .next()
        .and_then(|part| {
            part.trim_end_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .ok()
        })
        .or(Some(0));

    match (major, minor, patch) {
        (Some(major), Some(minor), Some(patch)) if major <= KNOWN_MAX_MAJOR => {
            (major, minor, patch) >= (KNOWN_MIN_MAJOR, KNOWN_MIN_MINOR, KNOWN_MIN_PATCH)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_cursor_version() {
        let profile = detect_version(Some("1.7.2"), Some(1));
        assert_eq!(
            profile,
            CursorVersionProfile::Known {
                cursor_version: "1.7.2".into(),
                hooks_schema_version: 1,
            }
        );
    }

    #[test]
    fn unknown_semver_is_observation_only_profile() {
        let profile = detect_version(Some("not-a-version"), Some(1));
        assert!(matches!(profile, CursorVersionProfile::Unknown { .. }));
    }

    #[test]
    fn missing_cursor_version_is_unknown() {
        let profile = detect_version(None, Some(1));
        assert!(matches!(
            profile,
            CursorVersionProfile::Unknown {
                cursor_version: None,
                ..
            }
        ));
    }

    #[test]
    fn semver_ordering_matches_expectations() {
        assert!((1, 7, 2) >= (0, 45, 0));
    }
}
