//! `GET /api/v1/flow/changes`: what changed from the definition compared against to the one shown.

use serde_json::json;

use crate::{Shared, edit, get_json, serve, tiny};

#[tokio::test]
async fn with_nothing_to_compare_against_the_answer_says_so() {
    let router = serve(&Shared::new(Ok(tiny())), None);
    assert_eq!(
        get_json(&router, "/api/v1/flow/changes").await,
        json!({ "state": "nothingToCompare" })
    );
}

#[tokio::test]
async fn the_changes_are_given_in_their_order_with_the_definition_compared_against() {
    let after = edit(
        tiny(),
        Some("review"),
        "title = \"Review\"",
        "title = \"Review the plan\"",
    );
    let after = edit(
        after,
        Some("review"),
        "inputs = [\"plan\"]",
        "inputs = [\"plan\", \"request\"]",
    );
    let after = edit(
        after,
        None,
        "values = [\"yes\", \"no\"]",
        "values = [\"yes\", \"no\", \"later\"]",
    );
    let after = edit(
        after,
        None,
        "says = \"The owner let the work merge\"",
        "says = \"The owner confirmed the merge\"",
    );
    let router = serve(&Shared::new(Ok(after)), Some(&Shared::new(Ok(tiny()))));
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    assert_eq!(answer["state"], "compared");
    assert_eq!(answer["against"]["state"], "compiled");
    assert_eq!(answer["against"]["flow"]["cells"][1]["title"], "Review");
    assert_eq!(
        answer["changes"],
        json!([
            {
                "kind": "testReworded",
                "test": "may_merge",
                "before": "The owner let the work merge",
                "after": "The owner confirmed the merge",
            },
            { "kind": "title", "cell": "review", "before": "Review", "after": "Review the plan" },
            { "kind": "inputAdded", "cell": "review", "block": "request" },
            { "kind": "values", "block": "content", "before": ["yes", "no"], "after": ["yes", "no", "later"] },
        ])
    );
}

#[tokio::test]
async fn a_condition_and_a_precondition_changed_are_given_with_their_tests_as_declared() {
    let before = edit(tiny(), Some("merge"), "requires = [\"may_merge\"]", "requires = []");
    let before = edit(
        before,
        Some("review"),
        "limit = 5",
        "limit = 5\nwhen = { test = \"may_merge\" }",
    );
    let router = serve(&Shared::new(Ok(tiny())), Some(&Shared::new(Ok(before))));
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    let may_merge = json!({
        "name": "may_merge",
        "words": "The owner let the work merge",
        "kind": "all",
        "reads": ["go"],
        "is": "yes",
    });
    assert_eq!(
        answer["changes"],
        json!([
            {
                "kind": "condition",
                "cell": "review",
                "before": { "on": "test", "test": may_merge },
                "after": null,
            },
            { "kind": "preconditionAdded", "cell": "merge", "test": may_merge },
        ])
    );
}

#[tokio::test]
async fn a_test_put_in_or_changed_and_a_block_s_source_are_given_as_each_definition_declares_them() {
    let after = edit(
        tiny(),
        None,
        "kind = \"all\"\nreads = [\"go\"]",
        "kind = \"any\"\nreads = [\"go\", \"content\"]",
    );
    let after = edit(
        after,
        None,
        "[blocks.request]\npeople = true",
        "[blocks.request]\npeople = true\noutside = \"repository\"",
    );
    let checked =
        "\n[tests.checked]\nsays = \"The content is checked\"\nkind = \"none\"\nreads = [\"content\"]\nis = \"no\"\n";
    let after = edit(after, None, "is = \"yes\"\n", &format!("is = \"yes\"\n{checked}"));
    let after = edit(
        after,
        Some("merge"),
        "requires = [\"may_merge\"]",
        "requires = [\"may_merge\", \"checked\"]",
    );
    let router = serve(&Shared::new(Ok(after)), Some(&Shared::new(Ok(tiny()))));
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    let test = |kind: &str, reads: &[&str]| {
        json!({
            "name": "may_merge",
            "words": "The owner let the work merge",
            "kind": kind,
            "reads": reads,
            "is": "yes",
        })
    };
    let checked = json!({
        "name": "checked",
        "words": "The content is checked",
        "kind": "none",
        "reads": ["content"],
        "is": "no",
    });
    assert_eq!(
        answer["changes"],
        json!([
            { "kind": "testAdded", "test": checked },
            {
                "kind": "testChanged",
                "test": "may_merge",
                "before": test("all", &["go"]),
                "after": test("any", &["go", "content"]),
            },
            { "kind": "preconditionAdded", "cell": "merge", "test": checked },
            { "kind": "outside", "block": "request", "before": null, "after": "repository" },
        ])
    );
}

#[tokio::test]
async fn cells_put_in_taken_out_and_moved_and_limits_are_given_with_their_places_counted_from_naught() {
    let after = edit(tiny(), None, "runs = 3", "runs = 4");
    let after = edit(after, Some("review"), "limit = 5", "limit = 6");
    let router = serve(&Shared::new(Ok(after)), Some(&Shared::new(Ok(tiny()))));
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    assert_eq!(
        answer["changes"],
        json!([
            { "kind": "flowLimit", "limit": "runs", "before": 3, "after": 4 },
            { "kind": "cellLimit", "cell": "review", "limit": "runs", "before": 5, "after": 6 },
        ])
    );

    let mut after = edit(
        tiny(),
        None,
        r#"order = ["plan", "review", "merge"]"#,
        r#"order = ["check", "plan", "merge"]"#,
    );
    after.cells.remove("review");
    after.cells.insert(
        "check".to_string(),
        "title = \"Check\"\nkind = \"engine\"\noutputs = [\"content\", \"opinions\"]\n".to_string(),
    );
    let after = edit(after, None, "[blocks.plan]", "[blocks.plan]\nshown = true");
    let router = serve(&Shared::new(Ok(after)), Some(&Shared::new(Ok(tiny()))));
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    assert_eq!(
        answer["changes"],
        json!([
            { "kind": "cellAdded", "cell": "check", "place": 0 },
            { "kind": "cellRemoved", "cell": "review", "place": 1 },
            { "kind": "shown", "block": "plan", "before": false, "after": true },
        ])
    );
}

#[tokio::test]
async fn when_either_definition_does_not_compile_no_changes_are_given_and_the_other_says_why() {
    let broken = edit(
        tiny(),
        Some("merge"),
        "kind = \"engine\"",
        "kind = \"engine\"\nline = \"engineer\"",
    );
    let router = serve(&Shared::new(Ok(tiny())), Some(&Shared::new(Ok(broken.clone()))));
    assert_eq!(
        get_json(&router, "/api/v1/flow/changes").await,
        json!({
            "state": "compared",
            "against": {
                "state": "faults",
                "faults": [{
                    "file": "merge.toml",
                    "field": "line",
                    "what": { "kind": "engineHasLine", "line": "engineer" },
                }],
            },
            "changes": null,
        })
    );

    let router = serve(&Shared::new(Ok(broken)), Some(&Shared::new(Ok(tiny()))));
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    assert_eq!(answer["against"]["state"], "compiled");
    assert_eq!(answer["changes"], json!(null));
}
