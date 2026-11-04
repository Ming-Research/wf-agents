//! Makes the list of migrations from the files of `db/migrations/`, in the order their names sort, so that a migration
//! is added as a file alone and no list of them is written by hand: two works that each add one change no line in
//! common. A migration is named by the minute it was written, in UTC, and the words of what it does
//! (`202611040140-record-migrations-taken.sql`), so that two works written side by side never take the same name;
//! those named by a number, `0002.sql` to `0079.sql`, came before, and no other number is taken. `0001.sql` is the
//! schema at version 1, which the tests migrate from, and is no migration.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The last migration named by a number: every one after it is named by its time.
const LAST_NUMBERED: u32 = 79;

/// What a migration's file is named, for the messages that refuse another name.
const NAMED: &str = "a migration is named by the minute it was written, in UTC, a hyphen and the words of what it does \
                     in lowercase letters and digits joined by hyphens, as 202611040140-record-migrations-taken.sql";

/// Whether `name`, a file's name without `.sql`, names a migration by its time: twelve digits, a hyphen, and words of
/// lowercase letters and digits joined by hyphens.
fn by_time(name: &str) -> bool {
    let Some((time, words)) = name.split_once('-') else {
        return false;
    };
    time.len() == 12
        && time.bytes().all(|b| b.is_ascii_digit())
        && words
            .split('-')
            .all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../db/migrations");
    println!("cargo::rerun-if-changed={}", dir.display());
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let file = entry.file_name().to_string_lossy().into_owned();
        match file.strip_suffix(".sql") {
            Some(name) => names.push(name.to_string()),
            None => panic!("db/migrations/{file}: the directory holds only migrations, each a file ending in .sql"),
        }
    }
    names.sort();
    let mut numbered = 1;
    for name in &names {
        if name == "0001" {
            continue;
        }
        if name.len() == 4 && name.bytes().all(|b| b.is_ascii_digit()) {
            numbered += 1;
            if name.parse::<u32>() != Ok(numbered) || numbered > LAST_NUMBERED {
                panic!(
                    "db/migrations/{name}.sql: {NAMED}; the migrations named by a number are 0002.sql to \
                     {LAST_NUMBERED:04}.sql, each of them, and no other"
                );
            }
        } else if !by_time(name) {
            panic!("db/migrations/{name}.sql: {NAMED}");
        }
    }
    if numbered != LAST_NUMBERED || names.first().map(String::as_str) != Some("0001") {
        panic!("db/migrations holds 0001.sql, the schema at version 1, and 0002.sql to {LAST_NUMBERED:04}.sql, each");
    }
    let mut code = String::from("&[\n");
    for name in names.iter().filter(|n| *n != "0001") {
        let path = dir.join(format!("{name}.sql"));
        let path = path.canonicalize().unwrap_or(path);
        let _ = writeln!(
            code,
            "    Migration {{ name: {name:?}, sql: include_str!({:?}) }},",
            path.display().to_string()
        );
    }
    code.push_str("]\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("migrations.rs");
    if let Err(e) = std::fs::write(&out, code) {
        panic!("{}: {e}", out.display());
    }
}
