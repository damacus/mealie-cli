use mealie_cli::{ErrorCode, run_from};
use mockito::{Matcher, Server};

fn env(url: &str) -> Vec<(&str, &str)> {
    vec![
        ("MEALIE_URL", url),
        ("MEALIE_TOKEN", "secret-token"),
        ("MEALIE_ALLOW_INSECURE_HTTP", "true"),
    ]
}

#[test]
fn searches_recipes() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes")
        .match_header("authorization", "Bearer secret-token")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "pesto chicken".into()),
            Matcher::UrlEncoded("perPage".into(), "5".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"items":[{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken"}]}"#)
        .create();

    let output = run_from(
        [
            "mealie",
            "recipes",
            "search",
            "pesto chicken",
            "--limit",
            "5",
        ],
        env(&server.url()),
    )
    .expect("search output");

    assert_eq!(
        output,
        "{\"id\":\"r1\",\"name\":\"Pesto Chicken\",\"ok\":true,\"slug\":\"pesto-chicken\",\"type\":\"recipe\"}\n"
    );
}

#[test]
fn searches_recipes_from_data_wrapper() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "pesto".into()),
            Matcher::UrlEncoded("perPage".into(), "10".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"data":[{"id":"r1","slug":"pesto","name":"Pesto"}]}"#)
        .create();

    let output = run_from(["mealie", "recipes", "search", "pesto"], env(&server.url()))
        .expect("search output");

    assert_eq!(
        output,
        "{\"id\":\"r1\",\"name\":\"Pesto\",\"ok\":true,\"slug\":\"pesto\",\"type\":\"recipe\"}\n"
    );
}

#[test]
fn search_empty_outputs_empty_record() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "missing".into()),
            Matcher::UrlEncoded("perPage".into(), "10".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"items":[]}"#)
        .create();

    let output = run_from(
        ["mealie", "recipes", "search", "missing"],
        env(&server.url()),
    )
    .expect("empty search output");

    assert_eq!(
        output,
        "{\"ok\":true,\"query\":\"missing\",\"resource\":\"recipe\",\"type\":\"empty\"}\n"
    );
}

#[test]
fn gets_recipe() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(200)
        .with_body(r#"{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken"}"#)
        .create();

    let output = run_from(
        ["mealie", "--json", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect("recipe output");

    assert!(output.contains("\"type\": \"recipe\""));
    assert!(output.contains("\"slug\": \"pesto-chicken\""));
}

#[test]
fn maps_not_found() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/missing")
        .with_status(404)
        .create();

    let error =
        run_from(["mealie", "recipes", "get", "missing"], env(&server.url())).expect_err("404");

    assert_eq!(error.code(), ErrorCode::NotFound);
}

#[test]
fn lists_plan_with_type_filter() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_date".into(), "2026-05-13".into()),
            Matcher::UrlEncoded("end_date".into(), "2026-05-16".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("orderBy".into(), "date".into()),
            Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"items":[
                {"id":1,"date":"2026-05-13","entryType":"breakfast","title":"Toast"},
                {"id":2,"date":"2026-05-13","entryType":"dinner","title":"Pasta","recipe":{"id":"r2","name":"Pasta"}}
            ]}"#,
        )
        .create();

    let output = run_from(
        [
            "mealie",
            "plan",
            "list",
            "--from",
            "2026-05-13",
            "--to",
            "2026-05-16",
            "--type",
            "dinner",
        ],
        env(&server.url()),
    )
    .expect("plan list");

    assert!(output.contains("\"id\":2"));
    assert!(!output.contains("\"id\":1"));
    assert!(output.contains("\"recipe\":\"Pasta\""));
}

#[test]
fn lists_plan_from_top_level_array() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_date".into(), "2026-05-13".into()),
            Matcher::UrlEncoded("end_date".into(), "2026-05-16".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("orderBy".into(), "date".into()),
            Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"[
                {"id":1,"date":"2026-05-13","entryType":"dinner","title":"Pasta"}
            ]"#,
        )
        .create();

    let output = run_from(
        [
            "mealie",
            "plan",
            "list",
            "--from",
            "2026-05-13",
            "--to",
            "2026-05-16",
        ],
        env(&server.url()),
    )
    .expect("plan list");

    assert_eq!(
        output,
        "{\"date\":\"2026-05-13\",\"id\":1,\"meal\":\"dinner\",\"ok\":true,\"recipe\":null,\"recipeId\":null,\"title\":\"Pasta\",\"type\":\"plan_entry\"}\n"
    );
}

#[test]
fn sets_title_replacement() {
    let mut server = Server::new();
    let _list = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_date".into(), "2026-05-13".into()),
            Matcher::UrlEncoded("end_date".into(), "2026-05-13".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("orderBy".into(), "date".into()),
            Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"items":[{"id":10,"date":"2026-05-13","entryType":"dinner","title":"Old"}]}"#,
        )
        .create();
    let _delete = server
        .mock("DELETE", "/api/households/mealplans/10")
        .with_status(204)
        .create();
    let _create = server
        .mock("POST", "/api/households/mealplans")
        .match_body(Matcher::JsonString(
            r#"{"date":"2026-05-13","entryType":"dinner","title":"Bolognaise","text":""}"#.into(),
        ))
        .with_status(200)
        .with_body(r#"{"id":11,"date":"2026-05-13","entryType":"dinner","title":"Bolognaise"}"#)
        .create();

    let output = run_from(
        [
            "mealie",
            "plan",
            "set",
            "--date",
            "2026-05-13",
            "--type",
            "dinner",
            "--title",
            "Bolognaise",
        ],
        env(&server.url()),
    )
    .expect("plan set");

    assert!(output.contains("\"type\":\"plan_deleted\""));
    assert!(output.contains("\"id\":10"));
    assert!(output.contains("\"type\":\"plan_created\""));
    assert!(output.contains("\"id\":11"));
}

#[test]
fn sets_recipe_replacement() {
    let mut server = Server::new();
    let _recipe = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(200)
        .with_body(r#"{"id":"r2","slug":"pesto-chicken","name":"Pesto Chicken"}"#)
        .create();
    let _list = server
        .mock("GET", "/api/households/mealplans")
        .with_status(200)
        .with_body(r#"{"items":[]}"#)
        .create();
    let _create = server
        .mock("POST", "/api/households/mealplans")
        .match_body(Matcher::JsonString(
            r#"{"date":"2026-05-16","entryType":"dinner","recipeId":"r2"}"#.into(),
        ))
        .with_status(200)
        .with_body(
            r#"{"id":12,"date":"2026-05-16","entryType":"dinner","recipe":{"id":"r2","name":"Pesto Chicken"}}"#,
        )
        .create();

    let output = run_from(
        [
            "mealie",
            "plan",
            "set",
            "--date",
            "2026-05-16",
            "--type",
            "dinner",
            "--recipe",
            "pesto-chicken",
        ],
        env(&server.url()),
    )
    .expect("recipe plan set");

    assert!(output.contains("\"type\":\"plan_created\""));
    assert!(output.contains("\"recipeId\":\"r2\""));
    assert!(output.contains("\"recipe\":\"Pesto Chicken\""));
}

#[test]
fn deletes_plan_with_no_content_response() {
    let mut server = Server::new();
    let _delete = server
        .mock("DELETE", "/api/households/mealplans/123")
        .with_status(204)
        .create();

    let output = run_from(
        ["mealie", "--quiet", "plan", "delete", "--id", "123"],
        env(&server.url()),
    )
    .expect("delete output");

    assert_eq!(output, "123\n");
}

#[test]
fn maps_api_error() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(500)
        .create();

    let error = run_from(
        ["mealie", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect_err("api error");

    assert_eq!(error.code(), ErrorCode::ApiError);
}
