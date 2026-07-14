use std::fmt::Display;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    MissingConfig,
    InvalidArgs,
    Authentication,
    NotFound,
    Ambiguous,
    ApiError,
    NetworkError,
}

impl ErrorCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MissingConfig => "missing_config",
            Self::InvalidArgs => "invalid_args",
            Self::Authentication => "authentication",
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
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'a str>,
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

    pub fn hint(&self) -> Option<&'static str> {
        match self.code {
            ErrorCode::MissingConfig => {
                Some("Set MEALIE_URL and MEALIE_TOKEN, then run the command again.")
            }
            ErrorCode::Authentication => {
                Some("Check MEALIE_TOKEN and confirm it has access to this Mealie instance.")
            }
            ErrorCode::NetworkError => {
                Some("Check MEALIE_URL and confirm the Mealie server is reachable.")
            }
            _ => None,
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self.code {
            ErrorCode::InvalidArgs | ErrorCode::MissingConfig => 2,
            ErrorCode::NotFound => 3,
            ErrorCode::Authentication => 4,
            ErrorCode::NetworkError => 5,
            ErrorCode::Ambiguous | ErrorCode::ApiError => 1,
        }
    }

    pub fn to_human(&self) -> String {
        if self.code == ErrorCode::InvalidArgs && self.message.starts_with("error:") {
            return self.message.trim_end().to_string();
        }
        if self.code == ErrorCode::NotFound {
            return format!("Error {}", self.message);
        }
        match self.hint() {
            Some(hint) => format!("Error: {}\nHint: {hint}", self.message),
            None => format!("Error: {}", self.message),
        }
    }

    pub fn to_json_line(&self) -> String {
        let envelope = ErrorEnvelope {
            ok: false,
            error: self.code.as_str(),
            message: &self.message,
            hint: self.hint(),
        };

        serde_json::to_string(&envelope).unwrap_or_else(|_| {
            r#"{"ok":false,"error":"api_error","message":"failed to serialize error"}"#.to_string()
        })
    }
}
