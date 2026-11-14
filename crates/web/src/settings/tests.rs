//! The settings as the page shows them and changes them, over a file of their own in a directory of its own, read by
//! a reading that stands in for the engine's: it refuses no slots and a key it does not know, and gives a context
//! limit and a budget where the file leaves them out.

use std::process::Command;

use super::*;

const FILE: &str = r#"# The settings.

[owner]
# The language the owner reads.
language = "Chinese" # as the owner asked

[agents]
command = "claude"
slots = 2
args = [
  "--permission-mode", "acceptEdits",
]
permissions = { heavy = ["uv"] }

[agents.levels.1]
desc = "Easy work"
model = "claude-haiku-5-5"

[agents.levels.2]
desc = "Ordinary work"
model = "claude-sonnet-5-5"

[items]
default_level = "1"
"#;

/// The keys the stand-in reading knows, by table.
const KNOWN: [(&str, &[&str]); 4] = [
    ("owner", &["language"]),
    (
        "agents",
        &["command", "slots", "args", "permissions", "levels", "config_dir"],
    ),
    ("items", &["default_level", "highest_level"]),
    ("sessions", &["context_limit"]),
];

/// The stand-in for the engine's reading of the settings.
fn read(text: &str) -> Result<JsonValue, String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut json = serde_json::to_value(&table).map_err(|e| e.to_string())?;
    for (name, keys) in json.as_object().unwrap().clone() {
        let known = KNOWN
            .iter()
            .find(|(table, _)| *table == name)
            .ok_or(format!("{name}: no such table"))?;
        for key in keys.as_object().unwrap().keys() {
            if !known.1.contains(&key.as_str()) {
                return Err(format!("{name}.{key}: no such key"));
            }
        }
    }
    if json["agents"]["slots"] == 0 {
        return Err("agents.slots: at least one round must be able to run".to_string());
    }
    if json.get("sessions").is_none() {
        json["sessions"] = serde_json::json!({ "context_limit": 500000 });
    }
    json["agents"]["work_budget_usd"] = serde_json::json!(5.0);
    Ok(json)
}

/// The settings in a directory of their own, the engine having started on them as they are written.
fn install(dir: &Path, text: &str) -> Install {
    let file = dir.join("install.toml");
    std::fs::write(&file, text).unwrap();
    Install {
        file,
        read: Box::new(read),
        running: read(text).unwrap(),
        git: &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgSign=false"],
        program: Program {
            variable: "CLAUDE_CONFIG_DIR",
            sign_in: "/login",
            signed_in_as: |dir| std::fs::read_to_string(dir.join("account")).ok(),
        },
    }
}

fn git_in(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgSign=false",
        ])
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn table<'a>(page: &'a SettingsPage, name: &str) -> &'a SettingsTable {
    page.tables
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no table {name}: {page:#?}"))
}

fn key<'a>(page: &'a SettingsPage, name: &str, key: &str) -> &'a SettingsKey {
    (table(page, name).keys.iter())
        .find(|k| k.key == key)
        .unwrap_or_else(|| panic!("no key {name}.{key}: {page:#?}"))
}

fn set(base: &str, sets: &[(&str, JsonValue)]) -> SettingsChange {
    SettingsChange {
        base: base.to_string(),
        set: (sets.iter())
            .map(|(key, value)| SetKey {
                key: key.to_string(),
                value: value.clone(),
            })
            .collect(),
    }
}

fn refusal(result: Result<Saved, Refused>) -> (StatusCode, String) {
    match result {
        Ok(saved) => panic!("it was saved: {saved:?}"),
        Err(Refused(status, why)) => (status, why),
    }
}

#[test]
fn the_file_is_shown_as_it_writes_the_settings_in_its_order() {
    let dir = tempfile::tempdir().unwrap();
    let install = install(dir.path(), FILE);
    let page = shown(&install).map_err(|r| r.1).unwrap();
    let names: Vec<&str> = page.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "owner",
            "agents",
            "agents.levels.1",
            "agents.levels.2",
            "items",
            "sessions"
        ],
        "the file's tables in its order, then those it leaves out"
    );
    let language = key(&page, "owner", "language");
    assert_eq!(
        language.written.as_deref(),
        Some("\"Chinese\""),
        "no comment round the value"
    );
    assert_eq!(language.running, None, "the engine runs with the value written");
    assert_eq!(language.change, Some(Change::Name));
    assert_eq!(
        key(&page, "agents", "args").written.as_deref(),
        Some("[\n  \"--permission-mode\", \"acceptEdits\",\n]"),
        "a value as the file writes it, over lines"
    );
    assert_eq!(
        key(&page, "agents", "permissions").written.as_deref(),
        Some("{ heavy = [\"uv\"] }")
    );
    assert_eq!(key(&page, "agents", "command").change, None, "only the file changes it");
    assert_eq!(key(&page, "agents.levels.2", "model").change, Some(Change::Name));
    let budget = key(&page, "agents", "work_budget_usd");
    assert_eq!(
        (budget.written.as_deref(), budget.running.as_deref()),
        (None, Some("5.0"))
    );
    assert!(table(&page, "sessions").left_out);
    let limit = key(&page, "sessions", "context_limit");
    assert_eq!(
        (limit.written.as_deref(), limit.running.as_deref()),
        (None, Some("500000"))
    );
    assert_eq!(limit.change, Some(Change::Count));
    assert_eq!(page.levels, ["1", "2"]);
    assert_eq!(page.unreadable, None);
    assert_eq!(page.git, None, "the directory is no work tree of git");
}

#[test]
fn a_change_is_written_with_the_files_comments_and_layout_kept() {
    let dir = tempfile::tempdir().unwrap();
    let install = install(dir.path(), FILE);
    let base = shown(&install).map_err(|r| r.1).unwrap().base;
    let saved = changed(
        &install,
        &set(
            &base,
            &[
                ("owner.language", serde_json::json!("English")),
                ("agents.slots", serde_json::json!(3)),
                ("agents.levels.2.model", serde_json::json!("claude-opus-5-5")),
                ("items.highest_level", serde_json::json!("2")),
                ("sessions.context_limit", serde_json::json!(400000)),
            ],
        ),
    )
    .map_err(|r| r.1)
    .unwrap();
    assert_eq!(
        saved.said,
        "owner.language = \"English\" instead of \"Chinese\"; agents.slots = 3 instead of 2; \
         agents.levels.2.model = \"claude-opus-5-5\" instead of \"claude-sonnet-5-5\"; items.highest_level = \"2\"; \
         sessions.context_limit = 400000"
    );
    assert!(!saved.committed, "no git keeps it");
    let text = std::fs::read_to_string(&install.file).unwrap();
    assert!(
        text.contains("# The language the owner reads.\nlanguage = \"English\" # as the owner asked\n"),
        "{text}"
    );
    assert!(text.contains("slots = 3\n"), "{text}");
    assert!(text.contains("model = \"claude-opus-5-5\"\n"), "{text}");
    assert!(
        text.contains("[items]\ndefault_level = \"1\"\nhighest_level = \"2\"\n"),
        "{text}"
    );
    assert!(text.contains("[sessions]\ncontext_limit = 400000\n"), "{text}");
    let page = shown(&install).map_err(|r| r.1).unwrap();
    assert_eq!(
        key(&page, "agents", "slots").running.as_deref(),
        Some("2"),
        "the engine runs with what it started with until it starts again"
    );
    assert_eq!(key(&page, "owner", "language").running.as_deref(), Some("\"Chinese\""));
}

#[test]
fn a_change_the_page_may_not_make_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let install = install(dir.path(), FILE);
    let base = shown(&install).map_err(|r| r.1).unwrap().base;
    for (sets, why) in [
        (
            vec![("agents.command", serde_json::json!("other"))],
            "agents.command is changed only in the file",
        ),
        // What the engine refuses, its own reading refuses, in its words.
        (
            vec![("agents.slots", serde_json::json!(0))],
            "the settings would not read: agents.slots: at least one round must be able to run",
        ),
        (
            vec![("agents.slots", serde_json::json!(-1))],
            "agents.slots is a whole number",
        ),
        (
            vec![("agents.slots", serde_json::json!("3"))],
            "agents.slots is a whole number",
        ),
        (
            vec![("owner.language", serde_json::json!("Eng\nlish"))],
            "owner.language is a name on one line",
        ),
        (
            vec![("items.highest_level", serde_json::json!("3"))],
            "items.highest_level is one of the levels: 1, 2",
        ),
    ] {
        let (status, said) = refusal(changed(&install, &set(&base, &sets)));
        assert_eq!((status, said.as_str()), (StatusCode::UNPROCESSABLE_ENTITY, why));
    }
    assert_eq!(std::fs::read_to_string(&install.file).unwrap(), FILE);
}

#[test]
fn a_change_against_a_file_changed_since_or_one_that_would_not_read_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let install = install(dir.path(), FILE);
    let base = shown(&install).map_err(|r| r.1).unwrap().base;
    let edited = FILE.replace("slots = 2", "slots = 4");
    std::fs::write(&install.file, &edited).unwrap();
    let (status, said) = refusal(changed(
        &install,
        &set(&base, &[("agents.slots", serde_json::json!(3))]),
    ));
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(said.contains("changed since the page read it"), "{said}");
    let wrong = FILE.replace("[items]", "[items]\nunknown = 1");
    std::fs::write(&install.file, &wrong).unwrap();
    let page = shown(&install).map_err(|r| r.1).unwrap();
    assert_eq!(page.unreadable.as_deref(), Some("items.unknown: no such key"));
    let (status, said) = refusal(changed(
        &install,
        &set(&page.base, &[("agents.slots", serde_json::json!(3))]),
    ));
    assert_eq!(
        (status, said.as_str()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "the settings would not read: items.unknown: no such key"
        )
    );
    assert_eq!(std::fs::read_to_string(&install.file).unwrap(), wrong);
}

#[test]
fn where_git_keeps_the_file_a_change_is_committed_and_none_goes_onto_changes_not_committed() {
    let dir = tempfile::tempdir().unwrap();
    let install = install(dir.path(), FILE);
    git_in(dir.path(), &["init", "-q"]);
    git_in(dir.path(), &["config", "user.name", "t"]);
    git_in(dir.path(), &["config", "user.email", "t@t"]);
    git_in(dir.path(), &["add", "install.toml"]);
    git_in(dir.path(), &["commit", "-q", "-m", "The settings"]);
    let page = shown(&install).map_err(|r| r.1).unwrap();
    assert_eq!(page.git, Some(Kept { committed: true }));
    let saved = changed(&install, &set(&page.base, &[("agents.slots", serde_json::json!(3))]))
        .map_err(|r| r.1)
        .unwrap();
    assert!(saved.committed);
    assert_eq!(
        git_in(dir.path(), &["log", "-1", "--format=%s"]).trim(),
        "The owner changed the settings on the page: agents.slots = 3 instead of 2"
    );
    assert_eq!(git_in(dir.path(), &["status", "--porcelain"]).trim(), "");
    std::fs::write(
        &install.file,
        FILE.replace("command = \"claude\"", "command = \"claude2\""),
    )
    .unwrap();
    let page = shown(&install).map_err(|r| r.1).unwrap();
    assert_eq!(page.git, Some(Kept { committed: false }));
    let (status, said) = refusal(changed(
        &install,
        &set(&page.base, &[("agents.slots", serde_json::json!(4))]),
    ));
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(said.contains("has changes not committed"), "{said}");
}

#[test]
fn the_accounts_beside_the_one_named_are_shown_and_one_signed_in_is_switched_to() {
    let dir = tempfile::tempdir().unwrap();
    let agents = dir.path().join("agents");
    for name in ["claude", "claude-second", "claude-third", "claudette", "other"] {
        std::fs::create_dir_all(agents.join(name)).unwrap();
    }
    std::fs::write(agents.join("claude/account"), "owner@example.com").unwrap();
    std::fs::write(agents.join("claude-second/account"), "second@example.com").unwrap();
    let named = agents.join("claude");
    let text = FILE.replace(
        "command = \"claude\"",
        &format!("command = \"claude\"\nconfig_dir = \"{}\"", named.display()),
    );
    let install = install(dir.path(), &text);
    let page = accounts_of(&install).map_err(|r| r.1).unwrap();
    let names: Vec<(&str, Option<&str>, bool)> = (page.accounts.iter())
        .map(|a| (a.name.as_str(), a.signed_in_as.as_deref(), a.named))
        .collect();
    assert_eq!(
        names,
        [
            ("claude", Some("owner@example.com"), true),
            ("claude-second", Some("second@example.com"), false),
            ("claude-third", None, false),
        ]
    );
    let (status, said) = refusal(chosen(
        &install,
        &AccountChoice {
            base: page.base.clone(),
            name: "claude-third".to_string(),
        },
    ));
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        said,
        format!(
            "claude-third is not signed in: run `CLAUDE_CONFIG_DIR={} claude` and type /login there first",
            agents.join("claude-third").display()
        )
    );
    let saved = chosen(
        &install,
        &AccountChoice {
            base: page.base,
            name: "claude-second".to_string(),
        },
    )
    .map_err(|r| r.1)
    .unwrap();
    assert!(saved.said.starts_with("agents.config_dir = "), "{}", saved.said);
    let page = accounts_of(&install).map_err(|r| r.1).unwrap();
    assert!(
        page.accounts.iter().any(|a| a.name == "claude-second" && a.named),
        "{page:?}"
    );
}

#[test]
fn a_key_is_split_and_written_as_toml_writes_it() {
    assert_eq!(
        path_of("agents.levels.\"a b\".model"),
        ["agents", "levels", "a b", "model"]
    );
    assert_eq!(path_of(""), Vec::<String>::new());
    assert_eq!(key_text("a b"), "\"a b\"");
    assert_eq!(key_text("model_2"), "model_2");
    assert_eq!(
        toml_text(&serde_json::json!({ "most": 3, "names": ["a", "b"], "none": null })).as_deref(),
        Some("{ most = 3, names = [\"a\", \"b\"] }")
    );
}
