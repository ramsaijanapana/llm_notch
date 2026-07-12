//! Extensible metadata for agent integrations.
//!
//! The catalog deliberately uses validated string identifiers instead of a closed enum.
//! An entry being present does not mean an adapter is implemented: consumers must inspect
//! [`IntegrationMaturity`] and capability evidence before presenting support claims.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A stable, URL-safe agent identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidAgentId> {
        let value = value.into();
        if is_valid_id(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidAgentId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AgentId {
    type Err = InvalidAgentId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

fn is_valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.ends_with('-')
        && !value.contains("--")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAgentId(String);

impl fmt::Display for InvalidAgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid agent id {:?}: expected a lowercase ASCII slug",
            self.0
        )
    }
}

impl Error for InvalidAgentId {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdapterFamily {
    NativeHooks,
    JsonlHooks,
    EventLogWatcher,
    IdeExtensionBridge,
    GenericProtocol,
    Undetermined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationMaturity {
    /// The adapter is implemented and its local behavior is covered by this repository.
    VerifiedCurrent,
    /// Catalog-only entry. Presence must not be presented as implemented support.
    DeclaredUnverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    SessionEvents,
    ToolEvents,
    AttentionEvents,
    DecisionResponse,
    QuestionResponse,
    ContextOpen,
    ProcessAttribution,
    QuotaTracking,
    TerminalNavigation,
    SshMonitoring,
    SoundAlerts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityAvailability {
    Supported,
    Partial,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceQuality {
    Unverified,
    PubliclyAdvertised,
    VendorDocumented,
    VerifiedLocally,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityEvidence {
    pub capability: Capability,
    pub availability: CapabilityAvailability,
    pub quality: EvidenceQuality,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    Any,
    Windows,
    MacOs,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigScope {
    User,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigTarget {
    pub platform: Platform,
    pub scope: ConfigScope,
    /// A display/detection template such as `~/.claude/settings.json`.
    pub path_template: String,
    pub format: ConfigFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationDescriptor {
    pub id: AgentId,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub executable_names: Vec<String>,
    pub adapter_family: AdapterFamily,
    pub maturity: IntegrationMaturity,
    pub capabilities: Vec<CapabilityEvidence>,
    pub config_targets: Vec<ConfigTarget>,
}

impl IntegrationDescriptor {
    pub fn evidence_for(&self, capability: Capability) -> Option<&CapabilityEvidence> {
        self.capabilities
            .iter()
            .find(|evidence| evidence.capability == capability)
    }

    pub fn is_verified(&self) -> bool {
        self.maturity == IntegrationMaturity::VerifiedCurrent
    }
}

#[derive(Debug, Clone)]
pub struct AgentCatalog {
    integrations: Vec<IntegrationDescriptor>,
    names: HashMap<String, usize>,
}

impl Serialize for AgentCatalog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.integrations.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentCatalog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let integrations = Vec::<IntegrationDescriptor>::deserialize(deserializer)?;
        Self::new(integrations).map_err(serde::de::Error::custom)
    }
}

impl AgentCatalog {
    pub fn new(integrations: Vec<IntegrationDescriptor>) -> Result<Self, CatalogError> {
        let mut names = HashMap::new();
        for (index, descriptor) in integrations.iter().enumerate() {
            if descriptor.display_name.trim().is_empty() {
                return Err(CatalogError::EmptyDisplayName(descriptor.id.clone()));
            }

            let mut local_capabilities = HashSet::new();
            for evidence in &descriptor.capabilities {
                if !local_capabilities.insert(evidence.capability) {
                    return Err(CatalogError::DuplicateCapability {
                        id: descriptor.id.clone(),
                        capability: evidence.capability,
                    });
                }
                if descriptor.maturity == IntegrationMaturity::DeclaredUnverified
                    && evidence.quality == EvidenceQuality::VerifiedLocally
                {
                    return Err(CatalogError::ContradictoryEvidence(descriptor.id.clone()));
                }
            }

            let mut local_aliases = HashSet::new();
            for alias in &descriptor.aliases {
                let normalized = normalize_lookup(alias);
                if normalized.is_empty() {
                    return Err(CatalogError::EmptyAlias(descriptor.id.clone()));
                }
                if !local_aliases.insert(normalized.clone()) {
                    return Err(CatalogError::DuplicateName(normalized));
                }
            }

            for raw_name in std::iter::once(descriptor.id.as_str())
                .chain(std::iter::once(descriptor.display_name.as_str()))
                .chain(descriptor.aliases.iter().map(String::as_str))
            {
                let normalized = normalize_lookup(raw_name);
                if normalized.is_empty() {
                    return Err(CatalogError::EmptyAlias(descriptor.id.clone()));
                }
                if let Some(existing) = names.insert(normalized.clone(), index) {
                    if existing != index {
                        return Err(CatalogError::DuplicateName(normalized));
                    }
                }
            }
        }

        Ok(Self {
            integrations,
            names,
        })
    }

    /// The 25 agents publicly advertised by Vibe Island as of 2026-07-11.
    pub fn vibe_island_25() -> Self {
        Self::new(vibe_island_descriptors()).expect("built-in catalog must be valid")
    }

    pub fn integrations(&self) -> &[IntegrationDescriptor] {
        &self.integrations
    }

    pub fn get(&self, id_or_alias: &str) -> Option<&IntegrationDescriptor> {
        self.names
            .get(&normalize_lookup(id_or_alias))
            .map(|index| &self.integrations[*index])
    }

    pub fn verified(&self) -> impl Iterator<Item = &IntegrationDescriptor> {
        self.integrations.iter().filter(|entry| entry.is_verified())
    }

    /// Returns every candidate whose declared executable matches the supplied basename.
    pub fn find_by_executable(&self, executable: &str) -> Vec<&IntegrationDescriptor> {
        let executable = normalize_executable(executable);
        self.integrations
            .iter()
            .filter(|entry| {
                entry
                    .executable_names
                    .iter()
                    .any(|candidate| normalize_executable(candidate) == executable)
            })
            .collect()
    }

    /// Finds known config targets by normalized suffix; no file access is performed.
    pub fn find_by_config_path(&self, path: &str) -> Vec<&IntegrationDescriptor> {
        let path = normalize_path(path);
        self.integrations
            .iter()
            .filter(|entry| {
                entry.config_targets.iter().any(|target| {
                    let suffix = normalize_path(target.path_template.trim_start_matches("~/"));
                    path.ends_with(&suffix)
                })
            })
            .collect()
    }
}

fn normalize_lookup(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize_executable(value: &str) -> String {
    let basename = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let lowercase = basename.to_ascii_lowercase();
    lowercase
        .strip_suffix(".exe")
        .unwrap_or(&lowercase)
        .to_owned()
}

fn normalize_path(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/").to_ascii_lowercase();
    let mut compact = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        if character != '/' || !compact.ends_with('/') {
            compact.push(character);
        }
    }
    compact
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    EmptyDisplayName(AgentId),
    EmptyAlias(AgentId),
    DuplicateName(String),
    DuplicateCapability { id: AgentId, capability: Capability },
    ContradictoryEvidence(AgentId),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDisplayName(id) => write!(f, "integration {id} has an empty display name"),
            Self::EmptyAlias(id) => write!(f, "integration {id} has an empty alias"),
            Self::DuplicateName(name) => write!(f, "duplicate integration id or alias: {name}"),
            Self::DuplicateCapability { id, capability } => {
                write!(f, "integration {id} repeats capability {capability:?}")
            }
            Self::ContradictoryEvidence(id) => write!(
                f,
                "unverified integration {id} cannot contain locally verified evidence"
            ),
        }
    }
}

impl Error for CatalogError {}

fn evidence(
    capability: Capability,
    availability: CapabilityAvailability,
    note: &str,
) -> CapabilityEvidence {
    CapabilityEvidence {
        capability,
        availability,
        quality: EvidenceQuality::VerifiedLocally,
        note: note.into(),
    }
}

fn config(path: &str, scope: ConfigScope, format: ConfigFormat) -> ConfigTarget {
    ConfigTarget {
        platform: Platform::Any,
        scope,
        path_template: path.into(),
        format,
    }
}

fn verified(
    id: &str,
    display_name: &str,
    aliases: &[&str],
    executables: &[&str],
    family: AdapterFamily,
    capabilities: Vec<CapabilityEvidence>,
    config_targets: Vec<ConfigTarget>,
) -> IntegrationDescriptor {
    IntegrationDescriptor {
        id: AgentId::new(id).expect("constant id must be valid"),
        display_name: display_name.into(),
        aliases: aliases.iter().map(|value| (*value).into()).collect(),
        executable_names: executables.iter().map(|value| (*value).into()).collect(),
        adapter_family: family,
        maturity: IntegrationMaturity::VerifiedCurrent,
        capabilities,
        config_targets,
    }
}

fn declared(id: &str, display_name: &str, aliases: &[&str]) -> IntegrationDescriptor {
    IntegrationDescriptor {
        id: AgentId::new(id).expect("constant id must be valid"),
        display_name: display_name.into(),
        aliases: aliases.iter().map(|value| (*value).into()).collect(),
        executable_names: Vec::new(),
        adapter_family: AdapterFamily::Undetermined,
        maturity: IntegrationMaturity::DeclaredUnverified,
        capabilities: Vec::new(),
        config_targets: Vec::new(),
    }
}

fn vibe_island_descriptors() -> Vec<IntegrationDescriptor> {
    use AdapterFamily::{IdeExtensionBridge, JsonlHooks, NativeHooks};
    use Capability::{AttentionEvents, DecisionResponse, SessionEvents, ToolEvents};
    use CapabilityAvailability::{Partial, Supported, Unsupported};
    use ConfigFormat::{Json, Toml};
    use ConfigScope::{Project, User};

    vec![
        verified(
            "claude-code",
            "Claude Code",
            &["claude"],
            &["claude"],
            NativeHooks,
            vec![
                evidence(
                    SessionEvents,
                    Supported,
                    "Covered by the shipped Claude Code hooks.",
                ),
                evidence(
                    ToolEvents,
                    Partial,
                    "The shipped hooks observe supported tool events.",
                ),
                evidence(
                    AttentionEvents,
                    Partial,
                    "Permission attention is observed for supported events.",
                ),
                evidence(
                    DecisionResponse,
                    Partial,
                    "Responses are gated to verified Claude Code versions and event types.",
                ),
            ],
            vec![
                config("~/.claude/settings.json", User, Json),
                config(".claude/settings.json", Project, Json),
            ],
        ),
        verified(
            "codex",
            "Codex",
            &["openai codex", "codex cli"],
            &["codex"],
            JsonlHooks,
            vec![
                evidence(
                    SessionEvents,
                    Supported,
                    "Covered by the shipped Codex hooks.",
                ),
                evidence(
                    ToolEvents,
                    Partial,
                    "Codex does not expose every tool path to hooks.",
                ),
                evidence(
                    AttentionEvents,
                    Partial,
                    "Permission requests are observation-only.",
                ),
                evidence(
                    DecisionResponse,
                    Unsupported,
                    "The current Codex integration never returns decisions.",
                ),
            ],
            vec![
                config("~/.codex/hooks.json", User, Json),
                config(".codex/hooks.json", Project, Json),
                config("~/.codex/config.toml", User, Toml),
            ],
        ),
        declared("zcode", "ZCode", &[]),
        verified(
            "gemini-cli",
            "Gemini CLI",
            &["gemini"],
            &["gemini"],
            NativeHooks,
            vec![
                evidence(
                    SessionEvents,
                    Supported,
                    "Covered by the shipped Gemini lifecycle hooks.",
                ),
                evidence(
                    ToolEvents,
                    Partial,
                    "BeforeTool and AfterTool are observed.",
                ),
                evidence(
                    AttentionEvents,
                    Partial,
                    "ToolPermission notifications are observation-only.",
                ),
                evidence(
                    DecisionResponse,
                    Unsupported,
                    "The current Gemini integration never returns decisions.",
                ),
            ],
            vec![
                config("~/.gemini/settings.json", User, Json),
                config(".gemini/settings.json", Project, Json),
            ],
        ),
        verified(
            "antigravity-cli",
            "Antigravity CLI",
            &["antigravity", "agy"],
            &["antigravity", "agy"],
            NativeHooks,
            vec![
                evidence(
                    SessionEvents,
                    Partial,
                    "Stop hooks are observed; session start/end hooks are not yet mapped.",
                ),
                evidence(
                    ToolEvents,
                    Partial,
                    "PreToolUse and PostToolUse are observed from Antigravity fixtures.",
                ),
                evidence(
                    AttentionEvents,
                    Unsupported,
                    "Antigravity hooks do not expose permission attention events.",
                ),
                evidence(
                    DecisionResponse,
                    Unsupported,
                    "The current Antigravity integration never returns decisions.",
                ),
            ],
            vec![
                config(".agents/hooks.json", Project, Json),
                config(".gemini/antigravity-cli/hooks.json", User, Json),
            ],
        ),
        verified(
            "cursor",
            "Cursor",
            &["cursor agent", "cursor agent cli"],
            &["cursor", "agent"],
            IdeExtensionBridge,
            vec![
                evidence(
                    SessionEvents,
                    Supported,
                    "Covered by the shipped Cursor hooks.",
                ),
                evidence(ToolEvents, Partial, "Supported hook events are observed."),
                evidence(
                    AttentionEvents,
                    Unsupported,
                    "Current Cursor hooks do not expose permission attention.",
                ),
                evidence(
                    DecisionResponse,
                    Unsupported,
                    "The current Cursor integration never returns decisions.",
                ),
            ],
            vec![
                config("~/.cursor/hooks.json", User, Json),
                config(".cursor/hooks.json", Project, Json),
            ],
        ),
        declared("trae", "Trae", &[]),
        declared("opencode", "OpenCode", &["open code"]),
        declared("mimocode", "MiMoCode", &["mimo code"]),
        declared("droid", "Droid", &["factory droid"]),
        declared("qoder", "Qoder", &[]),
        verified(
            "qwen",
            "Qwen",
            &["qwen code"],
            &["qwen"],
            NativeHooks,
            vec![
                evidence(
                    SessionEvents,
                    Supported,
                    "Covered by the shipped Qwen Code hooks.",
                ),
                evidence(
                    ToolEvents,
                    Partial,
                    "PreToolUse and PostToolUse are observed.",
                ),
                evidence(
                    AttentionEvents,
                    Partial,
                    "PermissionRequest hooks are observation-only.",
                ),
                evidence(
                    DecisionResponse,
                    Unsupported,
                    "The current Qwen integration never returns decisions.",
                ),
            ],
            vec![
                config("~/.qwen/settings.json", User, Json),
                config(".qwen/settings.json", Project, Json),
            ],
        ),
        declared("kimi-code", "Kimi Code", &["kimi code cli"]),
        declared("deepseek", "DeepSeek", &[]),
        declared("mistral-vibe", "Mistral Vibe", &["vibe cli"]),
        verified(
            "copilot",
            "Copilot",
            &["github copilot", "copilot cli"],
            &["copilot"],
            NativeHooks,
            vec![
                evidence(
                    SessionEvents,
                    Supported,
                    "Covered by the shipped Copilot CLI lifecycle hooks.",
                ),
                evidence(
                    ToolEvents,
                    Partial,
                    "preToolUse and postToolUse are observed.",
                ),
                evidence(
                    AttentionEvents,
                    Partial,
                    "permissionRequest hooks are observation-only.",
                ),
                evidence(
                    DecisionResponse,
                    Unsupported,
                    "The current Copilot integration never returns decisions.",
                ),
            ],
            vec![
                config("~/.copilot/hooks/llm-notch.json", User, Json),
                config(".github/hooks/llm-notch.json", Project, Json),
            ],
        ),
        declared("codebuddy", "CodeBuddy", &["code buddy"]),
        declared("workbuddy", "WorkBuddy", &["work buddy"]),
        declared("kiro", "Kiro", &["kiro cli"]),
        declared("hermes", "Hermes", &["hermes agent"]),
        declared("amp", "Amp", &["amp code"]),
        declared("pi-agent", "Pi Agent", &["pi coding agent"]),
        declared("oh-my-pi", "Oh My Pi", &["omp"]),
        declared("gajae-code", "Gajae Code", &["gajae"]),
        declared("kimi", "Kimi", &["kimi cli"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_catalog_has_exactly_25_unique_entries() {
        let catalog = AgentCatalog::vibe_island_25();
        assert_eq!(catalog.integrations().len(), 25);
        let ids: HashSet<_> = catalog
            .integrations()
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(ids.len(), 25);
    }

    #[test]
    fn only_current_first_party_integrations_are_verified() {
        let catalog = AgentCatalog::vibe_island_25();
        let verified: HashSet<_> = catalog.verified().map(|entry| entry.id.as_str()).collect();
        assert_eq!(
            verified,
            HashSet::from([
                "antigravity-cli",
                "claude-code",
                "codex",
                "copilot",
                "cursor",
                "gemini-cli",
                "qwen"
            ])
        );
        assert!(
            catalog
                .integrations()
                .iter()
                .filter(|entry| !entry.is_verified())
                .all(|entry| entry.capabilities.is_empty()
                    && entry.config_targets.is_empty()
                    && entry.executable_names.is_empty())
        );
    }

    #[test]
    fn lookup_accepts_ids_display_names_and_aliases() {
        let catalog = AgentCatalog::vibe_island_25();
        assert_eq!(
            catalog.get("gemini-cli").unwrap().display_name,
            "Gemini CLI"
        );
        assert_eq!(catalog.get("Gemini CLI").unwrap().id.as_str(), "gemini-cli");
        assert_eq!(catalog.get("Open AI Codex").unwrap().id.as_str(), "codex");
        assert!(catalog.get("not-a-real-agent").is_none());
    }

    #[test]
    fn executable_detection_matches_agy_binary() {
        let catalog = AgentCatalog::vibe_island_25();
        for executable in ["agy", "agy.exe", r"C:\tools\agy.exe"] {
            let matches = catalog.find_by_executable(executable);
            assert_eq!(matches.len(), 1, "failed for {executable}");
            assert_eq!(matches[0].id.as_str(), "antigravity-cli");
        }
    }

    #[test]
    fn executable_detection_is_case_insensitive_and_windows_aware() {
        let catalog = AgentCatalog::vibe_island_25();
        let matches = catalog.find_by_executable(r"C:\\tools\\CLAUDE.EXE");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id.as_str(), "claude-code");
    }

    #[test]
    fn config_path_detection_normalizes_separators() {
        let catalog = AgentCatalog::vibe_island_25();
        let matches = catalog.find_by_config_path(r"C:\\Users\\dev\\.gemini\\settings.json");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id.as_str(), "gemini-cli");
    }

    #[test]
    fn agent_ids_reject_noncanonical_or_unsafe_values() {
        for invalid in [
            "",
            "Claude",
            "two words",
            "../codex",
            "-codex",
            "codex-",
            "a--b",
        ] {
            assert!(AgentId::new(invalid).is_err(), "accepted {invalid:?}");
        }
        assert_eq!(AgentId::new("agent-25").unwrap().as_str(), "agent-25");
    }

    #[test]
    fn serde_round_trip_preserves_descriptors_and_validates_ids() {
        let descriptor = AgentCatalog::vibe_island_25().get("codex").unwrap().clone();
        let json = serde_json::to_string(&descriptor).unwrap();
        let decoded: IntegrationDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, descriptor);
        assert!(serde_json::from_str::<AgentId>(r#""Invalid Id""#).is_err());
    }

    #[test]
    fn catalog_serde_round_trip_rebuilds_validated_lookup_index() {
        let catalog = AgentCatalog::vibe_island_25();
        let json = serde_json::to_string(&catalog).unwrap();
        let decoded: AgentCatalog = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.integrations().len(), 25);
        assert_eq!(decoded.get("Claude").unwrap().id.as_str(), "claude-code");

        let mut invalid: Vec<IntegrationDescriptor> = serde_json::from_str(&json).unwrap();
        invalid[1].aliases.push("claude".into());
        let invalid_json = serde_json::to_string(&invalid).unwrap();
        assert!(serde_json::from_str::<AgentCatalog>(&invalid_json).is_err());
    }

    #[test]
    fn catalog_rejects_alias_collisions() {
        let mut entries = vibe_island_descriptors();
        entries[2].aliases.push("claude".into());
        assert_eq!(
            AgentCatalog::new(entries).unwrap_err(),
            CatalogError::DuplicateName("claude".into())
        );
    }

    #[test]
    fn catalog_rejects_duplicate_capability_evidence() {
        let mut entries = vibe_island_descriptors();
        let duplicate = entries[0].capabilities[0].clone();
        entries[0].capabilities.push(duplicate);
        assert!(matches!(
            AgentCatalog::new(entries),
            Err(CatalogError::DuplicateCapability { .. })
        ));
    }

    #[test]
    fn unverified_entries_cannot_claim_locally_verified_evidence() {
        let mut entry = declared("future-agent", "Future Agent", &[]);
        entry.capabilities.push(evidence(
            Capability::SessionEvents,
            CapabilityAvailability::Supported,
            "not actually implemented",
        ));
        assert_eq!(
            AgentCatalog::new(vec![entry]).unwrap_err(),
            CatalogError::ContradictoryEvidence(AgentId::new("future-agent").unwrap())
        );
    }
}
