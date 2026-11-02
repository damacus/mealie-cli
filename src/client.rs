use std::time::Duration;

use reqwest::{StatusCode, blocking::Client};
use serde::Serialize;
use serde_json::Value;

use crate::{AppError, ErrorCode, config::Config};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RECIPE_RESOLUTION_PAGE_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct MealieClient {
    config: Config,
    http: Client,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Recipe {
    pub raw: Value,
    pub id: String,
    pub slug: String,
    pub name: String,
    pub ingredients: Vec<Ingredient>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ingredient {
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub food: Option<String>,
    pub note: Option<String>,
    pub display: Option<String>,
    pub original_text: Option<String>,
    pub title: Option<String>,
    pub reference_id: Option<String>,
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
    pub fn bulk_import(&self, payload: &Value) -> Result<Value, AppError> {
        self.http
            .post(self.config.endpoint("/api/recipes/create/url/bulk"))
            .bearer_auth(&self.config.token)
            .json(payload)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "bulk import recipes"))
    }
    pub fn get_import_report(&self, id: &str) -> Result<Value, AppError> {
        self.http
            .get(self.config.endpoint(&format!("/api/groups/reports/{id}")))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "get import report"))
    }
    pub fn duplicate_recipe(&self, slug: &str, name: Option<&str>) -> Result<Value, AppError> {
        self.http
            .post(
                self.config
                    .endpoint(&format!("/api/recipes/{slug}/duplicate")),
            )
            .bearer_auth(&self.config.token)
            .json(&serde_json::json!({"name":name}))
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "duplicate recipe"))
    }
    pub fn set_last_made(&self, slug: &str, timestamp: &str) -> Result<Value, AppError> {
        self.http
            .patch(
                self.config
                    .endpoint(&format!("/api/recipes/{slug}/last-made")),
            )
            .bearer_auth(&self.config.token)
            .json(&serde_json::json!({"timestamp":timestamp}))
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "update last made"))
    }
    pub fn suggest_recipes(&self, query: &[(String, String)]) -> Result<Vec<Value>, AppError> {
        let value = self
            .http
            .get(self.config.endpoint("/api/recipes/suggestions"))
            .bearer_auth(&self.config.token)
            .query(query)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "suggest recipes"))?;
        value
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| AppError::new(ErrorCode::ApiError, "suggestion response missing items"))
    }
    pub fn list_bulk_exports(&self) -> Result<Value, AppError> {
        self.http
            .get(self.config.endpoint("/api/recipes/bulk-actions/export"))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "list recipe exports"))
    }
    pub fn download_bulk_export(&self, id: &str) -> Result<Vec<u8>, AppError> {
        let value = self
            .http
            .get(
                self.config
                    .endpoint(&format!("/api/recipes/bulk-actions/export/{id}/download")),
            )
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "get recipe export"))?;
        let token = value
            .get("fileToken")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                AppError::new(ErrorCode::ApiError, "export response missing file token")
            })?;
        let download_client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .build()
            .map_err(|error| AppError::from(error.without_url()))?;
        let response = download_client
            .get(self.config.endpoint("/api/utils/download"))
            .query(&[("token", token)])
            .send()
            .map_err(|error| AppError::from(error.without_url()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::new(
                if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
                    ErrorCode::Authentication
                } else {
                    ErrorCode::ApiError
                },
                format!("download recipe export failed (HTTP {status})"),
            ));
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| AppError::from(error.without_url()))
    }
    pub fn purge_bulk_exports(&self) -> Result<(), AppError> {
        self.http
            .delete(
                self.config
                    .endpoint("/api/recipes/bulk-actions/export/purge"),
            )
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_optional_json(response, "purge recipe exports"))?;
        Ok(())
    }
    pub fn bulk_action(&self, action: &str, payload: &Value) -> Result<(), AppError> {
        self.http
            .post(
                self.config
                    .endpoint(&format!("/api/recipes/bulk-actions/{action}")),
            )
            .bearer_auth(&self.config.token)
            .json(payload)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_optional_json(response, "bulk recipe update"))?;
        Ok(())
    }
    pub fn import_recipe(&self, path: &str, payload: &Value) -> Result<String, AppError> {
        let value = self
            .http
            .post(self.config.endpoint(path))
            .bearer_auth(&self.config.token)
            .json(payload)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "import recipe"))?;
        value
            .as_str()
            .map(str::to_owned)
            .filter(|slug| !slug.is_empty())
            .ok_or_else(|| {
                AppError::new(ErrorCode::ApiError, "import recipe response missing slug")
            })
    }
    pub fn export_formats(&self) -> Result<Value, AppError> {
        self.http
            .get(self.config.endpoint("/api/recipes/exports"))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "get export formats"))
    }
    pub fn export_recipe(&self, slug: &str, template: &str) -> Result<Vec<u8>, AppError> {
        let response = self
            .http
            .get(
                self.config
                    .endpoint(&format!("/api/recipes/{slug}/exports")),
            )
            .bearer_auth(&self.config.token)
            .query(&[("template_name", template)])
            .send()
            .map_err(AppError::from)?;
        if !response.status().is_success() {
            return checked_json(response, "export recipe").and_then(|_| {
                Err(AppError::new(
                    ErrorCode::ApiError,
                    "unexpected export response",
                ))
            });
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(AppError::from)
    }
    pub fn create_recipe(&self, name: &str) -> Result<String, AppError> {
        let value = self
            .http
            .post(self.config.endpoint("/api/recipes"))
            .bearer_auth(&self.config.token)
            .json(&serde_json::json!({"name":name}))
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "create recipe"))?;
        value
            .as_str()
            .map(str::to_owned)
            .filter(|slug| !slug.is_empty())
            .ok_or_else(|| {
                AppError::new(ErrorCode::ApiError, "create recipe response missing slug")
            })
    }
    pub fn delete_recipe(&self, slug: &str) -> Result<(), AppError> {
        self.http
            .delete(self.config.endpoint(&format!("/api/recipes/{slug}")))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_optional_json(response, "delete recipe"))?;
        Ok(())
    }
    pub fn set_image_url(&self, slug: &str, url: &str) -> Result<Value, AppError> {
        self.http
            .post(self.config.endpoint(&format!("/api/recipes/{slug}/image")))
            .bearer_auth(&self.config.token)
            .json(&serde_json::json!({"url":url}))
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "set recipe image"))
    }
    pub fn delete_image(&self, slug: &str) -> Result<(), AppError> {
        self.http
            .delete(self.config.endpoint(&format!("/api/recipes/{slug}/image")))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_optional_json(response, "delete recipe image"))?;
        Ok(())
    }
    pub fn merge_organizer(&self, path: &str, from: &str, to: &str) -> Result<Value, AppError> {
        self.http
            .post(self.config.endpoint(&format!("{path}/merge")))
            .bearer_auth(&self.config.token)
            .json(&serde_json::json!({"fromId":from,"toId":to}))
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "merge organiser"))
    }
    pub fn delete_organizer(&self, path: &str) -> Result<(), AppError> {
        self.http
            .delete(self.config.endpoint(path))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_optional_json(response, "delete organiser"))?;
        Ok(())
    }
    pub fn get_organizer(&self, path: &str) -> Result<Value, AppError> {
        self.http
            .get(self.config.endpoint(path))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "get organiser"))
    }

    pub fn rename_organizer(&self, path: &str, name: &str) -> Result<Value, AppError> {
        self.http
            .put(self.config.endpoint(path))
            .bearer_auth(&self.config.token)
            .json(&serde_json::json!({"name": name}))
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "update organiser"))
    }
    pub fn create_organizer(&self, path: &str, name: &str) -> Result<Value, AppError> {
        self.http
            .post(self.config.endpoint(path))
            .bearer_auth(&self.config.token)
            .json(&serde_json::json!({"name": name}))
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "create organiser"))
    }
    pub fn update_recipe(&self, slug: &str, patch: &Value) -> Result<Value, AppError> {
        self.http
            .patch(self.config.endpoint(&format!("/api/recipes/{slug}")))
            .bearer_auth(&self.config.token)
            .json(patch)
            .send()
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "update recipe"))
    }
    pub fn list_recipes(&self) -> Result<Vec<Value>, AppError> {
        self.list_collection("/api/recipes")
    }

    pub fn list_collection(&self, path: &str) -> Result<Vec<Value>, AppError> {
        let mut page = 1_u64;
        let mut result = Vec::new();
        loop {
            let value = self
                .http
                .get(self.config.endpoint(path))
                .bearer_auth(&self.config.token)
                .query(&[("perPage", "100".to_string()), ("page", page.to_string())])
                .send()
                .map_err(AppError::from)
                .and_then(|response| checked_json(response, "list recipes"))?;
            let items = value
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    AppError::new(ErrorCode::ApiError, "recipe list response missing items")
                })?;
            let item_count = items.len();
            result.extend(items.iter().cloned());
            let total_pages = value
                .get("total_pages")
                .or_else(|| value.get("totalPages"))
                .and_then(Value::as_u64);
            if total_pages.is_some_and(|total| page >= total)
                || total_pages.is_none() && item_count < 100
            {
                break;
            }
            page += 1;
        }
        Ok(result)
    }
    pub fn new(config: Config) -> Result<Self, AppError> {
        let http = Client::builder()
            // Bound external API calls so an unresponsive Mealie server cannot hang automation indefinitely.
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| AppError::new(ErrorCode::NetworkError, error.to_string()))?;

        Ok(Self { config, http })
    }

    pub fn check_authentication(&self) -> Result<(), AppError> {
        let response = self
            .http
            .get(self.config.endpoint("/api/users/self"))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(AppError::from)?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let code = match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ErrorCode::Authentication,
            _ => ErrorCode::ApiError,
        };
        Err(AppError::new(
            code,
            format!("status check failed (HTTP {status})"),
        ))
    }

    pub fn search_recipes(&self, query: &str, limit: u32) -> Result<Vec<Recipe>, AppError> {
        let value = self
            .http
            .get(self.config.endpoint("/api/recipes"))
            .bearer_auth(&self.config.token)
            .query(&[("search", query), ("perPage", &limit.to_string())])
            .send()
            .map_err(AppError::from)
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
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "get recipe"))?;

        recipe_from_value(&value)
    }

    pub fn resolve_recipe(&self, query: &str) -> Result<Recipe, AppError> {
        match self.get_recipe(query) {
            Ok(recipe) => Ok(recipe),
            Err(not_found) if not_found.code() == ErrorCode::NotFound => {
                self.resolve_recipe_name(query, not_found)
            }
            Err(error) => Err(error),
        }
    }

    pub fn resolve_recipe_details(&self, query: &str) -> Result<Recipe, AppError> {
        match self.get_recipe(query) {
            Ok(recipe) => Ok(recipe),
            Err(error) if error.code() == ErrorCode::NotFound => {
                let summary = self.resolve_recipe_name(query, error)?;
                self.get_recipe(&summary.slug)
            }
            Err(error) => Err(error),
        }
    }

    fn resolve_recipe_name(&self, query: &str, not_found: AppError) -> Result<Recipe, AppError> {
        let normalized_query = query.to_lowercase();
        let mut page = 1;
        let mut matches = Vec::new();

        loop {
            let mut parameters = vec![
                ("search", query.to_string()),
                ("perPage", RECIPE_RESOLUTION_PAGE_SIZE.to_string()),
            ];
            if page > 1 {
                parameters.push(("page", page.to_string()));
            }

            let value = self
                .http
                .get(self.config.endpoint("/api/recipes"))
                .bearer_auth(&self.config.token)
                .query(&parameters)
                .send()
                .map_err(AppError::from)
                .and_then(|response| checked_json(response, "search recipes"))?;

            let items = collection_items(&value);
            let item_count = items.len();
            matches.extend(
                items
                    .iter()
                    .map(recipe_from_value)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|recipe| recipe.name.to_lowercase() == normalized_query),
            );

            let total_pages = value
                .get("total_pages")
                .or_else(|| value.get("totalPages"))
                .and_then(Value::as_u64);
            if total_pages.is_some_and(|last_page| page >= last_page as usize)
                || total_pages.is_none() && item_count < RECIPE_RESOLUTION_PAGE_SIZE
            {
                break;
            }

            page += 1;
        }

        match matches.len() {
            0 => Err(not_found),
            1 => Ok(matches.pop().expect("one recipe match")),
            _ => Err(AppError::new(
                ErrorCode::Ambiguous,
                format!(
                    "recipe name \"{query}\" matches multiple recipes: {}",
                    matches
                        .iter()
                        .map(|recipe| format!("{} ({})", recipe.name, recipe.slug))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }

    pub fn list_plan(&self, from: &str, to: &str) -> Result<Vec<PlanEntry>, AppError> {
        const PAGE_SIZE: usize = 100;

        let mut page = 1;
        let mut entries = Vec::new();

        loop {
            let mut query = vec![
                ("start_date", from.to_string()),
                ("end_date", to.to_string()),
                ("perPage", PAGE_SIZE.to_string()),
                ("orderBy", "date".to_string()),
                ("orderDirection", "asc".to_string()),
            ];
            if page > 1 {
                query.push(("page", page.to_string()));
            }

            let value = self
                .http
                .get(self.config.endpoint("/api/households/mealplans"))
                .bearer_auth(&self.config.token)
                .query(&query)
                .send()
                .map_err(AppError::from)
                .and_then(|response| checked_json(response, "list meal plans"))?;

            let items = collection_items(&value);
            let item_count = items.len();
            entries.extend(
                items
                    .iter()
                    .map(plan_entry_from_value)
                    .collect::<Result<Vec<_>, _>>()?,
            );

            let total_pages = value
                .get("total_pages")
                .or_else(|| value.get("totalPages"))
                .and_then(Value::as_u64);
            if total_pages.is_some_and(|last_page| page >= last_page as usize)
                || total_pages.is_none() && item_count < PAGE_SIZE
            {
                break;
            }

            page += 1;
        }

        Ok(entries)
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
            .map_err(AppError::from)
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
            .map_err(AppError::from)
            .and_then(|response| checked_json(response, "create meal plan"))?;

        plan_entry_from_value(&value)
    }
}

fn checked_json(response: reqwest::blocking::Response, action: &str) -> Result<Value, AppError> {
    let status = response.status();
    if status.is_success() {
        return response.json().map_err(AppError::from);
    }

    let code = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ErrorCode::Authentication,
        StatusCode::NOT_FOUND => ErrorCode::NotFound,
        _ => ErrorCode::ApiError,
    };
    let message = if code == ErrorCode::NotFound {
        not_found_message(action)
    } else {
        let detail = response.text().ok().and_then(|body| error_detail(&body));
        match detail {
            Some(detail) => format!("{action}: {detail} (HTTP {status})"),
            None => format!("{action} failed (HTTP {status})"),
        }
    };
    Err(AppError::new(code, message))
}

fn not_found_message(action: &str) -> String {
    let (verb, resource) = action.split_once(' ').unwrap_or((action, "resource"));
    let action = match verb {
        "get" => "getting",
        "search" => "searching",
        "list" => "listing",
        "delete" => "deleting",
        "create" => "creating",
        _ => verb,
    };
    let mut characters = resource.chars();
    let resource_name = match characters.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), characters.as_str()),
        None => "Resource".to_string(),
    };
    format!("{action} {resource}: {resource_name} not found")
}

fn checked_optional_json(
    response: reqwest::blocking::Response,
    action: &str,
) -> Result<Option<Value>, AppError> {
    let status = response.status();
    if status == StatusCode::NO_CONTENT {
        return Ok(None);
    }

    checked_json(response, action).map(Some)
}

fn error_detail(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return nested_error_message(&value).map(|detail| detail.chars().take(200).collect());
    }

    let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(compact.chars().take(200).collect())
}

fn nested_error_message(value: &Value) -> Option<&str> {
    match value {
        Value::String(message) if !message.trim().is_empty() => Some(message),
        Value::Object(object) => ["message", "detail", "error", "exception"]
            .iter()
            .filter_map(|key| object.get(*key))
            .find_map(nested_error_message),
        Value::Array(values) => values.iter().find_map(nested_error_message),
        _ => None,
    }
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
        raw: value.clone(),
        id: required_string(value, &["id", "recipeId"])?,
        slug: required_string(value, &["slug"])?,
        name: required_string(value, &["name"])?,
        ingredients: value
            .get("recipeIngredient")
            .and_then(Value::as_array)
            .map(|ingredients| {
                ingredients
                    .iter()
                    .filter_map(ingredient_from_value)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn ingredient_from_value(value: &Value) -> Option<Ingredient> {
    let ingredient = Ingredient {
        quantity: optional_f64(value, &["quantity"]),
        unit: nested_optional_string(value, "unit", &["name"]),
        unit_abbreviation: nested_optional_string(value, "unit", &["abbreviation"]),
        food: nested_optional_string(value, "food", &["name"]),
        note: optional_non_empty_string(value, &["note"]),
        display: optional_non_empty_string(value, &["display"]),
        original_text: optional_non_empty_string(value, &["originalText"]),
        title: optional_non_empty_string(value, &["title"]),
        reference_id: optional_non_empty_string(value, &["referenceId"]),
    };

    (ingredient.quantity.is_some()
        || ingredient.food.is_some()
        || ingredient.display.is_some()
        || ingredient.original_text.is_some()
        || ingredient.title.is_some())
    .then_some(ingredient)
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

fn optional_non_empty_string(value: &Value, keys: &[&str]) -> Option<String> {
    optional_string(value, keys).and_then(|text| (!text.trim().is_empty()).then_some(text))
}

fn nested_optional_string(value: &Value, key: &str, keys: &[&str]) -> Option<String> {
    value
        .get(key)
        .and_then(|nested| optional_non_empty_string(nested, keys))
}

fn optional_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|value| match value {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.parse().ok(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configures_bounded_request_timeout() {
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn extracts_message_from_nested_api_error() {
        let body = r#"{"detail":{"message":"No Entry Found","error":true}}"#;

        assert_eq!(error_detail(body).as_deref(), Some("No Entry Found"));
    }

    #[test]
    fn does_not_expose_unrecognized_json_error_bodies() {
        assert_eq!(error_detail(r#"{"error":true}"#), None);
    }

    #[test]
    fn describes_not_found_errors_without_http_jargon() {
        assert_eq!(
            not_found_message("get recipe"),
            "getting recipe: Recipe not found"
        );
    }
}
