mod cli;
mod client;
mod config;
mod error;
mod meal_type;
mod output;

pub use error::{AppError, ErrorCode};

use std::{env, ffi::OsString};

use clap::{Parser, error::ErrorKind};
use cli::{Cli, Command, PlanCommand, RecipesCommand};
use client::{MealieClient, PlanCreateRequest};
use config::Config;
use meal_type::MealType;
use output::{CommandOutput, OutputMode, Presentation, record, write_output};

pub fn run_from_env() -> Result<String, AppError> {
    run_from(env::args_os(), env::vars())
}

pub fn run_from<I, T, E, K, V>(args: I, env_vars: E) -> Result<String, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    E: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            return Ok(error.to_string());
        }
        Err(error) => return Err(AppError::invalid_args(error)),
    };
    let mode = OutputMode::from_flags(cli.json, cli.ndjson, cli.quiet)?;
    let config = Config::from_env(env_vars)?;
    let client = MealieClient::new(config)?;
    let output = execute(&client, cli.command)?;

    write_output(mode, output)
}

fn execute(client: &MealieClient, command: Command) -> Result<CommandOutput, AppError> {
    match command {
        Command::Recipes(recipes) => match recipes {
            RecipesCommand::Search { query, limit } => Ok(CommandOutput {
                presentation: Presentation::RecipeSearch {
                    query: query.clone(),
                },
                values: recipes_search(client, &query, limit)?,
            }),
            RecipesCommand::Get { slug } => Ok(CommandOutput {
                presentation: Presentation::RecipeDetails,
                values: recipes_get(client, &slug)?,
            }),
        },
        Command::Plan(plan) => match plan {
            PlanCommand::List {
                from,
                to,
                meal_type,
            } => Ok(CommandOutput {
                presentation: Presentation::PlanList {
                    from: from.clone(),
                    to: to.clone(),
                },
                values: plan_list(client, &from, &to, meal_type.as_ref())?,
            }),
            PlanCommand::Set(cli::PlanSetArgs {
                date,
                meal_type,
                title,
                recipe,
            }) => Ok(CommandOutput {
                presentation: Presentation::PlanSet,
                values: plan_set(
                    client,
                    &date,
                    meal_type,
                    title.as_deref(),
                    recipe.as_deref(),
                )?,
            }),
            PlanCommand::Delete { id } => Ok(CommandOutput {
                presentation: Presentation::PlanDelete,
                values: plan_delete(client, id)?,
            }),
        },
    }
}

fn recipes_search(
    client: &MealieClient,
    query: &str,
    limit: u32,
) -> Result<Vec<serde_json::Value>, AppError> {
    if query.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::InvalidArgs,
            "recipe search query cannot be empty",
        ));
    }
    let recipes = client.search_recipes(query, limit)?;

    if recipes.is_empty() {
        return Ok(vec![record(
            "empty",
            [
                ("resource", serde_json::json!("recipe")),
                ("query", serde_json::json!(query)),
            ],
        )]);
    }

    Ok(recipes
        .into_iter()
        .map(|recipe| {
            record(
                "recipe",
                [
                    ("id", serde_json::json!(recipe.id)),
                    ("slug", serde_json::json!(recipe.slug)),
                    ("name", serde_json::json!(recipe.name)),
                ],
            )
        })
        .collect())
}

fn recipes_get(client: &MealieClient, slug: &str) -> Result<Vec<serde_json::Value>, AppError> {
    let recipe = client.get_recipe(slug)?;

    Ok(vec![record(
        "recipe",
        [
            ("id", serde_json::json!(recipe.id)),
            ("slug", serde_json::json!(recipe.slug)),
            ("name", serde_json::json!(recipe.name)),
            ("ingredients", serde_json::json!(recipe.ingredients)),
        ],
    )])
}

fn plan_list(
    client: &MealieClient,
    from: &str,
    to: &str,
    meal_type: Option<&MealType>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let from_date = validate_date("--from", from)?;
    let to_date = validate_date("--to", to)?;
    if from_date > to_date {
        return Err(AppError::new(
            ErrorCode::InvalidArgs,
            format!("--from ({from}) must be on or before --to ({to})"),
        ));
    }
    let entries = client.list_plan(from, to)?;
    let records: Vec<_> = entries
        .into_iter()
        .filter(|entry| {
            meal_type
                .as_ref()
                .is_none_or(|expected| entry.meal.as_deref() == Some(expected.as_str()))
        })
        .map(|entry| {
            record(
                "plan_entry",
                [
                    ("id", serde_json::json!(entry.id)),
                    ("date", serde_json::json!(entry.date)),
                    ("meal", serde_json::json!(entry.meal)),
                    ("title", serde_json::json!(entry.title)),
                    ("recipeId", serde_json::json!(entry.recipe_id)),
                    ("recipe", serde_json::json!(entry.recipe)),
                ],
            )
        })
        .collect();

    if records.is_empty() {
        return Ok(vec![record(
            "empty",
            [
                ("resource", serde_json::json!("plan_entry")),
                ("from", serde_json::json!(from)),
                ("to", serde_json::json!(to)),
            ],
        )]);
    }

    Ok(records)
}

fn plan_set(
    client: &MealieClient,
    date: &str,
    meal_type: MealType,
    title: Option<&str>,
    recipe_slug: Option<&str>,
) -> Result<Vec<serde_json::Value>, AppError> {
    validate_date("--date", date)?;

    match (title, recipe_slug) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(AppError::new(
                ErrorCode::InvalidArgs,
                "provide exactly one of --title or --recipe",
            ));
        }
        _ => {}
    }

    let recipe = recipe_slug
        .map(|slug| client.get_recipe(slug))
        .transpose()?;
    let existing = client.list_plan(date, date)?;
    let mut output = Vec::new();

    for entry in existing.into_iter().filter(|entry| {
        entry.date.as_deref() == Some(date) && entry.meal.as_deref() == Some(meal_type.as_str())
    }) {
        client.delete_plan(entry.id)?;
        output.push(record(
            "plan_deleted",
            [
                ("id", serde_json::json!(entry.id)),
                ("date", serde_json::json!(entry.date)),
                ("meal", serde_json::json!(entry.meal)),
                ("title", serde_json::json!(entry.title)),
                ("recipe", serde_json::json!(entry.recipe)),
            ],
        ));
    }

    let create_request = if let Some(recipe) = recipe.as_ref() {
        PlanCreateRequest::recipe(date, meal_type.as_str(), &recipe.id)
    } else {
        PlanCreateRequest::title(date, meal_type.as_str(), title.unwrap_or_default())
    };

    let created = client.create_plan(&create_request)?;
    output.push(record(
        "plan_created",
        [
            ("id", serde_json::json!(created.id)),
            ("date", serde_json::json!(created.date)),
            ("meal", serde_json::json!(created.meal)),
            ("title", serde_json::json!(created.title)),
            ("recipe", serde_json::json!(created.recipe)),
            ("recipeId", serde_json::json!(created.recipe_id)),
        ],
    ));

    Ok(output)
}

fn plan_delete(client: &MealieClient, id: i64) -> Result<Vec<serde_json::Value>, AppError> {
    let deleted = client.delete_plan(id)?;

    Ok(vec![record(
        "plan_deleted",
        [
            ("id", serde_json::json!(deleted.id.unwrap_or(id))),
            ("date", serde_json::json!(deleted.date)),
            ("meal", serde_json::json!(deleted.meal)),
            ("title", serde_json::json!(deleted.title)),
            ("recipe", serde_json::json!(deleted.recipe)),
        ],
    )])
}

fn validate_date(flag: &str, value: &str) -> Result<chrono::NaiveDate, AppError> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        AppError::new(
            ErrorCode::InvalidArgs,
            format!("{flag} must use YYYY-MM-DD (got \"{value}\")"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(url: &str) -> Vec<(&str, &str)> {
        vec![("MEALIE_URL", url), ("MEALIE_TOKEN", "secret-token")]
    }

    #[test]
    fn requires_config() {
        let error = run_from(
            ["mealie", "recipes", "get", "slug"],
            Vec::<(&str, &str)>::new(),
        )
        .expect_err("config should be required");

        assert_eq!(error.code(), ErrorCode::MissingConfig);
    }

    #[test]
    fn validates_dates() {
        let error = run_from(
            [
                "mealie",
                "plan",
                "list",
                "--from",
                "2026-99-99",
                "--to",
                "2026-05-16",
            ],
            env("https://mealie.example"),
        )
        .expect_err("invalid date should fail");

        assert_eq!(error.code(), ErrorCode::InvalidArgs);
    }

    #[test]
    fn rejects_invalid_meal_type() {
        let error = run_from(
            [
                "mealie",
                "plan",
                "list",
                "--from",
                "2026-05-13",
                "--to",
                "2026-05-16",
                "--type",
                "brunch",
            ],
            env("https://mealie.example"),
        )
        .expect_err("invalid meal type should fail");

        assert_eq!(error.code(), ErrorCode::InvalidArgs);
    }

    #[test]
    fn formats_pretty_json() {
        let output = write_output(
            OutputMode::Json,
            CommandOutput {
                presentation: Presentation::RecipeSearch {
                    query: "missing".to_string(),
                },
                values: vec![record("empty", [("resource", serde_json::json!("recipe"))])],
            },
        )
        .expect("json output");

        assert!(output.starts_with("[\n"));
        assert!(output.contains("\"ok\": true"));
    }

    #[test]
    fn formats_multiple_quiet_ids() {
        let values = vec![
            record("plan_deleted", [("id", serde_json::json!(10))]),
            record("plan_created", [("id", serde_json::json!(11))]),
            record("empty", [("resource", serde_json::json!("recipe"))]),
        ];
        let output = write_output(
            OutputMode::Quiet,
            CommandOutput {
                presentation: Presentation::PlanSet,
                values,
            },
        )
        .expect("quiet output");

        assert_eq!(output, "10\n11\n");
    }
}
