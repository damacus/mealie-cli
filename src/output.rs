use serde_json::{Map, Value};

use crate::{AppError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Ndjson,
    Quiet,
}

impl OutputMode {
    pub fn from_flags(json: bool, ndjson: bool, quiet: bool) -> Result<Self, AppError> {
        if json {
            return Ok(Self::Json);
        }
        if ndjson {
            return Ok(Self::Ndjson);
        }
        if quiet {
            return Ok(Self::Quiet);
        }
        Ok(Self::Human)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presentation {
    Status,
    RecipeSearch { query: String },
    RecipeDetails,
    PlanList { from: String, to: String },
    PlanSet,
    PlanDelete,
}

pub struct CommandOutput {
    pub presentation: Presentation,
    pub values: Vec<Value>,
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

pub fn write_output(mode: OutputMode, output: CommandOutput) -> Result<String, AppError> {
    match mode {
        OutputMode::Human => Ok(write_human(&output.presentation, &output.values)),
        OutputMode::Json => serde_json::to_string_pretty(&output.values)
            .map(|json| format!("{json}\n"))
            .map_err(|error| AppError::new(ErrorCode::ApiError, error.to_string())),
        OutputMode::Ndjson => {
            let mut lines = String::new();
            for value in output.values {
                let line = serde_json::to_string(&value)
                    .map_err(|error| AppError::new(ErrorCode::ApiError, error.to_string()))?;
                lines.push_str(&line);
                lines.push('\n');
            }
            Ok(lines)
        }
        OutputMode::Quiet => Ok(write_quiet(&output.values)),
    }
}

fn write_human(presentation: &Presentation, values: &[Value]) -> String {
    match presentation {
        Presentation::Status => write_status(&values[0]),
        Presentation::RecipeSearch { query } => {
            if is_empty(values) {
                return format!("No recipes found for \"{query}\".\n");
            }
            table(
                &["NAME", "SLUG", "ID"],
                values
                    .iter()
                    .map(|value| vec![text(value, "name"), text(value, "slug"), text(value, "id")])
                    .collect(),
            )
        }
        Presentation::RecipeDetails => {
            let value = &values[0];
            format!(
                "Name: {}\nSlug: {}\nID:   {}\n{}",
                text(value, "name"),
                text(value, "slug"),
                text(value, "id"),
                write_ingredients(value)
            )
        }
        Presentation::PlanList { from, to } => {
            if is_empty(values) {
                return format!("No meal plan entries found from {from} to {to}.\n");
            }
            table(
                &["DATE", "MEAL", "TITLE", "RECIPE", "ID"],
                values
                    .iter()
                    .map(|value| {
                        vec![
                            text(value, "date"),
                            text(value, "meal"),
                            text(value, "title"),
                            text(value, "recipe"),
                            text(value, "id"),
                        ]
                    })
                    .collect(),
            )
        }
        Presentation::PlanSet => {
            let created = values
                .iter()
                .find(|value| value.get("type").and_then(Value::as_str) == Some("plan_created"))
                .expect("plan set always creates an entry");
            let verb = if values
                .iter()
                .any(|value| value.get("type").and_then(Value::as_str) == Some("plan_deleted"))
            {
                "Replaced"
            } else {
                "Created"
            };
            let label = non_empty_text(created, "recipe")
                .or_else(|| non_empty_text(created, "title"))
                .unwrap_or_else(|| "meal plan entry".to_string());
            format!(
                "{verb} {} on {} with {} (ID {}).\n",
                text(created, "meal"),
                text(created, "date"),
                label,
                text(created, "id")
            )
        }
        Presentation::PlanDelete => {
            format!("Deleted meal plan entry {}.\n", text(&values[0], "id"))
        }
    }
}

fn write_status(value: &Value) -> String {
    let url = match value.get("url").and_then(Value::as_str) {
        Some(url) if value.get("url_valid").and_then(Value::as_bool) == Some(true) => {
            format!("configured and valid ({url})")
        }
        Some(_) => "configured but invalid".to_string(),
        None => "not configured".to_string(),
    };
    let token = if value.get("token_configured").and_then(Value::as_bool) == Some(true) {
        "configured"
    } else {
        "not configured"
    };
    let server = optional_check(value, "server_reachable", "reachable", "unreachable");
    let authentication = optional_check(value, "authenticated", "successful", "failed");
    let summary = if value.get("ok").and_then(Value::as_bool) == Some(true) {
        "ready"
    } else {
        "action required"
    };

    format!(
        "Mealie status: {summary}\nURL:            {url}\nToken:          {token}\nServer:         {server}\nAuthentication: {authentication}\n"
    )
}

fn optional_check(value: &Value, key: &str, yes: &str, no: &str) -> String {
    match value.get(key) {
        Some(Value::Bool(true)) => yes.to_string(),
        Some(Value::Bool(false)) => no.to_string(),
        _ => "not checked".to_string(),
    }
}

fn write_ingredients(recipe: &Value) -> String {
    let ingredients = recipe
        .get("ingredients")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if ingredients.is_empty() {
        return "Ingredients: None listed\n".to_string();
    }

    let mut output = format!("Ingredients ({}):\n", ingredients.len());
    for ingredient in ingredients {
        if let Some(title) = non_empty_text(ingredient, "title") {
            output.push_str(&format!("{title}:\n"));
        }
        if let Some(line) = ingredient_line(ingredient) {
            output.push_str("- ");
            output.push_str(&line);
            output.push('\n');
        }
    }
    output
}

fn ingredient_line(ingredient: &Value) -> Option<String> {
    for key in ["original_text", "display"] {
        if let Some(line) = non_empty_text(ingredient, key) {
            return Some(line.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }

    let mut parts = Vec::new();
    for key in ["quantity", "unit_abbreviation", "unit", "food", "note"] {
        if let Some(part) = non_empty_text(ingredient, key) {
            parts.push(part);
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn write_quiet(values: &[Value]) -> String {
    let ids: Vec<_> = values
        .iter()
        .filter(|value| {
            matches!(
                value.get("type").and_then(Value::as_str),
                Some("plan_created" | "plan_deleted")
            )
        })
        .filter_map(|value| value.get("id"))
        .filter_map(value_text)
        .collect();

    if ids.is_empty() {
        String::new()
    } else {
        format!("{}\n", ids.join("\n"))
    }
}

fn is_empty(values: &[Value]) -> bool {
    values.len() == 1 && values[0].get("type").and_then(Value::as_str) == Some("empty")
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(value_text)
        .unwrap_or_else(|| "-".to_string())
}

fn non_empty_text(value: &Value, key: &str) -> Option<String> {
    let value = value.get(key).and_then(value_text)?;
    (!value.is_empty()).then_some(value)
}

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => None,
        _ => Some(value.to_string()),
    }
}

fn table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    let mut output = String::new();
    append_row(
        &mut output,
        &headers
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>(),
        &widths,
    );
    for row in rows {
        append_row(&mut output, &row, &widths);
    }
    output
}

fn append_row(output: &mut String, row: &[String], widths: &[usize]) {
    for (index, cell) in row.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(cell);
        if index + 1 < row.len() {
            output.extend(std::iter::repeat_n(
                ' ',
                widths[index] - cell.chars().count(),
            ));
        }
    }
    output.push('\n');
}
