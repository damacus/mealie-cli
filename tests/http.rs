use mealie_cli::{ErrorCode, run_from};
use mockito::{Matcher, Server};

fn env(url: &str) -> Vec<(&str, &str)> {
    vec![
        ("MEALIE_URL", url),
        ("MEALIE_TOKEN", "secret-token"),
        ("USE_INSECURE_HTTP", "yes"),
    ]
}

// Schema: https://mealie.io/documentation/getting-started/api-usage/
#[test]
fn lists_untagged_recipes_across_pages() {
    let mut server = Server::new();
    let first = server.mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded("perPage".into(), "100".into()), Matcher::UrlEncoded("page".into(), "1".into())]))
        .with_body(r#"{"items":[{"id":"1","slug":"one","name":"One","tags":[],"recipeCategory":[]}],"total_pages":2}"#).create();
    let second = server.mock("GET", "/api/recipes")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded("perPage".into(), "100".into()), Matcher::UrlEncoded("page".into(), "2".into())]))
        .with_body(r#"{"items":[{"id":"2","slug":"two","name":"Two","tags":[{"name":"Dinner"}],"recipeCategory":[]}],"total_pages":2}"#).create();
    let output = run_from(
        ["mealie", "recipes", "list", "--untagged", "--json"],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values.as_array().unwrap().len(), 1);
    assert_eq!(values[0]["slug"], "one");
    first.assert();
    second.assert();
}

#[test]
fn collection_lists_support_existing_pagination_variants() {
    for body in [
        r#"{"items":[{"id":"t1","name":"Quick","slug":"quick"}],"totalPages":1}"#,
        r#"{"items":[{"id":"t1","name":"Quick","slug":"quick"}]}"#,
    ] {
        let mut server = Server::new();
        let _mock = server
            .mock("GET", "/api/organizers/tags")
            .match_query(Matcher::Any)
            .with_body(body)
            .create();
        let output = run_from(["mealie", "tags", "list", "--json"], env(&server.url())).unwrap();
        let values: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(values[0]["name"], "Quick");
    }
}

#[test]
fn exact_name_lookup_fetches_full_recipe() {
    let mut server = Server::new();
    let _slug = server
        .mock("GET", "/api/recipes/Pesto%20Chicken")
        .with_status(404)
        .create();
    let _search = server
        .mock("GET", "/api/recipes")
        .match_query(Matcher::Any)
        .with_body(r#"{"items":[{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken"}]}"#)
        .create();
    let detail = server.mock("GET", "/api/recipes/pesto-chicken").with_body(r#"{"id":"r1","slug":"pesto-chicken","name":"Pesto Chicken","recipeInstructions":[{"text":"Cook"}]}"#).create();
    let output = run_from(
        ["mealie", "recipes", "get", "Pesto Chicken", "--json"],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["recipeInstructions"][0]["text"], "Cook");
    detail.assert();
}

#[test]
fn human_description_dry_run_shows_proposed_text() {
    let server = Server::new();
    let output = run_from(
        [
            "mealie",
            "recipes",
            "update",
            "soup",
            "--description",
            "Fresh soup",
            "--dry-run",
        ],
        env(&server.url()),
    )
    .unwrap();
    assert!(output.contains("Fresh soup"));
}

#[test]
fn recipe_details_include_catalogue_metadata() {
    let mut server = Server::new();
    let _mock = server.mock("GET", "/api/recipes/soup").with_body(r#"{"id":"1","slug":"soup","name":"Soup","description":"A soup","tags":[{"name":"Quick"}],"recipeCategory":[{"name":"Starter"}],"recipeInstructions":[{"text":"Cook"}],"image":"abc"}"#).create();
    let output = run_from(
        ["mealie", "recipes", "get", "soup", "--json"],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["description"], "A soup");
    assert_eq!(values[0]["tags"][0]["name"], "Quick");
    assert_eq!(values[0]["recipeCategory"][0]["name"], "Starter");
    assert_eq!(values[0]["recipeInstructions"][0]["text"], "Cook");
    assert_eq!(values[0]["image"], "abc");
}

#[test]
fn recipe_details_preserve_other_mealie_fields() {
    let mut server = Server::new();
    let _mock = server.mock("GET", "/api/recipes/soup").with_body(r#"{"id":"1","slug":"soup","name":"Soup","notes":[{"title":"Tip","text":"Season"}],"prepTime":"10 minutes"}"#).create();
    let output = run_from(
        ["mealie", "recipes", "get", "soup", "--json"],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["notes"][0]["text"], "Season");
    assert_eq!(values[0]["prepTime"], "10 minutes");
}

#[test]
fn quiet_create_prints_the_created_id() {
    let mut server = Server::new();
    let _mock = server
        .mock("POST", "/api/organizers/tags")
        .match_body(Matcher::Json(serde_json::json!({"name":"Quick"})))
        .with_status(201)
        .with_body(r#"{"id":"t1","name":"Quick","slug":"quick"}"#)
        .create();
    let output = run_from(
        ["mealie", "tags", "create", "--name", "Quick", "--quiet"],
        env(&server.url()),
    )
    .unwrap();
    assert_eq!(output, "t1\n");
}

#[test]
fn dry_run_does_not_send_an_update() {
    let server = Server::new();
    let output = run_from(
        [
            "mealie",
            "recipes",
            "update",
            "soup",
            "--description",
            "Fresh",
            "--dry-run",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["type"], "recipe_update_plan");
    assert_eq!(values[0]["changes"]["description"], "Fresh");
}

#[test]
fn recipe_update_rejects_path_manipulation() {
    let server = Server::new();
    for slug in ["../other", "soup?x=1", "%2e%2e"] {
        let error = run_from(
            [
                "mealie",
                "recipes",
                "update",
                slug,
                "--description",
                "Fresh",
            ],
            env(&server.url()),
        )
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidArgs);
    }
}

#[test]
fn organizer_create_dry_run_is_clear_in_human_output() {
    let server = Server::new();
    let output = run_from(
        ["mealie", "tags", "create", "--name", "Quick", "--dry-run"],
        env(&server.url()),
    )
    .unwrap();
    assert_eq!(output, "Would create tag: Quick\n");
}

#[test]
fn empty_categories_are_described_as_categories() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/organizers/categories")
        .match_query(Matcher::Any)
        .with_body(r#"{"items":[],"total_pages":0}"#)
        .create();
    let output = run_from(["mealie", "categories", "list"], env(&server.url())).unwrap();
    assert_eq!(output, "No categories found.\n");
}

#[test]
fn rejects_missing_organiser_data_in_filtered_recipe_list() {
    let mut server = Server::new();
    let _mock = server
        .mock("GET", "/api/recipes")
        .with_body(r#"{"items":[{"id":"1","slug":"one","name":"One"}],"total_pages":1}"#)
        .create();
    let error = run_from(
        ["mealie", "recipes", "list", "--untagged"],
        env(&server.url()),
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ApiError);
}

#[test]
fn update_description_sends_only_the_requested_field() {
    let mut server = Server::new();
    let patch = server
        .mock("PATCH", "/api/recipes/soup")
        .match_header("authorization", "Bearer secret-token")
        .match_body(Matcher::Json(
            serde_json::json!({"description":"Fresh soup"}),
        ))
        .with_status(200)
        .with_body(r#"{"id":"1","slug":"soup","name":"Soup"}"#)
        .create();
    let output = run_from(
        [
            "mealie",
            "recipes",
            "update",
            "soup",
            "--description",
            "Fresh soup",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value[0]["slug"], "soup");
    patch.assert();
}

#[test]
fn lists_tags() {
    let mut server = Server::new();
    let mock = server
        .mock("GET", "/api/organizers/tags")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("perPage".into(), "100".into()),
            Matcher::UrlEncoded("page".into(), "1".into()),
        ]))
        .with_body(r#"{"items":[{"id":"1","name":"Quick","slug":"quick"}],"total_pages":1}"#)
        .create();
    let output = run_from(["mealie", "tags", "list", "--json"], env(&server.url())).unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["name"], "Quick");
    mock.assert();
}

#[test]
fn gets_tag_by_slug_with_recipes() {
    let mut server = Server::new();
    let mock = server
        .mock("GET", "/api/organizers/tags/slug/quick")
        .with_body(r#"{"id":"t1","name":"Quick","slug":"quick","recipes":[{"name":"Soup"}]}"#)
        .create();
    let output = run_from(
        ["mealie", "tags", "get", "--slug", "quick", "--json"],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["recipes"][0]["name"], "Soup");
    mock.assert();
}

#[test]
fn renames_category_by_id() {
    let mut server = Server::new();
    let mock = server
        .mock("PUT", "/api/organizers/categories/c1")
        .match_body(Matcher::Json(serde_json::json!({"name":"Pudding"})))
        .with_body(r#"{"id":"c1","name":"Pudding","slug":"pudding"}"#)
        .create();
    let output = run_from(
        [
            "mealie",
            "categories",
            "rename",
            "--id",
            "c1",
            "--name",
            "Pudding",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["name"], "Pudding");
    mock.assert();
}

#[test]
fn adds_a_tag_without_losing_existing_assignments() {
    let mut server = Server::new();
    let recipe = server.mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup","tags":[{"id":"old","name":"Old","slug":"old"}],"recipeCategory":[{"id":"cat","name":"Starter","slug":"starter"}]}"#).create();
    let tag = server
        .mock("GET", "/api/organizers/tags/new")
        .with_body(r#"{"id":"new","name":"Quick","slug":"quick"}"#)
        .create();
    let patch = server.mock("PATCH", "/api/recipes/soup")
        .match_body(Matcher::Json(serde_json::json!({"tags":[{"id":"old","name":"Old","slug":"old"},{"id":"new","name":"Quick","slug":"quick"}]})))
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup"}"#).create();
    run_from(
        [
            "mealie",
            "recipes",
            "organize",
            "soup",
            "--add-tag",
            "new",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    recipe.assert();
    tag.assert();
    patch.assert();
}

#[test]
fn organizer_removal_dry_run_preserves_other_tags() {
    let mut server = Server::new();
    let recipe = server.mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup","tags":[{"id":"old","name":"Old","slug":"old"},{"id":"keep","name":"Keep","slug":"keep"}]}"#).create();
    let output = run_from(
        [
            "mealie",
            "recipes",
            "organize",
            "soup",
            "--remove-tag",
            "old",
            "--dry-run",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        values[0]["changes"]["tags"],
        serde_json::json!([{"id":"keep","name":"Keep","slug":"keep"}])
    );
    recipe.assert();
}

#[test]
fn category_addition_uses_category_field_and_preview_does_not_patch() {
    let mut server = Server::new();
    let _recipe = server
        .mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup","recipeCategory":[]}"#)
        .create();
    let _category = server
        .mock("GET", "/api/organizers/categories/c1")
        .with_body(r#"{"id":"c1","name":"Starter","slug":"starter"}"#)
        .create();
    let output = run_from(
        [
            "mealie",
            "recipes",
            "organize",
            "soup",
            "--add-category",
            "c1",
            "--dry-run",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["changes"]["recipeCategory"][0]["id"], "c1");
}

#[test]
fn adding_duplicate_tag_is_rejected_without_a_write() {
    let mut server = Server::new();
    let _recipe = server
        .mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup","tags":[{"id":"t1","name":"Quick"}]}"#)
        .create();
    let error = run_from(
        ["mealie", "recipes", "organize", "soup", "--add-tag", "t1"],
        env(&server.url()),
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgs);
}

#[test]
fn sets_recipe_image_from_url() {
    let mut server = Server::new();
    let _recipe = server
        .mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup"}"#)
        .create();
    let mock = server
        .mock("POST", "/api/recipes/soup/image")
        .match_body(Matcher::Json(
            serde_json::json!({"url":"https://example.com/soup.jpg"}),
        ))
        .with_body(r#"{"image":"version2"}"#)
        .create();
    let output = run_from(
        [
            "mealie",
            "recipes",
            "image",
            "set-url",
            "soup",
            "--url",
            "https://example.com/soup.jpg",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["image"], "version2");
    mock.assert();
}

#[test]
fn recipe_image_delete_requires_confirmation() {
    let mut server = Server::new();
    let error = run_from(
        ["mealie", "recipes", "image", "delete", "soup"],
        env(&server.url()),
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgs);
    let _recipe = server
        .mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup"}"#)
        .create();
    let mock = server
        .mock("DELETE", "/api/recipes/soup/image")
        .with_status(204)
        .create();
    run_from(
        ["mealie", "recipes", "image", "delete", "soup", "--yes"],
        env(&server.url()),
    )
    .unwrap();
    mock.assert();
}

#[test]
fn recipe_image_preview_is_read_only_and_rejects_credential_urls() {
    let mut server = Server::new();
    let recipe = server
        .mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup"}"#)
        .expect(1)
        .create();
    let error = run_from(
        [
            "mealie",
            "recipes",
            "image",
            "set-url",
            "soup",
            "--url",
            "https://user:secret@example.com/a.jpg",
        ],
        env(&server.url()),
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgs);
    let output = run_from(
        [
            "mealie",
            "recipes",
            "image",
            "set-url",
            "soup",
            "--url",
            "https://example.com/a.jpg",
            "--dry-run",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["url"], "https://example.com/a.jpg");
    assert_eq!(values[0]["type"], "recipe_image_plan");
    recipe.assert();
}

#[test]
fn creates_blank_recipe_and_returns_its_slug() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/api/recipes")
        .match_body(Matcher::Json(serde_json::json!({"name":"Soup"})))
        .with_status(201)
        .with_body(r#""soup""#)
        .create();
    let output = run_from(
        ["mealie", "recipes", "create", "--name", "Soup", "--json"],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["slug"], "soup");
    mock.assert();
}

#[test]
fn recipe_delete_requires_explicit_confirmation() {
    let mut server = Server::new();
    let error = run_from(["mealie", "recipes", "delete", "soup"], env(&server.url())).unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgs);
    let recipe = server
        .mock("GET", "/api/recipes/soup")
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup"}"#)
        .create();
    let delete = server
        .mock("DELETE", "/api/recipes/soup")
        .with_status(204)
        .create();
    let output = run_from(
        ["mealie", "recipes", "delete", "soup", "--yes", "--quiet"],
        env(&server.url()),
    )
    .unwrap();
    assert_eq!(output, "r1\n");
    recipe.assert();
    delete.assert();
}

#[test]
fn applies_a_recipe_json_patch_from_file() {
    let mut server = Server::new();
    let path = std::env::temp_dir().join(format!("mealie-patch-{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{"prepTime":"10 minutes","recipeInstructions":[{"text":"Cook"}]}"#,
    )
    .unwrap();
    let patch = server
        .mock("PATCH", "/api/recipes/soup")
        .match_body(Matcher::Json(
            serde_json::json!({"prepTime":"10 minutes","recipeInstructions":[{"text":"Cook"}]}),
        ))
        .with_body(r#"{"id":"r1","slug":"soup","name":"Soup"}"#)
        .create();
    let filename = path.to_str().unwrap();
    let result = run_from(
        [
            "mealie", "recipes", "patch", "soup", "--file", filename, "--json",
        ],
        env(&server.url()),
    );
    std::fs::remove_file(path).unwrap();
    assert!(result.is_ok());
    patch.assert();
}

#[test]
fn merge_tags_sends_from_and_to_ids() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/api/organizers/tags/merge")
        .match_body(Matcher::Json(serde_json::json!({"fromId":"a","toId":"b"})))
        .with_body(r#"{"id":"b","name":"Target","slug":"target"}"#)
        .create();
    let output = run_from(
        [
            "mealie", "tags", "merge", "--from", "a", "--to", "b", "--yes", "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["id"], "b");
    mock.assert();
}

#[test]
fn deleting_category_requires_confirmation_and_accepts_empty_response() {
    let mut server = Server::new();
    let error = run_from(
        ["mealie", "categories", "delete", "--id", "c1"],
        env(&server.url()),
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgs);
    let mock = server
        .mock("DELETE", "/api/organizers/categories/c1")
        .with_status(204)
        .create();
    let output = run_from(
        [
            "mealie",
            "categories",
            "delete",
            "--id",
            "c1",
            "--yes",
            "--quiet",
        ],
        env(&server.url()),
    )
    .unwrap();
    assert_eq!(output, "c1\n");
    mock.assert();
}

#[test]
fn merge_dry_run_is_non_mutating_and_requires_distinct_ids() {
    let server = Server::new();
    let output = run_from(
        [
            "mealie",
            "tags",
            "merge",
            "--from",
            "a",
            "--to",
            "b",
            "--dry-run",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["type"], "organizer_merge_plan");
    let error = run_from(
        [
            "mealie", "tags", "merge", "--from", "a", "--to", "a", "--yes",
        ],
        env(&server.url()),
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgs);
}

#[test]
fn organizer_identifiers_cannot_escape_the_endpoint() {
    let server = Server::new();
    for id in ["../other", "id?x=1", "%2e%2e"] {
        let error = run_from(
            ["mealie", "tags", "rename", "--id", id, "--name", "Quick"],
            env(&server.url()),
        )
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidArgs);
    }
}

#[test]
fn creates_category_with_name() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/api/organizers/categories")
        .match_body(Matcher::Json(serde_json::json!({"name":"Starter"})))
        .with_status(201)
        .with_body(r#"{"id":"1","name":"Starter","slug":"starter"}"#)
        .create();
    let output = run_from(
        [
            "mealie",
            "categories",
            "create",
            "--name",
            "Starter",
            "--json",
        ],
        env(&server.url()),
    )
    .unwrap();
    let values: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(values[0]["name"], "Starter");
    mock.assert();
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

    let detail = server
        .mock("GET", "/api/recipes/pesto-chicken")
        .with_body(r#"{"id":"r1","slug":"pesto-chicken","name":"pEsTo ChIcKeN"}"#)
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
    detail.assert();
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
