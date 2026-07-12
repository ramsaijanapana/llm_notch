use std::fmt;

/// The strongest navigation guarantee supported by the supplied metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum NavigationTier {
    Unsupported,
    AppActivate,
    WindowFocus,
    ExactPane,
}

/// A terminal application or terminal execution environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalHost {
    WindowsTerminal,
    ConsoleHost,
    PowerShell,
    VsCode,
    Cursor,
    WezTerm,
    Wsl,
    Tmux,
    MacTerminal,
    ITerm2,
    Other(String),
    Unknown,
}

/// Process identity captured when an agent session begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessDescriptor {
    pub process_id: u32,
    pub process_started_at_ms: Option<u64>,
    pub executable: String,
    pub parent_executable: Option<String>,
    /// Terminal executable established by process-tree inspection, not a window title.
    pub terminal_executable: Option<String>,
    pub metadata: VerifiedTerminalMetadata,
}

impl ProcessDescriptor {
    pub fn new(process_id: u32, executable: impl Into<String>) -> Self {
        Self {
            process_id,
            process_started_at_ms: None,
            executable: executable.into(),
            parent_executable: None,
            terminal_executable: None,
            metadata: VerifiedTerminalMetadata::default(),
        }
    }
}

/// Navigation identifiers obtained directly from a terminal, IDE bridge, or OS collector.
///
/// Callers must not populate these values by parsing mutable window titles.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedTerminalMetadata {
    pub application_id: Option<String>,
    pub window_handle: Option<u64>,
    pub terminal_session_id: Option<String>,
    pub tab_id: Option<String>,
    pub pane_id: Option<String>,
    pub wsl_distribution: Option<String>,
    pub tmux_session: Option<String>,
}

/// Opaque, immutable result of navigation discovery.
#[derive(Clone, Eq, PartialEq)]
pub struct TerminalLocator {
    host: TerminalHost,
    tier: NavigationTier,
    process_id: u32,
    process_started_at_ms: Option<u64>,
    metadata: VerifiedTerminalMetadata,
    explanation: String,
}

impl TerminalLocator {
    pub(crate) fn resolved(
        process: &ProcessDescriptor,
        host: TerminalHost,
        tier: NavigationTier,
        explanation: impl Into<String>,
    ) -> Self {
        Self {
            host,
            tier,
            process_id: process.process_id,
            process_started_at_ms: process.process_started_at_ms,
            metadata: process.metadata.clone(),
            explanation: explanation.into(),
        }
    }

    pub fn host(&self) -> &TerminalHost {
        &self.host
    }

    pub fn tier(&self) -> NavigationTier {
        self.tier
    }

    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    pub fn process_started_at_ms(&self) -> Option<u64> {
        self.process_started_at_ms
    }

    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    /// Returns read-only identifiers for a trusted host bridge.
    pub fn verified_metadata(&self) -> &VerifiedTerminalMetadata {
        &self.metadata
    }
}

impl fmt::Debug for TerminalLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TerminalLocator")
            .field("host", &self.host)
            .field("tier", &self.tier)
            .field("process_id", &self.process_id)
            .field("process_started_at_ms", &self.process_started_at_ms)
            .field("explanation", &self.explanation)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationDisposition {
    /// Reserved for a backend that has actually completed activation.
    Activated,
    /// Discovery succeeded, but a terminal/IDE bridge must perform activation.
    RequiresHostBridge,
    /// The platform adapter has an extension point but no native activator yet.
    RequiresPlatformImplementation,
    /// A concrete platform activation was attempted and did not succeed.
    ActivationFailed,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationOutcome {
    pub tier: NavigationTier,
    pub disposition: NavigationDisposition,
    pub message: String,
}

impl NavigationOutcome {
    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self {
            tier: NavigationTier::Unsupported,
            disposition: NavigationDisposition::Unsupported,
            message: message.into(),
        }
    }
}

pub trait TerminalNavigator: Send + Sync {
    fn discover(&self, process: &ProcessDescriptor) -> TerminalLocator;

    /// Attempts navigation or reports which explicit bridge is still required.
    fn activate(&self, locator: &TerminalLocator) -> NavigationOutcome;
}

/// Executes activation for a previously discovered locator.
///
/// Implementations must use only verified opaque identifiers stored in the
/// locator. Window-title lookup is explicitly outside this contract.
pub trait HostActivationBridge: Send + Sync {
    fn activate(&self, locator: &TerminalLocator) -> NavigationOutcome;
}

/// Result of a terminal-host-specific exact-pane activation attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostBridgeOutcome {
    /// The host bridge completed exact-pane activation.
    Activated { message: String },
    /// The host is known but the bridge could not run (missing binary, unsupported metadata).
    Unavailable { message: String },
    /// No host bridge applies to this locator.
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformKind {
    Windows,
    MacOs,
    Other,
}
