use reqwest::{StatusCode, blocking::Client};
use serde::Serialize;
use serde_json::Value;

use crate::{AppError, ErrorCode, config::Config};

#[derive(Debug, Clone)]
pub struct MealieClient {
    config: Config,
    http: Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    pub id: i64,
    pub date: Option<String>,
    pub meal: Option<String>,
    pub title: Option<String>,
    pub recipe_id: Option<String>,
    pub recipe: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedPlanEntry {
    pub id: Option<i64>,
    pub date: Option<String>,
    pub meal: Option<String>,
    pub title: Option<String>,
    pub recipe: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlanCreateRequest<'a> {
    date: &'a str,
    #[serde(rename = "entryType")]
    entry_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(rename = "recipeId", skip_serializing_if = "Option::is_none")]
    recipe_id: Option<&'a str>,
}

impl<'a> PlanCreateRequest<'a> {
    pub fn title(date: &'a str, meal_type: &'a str, title: &'a str) -> Self {
        Self {
            date,
            entry_type: meal_type,
            title: Some(title),
            text: Some(""),
            recipe_id: None,
        }
    }

    pub fn recipe(date: &'a str, meal_type: &'a str, recipe_id: &'a str) -> Self {
        Self {
            date,
            entry_type: meal_type,
            title: None,
            text: None,
            recipe_id: Some(recipe_id),
        }
    }
}

impl MealieClient {
    pub fn new(config: Config) -> Result<Self, AppError> {
        let http = Client::builder()
            .build()
            .map_err(|error| AppError::new(ErrorCode::NetworkError, error.to_string()))?;

        Ok(Self { config, http })
    }

    pub fn search_recipes(&self, query: &str, limit: u32) -> Result<Vec<Recipe>, AppError> {
        let value = self
            .http
            .get(self.config.endpoint("/api/recipes"))
            .bearer_auth(&self.config.token)
            .query(&[("search", query), ("perPage", &limit.to_string())])
            .send()
            .and_then(|response| checked_json(response, "search recipes"))?;

        collection_items(&value)
            .iter()
            .map(recipe_from_value)
            .collect()
    }

    pub fn get_recipe(&self, slug: &str) -> Result<Recipe, AppError> {
        let value = self
            .http
            .get(self.config.endpoint(&format!("/api/recipes/{slug}")))
            .bearer_auth(&self.config.token)
            .send()
            .and_then(|response| checked_json(response, "get recipe"))?;

        recipe_from_value(&value)
    }

    pub fn list_plan(&self, from: &str, to: &str) -> Result<Vec<PlanEntry>, AppError> {
        let value = self
            .http
            .get(self.config.endpoint("/api/households/mealplans"))
            .bearer_auth(&self.config.token)
            .query(&[
                ("start_date", from),
                ("end_date", to),
                ("perPage", "100"),
                ("orderBy", "date"),
                ("orderDirection", "asc"),
            ])
            .send()
            .and_then(|response| checked_json(response, "list meal plans"))?;

        collection_items(&value)
            .iter()
            .map(plan_entry_from_value)
            .collect()
    }

    pub fn delete_plan(&self, id: i64) -> Result<DeletedPlanEntry, AppError> {
        let value = self
            .http
            .delete(
                self.config
                    .endpoint(&format!("/api/households/mealplans/{id}")),
            )
            .bearer_auth(&self.config.token)
            .send()
            .and_then(|response| checked_optional_json(response, "delete meal plan"))?;

        Ok(value
            .as_ref()
            .map(deleted_plan_entry_from_value)
            .transpose()?
            .unwrap_or(DeletedPlanEntry {
                id: Some(id),
                date: None,
                meal: None,
                title: None,
                recipe: None,
            }))
    }

    pub fn create_plan(&self, request: &PlanCreateRequest<'_>) -> Result<PlanEntry, AppError> {
        let value = self
            .http
            .post(self.config.endpoint("/api/households/mealplans"))
            .bearer_auth(&self.config.token)
            .json(request)
            .send()
            .and_then(|response| checked_json(response, "create meal plan"))?;

        plan_entry_from_value(&value)
    }
}

fn checked_json(
    response: reqwest::blocking::Response,
    action: &str,
) -> Result<Value, reqwest::Error> {
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(
            serde_json::json!({ "__error": "not_found", "message": format!("{action} returned 404") }),
        );
    }

    if !status.is_success() {
        return Ok(
            serde_json::json!({ "__error": "api_error", "message": format!("{action} returned HTTP {status}") }),
        );
    }

    response.json()
}

fn checked_optional_json(
    response: reqwest::blocking::Response,
    action: &str,
) -> Result<Option<Value>, reqwest::Error> {
    let status = response.status();
    if status == StatusCode::NO_CONTENT {
        return Ok(None);
    }

    checked_json(response, action).map(Some)
}

fn collection_items(value: &Value) -> &[Value] {
    if let Some(items) = value.get("items").and_then(Value::as_array) {
        return items;
    }

    if let Some(data) = value.get("data").and_then(Value::as_array) {
        return data;
    }

    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn recipe_from_value(value: &Value) -> Result<Recipe, AppError> {
    check_error_value(value)?;

    Ok(Recipe {
        id: required_string(value, &["id", "recipeId"])?,
        slug: required_string(value, &["slug"])?,
        name: required_string(value, &["name"])?,
    })
}

fn plan_entry_from_value(value: &Value) -> Result<PlanEntry, AppError> {
    check_error_value(value)?;

    Ok(PlanEntry {
        id: required_i64(value, &["id"])?,
        date: optional_string(value, &["date"]),
        meal: optional_string(value, &["entryType", "meal", "type"]),
        title: optional_string(value, &["title", "text", "name"]).or_else(|| {
            value
                .get("recipe")
                .and_then(|recipe| optional_string(recipe, &["name"]))
        }),
        recipe_id: optional_string(value, &["recipeId"]).or_else(|| {
            value
                .get("recipe")
                .and_then(|recipe| optional_string(recipe, &["id"]))
        }),
        recipe: value
            .get("recipe")
            .and_then(|recipe| optional_string(recipe, &["name"]))
            .or_else(|| optional_string(value, &["recipeName"])),
    })
}

fn deleted_plan_entry_from_value(value: &Value) -> Result<DeletedPlanEntry, AppError> {
    check_error_value(value)?;

    Ok(DeletedPlanEntry {
        id: optional_i64(value, &["id"]),
        date: optional_string(value, &["date"]),
        meal: optional_string(value, &["entryType", "meal", "type"]),
        title: optional_string(value, &["title", "text", "name"]),
        recipe: value
            .get("recipe")
            .and_then(|recipe| optional_string(recipe, &["name"]))
            .or_else(|| optional_string(value, &["recipeName"])),
    })
}

fn check_error_value(value: &Value) -> Result<(), AppError> {
    match value.get("__error").and_then(Value::as_str) {
        Some("not_found") => Err(AppError::new(
            ErrorCode::NotFound,
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("resource not found"),
        )),
        Some(_) => Err(AppError::new(
            ErrorCode::ApiError,
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("api error"),
        )),
        None => Ok(()),
    }
}

fn required_string(value: &Value, keys: &[&str]) -> Result<String, AppError> {
    optional_string(value, keys).ok_or_else(|| {
        AppError::new(
            ErrorCode::ApiError,
            format!("response missing string field: {}", keys.join("|")),
        )
    })
}

fn optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
}

fn required_i64(value: &Value, keys: &[&str]) -> Result<i64, AppError> {
    optional_i64(value, keys).ok_or_else(|| {
        AppError::new(
            ErrorCode::ApiError,
            format!("response missing integer field: {}", keys.join("|")),
        )
    })
}

fn optional_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.parse().ok(),
            _ => None,
        })
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_decode() {
            return Self::new(ErrorCode::ApiError, error.to_string());
        }

        Self::new(ErrorCode::NetworkError, error.to_string())
    }
}
