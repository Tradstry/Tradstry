use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("operation '{operation}' is not implemented")]
    NotImplemented { operation: &'static str },
    #[error("capability '{capability}' is not supported")]
    UnsupportedCapability { capability: &'static str },
    #[error("serialization error: {message}")]
    Serialization { message: String },
    #[error("storage error: {message}")]
    Storage { message: String },
}

impl StoreError {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn not_implemented(operation: &'static str) -> Self {
        Self::NotImplemented { operation }
    }

    pub fn unsupported_capability(capability: &'static str) -> Self {
        Self::UnsupportedCapability { capability }
    }

    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
        }
    }
}
