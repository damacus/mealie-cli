mod cli;
mod client;
mod config;
mod date_input;
mod error;
mod meal_type;
mod output;

pub use error::{AppError, ErrorCode};

use std::{env, ffi::OsString};

use clap::{CommandFactory, Parser, error::ErrorKind};
use cli::{Cli, Command, PlanCommand, RecipesCommand};
use client::{MealieClient, PlanCreateRequest};
use config::{Config, HTTPS_REQUIRED_MESSAGE, validate_base_url};
use date_input::{parse_date_input, resolve_plan_range};
use meal_type::MealType;
use output::{CommandOutput, OutputMode, Presentation, record, write_output};

pub fn run_from_env() -> Result<String, AppError> {
    run_from(env::args_os(), env::vars())
}

pub struct RunResult {
    pub output: String,
    pub exit_code: u8,
}

pub fn run_from_env_with_exit() -> Result<RunResult, AppError> {
    run_from_with_exit(env::args_os(), env::vars())
}

pub fn run_from<I, T, E, K, V>(args: I, env_vars: E) -> Result<String, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    E: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    run_from_with_exit(args, env_vars).map(|result| result.output)
}

fn run_from_with_exit<I, T, E, K, V>(args: I, env_vars: E) -> Result<RunResult, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    E: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    run_from_with_exit_at(args, env_vars, chrono::Local::now().date_naive())
}

fn run_from_with_exit_at<I, T, E, K, V>(
    args: I,
    env_vars: E,
    today: chrono::NaiveDate,
) -> Result<RunResult, AppError>
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
                ErrorKind::DisplayHelp
                    | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    | ErrorKind::DisplayVersion
            ) =>
        {
            return Ok(RunResult {
                output: error.to_string(),
                exit_code: 0,
            });
        }
        Err(error) => return Err(AppError::invalid_args(error)),
    };
    let mode = OutputMode::from_flags(cli.json, cli.ndjson, cli.quiet)?;
    let env_vars: Vec<(String, String)> = env_vars
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect();

    let command = match cli.command {
        Command::Completion { shell } => {
            let mut command = Cli::command();
            let mut output = Vec::new();
            clap_complete::generate(shell, &mut command, "mealie", &mut output);
            return Ok(RunResult {
                output: String::from_utf8(output)
                    .map_err(|error| AppError::new(ErrorCode::ApiError, error.to_string()))?,
                exit_code: 0,
            });
        }
        Command::Status => {
            let (output, exit_code) = status(&env_vars);
            return Ok(RunResult {
                output: write_output(mode, output)?,
                exit_code,
            });
        }
        command => command,
    };

    let config = Config::from_env(env_vars)?;
    let client = MealieClient::new(config)?;
    let output = execute(&client, command, today)?;

    Ok(RunResult {
        output: write_output(mode, output)?,
        exit_code: 0,
    })
}

fn status(env_vars: &[(String, String)]) -> (CommandOutput, u8) {
    let value = |name: &str| {
        env_vars
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let configured_url = value("MEALIE_URL");
    let token = value("MEALIE_TOKEN");
    let allow_insecure_http =
        value("USE_INSECURE_HTTP").is_some_and(|value| value.eq_ignore_ascii_case("yes"));
    let validated_url = configured_url
        .as_deref()
        .map(|url| validate_base_url(url, allow_insecure_http));

    let (url_valid, server_reachable, authenticated, failure) = match (&validated_url, &token) {
        (None, _) => (None, None, None, Some(StatusFailure::MissingConfig)),
        (Some(Err(error)), _) => (Some(false), None, None, Some(status_url_failure(error))),
        (Some(Ok(_)), None) => (Some(true), None, None, Some(StatusFailure::MissingConfig)),
        (Some(Ok(url)), Some(token)) => {
            let config = Config {
                base_url: url.clone(),
                token: token.clone(),
            };
            match MealieClient::new(config).and_then(|client| client.check_authentication()) {
                Ok(()) => (Some(true), Some(true), Some(true), None),
                Err(error) if error.code() == ErrorCode::NetworkError => {
                    (Some(true), Some(false), None, Some(StatusFailure::Network))
                }
                Err(error) if error.code() == ErrorCode::Authentication => (
                    Some(true),
                    Some(true),
                    Some(false),
                    Some(StatusFailure::Authentication),
                ),
                Err(_) => (
                    Some(true),
                    Some(true),
                    Some(false),
                    Some(StatusFailure::ApiResponse),
                ),
            }
        }
    };
    let error = failure.map(StatusFailure::code);
    let exit_code = error.map_or(0, |code| {
        AppError::new(code, "status check failed").exit_code()
    });
    let url_configured = configured_url.is_some();
    let token_configured = token.is_some();
    let mut status = serde_json::json!({
        "ok": error.is_none(),
        "type": "status",
        "url": configured_url,
        "url_configured": url_configured,
        "url_valid": url_valid,
        "token_configured": token_configured,
        "server_reachable": server_reachable,
        "authenticated": authenticated,
        "error": error.map(ErrorCode::as_str),
    });
    if let Some(failure) = failure {
        status["hint"] = serde_json::json!(status_hint(failure));
    }

    (
        CommandOutput {
            presentation: Presentation::Status,
            values: vec![status],
        },
        exit_code,
    )
}

#[derive(Clone, Copy)]
enum StatusFailure {
    MissingConfig,
    InvalidUrl,
    InsecureHttp,
    Authentication,
    Network,
    ApiResponse,
}

impl StatusFailure {
    fn code(self) -> ErrorCode {
        match self {
            Self::MissingConfig => ErrorCode::MissingConfig,
            Self::InvalidUrl | Self::InsecureHttp => ErrorCode::InvalidArgs,
            Self::Authentication => ErrorCode::Authentication,
            Self::Network => ErrorCode::NetworkError,
            Self::ApiResponse => ErrorCode::ApiError,
        }
    }
}

fn status_url_failure(error: &AppError) -> StatusFailure {
    if error.to_string() == HTTPS_REQUIRED_MESSAGE {
        StatusFailure::InsecureHttp
    } else {
        StatusFailure::InvalidUrl
    }
}

fn status_hint(failure: StatusFailure) -> &'static str {
    match failure {
        StatusFailure::MissingConfig => {
            "Set MEALIE_URL and MEALIE_TOKEN, then run `mealie status` again."
        }
        StatusFailure::InvalidUrl => {
            "Set MEALIE_URL to a valid URL, then run `mealie status` again."
        }
        StatusFailure::InsecureHttp => {
            "Use an HTTPS MEALIE_URL, or set USE_INSECURE_HTTP=yes only for a trusted local server, then run `mealie status` again."
        }
        StatusFailure::Authentication => {
            "Check MEALIE_TOKEN and confirm it has access to this Mealie instance, then run `mealie status` again."
        }
        StatusFailure::Network => {
            "Check MEALIE_URL and confirm the Mealie server is reachable, then run `mealie status` again."
        }
        StatusFailure::ApiResponse => {
            "Mealie returned an unexpected response while checking authentication. Check the Mealie server logs and reverse proxy, then run `mealie status` again."
        }
    }
}

fn execute(
    client: &MealieClient,
    command: Command,
    today: chrono::NaiveDate,
) -> Result<CommandOutput, AppError> {
    match command {
        Command::Completion { .. } => unreachable!("completion output is generated before config"),
        Command::Status => unreachable!("status is handled before client creation"),
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
            } => {
                let (from, to) = resolve_plan_range(from.as_deref(), to.as_deref(), today)?;
                let from = from.to_string();
                let to = to.to_string();
                Ok(CommandOutput {
                    presentation: Presentation::PlanList {
                        from: from.clone(),
                        to: to.clone(),
                    },
                    values: plan_list(client, &from, &to, meal_type.as_ref())?,
                })
            }
            PlanCommand::Set(cli::PlanSetArgs {
                date,
                meal_type,
                target: cli::PlanSetTargetArgs { title, recipe },
            }) => {
                let date = parse_date_input("--date", &date, today)?.to_string();
                Ok(CommandOutput {
                    presentation: Presentation::PlanSet,
                    values: plan_set(
                        client,
                        &date,
                        meal_type,
                        title.as_deref(),
                        recipe.as_deref(),
                    )?,
                })
            }
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

fn recipes_get(
    client: &MealieClient,
    recipe_query: &str,
) -> Result<Vec<serde_json::Value>, AppError> {
    let recipe = client.resolve_recipe(recipe_query)?;

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
    recipe_query: Option<&str>,
) -> Result<Vec<serde_json::Value>, AppError> {
    match (title, recipe_query) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(AppError::new(
                ErrorCode::InvalidArgs,
                "provide exactly one of --title or --recipe",
            ));
        }
        _ => {}
    }

    let recipe = recipe_query
        .map(|query| client.resolve_recipe(query))
        .transpose()?;
    let existing = client
        .list_plan(date, date)?
        .into_iter()
        .filter(|entry| {
            entry.date.as_deref() == Some(date) && entry.meal.as_deref() == Some(meal_type.as_str())
        })
        .collect::<Vec<_>>();

    if existing.len() > 1 {
        return Err(AppError::new(
            ErrorCode::ApiError,
            "multiple existing plan entries match this date and meal type; remove duplicates before replacing",
        ));
    }

    let create_request = if let Some(recipe) = recipe.as_ref() {
        PlanCreateRequest::recipe(date, meal_type.as_str(), &recipe.id)
    } else {
        PlanCreateRequest::title(date, meal_type.as_str(), title.unwrap_or_default())
    };

    let created = client.create_plan(&create_request)?;
    let mut output = vec![record(
        "plan_created",
        [
            ("id", serde_json::json!(created.id)),
            ("date", serde_json::json!(created.date)),
            ("meal", serde_json::json!(created.meal)),
            ("title", serde_json::json!(created.title)),
            ("recipe", serde_json::json!(created.recipe)),
            ("recipeId", serde_json::json!(created.recipe_id)),
        ],
    )];

    if let Some(entry) = existing.into_iter().next() {
        if let Err(delete_error) = client.delete_plan(entry.id) {
            let rollback = client.delete_plan(created.id);
            let message = if rollback.is_ok() {
                "replacement could not remove the original entry; the new entry was rolled back"
            } else {
                "replacement could not remove the original entry and the new entry could not be rolled back"
            };

            return Err(AppError::new(
                delete_error.code(),
                format!("{message}: {delete_error}"),
            ));
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    fn env(url: &str) -> Vec<(&str, &str)> {
        vec![
            ("MEALIE_URL", url),
            ("MEALIE_TOKEN", "secret-token"),
            ("USE_INSECURE_HTTP", "yes"),
        ]
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
    fn generates_fish_completion_without_configuration() {
        let output = run_from(["mealie", "completion", "fish"], Vec::<(&str, &str)>::new())
            .expect("fish completion");

        assert!(output.contains("complete -c mealie"));
        assert!(output.contains("completion"));
        assert!(output.contains("quiet"));
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

    #[test]
    fn fixed_today_requests_the_remainder_of_its_week() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 13).expect("valid date");
        let mut server = Server::new();
        let _mock = server
            .mock("GET", "/api/households/mealplans")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("start_date".into(), "2026-05-13".into()),
                Matcher::UrlEncoded("end_date".into(), "2026-05-17".into()),
                Matcher::UrlEncoded("perPage".into(), "100".into()),
                Matcher::UrlEncoded("orderBy".into(), "date".into()),
                Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
            ]))
            .with_status(200)
            .with_body(r#"{"items":[]}"#)
            .create();

        let output = run_from_with_exit_at(
            ["mealie", "--ndjson", "plan", "list"],
            env(&server.url()),
            today,
        )
        .expect("default plan list")
        .output;
        let record: serde_json::Value = serde_json::from_str(&output).expect("empty plan record");

        assert_eq!(record["from"], "2026-05-13");
        assert_eq!(record["to"], "2026-05-17");
    }

    #[test]
    fn fixed_today_normalizes_space_separated_negative_relative_plan_list_requests() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 13).expect("valid date");
        let mut server = Server::new();
        let _mock = server
            .mock("GET", "/api/households/mealplans")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("start_date".into(), "2026-05-06".into()),
                Matcher::UrlEncoded("end_date".into(), "2026-05-12".into()),
                Matcher::UrlEncoded("perPage".into(), "100".into()),
                Matcher::UrlEncoded("orderBy".into(), "date".into()),
                Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
            ]))
            .with_status(200)
            .with_body(r#"{"items":[]}"#)
            .create();

        let output = run_from_with_exit_at(
            [
                "mealie", "--ndjson", "plan", "list", "--from", "-1w", "--to", "-1d",
            ],
            env(&server.url()),
            today,
        )
        .expect("negative relative plan list")
        .output;
        let record: serde_json::Value = serde_json::from_str(&output).expect("empty plan record");

        assert_eq!(record["from"], "2026-05-06");
        assert_eq!(record["to"], "2026-05-12");
    }

    #[test]
    fn fixed_today_normalizes_space_separated_negative_relative_plan_set_requests() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 13).expect("valid date");
        let mut server = Server::new();
        let _list = server
            .mock("GET", "/api/households/mealplans")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("start_date".into(), "2026-05-12".into()),
                Matcher::UrlEncoded("end_date".into(), "2026-05-12".into()),
                Matcher::UrlEncoded("perPage".into(), "100".into()),
                Matcher::UrlEncoded("orderBy".into(), "date".into()),
                Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
            ]))
            .with_status(200)
            .with_body(r#"{"items":[]}"#)
            .create();
        let _create = server
            .mock("POST", "/api/households/mealplans")
            .match_body(Matcher::JsonString(
                r#"{"date":"2026-05-12","entryType":"dinner","title":"Soup","text":""}"#.into(),
            ))
            .with_status(200)
            .with_body(r#"{"id":1,"date":"2026-05-12","entryType":"dinner","title":"Soup"}"#)
            .create();

        let output = run_from_with_exit_at(
            [
                "mealie", "--ndjson", "plan", "set", "--date", "-1d", "--type", "dinner",
                "--title", "Soup",
            ],
            env(&server.url()),
            today,
        )
        .expect("relative plan set")
        .output;
        let record: serde_json::Value = serde_json::from_str(&output).expect("plan record");

        assert_eq!(record["date"], "2026-05-12");
    }
}
