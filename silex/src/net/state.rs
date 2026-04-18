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
            Self::Empty => String::new(),
            Self::Text(value) | Self::Json(value) => value.clone(),
            Self::Form(fields) => fields
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&"),
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
        let headers = self
            .headers
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        let timeout = self
            .timeout
            .map(|value| value.as_millis().to_string())
            .unwrap_or_default();
        format!(
            "{}|{}|{}|{}|{}",
            self.method.as_str(),
            self.url,
            headers,
            timeout,
            self.body.fingerprint()
        )
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Closing,
    Closed,
    Error,
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
