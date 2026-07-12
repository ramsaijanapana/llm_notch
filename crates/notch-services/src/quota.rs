use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// A credential value may be passed to a provider, but can never be serialized.
#[derive(Clone)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reliability {
    Authoritative,
    Reported,
    Estimated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Freshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthenticationState {
    NotRequired,
    Authenticated,
    Required,
    Expired,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub service: String,
    pub used: Option<f64>,
    pub remaining: Option<f64>,
    pub limit: Option<f64>,
    pub unit: String,
    /// Unix timestamp in milliseconds.
    pub reset_at_ms: Option<i64>,
    /// Unix timestamp in milliseconds.
    pub observed_at_ms: i64,
    pub reliability: Reliability,
    pub freshness: Freshness,
    pub authentication: AuthenticationState,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl QuotaSnapshot {
    pub fn validate(&self) -> Result<(), QuotaError> {
        if self.service.trim().is_empty() || self.unit.trim().is_empty() {
            return Err(QuotaError::InvalidSnapshot(
                "service and unit must not be empty".into(),
            ));
        }
        for amount in [self.used, self.remaining, self.limit]
            .into_iter()
            .flatten()
        {
            if !amount.is_finite() || amount < 0.0 {
                return Err(QuotaError::InvalidSnapshot(
                    "quota amounts must be finite and non-negative".into(),
                ));
            }
        }
        reject_sensitive_values(&Value::Object(
            self.metadata
                .clone()
                .into_iter()
                .collect::<serde_json::Map<_, _>>(),
        ))?;
        Ok(())
    }

    pub fn to_safe_json(&self) -> Result<Value, QuotaError> {
        self.validate()?;
        serde_json::to_value(self).map_err(QuotaError::Serialization)
    }
}

fn reject_sensitive_values(value: &Value) -> Result<(), QuotaError> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
                if [
                    "token",
                    "secret",
                    "password",
                    "apikey",
                    "authorization",
                    "cookie",
                ]
                .iter()
                .any(|needle| normalized.contains(needle))
                {
                    return Err(QuotaError::SensitiveMetadata(key.clone()));
                }
                reject_sensitive_values(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                reject_sensitive_values(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum QuotaError {
    #[error("invalid quota snapshot: {0}")]
    InvalidSnapshot(String),
    #[error("sensitive metadata key is not serializable: {0}")]
    SensitiveMetadata(String),
    #[error("quota provider `{0}` is already registered")]
    DuplicateProvider(String),
    #[error("quota provider `{0}` was not found")]
    ProviderNotFound(String),
    #[error("quota provider failed: {0}")]
    Provider(String),
    #[error("credentials not configured: {0}")]
    MissingCredentials(String),
    #[error("quota probe request failed: {0}")]
    HttpProbe(String),
    #[error("quota probe response could not be parsed: {0}")]
    ParseProbe(String),
    #[error("could not serialize quota snapshot: {0}")]
    Serialization(#[source] serde_json::Error),
    #[error("quota provider registry lock was poisoned")]
    RegistryPoisoned,
}

impl QuotaError {
    pub fn is_missing_credentials(&self) -> bool {
        matches!(self, Self::MissingCredentials(_))
    }
}

pub trait QuotaProvider: Send + Sync {
    fn id(&self) -> &str;
    fn refresh(&self) -> Result<QuotaSnapshot, QuotaError>;
}

#[derive(Default)]
pub struct QuotaProviderRegistry {
    providers: RwLock<BTreeMap<String, Arc<dyn QuotaProvider>>>,
}

impl QuotaProviderRegistry {
    pub fn register(&self, provider: Arc<dyn QuotaProvider>) -> Result<(), QuotaError> {
        let id = provider.id().trim();
        if id.is_empty() {
            return Err(QuotaError::InvalidSnapshot(
                "provider id must not be empty".into(),
            ));
        }
        let mut providers = self
            .providers
            .write()
            .map_err(|_| QuotaError::RegistryPoisoned)?;
        if providers.contains_key(id) {
            return Err(QuotaError::DuplicateProvider(id.into()));
        }
        providers.insert(id.into(), provider);
        Ok(())
    }

    pub fn ids(&self) -> Result<Vec<String>, QuotaError> {
        Ok(self
            .providers
            .read()
            .map_err(|_| QuotaError::RegistryPoisoned)?
            .keys()
            .cloned()
            .collect())
    }

    pub fn refresh(&self, id: &str) -> Result<QuotaSnapshot, QuotaError> {
        let provider = self
            .providers
            .read()
            .map_err(|_| QuotaError::RegistryPoisoned)?
            .get(id)
            .cloned()
            .ok_or_else(|| QuotaError::ProviderNotFound(id.into()))?;
        let snapshot = provider.refresh()?;
        snapshot.validate()?;
        Ok(snapshot)
    }
}

/// Resolves provider credentials without ever exposing them through IPC.
pub trait QuotaCredentialResolver: Send + Sync {
    fn resolve(&self, env_vars: &[&str]) -> Result<SecretValue, QuotaError>;
}

/// Reads the first configured environment variable for a provider probe.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvCredentialResolver;

impl QuotaCredentialResolver for EnvCredentialResolver {
    fn resolve(&self, env_vars: &[&str]) -> Result<SecretValue, QuotaError> {
        for env_var in env_vars {
            if let Ok(value) = std::env::var(env_var) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Ok(SecretValue::new(trimmed));
                }
            }
        }
        Err(QuotaError::MissingCredentials(credential_setup_message(env_vars)))
    }
}

pub fn credential_setup_message(env_vars: &[&str]) -> String {
    if env_vars.is_empty() {
        "provider credentials are not configured".into()
    } else if env_vars.len() == 1 {
        format!("set {} to enable quota probes", env_vars[0])
    } else {
        format!(
            "set one of {} to enable quota probes",
            env_vars.join(", ")
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitHeaderStyle {
    OpenAi,
    Anthropic,
    Google,
    Moonshot,
}

#[derive(Debug, Clone)]
pub struct QuotaProbeSpec {
    pub service_id: &'static str,
    pub service_label: &'static str,
    pub credential_env_vars: &'static [&'static str],
    pub probe_url: &'static str,
    pub header_style: RateLimitHeaderStyle,
    pub unit: &'static str,
}

#[derive(Debug, Clone)]
pub struct HttpProbeRequest {
    pub url: String,
    pub header_style: RateLimitHeaderStyle,
    pub credential: SecretValue,
}

#[derive(Debug, Clone)]
pub struct HttpProbeResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
}

pub trait HttpQuotaProbeClient: Send + Sync {
    fn probe(&self, request: &HttpProbeRequest) -> Result<HttpProbeResponse, QuotaError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UreqHttpQuotaProbeClient;

impl HttpQuotaProbeClient for UreqHttpQuotaProbeClient {
    fn probe(&self, request: &HttpProbeRequest) -> Result<HttpProbeResponse, QuotaError> {
        let mut http_request = ureq::get(&request.url).header(
            "user-agent",
            "llm-notch-quota-probe/0.1 (+https://github.com/llm-notch/llm_notch)",
        );
        http_request = match request.header_style {
            RateLimitHeaderStyle::OpenAi => {
                http_request.header("authorization", format!("Bearer {}", request.credential.expose()))
            }
            RateLimitHeaderStyle::Anthropic => http_request
                .header("x-api-key", request.credential.expose())
                .header("anthropic-version", "2023-06-01"),
            RateLimitHeaderStyle::Google => {
                http_request.header("x-goog-api-key", request.credential.expose())
            }
            RateLimitHeaderStyle::Moonshot => http_request.header(
                "authorization",
                format!("Bearer {}", request.credential.expose()),
            ),
        };

        let response = http_request
            .call()
            .map_err(|error| QuotaError::HttpProbe(error.to_string()))?;
        let status = response.status().as_u16();
        let mut headers = BTreeMap::new();
        for (name, value) in response.headers().iter() {
            if let Ok(value) = value.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), value.to_string());
            }
        }
        Ok(HttpProbeResponse { status, headers })
    }
}

/// Credential-gated provider that probes vendor rate-limit headers without inventing usage.
pub struct CredentialGatedQuotaProvider {
    spec: QuotaProbeSpec,
    credentials: Arc<dyn QuotaCredentialResolver>,
    http: Arc<dyn HttpQuotaProbeClient>,
}

impl CredentialGatedQuotaProvider {
    pub fn new(
        spec: QuotaProbeSpec,
        credentials: Arc<dyn QuotaCredentialResolver>,
        http: Arc<dyn HttpQuotaProbeClient>,
    ) -> Self {
        Self {
            spec,
            credentials,
            http,
        }
    }
}

impl QuotaProvider for CredentialGatedQuotaProvider {
    fn id(&self) -> &str {
        self.spec.service_id
    }

    fn refresh(&self) -> Result<QuotaSnapshot, QuotaError> {
        let credential = self.credentials.resolve(self.spec.credential_env_vars)?;
        let response = self.http.probe(&HttpProbeRequest {
            url: self.spec.probe_url.into(),
            header_style: self.spec.header_style,
            credential,
        })?;
        snapshot_from_probe_response(&self.spec, response)
    }
}

pub fn builtin_probe_specs() -> &'static [QuotaProbeSpec] {
    const SPECS: [QuotaProbeSpec; 4] = [
        QuotaProbeSpec {
            service_id: "claude",
            service_label: "Claude",
            credential_env_vars: &["ANTHROPIC_API_KEY"],
            probe_url: "https://api.anthropic.com/v1/models",
            header_style: RateLimitHeaderStyle::Anthropic,
            unit: "requests",
        },
        QuotaProbeSpec {
            service_id: "codex",
            service_label: "Codex",
            credential_env_vars: &["OPENAI_API_KEY"],
            probe_url: "https://api.openai.com/v1/models",
            header_style: RateLimitHeaderStyle::OpenAi,
            unit: "requests",
        },
        QuotaProbeSpec {
            service_id: "gemini",
            service_label: "Gemini",
            credential_env_vars: &["GOOGLE_API_KEY", "GEMINI_API_KEY"],
            probe_url: "https://generativelanguage.googleapis.com/v1beta/models",
            header_style: RateLimitHeaderStyle::Google,
            unit: "requests",
        },
        QuotaProbeSpec {
            service_id: "kimi",
            service_label: "Kimi",
            credential_env_vars: &["MOONSHOT_API_KEY"],
            probe_url: "https://api.moonshot.ai/v1/models",
            header_style: RateLimitHeaderStyle::Moonshot,
            unit: "requests",
        },
    ];
    &SPECS
}

pub fn build_credential_gated_providers(
    credentials: Arc<dyn QuotaCredentialResolver>,
    http: Arc<dyn HttpQuotaProbeClient>,
) -> Vec<Arc<dyn QuotaProvider>> {
    builtin_probe_specs()
        .iter()
        .map(|spec| {
            Arc::new(CredentialGatedQuotaProvider::new(
                spec.clone(),
                Arc::clone(&credentials),
                Arc::clone(&http),
            )) as Arc<dyn QuotaProvider>
        })
        .collect()
}

fn snapshot_from_probe_response(
    spec: &QuotaProbeSpec,
    response: HttpProbeResponse,
) -> Result<QuotaSnapshot, QuotaError> {
    let observed_at_ms = current_time_ms();
    if response.status == 401 || response.status == 403 {
        return Err(QuotaError::Provider(format!(
            "{} probe rejected credentials with HTTP {}",
            spec.service_label, response.status
        )));
    }
    if !(200..300).contains(&response.status) {
        return Err(QuotaError::HttpProbe(format!(
            "{} probe returned HTTP {}",
            spec.service_label, response.status
        )));
    }

    let parsed = match spec.header_style {
        RateLimitHeaderStyle::OpenAi => parse_openai_rate_limit_headers(&response.headers)?,
        RateLimitHeaderStyle::Anthropic => {
            parse_anthropic_rate_limit_headers(&response.headers)?
        }
        RateLimitHeaderStyle::Google => parse_google_rate_limit_headers(&response.headers)?,
        RateLimitHeaderStyle::Moonshot => {
            parse_moonshot_rate_limit_headers(&response.headers)?
        }
    };

    let mut snapshot = QuotaSnapshot {
        service: spec.service_id.into(),
        used: parsed.used,
        remaining: parsed.remaining,
        limit: parsed.limit,
        unit: spec.unit.into(),
        reset_at_ms: parsed
            .reset_at_ms
            .or_else(|| parsed.reset_after_ms.map(|delta| observed_at_ms + delta)),
        observed_at_ms,
        reliability: Reliability::Reported,
        freshness: Freshness::Fresh,
        authentication: AuthenticationState::Authenticated,
        metadata: BTreeMap::from([(
            "probeSource".into(),
            Value::String(spec.probe_url.into()),
        )]),
    };
    if let (Some(limit), Some(remaining)) = (snapshot.limit, snapshot.remaining) {
        if snapshot.used.is_none() {
            snapshot.used = Some((limit - remaining).max(0.0));
        }
    }
    snapshot.validate()?;
    Ok(snapshot)
}

#[derive(Debug, Clone, Copy)]
struct ParsedRateLimitHeaders {
    limit: Option<f64>,
    remaining: Option<f64>,
    used: Option<f64>,
    reset_at_ms: Option<i64>,
    reset_after_ms: Option<i64>,
}

fn parse_openai_rate_limit_headers(
    headers: &BTreeMap<String, String>,
) -> Result<ParsedRateLimitHeaders, QuotaError> {
    let limit = header_amount(headers, "x-ratelimit-limit-requests")?;
    let remaining = header_amount(headers, "x-ratelimit-remaining-requests")?;
    let reset_after_ms = headers
        .get("x-ratelimit-reset-requests")
        .map(|value| parse_openai_reset_delay_ms(value))
        .transpose()?;
    ensure_probe_has_signal(limit, remaining, "OpenAI")?;
    Ok(ParsedRateLimitHeaders {
        limit,
        remaining,
        used: None,
        reset_at_ms: None,
        reset_after_ms,
    })
}

fn parse_google_rate_limit_headers(
    headers: &BTreeMap<String, String>,
) -> Result<ParsedRateLimitHeaders, QuotaError> {
    let limit = header_amount(headers, "x-ratelimit-limit-requests")?;
    let remaining = header_amount(headers, "x-ratelimit-remaining-requests")?;
    let reset_at_ms = headers
        .get("x-ratelimit-reset-requests")
        .map(|value| parse_rfc3339_ms(value))
        .transpose()?;
    ensure_probe_has_signal(limit, remaining, "Gemini")?;
    Ok(ParsedRateLimitHeaders {
        limit,
        remaining,
        used: None,
        reset_at_ms,
        reset_after_ms: None,
    })
}

fn parse_moonshot_rate_limit_headers(
    headers: &BTreeMap<String, String>,
) -> Result<ParsedRateLimitHeaders, QuotaError> {
    let limit = header_amount(headers, "x-ratelimit-limit")?;
    let remaining = header_amount(headers, "x-ratelimit-remaining")?;
    let reset_at_ms = headers
        .get("x-ratelimit-reset")
        .map(|value| parse_moonshot_reset_ms(value))
        .transpose()?;
    ensure_probe_has_signal(limit, remaining, "Kimi")?;
    Ok(ParsedRateLimitHeaders {
        limit,
        remaining,
        used: None,
        reset_at_ms,
        reset_after_ms: None,
    })
}

fn parse_anthropic_rate_limit_headers(
    headers: &BTreeMap<String, String>,
) -> Result<ParsedRateLimitHeaders, QuotaError> {
    let limit = header_amount(headers, "anthropic-ratelimit-requests-limit")?;
    let remaining = header_amount(headers, "anthropic-ratelimit-requests-remaining")?;
    let reset_at_ms = headers
        .get("anthropic-ratelimit-requests-reset")
        .map(|value| parse_rfc3339_ms(value))
        .transpose()?;
    ensure_probe_has_signal(limit, remaining, "Anthropic")?;
    Ok(ParsedRateLimitHeaders {
        limit,
        remaining,
        used: None,
        reset_at_ms,
        reset_after_ms: None,
    })
}

fn ensure_probe_has_signal(
    limit: Option<f64>,
    remaining: Option<f64>,
    vendor: &str,
) -> Result<(), QuotaError> {
    if limit.is_some() || remaining.is_some() {
        Ok(())
    } else {
        Err(QuotaError::ParseProbe(format!(
            "{vendor} probe response did not include request rate-limit headers"
        )))
    }
}

fn header_amount(headers: &BTreeMap<String, String>, key: &str) -> Result<Option<f64>, QuotaError> {
    match headers.get(key) {
        Some(value) => parse_non_negative_amount(value)
            .map(Some)
            .ok_or_else(|| QuotaError::ParseProbe(format!("invalid numeric header `{key}`"))),
        None => Ok(None),
    }
}

fn parse_non_negative_amount(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    trimmed
        .parse::<f64>()
        .ok()
        .filter(|amount| amount.is_finite() && *amount >= 0.0)
}

fn parse_openai_reset_delay_ms(value: &str) -> Result<i64, QuotaError> {
    let trimmed = value.trim().to_ascii_lowercase();
    if let Some(millis) = trimmed.strip_suffix("ms") {
        let amount = millis
            .trim()
            .parse::<f64>()
            .map_err(|_| QuotaError::ParseProbe(format!("invalid reset header `{value}`")))?;
        return Ok((amount.round() as i64).max(0));
    }
    if let Some(seconds) = trimmed.strip_suffix('s') {
        let amount = seconds
            .trim()
            .parse::<f64>()
            .map_err(|_| QuotaError::ParseProbe(format!("invalid reset header `{value}`")))?;
        return Ok((amount * 1_000.0).round() as i64);
    }
    if let Some(minutes) = trimmed.strip_suffix('m') {
        let amount = minutes
            .trim()
            .parse::<f64>()
            .map_err(|_| QuotaError::ParseProbe(format!("invalid reset header `{value}`")))?;
        return Ok((amount * 60_000.0).round() as i64);
    }
    trimmed
        .parse::<f64>()
        .map(|seconds| (seconds * 1_000.0).round() as i64)
        .map_err(|_| QuotaError::ParseProbe(format!("invalid reset header `{value}`")))
}

fn parse_rfc3339_ms(value: &str) -> Result<i64, QuotaError> {
    let parsed = chrono::DateTime::parse_from_rfc3339(value.trim())
        .map_err(|_| QuotaError::ParseProbe(format!("invalid RFC3339 reset header `{value}`")))?;
    Ok(parsed.timestamp_millis())
}

fn parse_moonshot_reset_ms(value: &str) -> Result<i64, QuotaError> {
    if let Ok(parsed) = parse_rfc3339_ms(value) {
        return Ok(parsed);
    }
    let trimmed = value.trim();
    let amount = trimmed
        .parse::<f64>()
        .map_err(|_| QuotaError::ParseProbe(format!("invalid reset header `{value}`")))?;
    if amount >= 1_000_000_000_000.0 {
        Ok(amount.round() as i64)
    } else if amount >= 1_000_000_000.0 {
        Ok((amount * 1_000.0).round() as i64)
    } else {
        Ok(current_time_ms() + (amount * 1_000.0).round() as i64)
    }
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Deterministic provider for local configuration, development, and tests.
pub struct StaticQuotaProvider {
    id: String,
    snapshot: QuotaSnapshot,
}

impl StaticQuotaProvider {
    pub fn new(id: impl Into<String>, snapshot: QuotaSnapshot) -> Self {
        Self {
            id: id.into(),
            snapshot,
        }
    }
}

impl QuotaProvider for StaticQuotaProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn refresh(&self) -> Result<QuotaSnapshot, QuotaError> {
        Ok(self.snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn snapshot() -> QuotaSnapshot {
        QuotaSnapshot {
            service: "test-service".into(),
            used: Some(3.0),
            remaining: Some(7.0),
            limit: Some(10.0),
            unit: "requests".into(),
            reset_at_ms: Some(2_000),
            observed_at_ms: 1_000,
            reliability: Reliability::Reported,
            freshness: Freshness::Fresh,
            authentication: AuthenticationState::NotRequired,
            metadata: BTreeMap::new(),
        }
    }

    struct MockCredentialResolver {
        value: Mutex<Option<Result<SecretValue, QuotaError>>>,
    }

    impl MockCredentialResolver {
        fn missing(env_vars: &[&'static str]) -> Self {
            Self {
                value: Mutex::new(Some(Err(QuotaError::MissingCredentials(
                    credential_setup_message(env_vars),
                )))),
            }
        }

        fn with_secret(secret: &str) -> Self {
            Self {
                value: Mutex::new(Some(Ok(SecretValue::new(secret)))),
            }
        }
    }

    impl QuotaCredentialResolver for MockCredentialResolver {
        fn resolve(&self, _env_vars: &[&str]) -> Result<SecretValue, QuotaError> {
            self.value
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Err(QuotaError::MissingCredentials("missing".into())))
        }
    }

    struct MockHttpClient {
        response: Mutex<Option<Result<HttpProbeResponse, QuotaError>>>,
    }

    impl MockHttpClient {
        fn with_response(response: HttpProbeResponse) -> Self {
            Self {
                response: Mutex::new(Some(Ok(response))),
            }
        }

        fn with_error(message: &str) -> Self {
            Self {
                response: Mutex::new(Some(Err(QuotaError::HttpProbe(message.into())))),
            }
        }
    }

    impl HttpQuotaProbeClient for MockHttpClient {
        fn probe(&self, _request: &HttpProbeRequest) -> Result<HttpProbeResponse, QuotaError> {
            self.response
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Err(QuotaError::HttpProbe("missing mock response".into())))
        }
    }

    fn openai_fixture_headers() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("x-ratelimit-limit-requests".into(), "5000".into()),
            ("x-ratelimit-remaining-requests".into(), "4999".into()),
            ("x-ratelimit-reset-requests".into(), "12ms".into()),
        ])
    }

    fn anthropic_fixture_headers() -> BTreeMap<String, String> {
        BTreeMap::from([
            (
                "anthropic-ratelimit-requests-limit".into(),
                "4000".into(),
            ),
            (
                "anthropic-ratelimit-requests-remaining".into(),
                "3995".into(),
            ),
            (
                "anthropic-ratelimit-requests-reset".into(),
                "2026-07-11T21:30:00Z".into(),
            ),
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

    fn gemini_spec() -> QuotaProbeSpec {
        builtin_probe_specs()
            .iter()
            .find(|spec| spec.service_id == "gemini")
            .cloned()
            .expect("gemini probe spec")
    }

    fn kimi_spec() -> QuotaProbeSpec {
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
    fn registry_registers_and_refreshes_valid_provider() {
        let registry = QuotaProviderRegistry::default();
        registry
            .register(Arc::new(StaticQuotaProvider::new("local", snapshot())))
            .unwrap();
        assert_eq!(registry.ids().unwrap(), ["local"]);
        assert_eq!(registry.refresh("local").unwrap(), snapshot());
    }

    #[test]
    fn registry_rejects_duplicate_and_unknown_provider() {
        let registry = QuotaProviderRegistry::default();
        registry
            .register(Arc::new(StaticQuotaProvider::new("local", snapshot())))
            .unwrap();
        assert!(matches!(
            registry.register(Arc::new(StaticQuotaProvider::new("local", snapshot()))),
            Err(QuotaError::DuplicateProvider(_))
        ));
        assert!(matches!(
            registry.refresh("missing"),
            Err(QuotaError::ProviderNotFound(_))
        ));
    }

    #[test]
    fn sensitive_values_are_redacted_and_not_serializable() {
        let secret = SecretValue::new("do-not-leak");
        assert_eq!(format!("{secret:?}"), "SecretValue([REDACTED])");
        assert_eq!(secret.expose(), "do-not-leak");

        let mut unsafe_snapshot = snapshot();
        unsafe_snapshot.metadata.insert(
            "nested".into(),
            serde_json::json!({ "api_token": "do-not-leak" }),
        );
        assert!(matches!(
            unsafe_snapshot.to_safe_json(),
            Err(QuotaError::SensitiveMetadata(key)) if key == "api_token"
        ));
    }

    #[test]
    fn safe_serialization_includes_reliability_and_authentication() {
        let value = snapshot().to_safe_json().unwrap();
        assert_eq!(value["reliability"], "reported");
        assert_eq!(value["freshness"], "fresh");
        assert_eq!(value["authentication"], "notRequired");
        assert!(!value.to_string().contains("token"));
    }

    #[test]
    fn invalid_amounts_are_rejected() {
        let mut value = snapshot();
        value.remaining = Some(f64::NAN);
        assert!(matches!(
            value.validate(),
            Err(QuotaError::InvalidSnapshot(_))
        ));
        value.remaining = Some(-1.0);
        assert!(matches!(
            value.validate(),
            Err(QuotaError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn missing_credentials_are_explicit_without_secrets() {
        let provider = CredentialGatedQuotaProvider::new(
            builtin_probe_specs()[0].clone(),
            Arc::new(MockCredentialResolver::missing(&["ANTHROPIC_API_KEY"])),
            Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                status: 200,
                headers: openai_fixture_headers(),
            })),
        );
        let error = provider.refresh().unwrap_err();
        assert!(error.is_missing_credentials());
        assert!(error.to_string().contains("ANTHROPIC_API_KEY"));
        assert!(!error.to_string().contains("sk-"));
    }

    #[test]
    fn fetch_errors_surface_without_usage_numbers() {
        let provider = CredentialGatedQuotaProvider::new(
            builtin_probe_specs()[1].clone(),
            Arc::new(MockCredentialResolver::with_secret("sk-test")),
            Arc::new(MockHttpClient::with_error("connection refused")),
        );
        let error = provider.refresh().unwrap_err();
        assert!(matches!(error, QuotaError::HttpProbe(_)));
        assert!(error.to_string().contains("connection refused"));
    }

    #[test]
    fn openai_fixture_parses_into_reported_snapshot() {
        let snapshot = snapshot_from_probe_response(
            &builtin_probe_specs()[1],
            HttpProbeResponse {
                status: 200,
                headers: openai_fixture_headers(),
            },
        )
        .unwrap();
        assert_eq!(snapshot.service, "codex");
        assert_eq!(snapshot.limit, Some(5000.0));
        assert_eq!(snapshot.remaining, Some(4999.0));
        assert_eq!(snapshot.used, Some(1.0));
        assert_eq!(snapshot.unit, "requests");
        assert_eq!(snapshot.reliability, Reliability::Reported);
        assert_eq!(snapshot.authentication, AuthenticationState::Authenticated);
    }

    #[test]
    fn anthropic_fixture_parses_reset_timestamp() {
        let snapshot = snapshot_from_probe_response(
            &builtin_probe_specs()[0],
            HttpProbeResponse {
                status: 200,
                headers: anthropic_fixture_headers(),
            },
        )
        .unwrap();
        assert_eq!(snapshot.service, "claude");
        assert_eq!(snapshot.remaining, Some(3995.0));
        assert_eq!(snapshot.reset_at_ms, Some(1_783_805_400_000));
    }

    #[test]
    fn rejected_credentials_do_not_emit_usage() {
        let error = snapshot_from_probe_response(
            &builtin_probe_specs()[0],
            HttpProbeResponse {
                status: 401,
                headers: anthropic_fixture_headers(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, QuotaError::Provider(_)));
        assert!(error.to_string().contains("HTTP 401"));
    }

    #[test]
    fn gemini_missing_credentials_are_explicit_without_secrets() {
        let provider = CredentialGatedQuotaProvider::new(
            gemini_spec(),
            Arc::new(MockCredentialResolver::missing(&["GOOGLE_API_KEY", "GEMINI_API_KEY"])),
            Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                status: 200,
                headers: gemini_fixture_headers(),
            })),
        );
        let error = provider.refresh().unwrap_err();
        assert!(error.is_missing_credentials());
        assert!(error.to_string().contains("GOOGLE_API_KEY"));
        assert!(error.to_string().contains("GEMINI_API_KEY"));
        assert!(!error.to_string().contains("AIza"));
    }

    #[test]
    fn gemini_fetch_errors_surface_without_usage_numbers() {
        let provider = CredentialGatedQuotaProvider::new(
            gemini_spec(),
            Arc::new(MockCredentialResolver::with_secret("AIza-test")),
            Arc::new(MockHttpClient::with_error("dns failure")),
        );
        let error = provider.refresh().unwrap_err();
        assert!(matches!(error, QuotaError::HttpProbe(_)));
        assert!(error.to_string().contains("dns failure"));
    }

    #[test]
    fn gemini_fixture_parses_into_reported_snapshot() {
        let snapshot = snapshot_from_probe_response(
            &gemini_spec(),
            HttpProbeResponse {
                status: 200,
                headers: gemini_fixture_headers(),
            },
        )
        .unwrap();
        assert_eq!(snapshot.service, "gemini");
        assert_eq!(snapshot.limit, Some(1500.0));
        assert_eq!(snapshot.remaining, Some(1492.0));
        assert_eq!(snapshot.used, Some(8.0));
        assert_eq!(snapshot.unit, "requests");
        assert_eq!(snapshot.reset_at_ms, Some(1_783_807_200_000));
        assert_eq!(snapshot.reliability, Reliability::Reported);
        assert_eq!(snapshot.authentication, AuthenticationState::Authenticated);
    }

    #[test]
    fn kimi_missing_credentials_are_explicit_without_secrets() {
        let provider = CredentialGatedQuotaProvider::new(
            kimi_spec(),
            Arc::new(MockCredentialResolver::missing(&["MOONSHOT_API_KEY"])),
            Arc::new(MockHttpClient::with_response(HttpProbeResponse {
                status: 200,
                headers: kimi_fixture_headers(),
            })),
        );
        let error = provider.refresh().unwrap_err();
        assert!(error.is_missing_credentials());
        assert!(error.to_string().contains("MOONSHOT_API_KEY"));
        assert!(!error.to_string().contains("sk-"));
    }

    #[test]
    fn kimi_fixture_parses_into_reported_snapshot() {
        let snapshot = snapshot_from_probe_response(
            &kimi_spec(),
            HttpProbeResponse {
                status: 200,
                headers: kimi_fixture_headers(),
            },
        )
        .unwrap();
        assert_eq!(snapshot.service, "kimi");
        assert_eq!(snapshot.limit, Some(60.0));
        assert_eq!(snapshot.remaining, Some(57.0));
        assert_eq!(snapshot.used, Some(3.0));
        assert_eq!(snapshot.reset_at_ms, Some(1_783_808_100_000));
        assert_eq!(snapshot.reliability, Reliability::Reported);
    }

    #[test]
    fn kimi_probe_without_rate_limit_headers_stays_unavailable() {
        let error = snapshot_from_probe_response(
            &kimi_spec(),
            HttpProbeResponse {
                status: 200,
                headers: BTreeMap::new(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, QuotaError::ParseProbe(_)));
        assert!(error.to_string().contains("Kimi"));
    }
}
