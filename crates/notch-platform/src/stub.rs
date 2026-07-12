use crate::{
    HostActivationBridge, NavigationOutcome, NavigationTier, ProcessDescriptor, TerminalHost,
    TerminalLocator, TerminalNavigator,
};

#[derive(Debug, Default)]
pub struct UnsupportedTerminalNavigator;

#[derive(Debug, Default)]
pub struct UnsupportedHostActivationBridge;

impl HostActivationBridge for UnsupportedHostActivationBridge {
    fn activate(&self, locator: &TerminalLocator) -> NavigationOutcome {
        NavigationOutcome::unsupported(locator.explanation())
    }
}

impl TerminalNavigator for UnsupportedTerminalNavigator {
    fn discover(&self, process: &ProcessDescriptor) -> TerminalLocator {
        TerminalLocator::resolved(
            process,
            TerminalHost::Unknown,
            NavigationTier::Unsupported,
            "terminal navigation is not implemented on this platform",
        )
    }

    fn activate(&self, locator: &TerminalLocator) -> NavigationOutcome {
        NavigationOutcome::unsupported(locator.explanation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NavigationDisposition;

    #[test]
    fn unsupported_backend_never_claims_activation() {
        let locator = UnsupportedTerminalNavigator.discover(&ProcessDescriptor::new(1, "agent"));
        let outcome = UnsupportedTerminalNavigator.activate(&locator);

        assert_eq!(locator.tier(), NavigationTier::Unsupported);
        assert_eq!(outcome.disposition, NavigationDisposition::Unsupported);
    }
}
