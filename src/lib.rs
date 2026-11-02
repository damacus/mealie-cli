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
use date_input::{parse_date_input, resolve_plan_range, resolve_week_range};
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

    let columns = terminal_columns(&env_vars);
    let config = Config::from_env(env_vars)?;
    let client = MealieClient::new(config)?;
    let output = execute(&client, command, today, columns)?;

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
    columns: usize,
) -> Result<CommandOutput, AppError> {
    match command {
        Command::Completion { .. } => unreachable!("completion output is generated before config"),
        Command::Status => unreachable!("status is handled before client creation"),
        Command::Tags(cli::OrganizerCommand::Merge {
            ref from,
            ref to,
            yes,
            dry_run,
        })
        | Command::Categories(cli::OrganizerCommand::Merge {
            ref from,
            ref to,
            yes,
            dry_run,
        }) => {
            validate_path_segment(from)?;
            validate_path_segment(to)?;
            if from == to {
                return Err(AppError::new(
                    ErrorCode::InvalidArgs,
                    "source and target must differ",
                ));
            }
            if !yes && !dry_run {
                return Err(AppError::new(
                    ErrorCode::InvalidArgs,
                    "merge removes the source; pass --yes or --dry-run",
                ));
            }
            let base = if matches!(command, Command::Tags(_)) {
                "/api/organizers/tags"
            } else {
                "/api/organizers/categories"
            };
            let mut value = if dry_run {
                serde_json::json!({"id":to,"fromId":from,"toId":to})
            } else {
                client.merge_organizer(base, from, to)?
            };
            let object = value
                .as_object_mut()
                .ok_or_else(|| AppError::new(ErrorCode::ApiError, "invalid organiser response"))?;
            object.insert("ok".into(), true.into());
            object.insert(
                "type".into(),
                if dry_run {
                    "organizer_merge_plan"
                } else {
                    "organizer_merged"
                }
                .into(),
            );
            Ok(CommandOutput {
                presentation: Presentation::OrganizerChange {
                    action: if dry_run {
                        "Would merge into"
                    } else {
                        "Merged into"
                    },
                },
                values: vec![value],
            })
        }
        Command::Tags(cli::OrganizerCommand::Delete {
            ref id,
            yes,
            dry_run,
        })
        | Command::Categories(cli::OrganizerCommand::Delete {
            ref id,
            yes,
            dry_run,
        }) => {
            validate_path_segment(id)?;
            if !yes && !dry_run {
                return Err(AppError::new(
                    ErrorCode::InvalidArgs,
                    "delete removes the organiser; pass --yes or --dry-run",
                ));
            }
            let base = if matches!(command, Command::Tags(_)) {
                "/api/organizers/tags"
            } else {
                "/api/organizers/categories"
            };
            if !dry_run {
                client.delete_organizer(&format!("{base}/{id}"))?;
            }
            Ok(CommandOutput {
                presentation: Presentation::OrganizerChange {
                    action: if dry_run { "Would delete" } else { "Deleted" },
                },
                values: vec![record(
                    if dry_run {
                        "organizer_delete_plan"
                    } else {
                        "organizer_deleted"
                    },
                    [("id", serde_json::json!(id))],
                )],
            })
        }
        Command::Tags(cli::OrganizerCommand::Get { ref id, ref slug })
        | Command::Categories(cli::OrganizerCommand::Get { ref id, ref slug }) => {
            let base = if matches!(command, Command::Tags(_)) {
                "/api/organizers/tags"
            } else {
                "/api/organizers/categories"
            };
            let (prefix, identifier) = if let Some(id) = id {
                ("", id)
            } else {
                ("slug/", slug.as_ref().expect("identifier is required"))
            };
            validate_path_segment(identifier)?;
            let mut value = client.get_organizer(&format!("{base}/{prefix}{identifier}"))?;
            let object = value
                .as_object_mut()
                .ok_or_else(|| AppError::new(ErrorCode::ApiError, "invalid organiser response"))?;
            object.insert("ok".into(), true.into());
            object.insert("type".into(), "organizer".into());
            Ok(CommandOutput {
                presentation: Presentation::OrganizerList {
                    plural: if base.ends_with("tags") {
                        "tags"
                    } else {
                        "categories"
                    },
                },
                values: vec![value],
            })
        }
        Command::Tags(cli::OrganizerCommand::Rename {
            ref id,
            ref name,
            dry_run,
        })
        | Command::Categories(cli::OrganizerCommand::Rename {
            ref id,
            ref name,
            dry_run,
        }) => {
            validate_path_segment(id)?;
            if name.trim().is_empty() {
                return Err(AppError::new(
                    ErrorCode::InvalidArgs,
                    "organiser name cannot be empty",
                ));
            }
            let base = if matches!(command, Command::Tags(_)) {
                "/api/organizers/tags"
            } else {
                "/api/organizers/categories"
            };
            let mut value = if dry_run {
                serde_json::json!({"id":id,"name":name})
            } else {
                client.rename_organizer(&format!("{base}/{id}"), name)?
            };
            let object = value
                .as_object_mut()
                .ok_or_else(|| AppError::new(ErrorCode::ApiError, "invalid organiser response"))?;
            object.insert("ok".into(), true.into());
            object.insert(
                "type".into(),
                if dry_run {
                    "organizer_rename_plan"
                } else {
                    "organizer_updated"
                }
                .into(),
            );
            Ok(CommandOutput {
                presentation: Presentation::OrganizerRename {
                    kind: if base.ends_with("tags") {
                        "tag"
                    } else {
                        "category"
                    },
                    dry_run,
                },
                values: vec![value],
            })
        }
        Command::Tags(cli::OrganizerCommand::Create { ref name, dry_run })
        | Command::Categories(cli::OrganizerCommand::Create { ref name, dry_run }) => {
            if name.trim().is_empty() {
                return Err(AppError::new(
                    ErrorCode::InvalidArgs,
                    "organiser name cannot be empty",
                ));
            }
            let path = if matches!(command, Command::Tags(_)) {
                "/api/organizers/tags"
            } else {
                "/api/organizers/categories"
            };
            let mut value = if dry_run {
                serde_json::json!({"name": name, "path": path})
            } else {
                client.create_organizer(path, name)?
            };
            let object = value
                .as_object_mut()
                .ok_or_else(|| AppError::new(ErrorCode::ApiError, "invalid organiser response"))?;
            object.insert("ok".into(), true.into());
            object.insert(
                "type".into(),
                if dry_run {
                    "organizer_create_plan"
                } else {
                    "organizer_created"
                }
                .into(),
            );
            Ok(CommandOutput {
                presentation: Presentation::OrganizerCreate {
                    kind: if path.ends_with("tags") {
                        "tag"
                    } else {
                        "category"
                    },
                    dry_run,
                },
                values: vec![value],
            })
        }
        Command::Tags(cli::OrganizerCommand::List)
        | Command::Categories(cli::OrganizerCommand::List) => {
            let path = if matches!(command, Command::Tags(_)) {
                "/api/organizers/tags"
            } else {
                "/api/organizers/categories"
            };
            let values = client
                .list_collection(path)?
                .into_iter()
                .map(|mut value| {
                    if let Some(object) = value.as_object_mut() {
                        object.insert("ok".into(), true.into());
                        object.insert("type".into(), "organizer".into());
                    }
                    value
                })
                .collect();
            Ok(CommandOutput {
                presentation: Presentation::OrganizerList {
                    plural: if path.ends_with("tags") {
                        "tags"
                    } else {
                        "categories"
                    },
                },
                values,
            })
        }
        Command::Recipes(recipes) => match recipes {
            RecipesCommand::BulkSettings {
                recipe,
                file,
                dry_run,
            } => {
                for slug in &recipe {
                    validate_path_segment(slug)?;
                }
                if recipe
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != recipe.len()
                {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "duplicate recipes are not allowed",
                    ));
                }
                let data = std::fs::read_to_string(&file).map_err(|error| {
                    AppError::new(
                        ErrorCode::InvalidArgs,
                        format!("cannot read settings file: {error}"),
                    )
                })?;
                let settings: serde_json::Value = serde_json::from_str(&data).map_err(|error| {
                    AppError::new(
                        ErrorCode::InvalidArgs,
                        format!("invalid settings JSON: {error}"),
                    )
                })?;
                const FIELDS: [&str; 5] = [
                    "public",
                    "showNutrition",
                    "showAssets",
                    "landscapeView",
                    "disableComments",
                ];
                let object = settings.as_object().ok_or_else(|| {
                    AppError::new(ErrorCode::InvalidArgs, "settings must be a JSON object")
                })?;
                if object.len() != FIELDS.len()
                    || FIELDS.iter().any(|field| {
                        object
                            .get(*field)
                            .and_then(serde_json::Value::as_bool)
                            .is_none()
                    })
                {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "settings must contain exactly the five boolean fields public, showNutrition, showAssets, landscapeView and disableComments; locked is preserved by Mealie",
                    ));
                }
                let payload = serde_json::json!({"recipes":recipe,"settings":settings});
                if dry_run {
                    return Ok(CommandOutput {
                        presentation: Presentation::BulkPlan,
                        values: vec![record(
                            "recipe_bulk_plan",
                            [
                                ("action", serde_json::json!("settings")),
                                ("changes", payload),
                            ],
                        )],
                    });
                }
                client.bulk_action("settings", &payload).map_err(|error| {
                    AppError::new(
                        error.code(),
                        format!("bulk settings failed; changes may be partial: {error}"),
                    )
                })?;
                let mut results = Vec::new();
                for slug in &recipe {
                    let updated = client.get_recipe(slug).map_err(|error| AppError::new(error.code(), format!("bulk settings were sent but verification failed for {slug}; changes may be partial: {error}")))?;
                    let actual = updated.raw.get("settings").and_then(serde_json::Value::as_object).ok_or_else(|| AppError::new(ErrorCode::ApiError, format!("bulk settings were sent but {slug} has no settings; changes may be partial")))?;
                    if FIELDS
                        .iter()
                        .any(|field| actual.get(*field) != object.get(*field))
                    {
                        return Err(AppError::new(
                            ErrorCode::ApiError,
                            format!(
                                "bulk settings were sent but {slug} does not match; changes may be partial"
                            ),
                        ));
                    }
                    results.push(record(
                        "recipe_bulk_verified",
                        [
                            ("id", serde_json::json!(updated.id)),
                            ("slug", serde_json::json!(updated.slug)),
                            ("name", serde_json::json!(updated.name)),
                        ],
                    ));
                }
                Ok(CommandOutput {
                    presentation: Presentation::RecipeList,
                    values: results,
                })
            }
            RecipesCommand::BulkDelete {
                recipe,
                yes,
                dry_run,
            } => {
                if !yes && !dry_run {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "bulk delete removes recipes; pass --yes or --dry-run",
                    ));
                }
                for slug in &recipe {
                    validate_path_segment(slug)?;
                }
                if recipe
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != recipe.len()
                {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "duplicate recipes are not allowed",
                    ));
                }
                let payload = serde_json::json!({"recipes":recipe});
                if dry_run {
                    return Ok(CommandOutput {
                        presentation: Presentation::BulkPlan,
                        values: vec![record(
                            "recipe_bulk_plan",
                            [
                                ("action", serde_json::json!("delete")),
                                ("changes", payload),
                            ],
                        )],
                    });
                }
                client.bulk_action("delete", &payload).map_err(|error| {
                    AppError::new(
                        error.code(),
                        format!("bulk delete failed; changes may be partial: {error}"),
                    )
                })?;
                let mut values = Vec::new();
                for slug in &recipe {
                    match client.get_recipe(slug) {
                        Err(error) if error.code() == ErrorCode::NotFound => values.push(record(
                            "recipe_bulk_deleted",
                            [("slug", serde_json::json!(slug))],
                        )),
                        Ok(_) => {
                            return Err(AppError::new(
                                ErrorCode::ApiError,
                                format!(
                                    "bulk delete was sent but {slug} still exists; changes may be partial"
                                ),
                            ));
                        }
                        Err(error) => {
                            return Err(AppError::new(
                                error.code(),
                                format!(
                                    "bulk delete was sent but verification failed for {slug}; changes may be partial: {error}"
                                ),
                            ));
                        }
                    }
                }
                Ok(CommandOutput {
                    presentation: Presentation::BulkDeleted,
                    values,
                })
            }
            RecipesCommand::BulkTag {
                recipe,
                tag,
                dry_run,
            } => bulk_organize(client, &recipe, &tag, "tags", "tag", dry_run),
            RecipesCommand::BulkCategorize {
                recipe,
                category,
                dry_run,
            } => bulk_organize(
                client,
                &recipe,
                &category,
                "categories",
                "categorize",
                dry_run,
            ),
            RecipesCommand::ImportUrl {
                url,
                include_tags,
                include_categories,
                dry_run,
            } => {
                let parsed = reqwest::Url::parse(&url).map_err(|_| {
                    AppError::new(
                        ErrorCode::InvalidArgs,
                        "recipe URL must be an HTTP or HTTPS URL",
                    )
                })?;
                if !matches!(parsed.scheme(), "http" | "https")
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "recipe URL must use HTTP or HTTPS and contain no credentials",
                    ));
                }
                let payload = serde_json::json!({"url":url,"includeTags":include_tags,"includeCategories":include_categories});
                let slug = if dry_run {
                    None
                } else {
                    Some(client.import_recipe("/api/recipes/create/url", &payload)?)
                };
                Ok(CommandOutput {
                    presentation: Presentation::RecipeImport { dry_run },
                    values: vec![record(
                        if dry_run {
                            "recipe_import_plan"
                        } else {
                            "recipe_imported"
                        },
                        [("slug", serde_json::json!(slug)), ("source", payload)],
                    )],
                })
            }
            RecipesCommand::ImportFile {
                file,
                include_tags,
                include_categories,
                dry_run,
            } => {
                let data = std::fs::read_to_string(&file).map_err(|error| {
                    AppError::new(
                        ErrorCode::InvalidArgs,
                        format!("cannot read import file: {error}"),
                    )
                })?;
                if data.trim().is_empty() {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "import file is empty",
                    ));
                }
                let payload = serde_json::json!({"data":data,"includeTags":include_tags,"includeCategories":include_categories});
                let slug = if dry_run {
                    None
                } else {
                    Some(client.import_recipe("/api/recipes/create/html-or-json", &payload)?)
                };
                Ok(CommandOutput {
                    presentation: Presentation::RecipeImport { dry_run },
                    values: vec![record(
                        if dry_run {
                            "recipe_import_plan"
                        } else {
                            "recipe_imported"
                        },
                        [
                            ("slug", serde_json::json!(slug)),
                            (
                                "source",
                                serde_json::json!({"file":file,"includeTags":include_tags,"includeCategories":include_categories}),
                            ),
                        ],
                    )],
                })
            }
            RecipesCommand::ExportFormats => {
                let formats = client.export_formats()?;
                Ok(CommandOutput {
                    presentation: Presentation::ExportFormats,
                    values: vec![record("export_formats", [("formats", formats)])],
                })
            }
            RecipesCommand::Export {
                slug,
                template,
                output,
            } => {
                use std::io::Write;
                validate_path_segment(&slug)?;
                if template.trim().is_empty() {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "export template cannot be empty",
                    ));
                }
                if output.exists() {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "export output already exists",
                    ));
                }
                let bytes = client.export_recipe(&slug, &template)?;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output)
                    .map_err(|error| {
                        AppError::new(
                            ErrorCode::InvalidArgs,
                            format!("cannot create export file: {error}"),
                        )
                    })?;
                file.write_all(&bytes).map_err(|error| {
                    AppError::new(
                        ErrorCode::ApiError,
                        format!("cannot write export file: {error}"),
                    )
                })?;
                Ok(CommandOutput {
                    presentation: Presentation::RecipeExport,
                    values: vec![record(
                        "recipe_exported",
                        [
                            ("slug", serde_json::json!(slug)),
                            ("output", serde_json::json!(output)),
                            ("template", serde_json::json!(template)),
                        ],
                    )],
                })
            }
            RecipesCommand::Patch {
                slug,
                file,
                dry_run,
            } => {
                validate_path_segment(&slug)?;
                let data = std::fs::read_to_string(&file).map_err(|error| {
                    AppError::new(
                        ErrorCode::InvalidArgs,
                        format!("cannot read patch file: {error}"),
                    )
                })?;
                let patch: serde_json::Value = serde_json::from_str(&data).map_err(|error| {
                    AppError::new(
                        ErrorCode::InvalidArgs,
                        format!("invalid patch JSON: {error}"),
                    )
                })?;
                if patch.as_object().is_none_or(serde_json::Map::is_empty) {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "patch must be a non-empty JSON object",
                    ));
                }
                let mut value = if dry_run {
                    serde_json::json!({"slug":slug,"changes":patch})
                } else {
                    client.update_recipe(&slug, &patch)?
                };
                let object = value.as_object_mut().ok_or_else(|| {
                    AppError::new(ErrorCode::ApiError, "invalid recipe update response")
                })?;
                object.insert("ok".into(), true.into());
                object.insert(
                    "type".into(),
                    if dry_run {
                        "recipe_patch_plan"
                    } else {
                        "recipe_updated"
                    }
                    .into(),
                );
                Ok(CommandOutput {
                    presentation: Presentation::RecipeOrganize { dry_run },
                    values: vec![value],
                })
            }
            RecipesCommand::Create { name, dry_run } => {
                if name.trim().is_empty() {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "recipe name cannot be empty",
                    ));
                }
                let slug = if dry_run {
                    None
                } else {
                    Some(client.create_recipe(&name)?)
                };
                Ok(CommandOutput {
                    presentation: Presentation::RecipeCreate { dry_run },
                    values: vec![record(
                        if dry_run {
                            "recipe_create_plan"
                        } else {
                            "recipe_created"
                        },
                        [
                            ("name", serde_json::json!(name)),
                            ("slug", serde_json::json!(slug)),
                        ],
                    )],
                })
            }
            RecipesCommand::Delete { slug, yes, dry_run } => {
                validate_path_segment(&slug)?;
                if !yes && !dry_run {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "delete removes the recipe; pass --yes or --dry-run",
                    ));
                }
                let recipe = client.get_recipe(&slug)?;
                if !dry_run {
                    client.delete_recipe(&slug)?;
                }
                Ok(CommandOutput {
                    presentation: Presentation::RecipeDelete { dry_run },
                    values: vec![record(
                        if dry_run {
                            "recipe_delete_plan"
                        } else {
                            "recipe_deleted"
                        },
                        [
                            ("id", serde_json::json!(recipe.id)),
                            ("slug", serde_json::json!(recipe.slug)),
                            ("name", serde_json::json!(recipe.name)),
                        ],
                    )],
                })
            }
            RecipesCommand::Image(image) => match image {
                cli::RecipeImageCommand::SetUrl { slug, url, dry_run } => {
                    validate_path_segment(&slug)?;
                    let parsed = reqwest::Url::parse(&url).map_err(|_| {
                        AppError::new(
                            ErrorCode::InvalidArgs,
                            "image URL must be an HTTP or HTTPS URL",
                        )
                    })?;
                    if !matches!(parsed.scheme(), "http" | "https")
                        || !parsed.username().is_empty()
                        || parsed.password().is_some()
                    {
                        return Err(AppError::new(
                            ErrorCode::InvalidArgs,
                            "image URL must use HTTP or HTTPS and contain no credentials",
                        ));
                    }
                    let recipe = client.get_recipe(&slug)?;
                    let image = if dry_run {
                        serde_json::json!(null)
                    } else {
                        client
                            .set_image_url(&slug, &url)?
                            .get("image")
                            .cloned()
                            .ok_or_else(|| {
                                AppError::new(ErrorCode::ApiError, "image response missing image")
                            })?
                    };
                    Ok(CommandOutput {
                        presentation: Presentation::RecipeImage {
                            action: if dry_run {
                                "Would set image from"
                            } else {
                                "Updated image for"
                            },
                        },
                        values: vec![record(
                            if dry_run {
                                "recipe_image_plan"
                            } else {
                                "recipe_image_updated"
                            },
                            [
                                ("id", serde_json::json!(recipe.id)),
                                ("slug", serde_json::json!(slug)),
                                ("url", serde_json::json!(url)),
                                ("image", image),
                            ],
                        )],
                    })
                }
                cli::RecipeImageCommand::Delete { slug, yes, dry_run } => {
                    validate_path_segment(&slug)?;
                    if !yes && !dry_run {
                        return Err(AppError::new(
                            ErrorCode::InvalidArgs,
                            "delete removes the image; pass --yes or --dry-run",
                        ));
                    }
                    let recipe = client.get_recipe(&slug)?;
                    if !dry_run {
                        client.delete_image(&slug)?;
                    }
                    Ok(CommandOutput {
                        presentation: Presentation::RecipeImage {
                            action: if dry_run {
                                "Would delete image for"
                            } else {
                                "Deleted image for"
                            },
                        },
                        values: vec![record(
                            if dry_run {
                                "recipe_image_delete_plan"
                            } else {
                                "recipe_image_deleted"
                            },
                            [
                                ("id", serde_json::json!(recipe.id)),
                                ("slug", serde_json::json!(slug)),
                            ],
                        )],
                    })
                }
            },
            RecipesCommand::Organize {
                slug,
                add_tag,
                remove_tag,
                add_category,
                remove_category,
                dry_run,
            } => {
                let (field, base, id, add) =
                    match (add_tag, remove_tag, add_category, remove_category) {
                        (Some(id), None, None, None) => ("tags", "/api/organizers/tags", id, true),
                        (None, Some(id), None, None) => ("tags", "/api/organizers/tags", id, false),
                        (None, None, Some(id), None) => {
                            ("recipeCategory", "/api/organizers/categories", id, true)
                        }
                        (None, None, None, Some(id)) => {
                            ("recipeCategory", "/api/organizers/categories", id, false)
                        }
                        _ => {
                            return Err(AppError::new(
                                ErrorCode::InvalidArgs,
                                "choose exactly one organiser change",
                            ));
                        }
                    };
                validate_path_segment(&slug)?;
                validate_path_segment(&id)?;
                let recipe = client.get_recipe(&slug)?;
                let mut assignments = recipe
                    .raw
                    .get(field)
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::new(
                            ErrorCode::ApiError,
                            format!("recipe response missing {field}"),
                        )
                    })?;
                if assignments
                    .iter()
                    .any(|item| item.get("id").and_then(serde_json::Value::as_str).is_none())
                {
                    return Err(AppError::new(
                        ErrorCode::ApiError,
                        "organiser assignment missing id",
                    ));
                }
                let present = assignments.iter().any(|item| {
                    item.get("id").and_then(serde_json::Value::as_str) == Some(id.as_str())
                });
                if add && !present {
                    let organizer = client.get_organizer(&format!("{base}/{id}"))?;
                    if organizer.get("id").and_then(serde_json::Value::as_str) != Some(id.as_str())
                        || organizer
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .is_none()
                    {
                        return Err(AppError::new(
                            ErrorCode::ApiError,
                            "invalid organiser response",
                        ));
                    }
                    assignments.push(organizer);
                } else if !add && present {
                    assignments.retain(|item| {
                        item.get("id").and_then(serde_json::Value::as_str) != Some(id.as_str())
                    });
                } else {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        if add {
                            "organiser already assigned"
                        } else {
                            "organiser is not assigned"
                        },
                    ));
                }
                let mut patch = serde_json::Map::new();
                patch.insert(field.to_string(), assignments.into());
                let patch = serde_json::Value::Object(patch);
                let mut value = if dry_run {
                    serde_json::json!({"slug":slug,"changes":patch})
                } else {
                    client.update_recipe(&slug, &patch)?
                };
                let object = value.as_object_mut().ok_or_else(|| {
                    AppError::new(ErrorCode::ApiError, "invalid recipe update response")
                })?;
                object.insert("ok".into(), true.into());
                object.insert(
                    "type".into(),
                    if dry_run {
                        "recipe_organize_plan"
                    } else {
                        "recipe_updated"
                    }
                    .into(),
                );
                Ok(CommandOutput {
                    presentation: Presentation::RecipeOrganize { dry_run },
                    values: vec![value],
                })
            }
            RecipesCommand::Update {
                slug,
                description,
                dry_run,
            } => {
                if slug.is_empty()
                    || matches!(slug.as_str(), "." | "..")
                    || slug
                        .chars()
                        .any(|c| matches!(c, '/' | '\\' | '?' | '#' | '%'))
                {
                    return Err(AppError::new(
                        ErrorCode::InvalidArgs,
                        "recipe slug must be a single path segment",
                    ));
                }
                let patch = serde_json::json!({"description": description});
                let value = if dry_run {
                    serde_json::json!({"slug":slug,"changes":patch})
                } else {
                    client.update_recipe(&slug, &patch)?
                };
                let mut value = value;
                let object = value.as_object_mut().ok_or_else(|| {
                    AppError::new(ErrorCode::ApiError, "invalid recipe update response")
                })?;
                object.insert("ok".into(), true.into());
                object.insert(
                    "type".into(),
                    if dry_run {
                        "recipe_update_plan"
                    } else {
                        "recipe_updated"
                    }
                    .into(),
                );
                Ok(CommandOutput {
                    presentation: Presentation::RecipeUpdate,
                    values: vec![value],
                })
            }
            RecipesCommand::List {
                untagged,
                uncategorised,
            } => {
                let mut values = Vec::new();
                for recipe in client.list_recipes()? {
                    let missing = |field: &str| -> Result<bool, AppError> {
                        recipe
                            .get(field)
                            .and_then(serde_json::Value::as_array)
                            .map(Vec::is_empty)
                            .ok_or_else(|| {
                                AppError::new(
                                    ErrorCode::ApiError,
                                    format!("recipe list response missing {field}"),
                                )
                            })
                    };
                    if (untagged && !missing("tags")?)
                        || (uncategorised && !missing("recipeCategory")?)
                    {
                        continue;
                    }
                    let mut recipe = recipe;
                    let object = recipe.as_object_mut().ok_or_else(|| {
                        AppError::new(ErrorCode::ApiError, "invalid recipe summary")
                    })?;
                    object.insert("ok".into(), true.into());
                    object.insert("type".into(), "recipe".into());
                    values.push(recipe);
                }
                Ok(CommandOutput {
                    presentation: Presentation::RecipeList,
                    values,
                })
            }
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
            PlanCommand::Week { date, offset } => {
                let (from, to) = resolve_week_range(date.as_deref(), offset, today)?;
                let from = from.to_string();
                let to = to.to_string();
                Ok(CommandOutput {
                    presentation: Presentation::PlanWeek {
                        from: from.clone(),
                        to: to.clone(),
                        columns,
                    },
                    values: plan_list(client, &from, &to, None)?,
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

fn bulk_organize(
    client: &MealieClient,
    recipes: &[String],
    ids: &[String],
    kind: &str,
    action: &str,
    dry_run: bool,
) -> Result<CommandOutput, AppError> {
    for value in recipes.iter().chain(ids) {
        validate_path_segment(value)?;
    }
    if recipes
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != recipes.len()
        || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
    {
        return Err(AppError::new(
            ErrorCode::InvalidArgs,
            "duplicate recipes or organisers are not allowed",
        ));
    }
    let mut organizers = Vec::new();
    for id in ids {
        let value = client.get_organizer(&format!("/api/organizers/{kind}/{id}"))?;
        if value.get("id").and_then(serde_json::Value::as_str) != Some(id.as_str())
            || value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_none()
            || value
                .get("slug")
                .and_then(serde_json::Value::as_str)
                .is_none()
        {
            return Err(AppError::new(
                ErrorCode::ApiError,
                "invalid organiser response",
            ));
        }
        organizers.push(value);
    }
    let mut payload = serde_json::Map::new();
    payload.insert("recipes".into(), serde_json::json!(recipes));
    payload.insert(kind.into(), serde_json::json!(organizers));
    let payload = serde_json::Value::Object(payload);
    if dry_run {
        return Ok(CommandOutput {
            presentation: Presentation::BulkPlan,
            values: vec![record(
                "recipe_bulk_plan",
                [("action", serde_json::json!(action)), ("changes", payload)],
            )],
        });
    }
    client.bulk_action(action, &payload).map_err(|error| {
        AppError::new(
            error.code(),
            format!("bulk request failed; changes may be partial: {error}"),
        )
    })?;
    let mut results = Vec::new();
    let field = if kind == "tags" {
        "tags"
    } else {
        "recipeCategory"
    };
    for slug in recipes {
        let recipe = client.get_recipe(slug).map_err(|error| {
            AppError::new(
                ErrorCode::ApiError,
                format!("bulk request was sent but verification failed for {slug}: {error}"),
            )
        })?;
        let found = recipe
            .raw
            .get(field)
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::ApiError,
                    format!("bulk request was sent but {slug} has no {field} data"),
                )
            })?;
        if !ids.iter().all(|id| {
            found
                .iter()
                .any(|item| item.get("id").and_then(serde_json::Value::as_str) == Some(id.as_str()))
        }) {
            return Err(AppError::new(
                ErrorCode::ApiError,
                format!(
                    "bulk request was sent but {slug} is missing an assignment; changes may be partial"
                ),
            ));
        }
        results.push(record(
            "recipe_bulk_verified",
            [
                ("id", serde_json::json!(recipe.id)),
                ("slug", serde_json::json!(recipe.slug)),
                ("name", serde_json::json!(recipe.name)),
            ],
        ));
    }
    Ok(CommandOutput {
        presentation: Presentation::RecipeList,
        values: results,
    })
}

fn validate_path_segment(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value
            .chars()
            .any(|c| matches!(c, '/' | '\\' | '?' | '#' | '%'))
    {
        return Err(AppError::new(
            ErrorCode::InvalidArgs,
            "identifier must be a single path segment",
        ));
    }
    Ok(())
}

fn terminal_columns(env_vars: &[(String, String)]) -> usize {
    env_vars
        .iter()
        .rev()
        .find(|(key, _)| key == "COLUMNS")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .filter(|columns| *columns > 0)
        .unwrap_or(80)
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
    let recipe = client.resolve_recipe_details(recipe_query)?;

    let mut result = record(
        "recipe",
        [
            ("id", serde_json::json!(recipe.id)),
            ("slug", serde_json::json!(recipe.slug)),
            ("name", serde_json::json!(recipe.name)),
            ("ingredients", serde_json::json!(recipe.ingredients)),
        ],
    );
    if let (Some(object), Some(raw)) = (result.as_object_mut(), recipe.raw.as_object()) {
        for (field, value) in raw {
            object.entry(field.clone()).or_insert_with(|| value.clone());
        }
    }
    Ok(vec![result])
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

    #[test]
    fn fixed_today_requests_current_anchored_and_offset_weeks() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 13).expect("valid date");
        let mut server = Server::new();
        for (from, to) in [
            ("2026-05-11", "2026-05-17"),
            ("2025-12-29", "2026-01-04"),
            ("2026-05-04", "2026-05-10"),
            ("2026-05-18", "2026-05-24"),
        ] {
            server
                .mock("GET", "/api/households/mealplans")
                .match_query(Matcher::AllOf(vec![
                    Matcher::UrlEncoded("start_date".into(), from.into()),
                    Matcher::UrlEncoded("end_date".into(), to.into()),
                    Matcher::UrlEncoded("perPage".into(), "100".into()),
                    Matcher::UrlEncoded("orderBy".into(), "date".into()),
                    Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
                ]))
                .with_status(200)
                .with_body(r#"{"items":[]}"#)
                .expect(1)
                .create();
        }

        let cases: &[&[&str]] = &[
            &["mealie", "plan", "week"],
            &["mealie", "plan", "week", "--date", "2026-01-01"],
            &["mealie", "plan", "week", "--offset", "-1"],
            &["mealie", "plan", "week", "--offset", "1"],
        ];
        for args in cases {
            let output = run_from_with_exit_at(*args, env(&server.url()), today)
                .expect("week request")
                .output;
            assert!(output.contains("Meal plan week"));
        }
    }

    #[test]
    fn fixed_today_week_keeps_plan_entry_records_and_renders_every_day() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 13).expect("valid date");
        let mut server = Server::new();
        let _mock = server
            .mock("GET", "/api/households/mealplans")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("start_date".into(), "2026-05-11".into()),
                Matcher::UrlEncoded("end_date".into(), "2026-05-17".into()),
                Matcher::UrlEncoded("perPage".into(), "100".into()),
                Matcher::UrlEncoded("orderBy".into(), "date".into()),
                Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
            ]))
            .with_status(200)
            .with_body(
                r#"{"items":[
                    {"id":1,"date":"2026-05-11","entryType":"breakfast","title":"Toast"},
                    {"id":2,"date":"2026-05-11","entryType":"dinner","title":"Pasta"},
                    {"id":3,"date":"2026-05-13","entryType":"dinner","recipe":{"id":"r3","name":"Curry"}}
                ]}"#,
            )
            .expect(2)
            .create();

        let human = run_from_with_exit_at(["mealie", "plan", "week"], env(&server.url()), today)
            .expect("human week output")
            .output;
        for weekday in [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ] {
            assert!(human.contains(weekday));
        }
        assert!(human.contains("breakfast: Toast (ID 1); dinner: Pasta (ID 2)"));
        assert!(human.contains("Wednesday 2026-05-13  dinner: Curry (ID 3)"));
        assert!(human.contains("Tuesday   2026-05-12  (no meals planned)"));

        let structured = run_from_with_exit_at(
            ["mealie", "--ndjson", "plan", "week"],
            env(&server.url()),
            today,
        )
        .expect("structured week output")
        .output;
        let records = structured
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("plan record"))
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|record| record["type"] == "plan_entry"));
        assert!(records.iter().all(|record| record.get("week").is_none()));
        assert_eq!(records[0]["date"], "2026-05-11");
        assert_eq!(records[0]["title"], "Toast");
        assert_eq!(records[2]["recipe"], "Curry");
    }

    #[test]
    fn fixed_today_week_ndjson_preserves_empty_record_without_terminal_layout_fields() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 5, 13).expect("valid date");
        let mut server = Server::new();
        let _mock = server
            .mock("GET", "/api/households/mealplans")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("start_date".into(), "2026-05-11".into()),
                Matcher::UrlEncoded("end_date".into(), "2026-05-17".into()),
                Matcher::UrlEncoded("perPage".into(), "100".into()),
                Matcher::UrlEncoded("orderBy".into(), "date".into()),
                Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
            ]))
            .with_status(200)
            .with_body(r#"{"items":[]}"#)
            .expect(1)
            .create();

        let output = run_from_with_exit_at(
            ["mealie", "--ndjson", "plan", "week"],
            env(&server.url()),
            today,
        )
        .expect("empty structured week output")
        .output;
        let record: serde_json::Value = serde_json::from_str(&output).expect("empty plan record");

        assert_eq!(record["type"], "empty");
        assert_eq!(record["ok"], true);
        assert_eq!(record["resource"], "plan_entry");
        assert_eq!(record["from"], "2026-05-11");
        assert_eq!(record["to"], "2026-05-17");
        assert!(record.get("columns").is_none());
        assert!(record.get("week").is_none());
    }

    #[test]
    fn accepts_valid_terminal_width_or_uses_a_stable_fallback() {
        assert_eq!(
            terminal_columns(&[("COLUMNS".to_string(), "59".to_string())]),
            59
        );
        assert_eq!(
            terminal_columns(&[("COLUMNS".to_string(), "0".to_string())]),
            80
        );
        assert_eq!(
            terminal_columns(&[("COLUMNS".to_string(), "wide".to_string())]),
            80
        );
    }

    #[test]
    fn rejects_week_offset_values_outside_the_signed_integer_range() {
        let error = run_from(
            ["mealie", "plan", "week", "--offset", "9223372036854775808"],
            Vec::<(&str, &str)>::new(),
        )
        .expect_err("an offset outside i64 should be rejected before configuration");

        assert_eq!(error.code(), ErrorCode::InvalidArgs);
    }
}
