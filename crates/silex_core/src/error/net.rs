use super::{ErrorSeverity, SilexError, SilexErrorKind};
use std::fmt;
use wasm_bindgen::JsValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Closing,
    Closed,
    Error,
}

impl NetConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connecting | Self::Connected | Self::Closing)
    }
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum NetErrorKind {
    BrowserUnavailable,
    TransportUnavailable,
    Timeout,
    Aborted,
    HttpStatus { status: u16, body: String },
    DecodeError(String),
    SerializeError(String),
    ConnectionNotReady { state: NetConnectionState },
    ConnectionClosed,
    JsError(String),
    InvalidConfiguration(String),
    Core(Box<SilexErrorKind>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum NetError {
    Recoverable(NetErrorKind),
    Fatal(NetErrorKind),
}

impl NetError {
    pub fn recoverable(kind: NetErrorKind) -> Self {
        Self::Recoverable(kind)
    }
    pub fn fatal(kind: NetErrorKind) -> Self {
        Self::Fatal(kind)
    }
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Recoverable(_))
    }
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal(_))
    }
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Recoverable(_) => ErrorSeverity::Recoverable,
            Self::Fatal(_) => ErrorSeverity::Fatal,
        }
    }
    pub fn kind(&self) -> &NetErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }
    pub fn into_kind(self) -> NetErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }
    pub fn is_retryable(&self) -> bool {
        self.kind().is_retryable()
    }
    pub fn is_retryable_http_status(status: u16) -> bool {
        matches!(status, 408 | 429 | 500..=599)
    }
    pub(crate) fn into_silex_error(self) -> SilexError {
        match self {
            Self::Recoverable(kind) => {
                SilexError::Recoverable(super::SilexErrorKind::Net(Self::Recoverable(kind)))
            }
            Self::Fatal(kind) => SilexError::Fatal(super::SilexErrorKind::Net(Self::Fatal(kind))),
        }
    }
}

impl NetErrorKind {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Timeout | Self::TransportUnavailable => true,
            Self::HttpStatus { status, .. } => NetError::is_retryable_http_status(*status),
            _ => false,
        }
    }
}

impl From<JsValue> for NetError {
    fn from(value: JsValue) -> Self {
        Self::Recoverable(NetErrorKind::JsError(format!("{value:?}")))
    }
}

impl From<SilexError> for NetError {
    fn from(error: SilexError) -> Self {
        let severity = error.severity();
        let kind = match error.into_kind() {
            SilexErrorKind::Net(error) => error.into_kind(),
            kind => NetErrorKind::Core(Box::new(kind)),
        };
        match severity {
            ErrorSeverity::Recoverable => Self::Recoverable(kind),
            ErrorSeverity::Fatal => Self::Fatal(kind),
        }
    }
}

impl fmt::Display for NetErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for NetErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Core(error) => Some(&**error),
            _ => None,
        }
    }
}
impl fmt::Display for NetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.severity(), self.kind())
    }
}
impl std::error::Error for NetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.kind())
    }
}
