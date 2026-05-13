use std::fmt::Display;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    MissingConfig,
    InvalidArgs,
    NotFound,
    Ambiguous,
    ApiError,
    NetworkError,
}

impl ErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingConfig => "missing_config",
            Self::InvalidArgs => "invalid_args",
            Self::NotFound => "not_found",
            Self::Ambiguous => "ambiguous",
            Self::ApiError => "api_error",
            Self::NetworkError => "network_error",
        }
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AppError {
    code: ErrorCode,
    message: String,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    ok: bool,
    error: &'a str,
    message: &'a str,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_args(error: impl Display) -> Self {
        Self::new(ErrorCode::InvalidArgs, error.to_string())
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn to_json_line(&self) -> String {
        let envelope = ErrorEnvelope {
            ok: false,
            error: self.code.as_str(),
            message: &self.message,
        };

        serde_json::to_string(&envelope).unwrap_or_else(|_| {
            r#"{"ok":false,"error":"api_error","message":"failed to serialize error"}"#.to_string()
        })
    }
}
