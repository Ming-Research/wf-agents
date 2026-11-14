//! The tool `query`: one read-only SQL query on the engine's store, run here by the engine, which holds the store; the
//! agent only asks for it in its return and is answered in its next message. The connection refuses every write itself
//! (it is opened read-only, its `query_only` is on, and its authorizer allows nothing but a select of a table agents
//! may read), and a query is bounded in rows, in time and in bytes. The words an agent is told, and each place the
//! tool speaks of its own answers, are the tool's file in the flow (`agents/tools/query.toml`); what the code makes is
//! JSON and a query.
//!
//! The answer is `{"rows": [...]}`, each row an object by the column names. Where the rows past a bound were not kept,
//! it holds `"cut": {"kept": n, "rest": "<query>"}` too; and a message gives a result longer than the tool's file says
//! cut to its first rows, with `"cut": {"bytes": <the whole's size>, "given": n, "rest": "<query>"}`, the whole being
//! kept in the records as it is answered here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use flow::agents::{Tool, fill};
use rusqlite::hooks::{AuthAction, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, ErrorCode, OpenFlags};
use serde_json::{Map, Value, json};

use crate::settings;

/// The name of the tool.
pub const QUERY: &str = "query";

/// What the comment of a table says to keep it from the agents: the table is not described to them and a query of
/// theirs may not read it. Matched without regard to case.
pub const HIDDEN: &str = "hidden from agents";

/// A table of the store, as `db/schema.sql` says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    /// Its comment, the lines joined by a space.
    pub comment: String,
    pub columns: Vec<Column>,
}

/// A column of a table of the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    /// Its type as the schema writes it, empty where it has none.
    pub kind: String,
    /// The comment above it and the comment after it on its line, joined by a space.
    pub comment: String,
}

impl Table {
    /// Whether its comment keeps it from the agents.
    pub fn hidden(&self) -> bool {
        self.comment.to_lowercase().contains(HIDDEN)
    }
}

/// The tables of a schema, with the comment above each and above and after each of its columns. A comment is the
/// lines of `--` directly above what it is of, with no blank line between.
pub fn tables(schema: &str) -> Vec<Table> {
    let mut found = Vec::new();
    let mut comment: Vec<String> = Vec::new();
    let mut open: Option<Table> = None;
    for line in schema.lines() {
        let line = line.trim();
        if let Some(table) = open.as_mut() {
            if line.starts_with(')') {
                found.extend(open.take());
            } else if let Some(said) = line.strip_prefix("--") {
                comment.push(said.trim().to_string());
            } else {
                let (code, after) = line
                    .split_once("--")
                    .map_or((line, None), |(c, a)| (c.trim(), Some(a.trim())));
                let mut words = code.split_whitespace();
                let name = words.next().unwrap_or_default();
                let constraint = ["PRIMARY", "UNIQUE", "FOREIGN", "CHECK", "CONSTRAINT"].contains(&name);
                if !name.is_empty() && !constraint {
                    comment.extend(after.map(str::to_string));
                    table.columns.push(Column {
                        name: name.to_string(),
                        kind: words.next().unwrap_or_default().trim_end_matches(',').to_string(),
                        comment: comment.join(" "),
                    });
                }
                comment.clear();
            }
            continue;
        }
        if let Some(said) = line.strip_prefix("--") {
            comment.push(said.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("CREATE TABLE ") {
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or_default();
            open = Some(Table {
                name: name.to_string(),
                comment: comment.join(" "),
                columns: Vec::new(),
            });
            comment.clear();
        } else {
            comment.clear();
        }
    }
    found
}

/// What is given for `{store}` where the store is open: each table agents may read, as the tool's file lays it out
/// (`store_table`, with its columns each as `store_column`, or `store_column_commented` where the schema has a comment
/// on it), from the comments of the schema.
pub fn describe(schema: &str, texts: &BTreeMap<String, String>) -> String {
    let text = |name: &str| texts.get(name).map_or("", String::as_str);
    let described: Vec<String> = (tables(schema).iter().filter(|t| !t.hidden()))
        .map(|table| {
            let columns: Vec<String> = (table.columns.iter())
                .map(|column| {
                    let which = if column.comment.is_empty() {
                        "store_column"
                    } else {
                        "store_column_commented"
                    };
                    let slots = [
                        ("column", column.name.as_str()),
                        ("type", column.kind.as_str()),
                        ("comment", column.comment.as_str()),
                    ];
                    fill(text(which), &slots)
                })
                .collect();
            let slots = [
                ("table", table.name.as_str()),
                ("comment", table.comment.as_str()),
                ("columns", &columns.join("\n")),
            ];
            fill(text("store_table"), &slots)
        })
        .collect();
    described.join("\n\n")
}

/// The bounds of a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_rows: usize,
    pub timeout: Duration,
    pub max_bytes: usize,
}

impl Limits {
    fn of(settings: &settings::Query) -> Self {
        Limits {
            max_rows: settings.max_rows,
            timeout: Duration::from_millis(settings.timeout_ms),
            max_bytes: settings.max_bytes,
        }
    }
}

/// The store as it is open to queries.
#[derive(Clone, Debug)]
struct Open {
    store: PathBuf,
    limits: Limits,
    hidden: Vec<String>,
}

/// The tool `query` on this installation: its file in the flow, and the store where the settings open it.
#[derive(Clone, Debug)]
pub struct Querying {
    words: String,
    texts: BTreeMap<String, String>,
    schema: String,
    open: Option<Open>,
}

impl Querying {
    /// The tool as it is before the settings are known: closed, and saying nothing of its answers.
    pub fn closed() -> Self {
        Querying {
            words: String::new(),
            texts: BTreeMap::new(),
            schema: String::new(),
            open: None,
        }
    }

    /// The tool as the settings and the flow's file for it say, on the store in this data directory, which `schema`
    /// describes. Without the tool's file nothing is told, and nothing is said of an answer.
    pub fn new(settings: &settings::Query, tool: Option<&Tool>, data: &Path, schema: &str) -> Self {
        let open = settings.open.then(|| Open {
            store: data.join(store::FILE),
            limits: Limits::of(settings),
            hidden: tables(schema)
                .into_iter()
                .filter(Table::hidden)
                .map(|t| t.name)
                .collect(),
        });
        Querying {
            words: tool.map(|t| t.words.clone()).unwrap_or_default(),
            texts: tool.map(|t| t.texts.clone()).unwrap_or_default(),
            schema: schema.to_string(),
            open,
        }
    }

    /// The tool's words with `{store}` filled: by what the schema's comments say of the store where it is open, and
    /// by the file's own answer that it is not where it is not.
    pub fn words(&self) -> String {
        let store = match &self.open {
            Some(_) => describe(&self.schema, &self.texts),
            None => self.text("closed", &[]),
        };
        fill(&self.words, &[("store", &store)])
    }

    fn text(&self, name: &str, slots: &[(&str, &str)]) -> String {
        fill(self.texts.get(name).map_or("", String::as_str), slots)
    }

    /// The answer to a request, as `{"sql": "..."}`. It blocks while the query runs, within its time limit; where the
    /// store is not open it touches nothing.
    pub fn answer(&self, input: &str) -> String {
        let Some(open) = &self.open else {
            return self.text("closed", &[]);
        };
        let sql = match serde_json::from_str::<Value>(input) {
            Ok(Value::Object(request)) => match request.get("sql") {
                Some(Value::String(sql)) => sql.clone(),
                _ => return self.text("unreadable", &[("why", "it has no `sql`, a string")]),
            },
            Ok(_) => return self.text("unreadable", &[("why", "it is not an object")]),
            Err(why) => return self.text("unreadable", &[("why", &why.to_string())]),
        };
        match run(open, &sql) {
            Ok(result) => result.to_string(),
            Err(Failed::Timeout) => self.text("timeout", &[("ms", &open.limits.timeout.as_millis().to_string())]),
            Err(Failed::Sql(why)) => self.text("error", &[("why", &why)]),
        }
    }

    /// What a message gives of a result: the result, or where it is longer than the tool's file says a message gives,
    /// the first of its rows that fit with the size of the whole and the query that reads the rest.
    pub fn cut(result: &str, input: &str, given_bytes: usize) -> String {
        if result.len() <= given_bytes {
            return result.to_string();
        }
        let Ok(Value::Object(mut whole)) = serde_json::from_str::<Value>(result) else {
            return result.to_string();
        };
        let Some(Value::Array(rows)) = whole.remove("rows") else {
            return result.to_string();
        };
        let Some(sql) = (serde_json::from_str::<Value>(input).ok()).and_then(|r| r["sql"].as_str().map(str::to_string))
        else {
            return result.to_string();
        };
        let mut room = given_bytes;
        let first: Vec<Value> = (rows.into_iter())
            .take_while(|row| {
                let size = row.to_string().len() + 1;
                let fits = size <= room;
                room = room.saturating_sub(size);
                fits
            })
            .collect();
        let rest = rest(&sql, first.len());
        json!({ "rows": first, "cut": { "bytes": result.len(), "given": first.len(), "rest": rest } }).to_string()
    }
}

/// Why a query gave no rows.
enum Failed {
    Timeout,
    Sql(String),
}

/// The query that reads the rows of this one from this many on.
fn rest(sql: &str, from: usize) -> String {
    format!(
        "SELECT * FROM (\n{}\n) LIMIT -1 OFFSET {from}",
        sql.trim().trim_end_matches(';')
    )
}

/// The store's file opened for a query: read-only as SQLite opens a file, with every write refused again by the
/// connection's `query_only` and by an authorizer that allows only a select, the reading of the tables not hidden, and
/// the functions of such a select; nothing may be attached, and no value may be longer than the bytes a result is kept
/// with.
fn connect(open: &Open) -> rusqlite::Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut conn = Connection::open_with_flags(&open.store, flags)?;
    conn.busy_timeout(open.limits.timeout)?;
    conn.pragma_update(None, "query_only", true)?;
    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    conn.set_limit(
        Limit::SQLITE_LIMIT_LENGTH,
        i32::try_from(open.limits.max_bytes).unwrap_or(i32::MAX),
    )?;
    let hidden = open.hidden.clone();
    conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| match ctx.action {
        AuthAction::Select | AuthAction::Recursive | AuthAction::Function { .. } => Authorization::Allow,
        AuthAction::Read { table_name, .. } => {
            let schema_table = table_name.starts_with("sqlite_");
            if schema_table || hidden.iter().any(|h| h.eq_ignore_ascii_case(table_name)) {
                Authorization::Deny
            } else {
                Authorization::Allow
            }
        }
        _ => Authorization::Deny,
    }))?;
    let deadline = Instant::now() + open.limits.timeout;
    conn.progress_handler(1000, Some(move || Instant::now() > deadline))?;
    Ok(conn)
}

/// Runs the query and gives its rows within the bounds.
fn run(open: &Open, sql: &str) -> Result<Value, Failed> {
    let failed = |e: rusqlite::Error| match e {
        rusqlite::Error::SqliteFailure(f, _) if f.code == ErrorCode::OperationInterrupted => Failed::Timeout,
        other => Failed::Sql(other.to_string()),
    };
    let conn = connect(open).map_err(failed)?;
    let mut statement = conn.prepare(sql).map_err(failed)?;
    let names = columns(&statement);
    let mut found = statement.query([]).map_err(failed)?;
    let mut rows: Vec<Value> = Vec::new();
    let mut bytes = 0;
    let mut more = false;
    while let Some(row) = found.next().map_err(failed)? {
        let mut object = Map::new();
        for (at, name) in names.iter().enumerate() {
            let value = row.get_ref(at).map_err(failed)?;
            object.insert(name.clone(), json_of(value));
        }
        let object = Value::Object(object);
        let size = object.to_string().len() + 1;
        // The first row is kept however long: no value of it is longer than the bytes a result is kept with.
        if rows.len() == open.limits.max_rows || (!rows.is_empty() && bytes + size > open.limits.max_bytes) {
            more = true;
            break;
        }
        bytes += size;
        rows.push(object);
    }
    let mut result = json!({ "rows": rows });
    if more {
        result["cut"] = json!({ "kept": rows_len(&result), "rest": rest(sql, rows_len(&result)) });
    }
    Ok(result)
}

fn rows_len(result: &Value) -> usize {
    result["rows"].as_array().map_or(0, Vec::len)
}

/// The names of a statement's columns, a name that comes again followed by `:` and its place among those of the name.
fn columns(statement: &rusqlite::Statement<'_>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in statement.column_names() {
        let times = names.iter().filter(|n| n.split(':').next() == Some(name)).count();
        names.push(if times == 0 {
            name.to_string()
        } else {
            format!("{name}:{}", times + 1)
        });
    }
    names
}

fn json_of(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => {
            let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
            Value::String(format!("x'{hex}'"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &str = "-- The things.\nCREATE TABLE things (\n    id INTEGER PRIMARY KEY,\n    -- What it is called.\n    name TEXT NOT NULL, -- Plain text.\n    size INTEGER\n);\n\n-- The secrets of the engine. Hidden from agents.\nCREATE TABLE secrets (\n    id INTEGER PRIMARY KEY,\n    value TEXT NOT NULL,\n    UNIQUE (value)\n);\n";

    fn texts() -> BTreeMap<String, String> {
        [
            ("closed", "The store is not open to agents yet."),
            ("unreadable", "Not read: {why}."),
            ("error", "The query failed: {why}"),
            ("timeout", "Stopped after {ms} ms."),
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
        _dir: tempfile::TempDir,
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
                "INSERT INTO things (name, size) VALUES (?1, ?2)",
                (format!("thing {n}"), n),
            )
            .unwrap();
        }
        db.execute("INSERT INTO secrets (value) VALUES ('hush')", []).unwrap();
        let querying = Querying::new(&limits, Some(&tool("Words. {store}")), dir.path(), SCHEMA);
        Kept {
            _dir: dir,
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
        querying.answer(&json!({ "sql": sql }).to_string())
    }

    fn rows(answer: &str) -> Vec<Value> {
        let answer: Value = serde_json::from_str(answer).unwrap_or_else(|e| panic!("{e}: {answer}"));
        answer["rows"].as_array().unwrap_or_else(|| panic!("{answer}")).clone()
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
        let same = ask(
            &kept.querying,
            "SELECT a.id, b.id FROM things a, things b WHERE a.id = 1 AND b.id = 2",
        );
        assert_eq!(rows(&same), [json!({"id": 1, "id:2": 2})]);
    }

    #[test]
    fn a_write_of_every_kind_is_refused_by_the_connection_and_the_store_is_left_as_it_was() {
        let kept = kept(2, on());
        let before = std::fs::read(&kept.file).unwrap();
        let elsewhere = kept.file.with_file_name("an-agent-made-this.sqlite");
        let (attach, copy) = (
            format!("ATTACH DATABASE '{}' AS other", elsewhere.display()),
            format!("VACUUM INTO '{}'", elsewhere.display()),
        );
        let writes = [
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
            "ATTACH DATABASE ':memory:' AS other",
            attach.as_str(),
            "VACUUM",
            copy.as_str(),
            "REINDEX",
            "ANALYZE",
            "BEGIN",
            "SAVEPOINT a",
            "COMMIT",
            "DETACH DATABASE main",
        ];
        for sql in writes {
            let answer = ask(&kept.querying, sql);
            assert!(answer.starts_with("The query failed: "), "{sql}: {answer}");
        }
        assert_eq!(std::fs::read(&kept.file).unwrap(), before, "the file is as it was");
        assert!(!elsewhere.exists());
        assert_eq!(
            rows(&ask(&kept.querying, "SELECT count(*) AS n FROM things")),
            [json!({"n": 2})]
        );
    }

    #[test]
    fn the_connection_alone_refuses_a_write_with_no_authorizer() {
        // A connection opened as `connect` opens it, but for its authorizer: SQLite's own read-only flag and
        // `query_only` refuse the write.
        let kept = kept(1, on());
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
        let db = Connection::open_with_flags(&kept.file, flags).unwrap();
        db.pragma_update(None, "query_only", true).unwrap();
        for sql in [
            "INSERT INTO things (name) VALUES ('x')",
            "DELETE FROM things",
            "CREATE TABLE more (a)",
        ] {
            assert!(db.execute(sql, []).is_err(), "{sql}");
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
    fn a_table_hidden_from_agents_is_not_read_and_nor_is_the_schema() {
        let kept = kept(1, on());
        for sql in [
            "SELECT * FROM secrets",
            "SELECT name FROM sqlite_master",
            "SELECT * FROM things WHERE id IN (SELECT id FROM secrets)",
        ] {
            let answer = ask(&kept.querying, sql);
            assert!(answer.starts_with("The query failed: "), "{sql}: {answer}");
            assert!(!answer.contains("hush"), "{answer}");
        }
    }

    #[test]
    fn rows_past_the_row_limit_are_not_kept_and_the_query_for_the_rest_is_given() {
        let kept = kept(5, settings::Query { max_rows: 2, ..on() });
        let answer = ask(&kept.querying, "SELECT id FROM things ORDER BY id;");
        assert_eq!(rows(&answer), [json!({"id": 1}), json!({"id": 2})]);
        let cut: Value = serde_json::from_str(&answer).unwrap();
        assert_eq!(cut["cut"]["kept"], 2);
        let rest = cut["cut"]["rest"].as_str().unwrap();
        let rest = ask(&kept.querying, rest);
        assert_eq!(rows(&rest), [json!({"id": 3}), json!({"id": 4})]);
    }

    #[test]
    fn rows_past_the_byte_limit_are_not_kept() {
        let kept = kept(50, settings::Query { max_bytes: 200, ..on() });
        let answer = ask(&kept.querying, "SELECT name FROM things ORDER BY id");
        let kept_rows = rows(&answer).len();
        assert!((1..50).contains(&kept_rows), "{kept_rows}");
        let cut: Value = serde_json::from_str(&answer).unwrap();
        assert_eq!(cut["cut"]["kept"], kept_rows);
        // A single value longer than the limit is refused where it is made.
        let long = ask(&kept.querying, "SELECT hex(zeroblob(1000)) AS long");
        assert!(long.starts_with("The query failed: "), "{long}");
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
        assert_eq!(cut["cut"]["given"], first.len());
        let rest = rows(&ask(&kept.querying, cut["cut"]["rest"].as_str().unwrap()));
        assert_eq!(rest.len(), 40 - first.len());
        assert_eq!(rest[0]["id"], first.len() + 1);
        // One short enough is given whole, and one that is no rows as it is.
        assert_eq!(Querying::cut(&whole, &input, whole.len()), whole);
        let said = kept.querying.answer("not json");
        assert_eq!(Querying::cut(&said, &input, 3), said);
    }

    #[test]
    fn a_request_that_is_not_a_query_is_said_so() {
        let kept = kept(1, on());
        assert!(kept.querying.answer("{}").starts_with("Not read: it has no `sql`"));
        assert!(kept.querying.answer("[]").starts_with("Not read: it is not an object"));
        assert!(kept.querying.answer("{\"sql\": 3}").starts_with("Not read: "));
    }

    #[test]
    fn the_store_is_described_from_the_comments_of_the_schema_with_the_hidden_tables_left_out() {
        let described = describe(SCHEMA, &texts());
        assert_eq!(
            described,
            "### things: The things.\n- id INTEGER\n- name TEXT: What it is called. Plain text.\n- size INTEGER"
        );
        assert!(
            !described.contains("secrets") && !described.contains("hush"),
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
    fn with_the_switch_off_the_request_is_answered_in_the_files_words_and_the_store_is_not_touched() {
        let dir = tempfile::tempdir().unwrap();
        // No store file is there: an open store would fail to be opened, and an opened one would make the file.
        let off = Querying::new(
            &settings::Query::default(),
            Some(&tool("Words. {store}")),
            dir.path(),
            SCHEMA,
        );
        let said = "The store is not open to agents yet.";
        assert_eq!(off.answer(&json!({ "sql": "SELECT 1" }).to_string()), said);
        assert_eq!(off.answer("garbage"), said);
        assert_eq!(off.words(), format!("Words. {said}"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "nothing was made");
    }
}
