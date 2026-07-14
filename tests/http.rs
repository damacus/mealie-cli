use chrono::{Datelike, Duration, Local};
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
fn status_checks_configuration_connectivity_and_authentication() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/users/self")
        .match_header("authorization", "Bearer secret-token")
        .with_status(200)
        .with_body(r#"{"id":"user-id"}"#)
        .create();

    let output = run_from(["mealie", "status"], env(&server.url())).expect("status output");

    assert_eq!(
        output,
        format!(
            "Mealie status: ready\nURL:            configured and valid ({})\nToken:          configured\nServer:         reachable\nAuthentication: successful\n",
            server.url()
        )
    );
}

#[test]
fn successful_structured_status_omits_the_optional_hint_field() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/users/self")
        .match_header("authorization", "Bearer secret-token")
        .with_status(200)
        .with_body(r#"{"id":"user-id"}"#)
        .create();

    let output =
        run_from(["mealie", "status", "--ndjson"], env(&server.url())).expect("status output");
    let status: serde_json::Value = serde_json::from_str(&output).expect("status JSON");

    assert_eq!(status["ok"], true);
    assert!(status.get("hint").is_none());
}

#[test]
fn status_json_has_a_stable_schema_without_the_token() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/users/self")
        .match_header("authorization", "Bearer secret-token")
        .with_status(401)
        .create();

    let output =
        run_from(["mealie", "status", "--json"], env(&server.url())).expect("status output");
    let values: serde_json::Value = serde_json::from_str(&output).expect("status JSON");
    let status = &values[0];

    assert_eq!(status["ok"], false);
    assert_eq!(status["type"], "status");
    assert_eq!(status["url_configured"], true);
    assert_eq!(status["url_valid"], true);
    assert_eq!(status["token_configured"], true);
    assert_eq!(status["server_reachable"], true);
    assert_eq!(status["authenticated"], false);
    assert_eq!(status["error"], "authentication");
    assert!(!output.contains("secret-token"));
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
    let search = server.mock("GET", "/api/recipes").expect(0).create();

    let output = run_from(
        ["mealie", "--json", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect("recipe output");

    assert!(output.contains("\"type\": \"recipe\""));
    assert!(output.contains("\"slug\": \"pesto-chicken\""));
    search.assert();
}

#[test]
fn gets_recipe_by_unique_case_insensitive_name_after_slug_lookup_misses() {
    let mut server = Server::new();
    let slug = server
        .mock("GET", "/api/recipes/Pesto%20Chicken")
        .with_status(404)
        .expect(1)
        .create();
    let search = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "Pesto Chicken".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"items":[{"id":"r1","slug":"pesto-chicken","name":"pEsTo ChIcKeN"}]}"#)
        .expect(1)
        .create();

    let output = run_from(
        ["mealie", "--json", "recipes", "get", "Pesto Chicken"],
        env(&server.url()),
    )
    .expect("recipe output");

    assert!(output.contains("\"slug\": \"pesto-chicken\""));
    assert!(output.contains("\"name\": \"pEsTo ChIcKeN\""));
    slug.assert();
    search.assert();
}

#[test]
fn name_lookup_preserves_not_found_when_no_exact_name_matches() {
    let mut server = Server::new();
    let slug = server
        .mock("GET", "/api/recipes/Pesto%20Chicken")
        .with_status(404)
        .expect(1)
        .create();
    let search = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "Pesto Chicken".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"items":[{"id":"r1","slug":"pesto-chicken","name":"Pesto chicken with rice"}]}"#,
        )
        .expect(1)
        .create();

    let error = run_from(
        ["mealie", "recipes", "get", "Pesto Chicken"],
        env(&server.url()),
    )
    .expect_err("no exact recipe name");

    assert_eq!(error.code(), ErrorCode::NotFound);
    slug.assert();
    search.assert();
}

#[test]
fn name_lookup_reports_all_exact_name_matches_across_search_pages() {
    let mut server = Server::new();
    let slug = server
        .mock("GET", "/api/recipes/Pesto%20Chicken")
        .with_status(404)
        .expect(1)
        .create();
    let first_page = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "Pesto Chicken".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"total_pages":2,"items":[{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken"}]}"#,
        )
        .expect(1)
        .create();
    let second_page = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "Pesto Chicken".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("page".into(), "2".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"totalPages":2,"items":[{"id":"r2","slug":"pesto-chicken-2","name":"PESTO CHICKEN"}]}"#,
        )
        .expect(1)
        .create();

    let error = run_from(
        ["mealie", "recipes", "get", "Pesto Chicken"],
        env(&server.url()),
    )
    .expect_err("ambiguous recipe name");

    assert_eq!(error.code(), ErrorCode::Ambiguous);
    assert!(error.to_string().contains("Pesto Chicken (pesto-chicken)"));
    assert!(
        error
            .to_string()
            .contains("PESTO CHICKEN (pesto-chicken-2)")
    );
    slug.assert();
    first_page.assert();
    second_page.assert();
}

#[test]
fn plan_set_does_not_mutate_when_recipe_name_has_no_exact_match() {
    let mut server = Server::new();
    let slug = server
        .mock("GET", "/api/recipes/Pesto%20Chicken")
        .with_status(404)
        .expect(1)
        .create();
    let search = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "Pesto Chicken".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"items":[]}"#)
        .expect(1)
        .create();
    let list = server
        .mock("GET", "/api/households/mealplans")
        .expect(0)
        .create();
    let create = server
        .mock("POST", "/api/households/mealplans")
        .expect(0)
        .create();
    let delete = server.mock("DELETE", Matcher::Any).expect(0).create();

    let error = run_from(
        [
            "mealie",
            "plan",
            "set",
            "--date",
            "2026-05-16",
            "--type",
            "dinner",
            "--recipe",
            "Pesto Chicken",
        ],
        env(&server.url()),
    )
    .expect_err("no exact recipe name");

    assert_eq!(error.code(), ErrorCode::NotFound);
    slug.assert();
    search.assert();
    list.assert();
    create.assert();
    delete.assert();
}

#[test]
fn plan_set_does_not_mutate_when_recipe_name_is_ambiguous() {
    let mut server = Server::new();
    let slug = server
        .mock("GET", "/api/recipes/Pesto%20Chicken")
        .with_status(404)
        .expect(1)
        .create();
    let search = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "Pesto Chicken".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"items":[{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken"},{"id":"r2","slug":"pesto-chicken-2","name":"PESTO CHICKEN"}]}"#,
        )
        .expect(1)
        .create();
    let list = server
        .mock("GET", "/api/households/mealplans")
        .expect(0)
        .create();
    let create = server
        .mock("POST", "/api/households/mealplans")
        .expect(0)
        .create();
    let delete = server.mock("DELETE", Matcher::Any).expect(0).create();

    let error = run_from(
        [
            "mealie",
            "plan",
            "set",
            "--date",
            "2026-05-16",
            "--type",
            "dinner",
            "--recipe",
            "Pesto Chicken",
        ],
        env(&server.url()),
    )
    .expect_err("ambiguous recipe name");

    assert_eq!(error.code(), ErrorCode::Ambiguous);
    slug.assert();
    search.assert();
    list.assert();
    create.assert();
    delete.assert();
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
    let _search = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("search".into(), "missing".into()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"items":[]}"#)
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
        "Meal plan entries from 2026-05-13 to 2026-05-16:\nDATE        MEAL    TITLE  RECIPE  ID\n2026-05-13  dinner  Pasta  -       1\n"
    );
}

#[test]
fn lists_the_remainder_of_the_current_week_when_no_dates_are_given() {
    let today = Local::now().date_naive();
    let sunday = today + Duration::days((6 - today.weekday().num_days_from_monday()).into());
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_date".into(), today.to_string()),
            Matcher::UrlEncoded("end_date".into(), sunday.to_string()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("orderBy".into(), "date".into()),
            Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"items":[]}"#)
        .create();

    let output = run_from(["mealie", "--ndjson", "plan", "list"], env(&server.url()))
        .expect("default plan list");
    let record: serde_json::Value = serde_json::from_str(&output).expect("empty plan record");

    assert_eq!(record["from"], today.to_string());
    assert_eq!(record["to"], sunday.to_string());
}

#[test]
fn plan_set_normalizes_relative_dates_before_listing_and_creating() {
    let date = Local::now().date_naive() + Duration::days(1);
    let date = date.to_string();
    let mut server = Server::new();
    let _list = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_date".into(), date.clone()),
            Matcher::UrlEncoded("end_date".into(), date.clone()),
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("orderBy".into(), "date".into()),
            Matcher::UrlEncoded("orderDirection".into(), "asc".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"items":[]}"#)
        .create();
    let _create = server
        .mock("POST", "/api/households/mealplans")
        .match_body(Matcher::JsonString(format!(
            r#"{{"date":"{date}","entryType":"dinner","title":"Soup","text":""}}"#
        )))
        .with_status(200)
        .with_body(format!(
            r#"{{"id":1,"date":"{date}","entryType":"dinner","title":"Soup"}}"#
        ))
        .create();

    let output = run_from(
        [
            "mealie", "--ndjson", "plan", "set", "--date", "+1d", "--type", "dinner", "--title",
            "Soup",
        ],
        env(&server.url()),
    )
    .expect("relative plan set");
    let record: serde_json::Value = serde_json::from_str(&output).expect("plan record");

    assert_eq!(record["date"], date);
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
fn keeps_existing_entry_when_replacement_create_fails() {
    let mut server = Server::new();
    let _list = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(
            r#"{"items":[{"id":10,"date":"2026-05-13","entryType":"dinner","title":"Old"}]}"#,
        )
        .create();
    let delete = server
        .mock("DELETE", "/api/households/mealplans/10")
        .expect(0)
        .create();
    let _create = server
        .mock("POST", "/api/households/mealplans")
        .with_status(500)
        .create();

    let error = run_from(
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
    .expect_err("create failure");

    assert_eq!(error.code(), ErrorCode::ApiError);
    delete.assert();
}

#[test]
fn rolls_back_new_entry_when_original_cannot_be_deleted() {
    let mut server = Server::new();
    let _list = server
        .mock("GET", "/api/households/mealplans")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(
            r#"{"items":[{"id":10,"date":"2026-05-13","entryType":"dinner","title":"Old"}]}"#,
        )
        .create();
    let _create = server
        .mock("POST", "/api/households/mealplans")
        .with_status(200)
        .with_body(r#"{"id":11,"date":"2026-05-13","entryType":"dinner","title":"Bolognaise"}"#)
        .create();
    let _delete_original = server
        .mock("DELETE", "/api/households/mealplans/10")
        .with_status(500)
        .create();
    let rollback = server
        .mock("DELETE", "/api/households/mealplans/11")
        .with_status(204)
        .expect(1)
        .create();

    let error = run_from(
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
    .expect_err("delete failure");

    assert_eq!(error.code(), ErrorCode::ApiError);
    assert!(
        error.to_human().contains("the new entry was rolled back"),
        "{}",
        error.to_human()
    );
    rollback.assert();
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
    let search = server.mock("GET", "/api/recipes").expect(0).create();

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
    search.assert();
}

#[test]
fn maps_authentication_error_with_hint() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_status(401)
        .with_body(r#"{"detail":"invalid token"}"#)
        .create();
    let search = server.mock("GET", "/api/recipes").expect(0).create();

    let error = run_from(
        ["mealie", "recipes", "get", "pesto-chicken"],
        env(&server.url()),
    )
    .expect_err("authentication error");

    assert_eq!(error.code(), ErrorCode::Authentication);
    assert_eq!(error.exit_code(), 4);
    assert!(error.to_human().contains("Check MEALIE_TOKEN"));
    search.assert();
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
