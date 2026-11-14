use super::*;

const SCHEMA: &str = "-- The things.
CREATE TABLE things (
    id INTEGER PRIMARY KEY,
    -- What it is called.
    name TEXT NOT NULL, -- Plain text.
    size INTEGER,
    -- Where it is kept. Hidden from agents.
    place TEXT
);

-- The secrets of the engine. Hidden from agents.
CREATE TABLE secrets (
    id INTEGER PRIMARY KEY,
    value TEXT NOT NULL,
    UNIQUE (value)
);
";

fn texts() -> BTreeMap<String, String> {
    [
        ("closed", "The store is not open to agents yet."),
        ("unreadable", "Not read."),
        ("error", "The query failed: {why}"),
        ("timeout", "Stopped after {ms} ms."),
        ("unavailable", "The store could not be read."),
        ("row_too_long", "A row is over {bytes} bytes."),
        ("hides_nothing", "Nothing is hidden."),
        ("store_table", "### {table}: {comment}\n{columns}"),
        ("store_column", "- {column} {type}"),
        ("store_column_commented", "- {column} {type}: {comment}"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn tool(words: &str) -> Tool {
    let mut tool: Tool = toml::from_str(&format!("words = {words:?}\n[request]\ntype = \"object\"\n")).unwrap();
    tool.texts = texts();
    tool
}

/// A store file with the test schema and `rows` things, open to queries with these limits.
struct Kept {
    dir: tempfile::TempDir,
    // The engine holds its store while agents query it.
    _db: Connection,
    file: PathBuf,
    querying: Querying,
}

fn kept(rows: usize, limits: settings::Query) -> Kept {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(store::FILE);
    let db = Connection::open(&file).unwrap();
    db.pragma_update(None, "journal_mode", "wal").unwrap();
    db.execute_batch(SCHEMA).unwrap();
    for n in 1..=rows {
        db.execute(
            "INSERT INTO things (name, size, place) VALUES (?1, ?2, '/home/somebody')",
            (format!("thing {n}"), n),
        )
        .unwrap();
    }
    db.execute("INSERT INTO secrets (value) VALUES ('hush')", []).unwrap();
    let querying = Querying::new(&limits, Some(&tool("Words. {store}")), dir.path(), SCHEMA).unwrap();
    Kept {
        dir,
        _db: db,
        file,
        querying,
    }
}

fn on() -> settings::Query {
    settings::Query {
        open: true,
        ..Default::default()
    }
}

fn ask(querying: &Querying, sql: &str) -> String {
    querying.answer(&json!({ "sql": sql }).to_string(), &Arc::new(Cancel::default()))
}

fn rows(answer: &str) -> Vec<Value> {
    let answer: Value = serde_json::from_str(answer).unwrap_or_else(|e| panic!("{e}: {answer}"));
    answer["rows"].as_array().unwrap_or_else(|| panic!("{answer}")).clone()
}

fn open_of(kept: &Kept) -> Open {
    Open {
        store: kept.file.clone(),
        limits: Limits::of(&on()),
        hidden: vec!["secrets".to_string()],
        hidden_columns: vec![("things".to_string(), "place".to_string())],
    }
}

/// What the store's files hold, the log of its writes with the file.
fn bytes_of(kept: &Kept) -> (Vec<u8>, Vec<u8>) {
    let log = kept.file.with_file_name(format!("{}-wal", store::FILE));
    (
        std::fs::read(&kept.file).unwrap(),
        std::fs::read(log).unwrap_or_default(),
    )
}

/// Every write there is, each of which must fail.
fn writes(kept: &Kept) -> Vec<String> {
    let elsewhere = kept.dir.path().join("an-agent-made-this.sqlite");
    let mut writes: Vec<String> = [
        "INSERT INTO things (name) VALUES ('x')",
        "UPDATE things SET name = 'x'",
        "DELETE FROM things",
        "REPLACE INTO things (id, name) VALUES (1, 'x')",
        "CREATE TABLE more (a)",
        "CREATE TEMP TABLE more (a)",
        "CREATE INDEX at_name ON things (name)",
        "CREATE VIEW v AS SELECT 1",
        "DROP TABLE things",
        "ALTER TABLE things ADD COLUMN more TEXT",
        "ALTER TABLE things RENAME TO other",
        "WITH gone AS (SELECT 1) DELETE FROM things",
        "PRAGMA user_version = 5",
        "PRAGMA writable_schema = 1",
        "PRAGMA query_only = 0",
        "PRAGMA journal_mode = delete",
        "VACUUM",
        "REINDEX",
        "ANALYZE",
        "BEGIN",
        "SAVEPOINT a",
        "COMMIT",
        "DETACH DATABASE main",
    ]
    .map(str::to_string)
    .to_vec();
    writes.push(format!("ATTACH DATABASE '{}' AS other", elsewhere.display()));
    writes.push(format!("VACUUM INTO '{}'", elsewhere.display()));
    writes
}

#[test]
fn a_query_gives_its_rows_as_objects() {
    let kept = kept(3, on());
    let answer = ask(
        &kept.querying,
        "SELECT id, name, size FROM things WHERE id > 1 ORDER BY id",
    );
    assert_eq!(
        rows(&answer),
        [
            json!({"id": 2, "name": "thing 2", "size": 2}),
            json!({"id": 3, "name": "thing 3", "size": 3})
        ]
    );
    assert!(!answer.contains("cut"), "{answer}");
}

#[test]
fn a_write_of_every_kind_is_refused_by_the_connection_and_changes_nothing_in_the_file_or_its_log() {
    let kept = kept(2, on());
    let before = bytes_of(&kept);
    assert!(
        !before.1.is_empty(),
        "the store's writes are in its log, which is what a write would grow"
    );
    let open = open_of(&kept);
    for sql in writes(&kept) {
        let answer = ask(&kept.querying, &sql);
        assert!(answer.starts_with("The query failed: "), "{sql}: {answer}");
    }
    assert_eq!(bytes_of(&kept), before, "the file and its log are as they were");
    assert!(!kept.dir.path().join("an-agent-made-this.sqlite").exists());
    assert_eq!(
        rows(&ask(&kept.querying, "SELECT count(*) AS n FROM things")),
        [json!({"n": 2})]
    );
    // The connection the query runs on is `connect`'s: its own refusals, with the authorizer's, are those above; the
    // refusals that are SQLite's own, with no authorizer, are these.
    let bare = open_read_only(&open).unwrap();
    for sql in [
        "INSERT INTO things (name) VALUES ('x')",
        "UPDATE things SET name = 'x'",
        "DELETE FROM things",
        "CREATE TABLE more (a)",
        "DROP TABLE things",
    ] {
        assert!(bare.execute(sql, []).is_err(), "{sql}");
    }
    let used = connect(&open, &Arc::new(Cancel::default())).unwrap();
    for sql in writes(&kept) {
        assert!(used.execute_batch(&sql).is_err(), "{sql}");
    }
}

#[test]
fn only_one_statement_is_run() {
    let kept = kept(1, on());
    let answer = ask(&kept.querying, "SELECT 1; DELETE FROM things");
    assert!(answer.starts_with("The query failed: "), "{answer}");
    assert_eq!(
        rows(&ask(&kept.querying, "SELECT count(*) AS n FROM things")),
        [json!({"n": 1})]
    );
}

#[test]
fn a_table_or_a_column_hidden_from_agents_is_not_read_and_nor_is_the_schema() {
    let kept = kept(1, on());
    for sql in [
        "SELECT * FROM secrets",
        "SELECT name FROM sqlite_master",
        "SELECT * FROM things WHERE id IN (SELECT id FROM secrets)",
        "SELECT place FROM things",
        "SELECT * FROM things",
        "SELECT t.place FROM things t",
        "SELECT id FROM things WHERE place LIKE '/home%'",
        "SELECT x.place FROM (SELECT place FROM things) x",
    ] {
        let answer = ask(&kept.querying, sql);
        assert!(answer.starts_with("The query failed: "), "{sql}: {answer}");
        assert!(!answer.contains("hush") && !answer.contains("/home"), "{answer}");
    }
    assert_eq!(
        rows(&ask(&kept.querying, "SELECT id, name FROM things")),
        [json!({"id": 1, "name": "thing 1"})]
    );
}

#[test]
fn rows_past_the_row_limit_are_not_kept_and_the_query_for_the_rest_is_given() {
    let kept = kept(5, settings::Query { max_rows: 2, ..on() });
    let answer = ask(&kept.querying, "SELECT id FROM things ORDER BY id;");
    assert_eq!(rows(&answer), [json!({"id": 1}), json!({"id": 2})]);
    let cut: Value = serde_json::from_str(&answer).unwrap();
    assert_eq!(cut["cut"]["count"], 2);
    let rest = ask(&kept.querying, cut["cut"]["rest"].as_str().unwrap());
    assert_eq!(rows(&rest), [json!({"id": 3}), json!({"id": 4})]);
}

#[test]
fn the_rest_of_a_query_keys_its_rows_as_the_query_did_and_is_not_undone_by_what_follows_it() {
    let kept = kept(4, settings::Query { max_rows: 2, ..on() });
    let sql = "SELECT a.id, b.id, c.id FROM things a, things b, things c WHERE b.id = 1 AND c.id = 2 ORDER BY a.id";
    let answer = ask(&kept.querying, sql);
    assert_eq!(rows(&answer)[0], json!({"id": 1, "id:1": 1, "id:2": 2}));
    let cut: Value = serde_json::from_str(&answer).unwrap();
    let rest = ask(&kept.querying, cut["cut"]["rest"].as_str().unwrap());
    assert_eq!(rows(&rest)[0], json!({"id": 3, "id:1": 1, "id:2": 2}));
    for ending in [
        ";",
        " ;",
        "; -- a comment",
        ";\n/* and another */ ;",
        " -- a comment",
        "\n",
    ] {
        let sql = format!("SELECT id, 'a;--b' AS text FROM things ORDER BY id{ending}");
        let rest = rest_of(&kept.querying, &sql);
        assert_eq!(rows(&rest)[0], json!({"id": 3, "text": "a;--b"}), "{ending:?}");
    }
}

/// The result of the query for the rest of this one, from its third row.
fn rest_of(querying: &Querying, sql: &str) -> String {
    ask(querying, &rest(sql, 2))
}

#[test]
fn the_end_of_a_statement_is_found_past_its_comments_and_semicolons() {
    assert_eq!(statement("  SELECT 1 ; -- c\n"), "SELECT 1");
    assert_eq!(statement("SELECT '--;' ;;"), "SELECT '--;'");
    assert_eq!(statement("SELECT 1 /* ; */"), "SELECT 1");
    assert_eq!(statement("SELECT \"a\"\"b\", [x]; "), "SELECT \"a\"\"b\", [x]");
    assert_eq!(statement("SELECT é; -- é"), "SELECT é");
    assert_eq!(statement("SELECT 'open"), "SELECT 'open");
}

#[test]
fn rows_past_the_byte_limit_are_not_kept() {
    let kept = kept(50, settings::Query { max_bytes: 200, ..on() });
    let answer = ask(&kept.querying, "SELECT name FROM things ORDER BY id");
    let kept_rows = rows(&answer).len();
    assert!((1..50).contains(&kept_rows), "{kept_rows}");
    let cut: Value = serde_json::from_str(&answer).unwrap();
    assert_eq!(cut["cut"]["count"], kept_rows);
}

#[test]
fn a_row_over_the_byte_limit_is_refused_even_the_first_and_even_of_many_columns() {
    let kept = kept(
        1,
        settings::Query {
            max_bytes: 1000,
            ..on()
        },
    );
    let wide = "SELECT hex(zeroblob(300)) a, hex(zeroblob(300)) b, hex(zeroblob(300)) c";
    assert_eq!(ask(&kept.querying, wide), "A row is over 1000 bytes.");
    let many = format!("SELECT {}", vec!["hex(zeroblob(300))"; 400].join(", "));
    assert_eq!(ask(&kept.querying, &many), "A row is over 1000 bytes.");
    let long = ask(&kept.querying, "SELECT hex(zeroblob(600)) a");
    assert_eq!(long, "A row is over 1000 bytes.");
    assert_eq!(rows(&ask(&kept.querying, "SELECT hex(zeroblob(100)) a")).len(), 1);
}

#[test]
fn a_query_that_runs_too_long_is_stopped() {
    let kept = kept(1, settings::Query { timeout_ms: 50, ..on() });
    let began = Instant::now();
    let answer = ask(
        &kept.querying,
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n) SELECT count(*) FROM n",
    );
    assert_eq!(answer, "Stopped after 50 ms.");
    assert!(began.elapsed() < Duration::from_secs(10));
}

#[test]
fn one_call_over_long_values_costs_what_the_value_limit_allows_however_the_values_are_made() {
    let kept = kept(1, settings::Query { timeout_ms: 50, ..on() });
    // The progress handler runs between instructions: this is one, over two strings of a megabyte.
    let began = Instant::now();
    let made = "SELECT instr(hex(zeroblob(500000)), substr(hex(zeroblob(500000)), 1, 200000) || '1')";
    let answer = ask(&kept.querying, made);
    assert!(answer.starts_with("The query failed: "), "{answer}");
    for builder in [
        "SELECT zeroblob(1000000)",
        "SELECT randomblob(1000000)",
        "SELECT printf('%1000000d', 1)",
        "SELECT hex(zeroblob(40000))",
        "SELECT replace(hex(zeroblob(30000)), '0', hex(zeroblob(30000)))",
    ] {
        assert!(
            ask(&kept.querying, builder).starts_with("The query failed: "),
            "{builder}"
        );
    }
    assert!(began.elapsed() < Duration::from_secs(2), "{:?}", began.elapsed());
    // One that fits the limit is as costly as the limit allows, and no more.
    let began = Instant::now();
    let fits = "SELECT instr(hex(zeroblob(30000)), substr(hex(zeroblob(30000)), 1, 20000) || '1') AS at";
    let answer = ask(&kept.querying, fits);
    assert!(
        answer.starts_with("{\"rows\"") || answer.starts_with("Stopped"),
        "{answer}"
    );
    assert!(began.elapsed() < Duration::from_secs(10), "{:?}", began.elapsed());
}

#[test]
fn a_query_told_to_stop_is_stopped_where_it_runs() {
    let kept = kept(
        1,
        settings::Query {
            timeout_ms: 60_000,
            ..on()
        },
    );
    let cancel = Arc::new(Cancel::default());
    let told = cancel.clone();
    let began = Instant::now();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        told.tell();
    });
    let answer = kept.querying.answer(
        &json!({"sql": "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n) SELECT count(*) FROM n"})
            .to_string(),
        &cancel,
    );
    stopper.join().unwrap();
    assert!(answer.starts_with("Stopped after "), "{answer}");
    assert!(began.elapsed() < Duration::from_secs(10), "{:?}", began.elapsed());
    // Told before it runs, it stops at once.
    let before = Arc::new(Cancel::default());
    before.tell();
    let began = Instant::now();
    let answer = kept.querying.answer(
        &json!({"sql": "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n) SELECT count(*) FROM n"})
            .to_string(),
        &before,
    );
    assert!(answer.starts_with("Stopped after "), "{answer}");
    assert!(began.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_failure_that_is_not_the_querys_is_told_in_the_files_words_never_the_librarys() {
    let dir = tempfile::tempdir().unwrap();
    // No store file is there to open.
    let querying = Querying::new(&on(), Some(&tool("Words.")), dir.path(), SCHEMA).unwrap();
    let said = ask(&querying, "SELECT 1");
    assert_eq!(said, "The store could not be read.");
    assert!(!said.contains(&dir.path().display().to_string()));
    // A file that is no database.
    std::fs::write(dir.path().join(store::FILE), vec![7u8; 4096]).unwrap();
    assert_eq!(
        ask(&querying, "SELECT count(*) FROM things"),
        "The store could not be read."
    );
}

#[test]
fn a_result_too_long_to_give_is_kept_whole_and_given_cut_with_its_size_and_the_query_for_the_rest() {
    let kept = kept(40, on());
    let sql = "SELECT id, name FROM things ORDER BY id";
    let whole = ask(&kept.querying, sql);
    assert_eq!(rows(&whole).len(), 40, "the whole is what the engine keeps");
    let input = json!({ "sql": sql }).to_string();
    let given = Querying::cut(&whole, &input, 300);
    let cut: Value = serde_json::from_str(&given).unwrap();
    let first = rows(&given);
    assert!(first.len() < 40 && !first.is_empty(), "{}", first.len());
    assert!(given.len() < whole.len());
    assert_eq!(cut["cut"]["bytes"], whole.len());
    assert_eq!(cut["cut"]["count"], first.len());
    let rest = rows(&ask(&kept.querying, cut["cut"]["rest"].as_str().unwrap()));
    assert_eq!(rest.len(), 40 - first.len());
    assert_eq!(rest[0]["id"], first.len() + 1);
    // One short enough is given whole, and what is no rows as it is.
    assert_eq!(Querying::cut(&whole, &input, whole.len()), whole);
    let said = kept.querying.unavailable();
    assert_eq!(Querying::cut(&said, &input, 3), said);
}

#[test]
fn a_first_row_longer_than_what_a_message_gives_is_given_shortened_and_the_rest_makes_progress() {
    let kept = kept(3, on());
    let sql = "SELECT id, hex(zeroblob(10000)) AS text FROM things ORDER BY id".to_string();
    let whole = ask(&kept.querying, &sql);
    assert!(whole.len() > 60_000);
    let input = json!({ "sql": sql }).to_string();
    let given = Querying::cut(&whole, &input, 1000);
    let cut: Value = serde_json::from_str(&given).unwrap();
    assert!(given.len() < 2500, "{}", given.len());
    assert_eq!(rows(&given).len(), 1);
    assert_eq!(rows(&given)[0]["id"], 1);
    assert!(rows(&given)[0]["text"].as_str().unwrap().len() < 1000);
    assert_eq!(
        (&cut["cut"]["trimmed"], &cut["cut"]["count"]),
        (&json!(true), &json!(1))
    );
    assert_eq!(cut["cut"]["bytes"], whole.len());
    // The rest is from the row after, which is the next to be given: each cut moves on.
    let rest = ask(&kept.querying, cut["cut"]["rest"].as_str().unwrap());
    assert_eq!(rows(&rest)[0]["id"], 2);
    let next = Querying::cut(&rest, &json!({ "sql": cut["cut"]["rest"] }).to_string(), 1000);
    let next: Value = serde_json::from_str(&next).unwrap();
    assert!(next["cut"]["rest"].as_str().unwrap().ends_with("OFFSET 1"));
}

#[test]
fn a_request_that_is_not_a_query_is_said_so() {
    let kept = kept(1, on());
    for input in ["{}", "[]", "{\"sql\": 3}", "not json"] {
        let said = kept.querying.answer(input, &Arc::new(Cancel::default()));
        assert_eq!(said, "Not read.", "{input}");
    }
}

#[test]
fn the_store_is_described_from_the_comments_of_the_schema_with_the_hidden_left_out() {
    let described = describe(SCHEMA, &texts());
    assert_eq!(
        described,
        "### things: The things.\n- id INTEGER\n- name TEXT: What it is called. Plain text.\n- size INTEGER"
    );
    assert!(
        !described.contains("secrets") && !described.contains("hush") && !described.contains("place"),
        "{described}"
    );
    // A comment of the schema changed changes the description.
    let changed = SCHEMA
        .replace("The things.", "What there is.")
        .replace("Plain text.", "Its words.");
    assert_eq!(
        describe(&changed, &texts()),
        "### things: What there is.\n- id INTEGER\n- name TEXT: What it is called. Its words.\n- size INTEGER"
    );
    let kept = kept(0, on());
    assert_eq!(kept.querying.store(), described);
    assert_eq!(kept.querying.words(), format!("Words. {described}"));
}

#[test]
fn the_real_schema_is_read_table_by_table() {
    let all = tables(store::SCHEMA);
    let names: Vec<&str> = all.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.len(), store::SCHEMA.matches("CREATE TABLE ").count());
    assert!(names.contains(&"items") && names.contains(&"results"));
    let items = all.iter().find(|t| t.name == "items").unwrap();
    assert!(
        items.comment.starts_with("An item: a thing to be done"),
        "{}",
        items.comment
    );
    assert_eq!(
        items.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["number", "title", "added", "level"]
    );
    assert!(all.iter().all(|t| !t.columns.is_empty()));
}

#[test]
fn the_switch_is_refused_while_the_schema_hides_nothing_from_agents() {
    let dir = tempfile::tempdir().unwrap();
    let nothing = "-- The things.\nCREATE TABLE things (\n    id INTEGER PRIMARY KEY\n);\n";
    let refused = Querying::new(&on(), Some(&tool("Words.")), dir.path(), nothing).unwrap_err();
    assert_eq!(refused, "Nothing is hidden.");
    // A hidden column alone is a mark, and a switch off needs none.
    let column = "CREATE TABLE things (\n    -- Hidden from agents.\n    id INTEGER PRIMARY KEY\n);\n";
    assert!(Querying::new(&on(), Some(&tool("Words.")), dir.path(), column).is_ok());
    assert!(Querying::new(&settings::Query::default(), Some(&tool("Words.")), dir.path(), nothing).is_ok());
}

#[test]
fn with_the_switch_off_the_request_is_answered_in_the_files_words_and_the_store_is_not_touched() {
    let dir = tempfile::tempdir().unwrap();
    // No store file is there: an open store would fail to be opened, and an opened one would make the file.
    let off = Querying::new(
        &settings::Query::default(),
        Some(&tool("Words. {store}")),
        dir.path(),
        SCHEMA,
    )
    .unwrap();
    let said = "The store is not open to agents yet.";
    let cancel = Arc::new(Cancel::default());
    assert_eq!(off.answer(&json!({ "sql": "SELECT 1" }).to_string(), &cancel), said);
    assert_eq!(off.answer("garbage", &cancel), said);
    assert_eq!(off.words(), format!("Words. {said}"));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "nothing was made");
}
