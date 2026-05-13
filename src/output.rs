use serde_json::{Map, Value};

use crate::{AppError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Json,
    Ndjson,
    Quiet,
}

impl OutputMode {
    pub fn from_flags(json: bool, _ndjson: bool, quiet: bool) -> Result<Self, AppError> {
        if json {
            return Ok(Self::Json);
        }

        if quiet {
            return Ok(Self::Quiet);
        }

        Ok(Self::Ndjson)
    }
}

pub fn record<const N: usize>(record_type: &str, fields: [(&str, Value); N]) -> Value {
    let mut object = Map::with_capacity(N + 2);
    object.insert("ok".to_string(), Value::Bool(true));
    object.insert("type".to_string(), Value::String(record_type.to_string()));

    for (key, value) in fields {
        object.insert(key.to_string(), value);
    }

    Value::Object(object)
}

pub fn write_output(mode: OutputMode, values: Vec<Value>) -> Result<String, AppError> {
    match mode {
        OutputMode::Json => serde_json::to_string_pretty(&values)
            .map(|json| format!("{json}\n"))
            .map_err(|error| AppError::new(ErrorCode::ApiError, error.to_string())),
        OutputMode::Ndjson => {
            let mut lines = String::new();
            for value in values {
                let line = serde_json::to_string(&value)
                    .map_err(|error| AppError::new(ErrorCode::ApiError, error.to_string()))?;
                lines.push_str(&line);
                lines.push('\n');
            }
            Ok(lines)
        }
        OutputMode::Quiet => {
            let mut output = String::new();
            for value in values.iter().filter(|value| {
                matches!(
                    value.get("type").and_then(Value::as_str),
                    Some("plan_created" | "plan_deleted")
                )
            }) {
                match value.get("id") {
                    Some(Value::Number(number)) => {
                        if !output.is_empty() {
                            output.push('\n');
                        }
                        output.push_str(&number.to_string());
                    }
                    Some(Value::String(text)) => {
                        if !output.is_empty() {
                            output.push('\n');
                        }
                        output.push_str(text);
                    }
                    _ => {}
                }
            }

            if !output.is_empty() {
                output.push('\n');
            }

            Ok(output)
        }
    }
}
