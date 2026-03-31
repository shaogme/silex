#[derive(Debug, Clone, PartialEq)]
pub enum PersistenceError {
    BackendUnavailable,
    ReadFailed(String),
    WriteFailed(String),
    RemoveFailed(String),
    DecodeFailed { raw: String, message: String },
    EncodeFailed(String),
    InvalidConfiguration(String),
}

impl PersistenceError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::BackendUnavailable => "backend unavailable".to_string(),
            Self::ReadFailed(message)
            | Self::WriteFailed(message)
            | Self::RemoveFailed(message)
            | Self::EncodeFailed(message)
            | Self::InvalidConfiguration(message) => message.clone(),
            Self::DecodeFailed { message, .. } => message.clone(),
        }
    }
}
