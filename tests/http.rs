use mealie_cli::{ErrorCode, run_from};
use mockito::{Matcher, Server};

fn env(url: &str) -> Vec<(&str, &str)> {
    vec![
        ("MEALIE_URL", url),
        ("MEALIE_TOKEN", "secret-token"),
        ("USE_INSECURE_HTTP", "yes"),
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
        "NAME           SLUG           ID\nPesto Chicken  pesto-chicken  r1\n"
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

    assert_eq!(output, "NAME   SLUG   ID\nPesto  pesto  r1\n");
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

    assert_eq!(output, "No recipes found for \"missing\".\n");
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
fn gets_recipe_as_human_readable_details_by_default() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(200)
        .with_body(r#"{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken"}"#)
        .create();

    let output = run_from(
        ["mealie", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect("recipe details");

    assert_eq!(
        output,
        "Name: Pesto Chicken\nSlug: pesto-chicken\nID:   r1\nIngredients: None listed\n"
    );
}

#[test]
fn gets_all_recipe_ingredients() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/chicken-casserole")
        .with_status(200)
        .with_body(
            r#"{
                "id":"r1",
                "slug":"chicken-casserole",
                "name":"Chicken Casserole",
                "recipeIngredient":[
                    {
                        "quantity":250,
                        "unit":{"name":"gram","abbreviation":"g"},
                        "food":{"name":"bacon"},
                        "note":"snipped",
                        "display":"250 grams bacon snipped",
                        "originalText":"250g dry cured bacon, snipped"
                    },
                    {
                        "quantity":8,
                        "unit":null,
                        "food":{"name":"chicken thighs"},
                        "display":"8 chicken thighs",
                        "originalText":"8 skinless chicken thighs, bone in"
                    }
                ]
            }"#,
        )
        .create();

    let output = run_from(
        ["mealie", "recipe", "get", "chicken-casserole"],
        env(&server.url()),
    )
    .expect("recipe ingredients");

    assert_eq!(
        output,
        "Name: Chicken Casserole\nSlug: chicken-casserole\nID:   r1\nIngredients (2):\n- 250g dry cured bacon, snipped\n- 8 skinless chicken thighs, bone in\n"
    );
}

#[test]
fn ndjson_output_remains_available_explicitly() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(200)
        .with_body(r#"{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken"}"#)
        .create();

    let output = run_from(
        ["mealie", "--ndjson", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect("NDJSON recipe");

    assert_eq!(
        output,
        "{\"id\":\"r1\",\"ingredients\":[],\"name\":\"Pesto Chicken\",\"ok\":true,\"slug\":\"pesto-chicken\",\"type\":\"recipe\"}\n"
    );
}

#[test]
fn maps_not_found() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/missing")
        .with_status(404)
        .with_body(r#"{"detail":{"message":"No Entry Found","error":true,"exception":null}}"#)
        .create();

    let error =
        run_from(["mealie", "recipes", "get", "missing"], env(&server.url())).expect_err("404");

    assert_eq!(error.code(), ErrorCode::NotFound);
    assert_eq!(error.to_human(), "Error getting recipe: Recipe not found");
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

    assert!(output.contains("2026-05-13  dinner  Pasta  Pasta   2"));
    assert!(!output.contains("breakfast"));
}

#[test]
fn lists_plan_across_all_pages() {
    let mut server = Server::new();
    let _first_page = server
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
            r#"{"page":1,"total_pages":2,"items":[{"id":1,"date":"2026-05-13","entryType":"dinner","title":"Pasta"}]}"#,
        )
        .create();
    let _second_page = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_date".into(), "2026-05-13".into()),
            Matcher::UrlEncoded("end_date".into(), "2026-05-16".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("orderBy".into(), "date".into()),
            Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
            Matcher::UrlEncoded("page".into(), "2".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"page":2,"total_pages":2,"items":[{"id":2,"date":"2026-05-14","entryType":"dinner","title":"Curry"}]}"#,
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
    .expect("paginated plan list");

    assert!(output.contains("2026-05-13  dinner  Pasta"));
    assert!(output.contains("2026-05-14  dinner  Curry"));
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
        "DATE        MEAL    TITLE  RECIPE  ID\n2026-05-13  dinner  Pasta  -       1\n"
    );
}

#[test]
fn empty_plan_list_explains_the_requested_range() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(r#"{"items":[]}"#)
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
    .expect("empty plan list");

    assert_eq!(
        output,
        "No meal plan entries found from 2026-05-13 to 2026-05-16.\n"
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

    assert_eq!(
        output,
        "Replaced dinner on 2026-05-13 with Bolognaise (ID 11).\n"
    );
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
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_date".into(), "2026-05-16".into()),
            Matcher::UrlEncoded("end_date".into(), "2026-05-16".into()),
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

    assert_eq!(
        output,
        "Created dinner on 2026-05-16 with Pesto Chicken (ID 12).\n"
    );
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
fn delete_confirms_which_plan_entry_was_removed() {
    let mut server = Server::new();
    let _delete = server
        .mock("DELETE", "/api/households/mealplans/123")
        .with_status(204)
        .create();

    let output = run_from(
        ["mealie", "plan", "delete", "--id", "123"],
        env(&server.url()),
    )
    .expect("delete output");

    assert_eq!(output, "Deleted meal plan entry 123.\n");
}

#[test]
fn maps_api_error() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(500)
        .with_body(r#"{"message":"database unavailable"}"#)
        .create();

    let error = run_from(
        ["mealie", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect_err("api error");

    assert_eq!(error.code(), ErrorCode::ApiError);
    assert_eq!(
        error.to_string(),
        "get recipe: database unavailable (HTTP 500 Internal Server Error)"
    );
}

#[test]
fn maps_authentication_error_with_hint() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(401)
        .with_body(r#"{"detail":"invalid token"}"#)
        .create();

    let error = run_from(
        ["mealie", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect_err("authentication error");

    assert_eq!(error.code(), ErrorCode::Authentication);
    assert_eq!(error.exit_code(), 4);
    assert!(error.to_human().contains("Check MEALIE_TOKEN"));
}

#[test]
fn rejects_reversed_plan_range_before_calling_api() {
    let server = Server::new();
    let error = run_from(
        [
            "mealie",
            "plan",
            "list",
            "--from",
            "2026-05-17",
            "--to",
            "2026-05-16",
        ],
        env(&server.url()),
    )
    .expect_err("reversed range");

    assert_eq!(error.code(), ErrorCode::InvalidArgs);
    assert_eq!(
        error.to_string(),
        "--from (2026-05-17) must be on or before --to (2026-05-16)"
    );
}

#[test]
fn rejects_blank_recipe_query() {
    let server = Server::new();
    let error = run_from(["mealie", "recipes", "search", "   "], env(&server.url()))
        .expect_err("blank query");

    assert_eq!(error.code(), ErrorCode::InvalidArgs);
    assert_eq!(error.to_string(), "recipe search query cannot be empty");
}

#[test]
fn rejects_recipe_limit_outside_api_range() {
    let error = run_from(
        ["mealie", "recipes", "search", "pesto", "--limit", "101"],
        Vec::<(&str, &str)>::new(),
    )
    .expect_err("limit should be validated before config");

    assert_eq!(error.code(), ErrorCode::InvalidArgs);
    assert!(error.to_string().contains("101 is not in 1..=100"));
}
