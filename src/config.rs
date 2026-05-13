use crate::{AppError, ErrorCode};

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub token: String,
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

        for (key, value) in env_vars {
            match key.into().as_str() {
                "MEALIE_URL" => url = Some(value.into()),
                "MEALIE_TOKEN" => token = Some(value.into()),
                _ => {}
            }
        }

        let base_url = required("MEALIE_URL", url)?;
        let token = required("MEALIE_TOKEN", token)?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        })
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
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
}
