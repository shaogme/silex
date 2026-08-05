use std::time::Duration;

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
    pub timeout: Option<Duration>,
    pub body: RequestBody,
}

impl RequestSpec {
    pub fn cache_key(&self) -> String {
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

fn canonical_url(url: &str) -> String {
    let Some((path, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let mut pairs = query.split('&').collect::<Vec<_>>();
    pairs.sort_unstable();
    let mut canonical = path.to_string();
    canonical.push('?');
    canonical.push_str(&pairs.join("&"));
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Closing,
    Closed,
    Error,
}

impl ConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting...",
            Self::Connected => "Connected",
            Self::Closing => "Closing...",
            Self::Closed => "Closed",
            Self::Error => "Error",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connecting | Self::Connected)
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

    use super::{ConnectionState, HttpMethod, HttpResponse, RequestBody, RequestSpec, RetryPolicy};

    fn request(url: &str, headers: Vec<(&str, &str)>, body: RequestBody) -> RequestSpec {
        RequestSpec {
            method: HttpMethod::Get,
            url: url.to_string(),
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
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
