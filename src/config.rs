use std::fmt;

use crate::{AppError, ErrorCode};

pub(crate) const INVALID_URL_MESSAGE: &str = "MEALIE_URL must be a valid URL";
pub(crate) const HTTPS_REQUIRED_MESSAGE: &str =
    "MEALIE_URL must use HTTPS; set USE_INSECURE_HTTP=yes to allow HTTP";

#[derive(Clone)]
pub struct Config {
    pub base_url: String,
    pub token: String,
}

impl fmt::Debug for Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Config")
            .field("base_url", &self.base_url)
            .field("token", &"[redacted]")
            .finish()
    }
}

impl Config {
    pub fn from_env<I, K, V>(env_vars: I) -> Result<Self, AppError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut url = None;
        let mut token = None;
        let mut allow_insecure_http = None;

        for (key, value) in env_vars {
            match key.into().as_str() {
                "MEALIE_URL" => url = Some(value.into()),
                "MEALIE_TOKEN" => token = Some(value.into()),
                "USE_INSECURE_HTTP" => allow_insecure_http = Some(value.into()),
                _ => {}
            }
        }

        let base_url = required("MEALIE_URL", url)?;
        let token = required("MEALIE_TOKEN", token)?;
        let allow_insecure_http = allow_insecure_http
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value.eq_ignore_ascii_case("yes"));
        let base_url = validate_base_url(&base_url, allow_insecure_http)?;

        Ok(Self { base_url, token })
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

pub(crate) fn validate_base_url(
    base_url: &str,
    allow_insecure_http: bool,
) -> Result<String, AppError> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    let url = reqwest::Url::parse(&base_url)
        .map_err(|_| AppError::new(ErrorCode::InvalidArgs, INVALID_URL_MESSAGE))?;
    if url.scheme() != "https" && !(allow_insecure_http && url.scheme() == "http") {
        return Err(AppError::new(
            ErrorCode::InvalidArgs,
            HTTPS_REQUIRED_MESSAGE,
        ));
    }

    Ok(base_url)
}

fn required(name: &str, value: Option<String>) -> Result<String, AppError> {
    let value = value.unwrap_or_default();
    if value.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::MissingConfig,
            format!("{name} is required"),
        ));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_base_url_slashes() {
        let config = Config::from_env([
            ("MEALIE_URL", "https://mealie.example///"),
            ("MEALIE_TOKEN", "token"),
        ])
        .expect("config");

        assert_eq!(config.base_url, "https://mealie.example");
        assert_eq!(
            config.endpoint("/api/recipes/test"),
            "https://mealie.example/api/recipes/test"
        );
    }

    #[test]
    fn redacts_token_from_debug_output() {
        let config = Config::from_env([
            ("MEALIE_URL", "https://mealie.example"),
            ("MEALIE_TOKEN", "secret-token"),
        ])
        .expect("config");

        let debug = format!("{config:?}");

        assert!(debug.contains("https://mealie.example"));
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("secret-token"));
    }

    #[test]
    fn rejects_http_without_explicit_opt_in() {
        let error = Config::from_env([
            ("MEALIE_URL", "http://mealie.example"),
            ("MEALIE_TOKEN", "token"),
        ])
        .expect_err("HTTP should require explicit opt-in");

        assert_eq!(error.code(), ErrorCode::InvalidArgs);
    }
}
