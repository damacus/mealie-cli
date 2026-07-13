use std::process::Command;

fn mealie(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mealie"))
        .args(args)
        .env_remove("MEALIE_URL")
        .env_remove("MEALIE_TOKEN")
        .output()
        .expect("run mealie")
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
