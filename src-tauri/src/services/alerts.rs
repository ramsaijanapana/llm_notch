//! Resource alert presentation: tray/beacon updates and optional sound (never focus-steals).

use std::collections::HashSet;
use std::sync::Mutex;

use notch_core::{ActiveAlert, AlertKind};

/// Stable key for deduplicating sustained resource alerts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AlertKey {
    kind: AlertKind,
    session_id: Option<String>,
}

/// Tracks which resource alerts have already triggered optional sound.
#[derive(Debug, Default)]
pub struct AlertNotifier {
    sounded: Mutex<HashSet<AlertKey>>,
}

impl AlertNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns tray indicator text for active resource alerts and plays optional sound once per alert key.
    pub fn observe(&self, alerts: &[ActiveAlert], sound_enabled: bool) -> Option<String> {
        let resource: Vec<_> = alerts
            .iter()
            .filter(|alert| {
                matches!(
                    alert.kind,
                    AlertKind::CpuWarn | AlertKind::CpuCritical | AlertKind::MemoryHigh
                )
            })
            .collect();

        if resource.is_empty() {
            self.sounded.lock().unwrap_or_else(|e| e.into_inner()).clear();
            return None;
        }

        if sound_enabled {
            let mut sounded = self.sounded.lock().unwrap_or_else(|e| e.into_inner());
            for alert in &resource {
                let key = AlertKey {
                    kind: alert.kind,
                    session_id: alert.session_id.clone(),
                };
                if sounded.insert(key) {
                    play_alert_sound();
                }
            }
        }

        let primary = resource
            .iter()
            .max_by_key(|alert| alert_severity(alert.kind))
            .map(|alert| alert.message.as_str())
            .unwrap_or("Resource alert");
        Some(primary.to_string())
    }
}

fn alert_severity(kind: AlertKind) -> u8 {
    match kind {
        AlertKind::CpuCritical => 3,
        AlertKind::MemoryHigh => 2,
        AlertKind::CpuWarn => 1,
        AlertKind::NewAttention => 0,
    }
}

/// Platform alert tone without activating application windows.
pub fn play_alert_sound() {
    #[cfg(windows)]
    {
        extern "system" {
            fn MessageBeep(uType: u32) -> i32;
        }
        const MB_ICONASTERISK: u32 = 0x0000_0040;
        unsafe {
            let _ = MessageBeep(MB_ICONASTERISK);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/afplay")
            .args(["/System/Library/Sounds/Tink.aiff", "-v", "0.35"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("/usr/bin/paplay")
            .arg("/usr/share/sounds/freedesktop/stereo/message.oga")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_core::AlertKind;

    fn resource_alert(kind: AlertKind, message: &str) -> ActiveAlert {
        ActiveAlert {
            kind,
            session_id: None,
            message: message.into(),
            raised_at_ms: 1,
        }
    }

    #[test]
    fn observe_returns_none_when_no_resource_alerts() {
        let notifier = AlertNotifier::new();
        let attention = vec![ActiveAlert {
            kind: AlertKind::NewAttention,
            session_id: Some("s1".into()),
            message: "attention".into(),
            raised_at_ms: 1,
        }];
        assert!(notifier.observe(&attention, true).is_none());
    }

    #[test]
    fn observe_reports_primary_resource_message() {
        let notifier = AlertNotifier::new();
        let alerts = vec![
            resource_alert(AlertKind::CpuWarn, "Host CPU sustained above 70%"),
            resource_alert(AlertKind::CpuCritical, "Host CPU sustained above 90%"),
        ];
        assert_eq!(
            notifier.observe(&alerts, false).as_deref(),
            Some("Host CPU sustained above 90%")
        );
    }

    #[test]
    fn sound_deduplicates_sustained_alerts() {
        let notifier = AlertNotifier::new();
        let alerts = vec![resource_alert(AlertKind::MemoryHigh, "RSS high")];
        notifier.observe(&alerts, true);
        assert_eq!(notifier.sounded.lock().unwrap().len(), 1);
        notifier.observe(&alerts, true);
        assert_eq!(notifier.sounded.lock().unwrap().len(), 1);
    }

    #[test]
    fn clearing_resource_alerts_resets_sound_state() {
        let notifier = AlertNotifier::new();
        let alerts = vec![resource_alert(AlertKind::CpuWarn, "warn")];
        notifier.observe(&alerts, true);
        notifier.observe(&[], true);
        assert!(notifier.sounded.lock().unwrap().is_empty());
    }
}
