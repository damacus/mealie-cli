use std::process::Command;

fn mealie(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mealie"))
        .args(args)
        .env_remove("MEALIE_URL")
        .env_remove("MEALIE_TOKEN")
        .output()
        .expect("run mealie")
}

fn mealie_with_env(args: &[&str], url: &str, token: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mealie"))
        .args(args)
        .env("MEALIE_URL", url)
        .env("MEALIE_TOKEN", token)
        .env("USE_INSECURE_HTTP", "yes")
        .output()
        .expect("run mealie")
}

#[test]
fn status_reports_missing_configuration_and_exits_with_missing_config_code() {
    let output = mealie(&["status"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("Mealie status: action required"));
    assert!(stdout.contains("URL:            not configured"));
    assert!(stdout.contains("Token:          not configured"));
    assert!(stdout.contains("Server:         not checked"));
    assert!(stdout.contains("Authentication: not checked"));
}

#[test]
fn status_uses_authentication_exit_code_for_a_rejected_token() {
    let mut server = mockito::Server::new();
    let _mock = server
        .mock("GET", "/api/users/self")
        .with_status(401)
        .create();

    let output = mealie_with_env(&["status", "--ndjson"], &server.url(), "do-not-print-me");

    assert_eq!(output.status.code(), Some(4));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("status JSON");
    assert_eq!(value["error"], "authentication");
    assert_eq!(value["server_reachable"], true);
    assert_eq!(value["authenticated"], false);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("do-not-print-me"));
}

#[test]
fn status_reports_an_invalid_url_before_attempting_connectivity() {
    let output = mealie_with_env(&["status", "--ndjson"], "not a URL", "do-not-print-me");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("status JSON");
    assert_eq!(value["error"], "invalid_args");
    assert_eq!(value["url_configured"], true);
    assert_eq!(value["url_valid"], false);
    assert!(value["server_reachable"].is_null());
    assert!(value["authenticated"].is_null());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("do-not-print-me"));
}

#[test]
fn human_errors_use_stderr_and_actionable_exit_code() {
    let output = mealie(&["recipes", "get", "pesto"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.starts_with("Error: MEALIE_URL is required\n"));
    assert!(stderr.contains("Hint: Set MEALIE_URL and MEALIE_TOKEN"));
}

#[test]
fn json_errors_remain_machine_readable_on_stderr() {
    let output = mealie(&["--json", "recipes", "get", "pesto"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).expect("JSON error");
    assert_eq!(value["error"], "missing_config");
    assert!(value["hint"].as_str().is_some());
}

#[test]
fn help_advertises_human_friendly_aliases_and_examples() {
    let output = mealie(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("[aliases: recipe]"));
    assert!(stdout.contains("[aliases: meal-plan]"));
    assert!(stdout.contains("Examples:"));
}

#[test]
fn invalid_meal_type_lists_allowed_values() {
    let output = mealie(&[
        "plan",
        "list",
        "--from",
        "2026-05-13",
        "--to",
        "2026-05-16",
        "--type",
        "brunch",
    ]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.contains("possible values: breakfast, lunch, dinner"));
}
