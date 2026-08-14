use std::time::Duration;

pub use silex_core::NetConnectionState as ConnectionState;

use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }

    pub fn supports_persistent_cache(&self) -> bool {
        matches!(self, Self::Get | Self::Head)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CredentialsMode {
    Omit,
    #[default]
    SameOrigin,
    Include,
}

impl CredentialsMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Omit => "omit",
            Self::SameOrigin => "same-origin",
            Self::Include => "include",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RequestBody {
    Empty,
    Text(String),
    Json(String),
    Form(Vec<(String, String)>),
}

impl RequestBody {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn fingerprint(&self) -> String {
        match self {
            Self::Empty => "empty".to_string(),
            Self::Text(value) => structured_key([("text", value.as_str())]),
            Self::Json(value) => structured_key([("json", value.as_str())]),
            Self::Form(fields) => {
                let mut fields = fields.clone();
                fields.sort();
                let mut key = String::from("form");
                append_segment(&mut key, &fields.len().to_string());
                for (name, value) in fields {
                    append_segment(&mut key, &name);
                    append_segment(&mut key, &value);
                }
                key
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestSpec {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub credentials: CredentialsMode,
    pub timeout: Option<Duration>,
    pub body: RequestBody,
}

impl RequestSpec {
    #[cfg(feature = "persist")]
    pub(crate) fn legacy_cache_key(&self) -> String {
        let mut headers = self
            .headers
            .iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
            .collect::<Vec<_>>();
        headers.sort();

        let mut key = String::from("net-request-v1");
        append_segment(&mut key, self.method.as_str());
        append_segment(&mut key, &canonical_url(&self.url));
        append_segment(&mut key, &headers.len().to_string());
        for (name, value) in headers {
            append_segment(&mut key, &name);
            append_segment(&mut key, &value);
        }
        if let Some(timeout) = self.timeout {
            append_segment(&mut key, "timeout");
            append_segment(&mut key, &timeout.as_secs().to_string());
            append_segment(&mut key, &timeout.subsec_nanos().to_string());
        } else {
            append_segment(&mut key, "no-timeout");
        }
        append_segment(&mut key, &self.body.fingerprint());
        key
    }

    pub fn cache_key(&self) -> String {
        let mut headers = self
            .headers
            .iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
            .collect::<Vec<_>>();
        headers.sort();

        let mut identity = String::from("net-request-v2");
        append_segment(&mut identity, self.method.as_str());
        append_segment(&mut identity, &canonical_url(&self.url));
        append_segment(&mut identity, self.credentials.as_str());
        append_segment(&mut identity, &headers.len().to_string());
        for (name, value) in headers {
            append_segment(&mut identity, &name);
            append_segment(&mut identity, &value);
        }
        if let Some(timeout) = self.timeout {
            append_segment(&mut identity, "timeout");
            append_segment(&mut identity, &timeout.as_secs().to_string());
            append_segment(&mut identity, &timeout.subsec_nanos().to_string());
        } else {
            append_segment(&mut identity, "no-timeout");
        }
        append_segment(&mut identity, &self.body.fingerprint());

        let digest = Sha256::digest(identity.as_bytes());
        let mut key = String::with_capacity("net-request-v2-".len() + digest.len() * 2);
        key.push_str("net-request-v2-");
        for byte in digest {
            key.push(hex_digit(byte >> 4));
            key.push(hex_digit(byte & 0x0f));
        }
        key
    }

    pub fn persistent_cache_rejection(&self) -> Option<&'static str> {
        if !self.method.supports_persistent_cache() {
            return Some("only GET and HEAD requests may use persistent cache");
        }
        if !matches!(self.credentials, CredentialsMode::Omit) {
            return Some("credentialed requests may not use persistent cache");
        }
        if url_contains_userinfo(&self.url) {
            return Some("URLs with embedded credentials may not use persistent cache");
        }
        if url_contains_sensitive_query(&self.url) {
            return Some("URLs with credential query parameters may not use persistent cache");
        }
        if !self.body.is_empty() {
            return Some("GET and HEAD requests with a body may not use persistent cache");
        }
        if self
            .headers
            .iter()
            .any(|(name, value)| is_sensitive_header_name(name) || is_credential_value(value))
        {
            return Some("requests with credential headers may not use persistent cache");
        }
        None
    }

    pub fn is_persistent_cache_safe(&self) -> bool {
        self.persistent_cache_rejection().is_none()
    }
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + value - 10) as char,
        _ => unreachable!("hex digit is limited to four bits"),
    }
}

fn is_sensitive_header_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "authorization"
        || name == "proxy-authorization"
        || name == "cookie"
        || name == "set-cookie"
        || name == "www-authenticate"
        || name == "api-key"
        || name == "x-api-key"
        || name.contains("auth")
        || name.contains("token")
        || name.contains("secret")
        || name.contains("password")
        || name.contains("credential")
        || name.contains("session")
        || name.contains("csrf")
}

fn is_credential_value(value: &str) -> bool {
    let mut parts = value.split_whitespace();
    let Some(scheme) = parts.next() else {
        return false;
    };
    parts.next().is_some()
        && (scheme.eq_ignore_ascii_case("basic")
            || scheme.eq_ignore_ascii_case("bearer")
            || scheme.eq_ignore_ascii_case("digest")
            || scheme.eq_ignore_ascii_case("token"))
}

fn url_contains_userinfo(url: &str) -> bool {
    let Some((_, remainder)) = url.split_once("://") else {
        return false;
    };
    remainder
        .split(['/', '?', '#'])
        .next()
        .is_some_and(|authority| authority.contains('@'))
}

fn url_contains_sensitive_query(url: &str) -> bool {
    let (url, _) = split_fragment(url);
    let Some((_, query)) = url.split_once('?') else {
        return false;
    };
    query
        .split('&')
        .filter_map(|pair| pair.split_once('=').map(|(name, _)| name))
        .any(is_sensitive_parameter_name)
}

fn is_sensitive_parameter_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("auth")
        || name.contains("token")
        || name.contains("secret")
        || name.contains("password")
        || name.contains("credential")
        || name.contains("session")
        || name.contains("cookie")
        || name.contains("csrf")
        || name == "key"
        || name.ends_with("_key")
}

fn append_segment(output: &mut String, value: &str) {
    output.push_str(&value.len().to_string());
    output.push(':');
    output.push_str(value);
}

fn structured_key<'a>(segments: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut key = String::new();
    for (name, value) in segments {
        append_segment(&mut key, name);
        append_segment(&mut key, value);
    }
    key
}

fn split_fragment(url: &str) -> (&str, Option<&str>) {
    match url.split_once('#') {
        Some((url, fragment)) => (url, Some(fragment)),
        None => (url, None),
    }
}

fn canonical_url(url: &str) -> String {
    let (url, fragment) = split_fragment(url);
    let Some((path, query)) = url.split_once('?') else {
        let mut canonical = url.to_string();
        if let Some(fragment) = fragment {
            canonical.push('#');
            canonical.push_str(fragment);
        }
        return canonical;
    };
    let mut pairs = query.split('&').collect::<Vec<_>>();
    pairs.sort_unstable();
    let mut canonical = path.to_string();
    canonical.push('?');
    canonical.push_str(&pairs.join("&"));
    if let Some(fragment) = fragment {
        canonical.push('#');
        canonical.push_str(fragment);
    }
    canonical
}

#[derive(Clone, Debug, PartialEq)]
pub struct HttpResponse {
    pub url: String,
    pub status: u16,
    pub status_text: String,
    pub raw_body: String,
}

impl HttpResponse {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventMessage {
    pub event: Option<String>,
    pub data: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePolicy {
    None,
    NetworkFirst,
    CacheFirst,
    StaleWhileRevalidate,
}

#[cfg(feature = "persist")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheEviction {
    RemovePersisted,
    KeepPersisted,
}

#[cfg(feature = "persist")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheConfig {
    pub capacity: usize,
    pub ttl: Option<Duration>,
    pub eviction: CacheEviction,
}

#[cfg(feature = "persist")]
impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            capacity: 32,
            ttl: None,
            eviction: CacheEviction::RemovePersisted,
        }
    }
}

#[cfg(feature = "persist")]
impl CacheConfig {
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn without_ttl(mut self) -> Self {
        self.ttl = None;
        self
    }

    pub fn eviction(mut self, eviction: CacheEviction) -> Self {
        self.eviction = eviction;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub delay: Duration,
    pub max_delay: Option<Duration>,
    pub max_elapsed: Option<Duration>,
    pub jitter: bool,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts,
            delay,
            max_delay: None,
            max_elapsed: None,
            jitter: true,
        }
    }

    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = Some(delay);
        self
    }

    pub fn max_elapsed(mut self, elapsed: Duration) -> Self {
        self.max_elapsed = Some(elapsed);
        self
    }

    pub fn no_jitter(mut self) -> Self {
        self.jitter = false;
        self
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1).min(31);
        let factor = 1u128 << exponent;
        let base_millis = self.delay.as_millis();
        let mut backoff_millis = base_millis.saturating_mul(factor);
        if let Some(max_delay) = self.max_delay {
            backoff_millis = backoff_millis.min(max_delay.as_millis());
        }
        let backoff_millis = backoff_millis.min(u128::from(u64::MAX));
        let backoff = Duration::from_millis(backoff_millis as u64);
        if self.jitter {
            let jitter_millis =
                (js_sys::Math::random() * backoff.as_millis() as f64).floor() as u128;
            Duration::from_millis(jitter_millis.min(u128::from(u64::MAX)) as u64)
        } else {
            backoff
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        ConnectionState, CredentialsMode, HttpMethod, HttpResponse, RequestBody, RequestSpec,
        RetryPolicy,
    };

    fn request(url: &str, headers: Vec<(&str, &str)>, body: RequestBody) -> RequestSpec {
        RequestSpec {
            method: HttpMethod::Get,
            url: url.to_string(),
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            credentials: CredentialsMode::Omit,
            timeout: None,
            body,
        }
    }

    #[test]
    fn cache_key_is_not_delimited_by_ambiguous_values() {
        let first = request("https://example.test/a|b", vec![], RequestBody::Empty);
        let second = request(
            "https://example.test/a",
            vec![],
            RequestBody::Text("b".into()),
        );

        assert_ne!(first.cache_key(), second.cache_key());
    }

    #[test]
    fn cache_key_normalizes_query_and_header_order() {
        let first = request(
            "https://example.test?a=1&b=2",
            vec![("X-Test", "one"), ("accept", "json")],
            RequestBody::Empty,
        );
        let second = request(
            "https://example.test?b=2&a=1",
            vec![("ACCEPT", "json"), ("x-test", "one")],
            RequestBody::Empty,
        );

        assert_eq!(first.cache_key(), second.cache_key());
    }

    #[test]
    fn cache_key_normalizes_query_without_crossing_url_fragment() {
        let first = request(
            "https://example.test/path?b=2&a=1#fragment",
            vec![],
            RequestBody::Empty,
        );
        let second = request(
            "https://example.test/path?a=1&b=2#fragment",
            vec![],
            RequestBody::Empty,
        );
        let different_fragment = request(
            "https://example.test/path?a=1&b=2#other",
            vec![],
            RequestBody::Empty,
        );
        let no_query_fragment = request(
            "https://example.test/path#fragment",
            vec![],
            RequestBody::Empty,
        );
        let other_no_query_fragment = request(
            "https://example.test/path#other",
            vec![],
            RequestBody::Empty,
        );

        assert_eq!(first.cache_key(), second.cache_key());
        assert_ne!(first.cache_key(), different_fragment.cache_key());
        assert_ne!(
            no_query_fragment.cache_key(),
            other_no_query_fragment.cache_key()
        );
    }

    #[test]
    fn cache_key_does_not_contain_request_secrets() {
        let request = request(
            "https://example.test",
            vec![("Authorization", "Bearer very-secret-token")],
            RequestBody::Text("private body".into()),
        );
        let key = request.cache_key();

        assert!(key.starts_with("net-request-v2-"));
        assert!(!key.contains("very-secret-token"));
        assert!(!key.contains("private body"));
        assert_eq!(key.len(), "net-request-v2-".len() + 64);
    }

    #[test]
    fn persistent_cache_requires_anonymous_safe_requests() {
        let mut request = request("https://example.test", vec![], RequestBody::Empty);
        assert!(request.is_persistent_cache_safe());

        request.credentials = CredentialsMode::SameOrigin;
        assert!(!request.is_persistent_cache_safe());

        request.credentials = CredentialsMode::Omit;
        request.method = HttpMethod::Post;
        assert!(!request.is_persistent_cache_safe());

        request.method = HttpMethod::Get;
        request
            .headers
            .push(("X-Session-Token".to_string(), "opaque-value".to_string()));
        assert!(!request.is_persistent_cache_safe());

        request.headers = vec![(
            "Authorization".to_string(),
            "Bearer opaque-value".to_string(),
        )];
        assert!(!request.is_persistent_cache_safe());

        request.headers = vec![("Cookie".to_string(), "session=opaque-value".to_string())];
        assert!(!request.is_persistent_cache_safe());

        request.headers.clear();
        request.url = "https://user:password@example.test/data".to_string();
        assert!(!request.is_persistent_cache_safe());

        request.url = "https://example.test/data?access_token=opaque-value".to_string();
        assert!(!request.is_persistent_cache_safe());

        request.url = "https://example.test/data#fragment?access_token=opaque-value".to_string();
        assert!(request.is_persistent_cache_safe());

        request.url = "https://example.test/data".to_string();
        request.body = RequestBody::Text("opaque-value".to_string());
        assert!(!request.is_persistent_cache_safe());
    }

    #[test]
    fn credentials_mode_is_part_of_cache_identity() {
        let first = request("https://example.test", vec![], RequestBody::Empty);
        let mut second = first.clone();
        second.credentials = CredentialsMode::Include;

        assert_ne!(first.cache_key(), second.cache_key());
    }

    #[test]
    fn form_fingerprint_normalizes_field_order() {
        let first = request(
            "https://example.test",
            vec![],
            RequestBody::Form(vec![("a".into(), "1".into()), ("b".into(), "2".into())]),
        );
        let second = request(
            "https://example.test",
            vec![],
            RequestBody::Form(vec![("b".into(), "2".into()), ("a".into(), "1".into())]),
        );

        assert_eq!(first.cache_key(), second.cache_key());
    }

    #[test]
    fn retry_backoff_respects_attempt_and_max_delay() {
        let policy = RetryPolicy::new(4, Duration::from_millis(10))
            .max_delay(Duration::from_millis(25))
            .no_jitter();

        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(10));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(20));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(25));
        assert_eq!(policy.delay_for_attempt(32), Duration::from_millis(25));
    }

    #[test]
    fn response_and_connection_state_helpers_keep_public_semantics() {
        let response = HttpResponse {
            url: "https://example.test".to_string(),
            status: 204,
            status_text: "No Content".to_string(),
            raw_body: String::new(),
        };
        assert!(response.ok());
        assert!(!ConnectionState::Connecting.is_connected());
        assert!(ConnectionState::Connected.is_active());
        assert!(ConnectionState::Closing.is_active());
        assert!(ConnectionState::Closed.as_str().contains("Closed"));
    }

    #[test]
    fn cache_key_distinguishes_timeout_and_body_kind() {
        let base = request(
            "https://example.test",
            vec![],
            RequestBody::Text("x".into()),
        );
        let mut timeout = base.clone();
        timeout.timeout = Some(Duration::from_millis(1));
        let json = request(
            "https://example.test",
            vec![],
            RequestBody::Json("x".into()),
        );

        assert_ne!(base.cache_key(), timeout.cache_key());
        assert_ne!(base.cache_key(), json.cache_key());
    }
}
