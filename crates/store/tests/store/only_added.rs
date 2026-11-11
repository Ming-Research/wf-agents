//! The tables are only inserted into: neither the crate's code nor its schema says anything that would change or
//! take out a row. A migration replaces the one row of the schema's version, which no work's entries are, and nothing
//! else.

use std::fs;

/// The words of SQL that change or take out rows.
const CHANGING: [&str; 4] = ["update", "delete", "replace", "upsert"];

/// The words of the text, lowercase: runs of letters, digits and underscores.
fn words(text: &str) -> Vec<String> {
    (text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')))
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

#[test]
fn neither_the_code_nor_the_schema_changes_or_takes_out_a_row() {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut files = vec![format!("{root}/../../db/schema.sql")];
    for dir in ["src", "../../db/migrations"] {
        for entry in fs::read_dir(format!("{root}/{dir}")).expect("the sources are listed") {
            files.push(entry.expect("a source").path().display().to_string());
        }
    }
    assert!(files.len() >= 4, "the sources were not found: {files:?}");
    for file in files {
        let text = fs::read_to_string(&file).expect("the source is read");
        // The one change a migration makes to a row: its replacing the version's.
        let text = text.replace("UPDATE schema_version SET version", "");
        let found: Vec<String> = (words(&text).into_iter())
            .filter(|w| CHANGING.contains(&w.as_str()))
            .collect();
        assert_eq!(found, Vec::<String>::new(), "{file} says {found:?}");
        // A conflict met by changing the row there is the same.
        assert!(
            !text.to_ascii_lowercase().contains("on conflict"),
            "{file} says on conflict"
        );
    }
}
