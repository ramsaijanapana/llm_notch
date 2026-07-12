//! Claude Code version detection from CLI metadata or hook payloads.

/// Profile returned by [`detect_version`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeVersionProfile {
    /// Semver Claude Code version we recognize for verified response paths.
    Known { claude_code_version: String },
    /// Unrecognized semver or missing version metadata — observation-only.
    Unknown { claude_code_version: Option<String> },
}

/// Minimum Claude Code semver treated as fully supported for the shipped template.
const KNOWN_MIN_MAJOR: u64 = 2;
const KNOWN_MIN_MINOR: u64 = 1;
const KNOWN_MIN_PATCH: u64 = 0;

/// Detect Claude Code version profile from CLI output or optional hook metadata.
///
/// Unknown versions downgrade to observation-only capabilities via [`crate::capabilities`].
pub fn detect_version(claude_code_version: Option<&str>) -> ClaudeVersionProfile {
    let Some(raw) = claude_code_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return ClaudeVersionProfile::Unknown {
            claude_code_version: None,
        };
    };

    let normalized = strip_version_prefix(raw);
    if is_known_claude_semver(normalized) {
        ClaudeVersionProfile::Known {
            claude_code_version: normalized.to_string(),
        }
    } else {
        ClaudeVersionProfile::Unknown {
            claude_code_version: Some(normalized.to_string()),
        }
    }
}

fn strip_version_prefix(raw: &str) -> &str {
    raw.strip_prefix("Claude Code ")
        .or_else(|| raw.strip_prefix("claude-code "))
        .unwrap_or(raw)
        .trim()
}

fn is_known_claude_semver(raw: &str) -> bool {
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
        (Some(major), Some(minor), Some(patch)) => {
            (major, minor, patch) >= (KNOWN_MIN_MAJOR, KNOWN_MIN_MINOR, KNOWN_MIN_PATCH)
        }
        _ => false,
    }
}

#[cfg(test)]
use std::cmp::Ordering;

#[cfg(test)]
fn compare_semver(a: (u64, u64, u64), b: (u64, u64, u64)) -> Ordering {
    a.cmp(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_claude_code_version() {
        let profile = detect_version(Some("2.1.205"));
        assert_eq!(
            profile,
            ClaudeVersionProfile::Known {
                claude_code_version: "2.1.205".into(),
            }
        );
    }

    #[test]
    fn strips_cli_prefix_before_matching() {
        let profile = detect_version(Some("Claude Code 2.1.10"));
        assert_eq!(
            profile,
            ClaudeVersionProfile::Known {
                claude_code_version: "2.1.10".into(),
            }
        );
    }

    #[test]
    fn unknown_semver_is_observation_only_profile() {
        let profile = detect_version(Some("not-a-version"));
        assert!(matches!(profile, ClaudeVersionProfile::Unknown { .. }));
    }

    #[test]
    fn missing_version_is_unknown() {
        let profile = detect_version(None);
        assert!(matches!(
            profile,
            ClaudeVersionProfile::Unknown {
                claude_code_version: None,
            }
        ));
    }

    #[test]
    fn semver_ordering_matches_expectations() {
        assert_eq!(compare_semver((2, 1, 10), (2, 1, 0)), Ordering::Greater);
    }
}
