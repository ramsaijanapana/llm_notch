use std::sync::Arc;

use notch_services::quota::{
    build_credential_gated_providers, EnvCredentialResolver, QuotaCredentialResolver, QuotaError,
    QuotaProviderRegistry, QuotaSnapshot, UreqHttpQuotaProbeClient,
};
use serde::Serialize;

const SUPPORTED_SERVICES: [(&str, &str); 6] = [
    ("claude", "Claude"),
    ("codex", "Codex"),
    ("gemini", "Gemini"),
    ("kimi", "Kimi"),
    ("glm", "GLM"),
    ("deepseek", "DeepSeek"),
];

/// IPC-safe quota projection. Provider metadata is deliberately excluded because it may
/// contain implementation details that should never cross the desktop boundary.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshotView {
    pub service: String,
    pub display_name: String,
    pub availability: QuotaAvailability,
    pub used: Option<f64>,
    pub remaining: Option<f64>,
    pub limit: Option<f64>,
    pub unit: Option<String>,
    pub reset_at_ms: Option<i64>,
    pub observed_at_ms: Option<i64>,
    pub reliability: Option<String>,
    pub freshness: Option<String>,
    pub authentication: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QuotaAvailability {
    Available,
    Unavailable,
}

/// Registry wrapper installs credential-gated providers without fabricating usage.
pub struct DesktopQuotaRegistry {
    providers: QuotaProviderRegistry,
}

impl Default for DesktopQuotaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopQuotaRegistry {
    pub fn new() -> Self {
        Self::with_dependencies(
            Arc::new(EnvCredentialResolver),
            Arc::new(UreqHttpQuotaProbeClient),
        )
    }

    pub fn with_dependencies(
        credentials: Arc<dyn QuotaCredentialResolver>,
        http: Arc<dyn notch_services::quota::HttpQuotaProbeClient>,
    ) -> Self {
        let providers = QuotaProviderRegistry::default();
        for provider in build_credential_gated_providers(credentials, http) {
            let _ = providers.register(provider);
        }
        Self { providers }
    }

    pub fn list_snapshots(&self) -> Vec<QuotaSnapshotView> {
        let registered = self.providers.ids().unwrap_or_default();
        SUPPORTED_SERVICES
            .iter()
            .map(|(service, display_name)| {
                if registered.iter().any(|id| id == service) {
                    match self.providers.refresh(service) {
                        Ok(snapshot) => available_view(display_name, snapshot),
                        Err(error) => unavailable_view(service, display_name, map_refresh_error(error)),
                    }
                } else {
                    unavailable_view(
                        service,
                        display_name,
                        UnavailableReason {
                            message: "quota provider is not configured".into(),
                            authentication: None,
                        },
                    )
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct UnavailableReason {
    message: String,
    authentication: Option<String>,
}

fn map_refresh_error(error: QuotaError) -> UnavailableReason {
    match error {
        QuotaError::MissingCredentials(message) => UnavailableReason {
            message,
            authentication: Some("required".into()),
        },
        QuotaError::HttpProbe(message) | QuotaError::ParseProbe(message) => UnavailableReason {
            message: format!("quota probe failed: {message}"),
            authentication: Some("authenticated".into()),
        },
        QuotaError::Provider(message) => UnavailableReason {
            message: format!("provider did not return a usable snapshot: {message}"),
            authentication: Some("expired".into()),
        },
        other => UnavailableReason {
            message: format!("provider did not return a usable snapshot: {other}"),
            authentication: None,
        },
    }
}

fn available_view(display_name: &str, snapshot: QuotaSnapshot) -> QuotaSnapshotView {
    QuotaSnapshotView {
        service: snapshot.service,
        display_name: display_name.into(),
        availability: QuotaAvailability::Available,
        used: snapshot.used,
        remaining: snapshot.remaining,
        limit: snapshot.limit,
        unit: Some(snapshot.unit),
        reset_at_ms: snapshot.reset_at_ms,
        observed_at_ms: Some(snapshot.observed_at_ms),
        reliability: serialized_label(&snapshot.reliability),
        freshness: serialized_label(&snapshot.freshness),
        authentication: serialized_label(&snapshot.authentication),
        message: None,
    }
}

fn serialized_label<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

fn unavailable_view(service: &str, display_name: &str, reason: UnavailableReason) -> QuotaSnapshotView {
    QuotaSnapshotView {
        service: service.into(),
        display_name: display_name.into(),
        availability: QuotaAvailability::Unavailable,
        used: None,
        remaining: None,
        limit: None,
        unit: None,
        reset_at_ms: None,
        observed_at_ms: None,
        reliability: None,
        freshness: None,
        authentication: reason.authentication,
        message: Some(reason.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_services::quota::{
        builtin_probe_specs, credential_setup_message, CredentialGatedQuotaProvider,
        HttpProbeRequest, HttpProbeResponse, HttpQuotaProbeClient, QuotaCredentialResolver,
        SecretValue,
    };
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    struct MockCredentialResolver {
        value: Mutex<Result<SecretValue, QuotaError>>,
    }

    impl MockCredentialResolver {
        fn missing(env_vars: &[&'static str]) -> Self {
            Self {
                value: Mutex::new(Err(QuotaError::MissingCredentials(
                    credential_setup_message(env_vars),
                ))),
            }
        }

        fn with_secret(secret: &str) -> Self {
            Self {
                value: Mutex::new(Ok(SecretValue::new(secret))),
            }
        }
    }

    impl QuotaCredentialResolver for MockCredentialResolver {
        fn resolve(&self, _env_vars: &[&str]) -> Result<SecretValue, QuotaError> {
            match &*self.value.lock().unwrap() {
                Ok(secret) => Ok(secret.clone()),
                Err(QuotaError::MissingCredentials(message)) => {
                    Err(QuotaError::MissingCredentials(message.clone()))
                }
                Err(other) => Err(QuotaError::Provider(other.to_string())),
            }
        }
    }

    struct MockHttpClient {
        response: Mutex<Result<HttpProbeResponse, QuotaError>>,
    }

    impl MockHttpClient {
        fn with_response(response: HttpProbeResponse) -> Self {
            Self {
                response: Mutex::new(Ok(response)),
            }
        }

        fn with_error(message: &str) -> Self {
            Self {
                response: Mutex::new(Err(QuotaError::HttpProbe(message.into()))),
            }
        }
    }

    impl HttpQuotaProbeClient for MockHttpClient {
        fn probe(&self, _request: &HttpProbeRequest) -> Result<HttpProbeResponse, QuotaError> {
            match &*self.response.lock().unwrap() {
                Ok(response) => Ok(response.clone()),
                Err(QuotaError::HttpProbe(message)) => Err(QuotaError::HttpProbe(message.clone())),
                Err(QuotaError::MissingCredentials(message)) => {
                    Err(QuotaError::MissingCredentials(message.clone()))
                }
                Err(QuotaError::ParseProbe(message)) => Err(QuotaError::ParseProbe(message.clone())),
                Err(QuotaError::Provider(message)) => Err(QuotaError::Provider(message.clone())),
                Err(other) => Err(QuotaError::Provider(other.to_string())),
            }
        }
    }

    fn openai_fixture_headers() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("x-ratelimit-limit-requests".into(), "60".into()),
            ("x-ratelimit-remaining-requests".into(), "42".into()),
            ("x-ratelimit-reset-requests".into(), "30s".into()),
        ])
    }

    fn gemini_fixture_headers() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("x-ratelimit-limit-requests".into(), "1500".into()),
            ("x-ratelimit-remaining-requests".into(), "1492".into()),
            (
                "x-ratelimit-reset-requests".into(),
                "2026-07-11T22:00:00Z".into(),
            ),
        ])
    }

    fn gemini_spec() -> notch_services::quota::QuotaProbeSpec {
        builtin_probe_specs()
            .iter()
            .find(|spec| spec.service_id == "gemini")
            .cloned()
            .expect("gemini probe spec")
    }

    fn kimi_spec() -> notch_services::quota::QuotaProbeSpec {
        builtin_probe_specs()
            .iter()
            .find(|spec| spec.service_id == "kimi")
            .cloned()
            .expect("kimi probe spec")
    }

    fn kimi_fixture_headers() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("x-ratelimit-limit".into(), "60".into()),
            ("x-ratelimit-remaining".into(), "57".into()),
            ("x-ratelimit-reset".into(), "2026-07-11T22:15:00Z".into()),
        ])
    }

    #[test]
    fn empty_registry_is_honest_about_unconfigured_services() {
        let registry = DesktopQuotaRegistry {
            providers: QuotaProviderRegistry::default(),
        };
        let snapshots = registry.list_snapshots();
        assert_eq!(snapshots.len(), 6);
        assert!(snapshots.iter().all(|snapshot| {
            snapshot.availability == QuotaAvailability::Unavailable
                && snapshot.used.is_none()
                && snapshot.remaining.is_none()
                && snapshot.limit.is_none()
        }));
    }

    #[test]
    fn missing_credentials_stay_unavailable_without_usage() {
        let registry = DesktopQuotaRegistry::with_dependencies(
            Arc::new(MockCredentialResolver::missing(&["ANTHROPIC_API_KEY"])),
            Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                status: 200,
                headers: openai_fixture_headers(),
            })),
        );
        let claude = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "claude")
            .unwrap();
        assert_eq!(claude.availability, QuotaAvailability::Unavailable);
        assert_eq!(claude.authentication.as_deref(), Some("required"));
        assert!(claude.message.unwrap().contains("ANTHROPIC_API_KEY"));
        assert!(claude.used.is_none());
    }

    #[test]
    fn gemini_missing_credentials_stay_unavailable_without_usage() {
        let registry = DesktopQuotaRegistry::with_dependencies(
            Arc::new(MockCredentialResolver::missing(&["GOOGLE_API_KEY", "GEMINI_API_KEY"])),
            Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                status: 200,
                headers: gemini_fixture_headers(),
            })),
        );
        let gemini = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "gemini")
            .unwrap();
        assert_eq!(gemini.availability, QuotaAvailability::Unavailable);
        assert_eq!(gemini.authentication.as_deref(), Some("required"));
        let message = gemini.message.unwrap();
        assert!(message.contains("GOOGLE_API_KEY"));
        assert!(message.contains("GEMINI_API_KEY"));
        assert!(gemini.used.is_none());
    }

    #[test]
    fn fetch_errors_stay_unavailable_without_usage() {
        let registry = DesktopQuotaRegistry::with_dependencies(
            Arc::new(MockCredentialResolver::with_secret("sk-test")),
            Arc::new(MockHttpClient::with_error("connection refused")),
        );
        let codex = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "codex")
            .unwrap();
        assert_eq!(codex.availability, QuotaAvailability::Unavailable);
        assert!(codex.message.unwrap().contains("connection refused"));
        assert!(codex.used.is_none());
    }

    #[test]
    fn gemini_fetch_errors_stay_unavailable_without_usage() {
        let registry = DesktopQuotaRegistry::with_dependencies(
            Arc::new(MockCredentialResolver::with_secret("AIza-test")),
            Arc::new(MockHttpClient::with_error("connection refused")),
        );
        let gemini = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "gemini")
            .unwrap();
        assert_eq!(gemini.availability, QuotaAvailability::Unavailable);
        assert!(gemini.message.unwrap().contains("connection refused"));
        assert!(gemini.used.is_none());
    }

    #[test]
    fn fixture_probe_returns_available_codex_snapshot() {
        let providers = QuotaProviderRegistry::default();
        providers
            .register(Arc::new(CredentialGatedQuotaProvider::new(
                builtin_probe_specs()[1].clone(),
                Arc::new(MockCredentialResolver::with_secret("sk-test")),
                Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                    status: 200,
                    headers: openai_fixture_headers(),
                })),
            )))
            .unwrap();

        let registry = DesktopQuotaRegistry { providers };
        let codex = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "codex")
            .unwrap();
        assert_eq!(codex.availability, QuotaAvailability::Available);
        assert_eq!(codex.remaining, Some(42.0));
        assert_eq!(codex.limit, Some(60.0));
        assert_eq!(codex.used, Some(18.0));
    }

    #[test]
    fn fixture_probe_returns_available_gemini_snapshot() {
        let providers = QuotaProviderRegistry::default();
        providers
            .register(Arc::new(CredentialGatedQuotaProvider::new(
                gemini_spec(),
                Arc::new(MockCredentialResolver::with_secret("AIza-test")),
                Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                    status: 200,
                    headers: gemini_fixture_headers(),
                })),
            )))
            .unwrap();

        let registry = DesktopQuotaRegistry { providers };
        let gemini = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "gemini")
            .unwrap();
        assert_eq!(gemini.availability, QuotaAvailability::Available);
        assert_eq!(gemini.remaining, Some(1492.0));
        assert_eq!(gemini.limit, Some(1500.0));
        assert_eq!(gemini.used, Some(8.0));
    }

    #[test]
    fn kimi_missing_credentials_stay_unavailable_without_usage() {
        let registry = DesktopQuotaRegistry::with_dependencies(
            Arc::new(MockCredentialResolver::missing(&["MOONSHOT_API_KEY"])),
            Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                status: 200,
                headers: kimi_fixture_headers(),
            })),
        );
        let kimi = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "kimi")
            .unwrap();
        assert_eq!(kimi.availability, QuotaAvailability::Unavailable);
        assert_eq!(kimi.authentication.as_deref(), Some("required"));
        assert!(kimi.message.unwrap().contains("MOONSHOT_API_KEY"));
        assert!(kimi.used.is_none());
    }

    #[test]
    fn fixture_probe_returns_available_kimi_snapshot() {
        let providers = QuotaProviderRegistry::default();
        providers
            .register(Arc::new(CredentialGatedQuotaProvider::new(
                kimi_spec(),
                Arc::new(MockCredentialResolver::with_secret("sk-kimi-test")),
                Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                    status: 200,
                    headers: kimi_fixture_headers(),
                })),
            )))
            .unwrap();

        let registry = DesktopQuotaRegistry { providers };
        let kimi = registry
            .list_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.service == "kimi")
            .unwrap();
        assert_eq!(kimi.availability, QuotaAvailability::Available);
        assert_eq!(kimi.remaining, Some(57.0));
        assert_eq!(kimi.limit, Some(60.0));
        assert_eq!(kimi.used, Some(3.0));
    }

    #[test]
    fn ipc_shape_has_no_secret_or_metadata_fields() {
        let value = serde_json::to_value(DesktopQuotaRegistry::new().list_snapshots()).unwrap();
        let serialized = value.to_string().to_ascii_lowercase();
        for forbidden in ["token", "secret", "password", "apikey", "metadata"] {
            assert!(!serialized.contains(forbidden), "leaked field: {forbidden}");
        }
    }
}
