//! The versions of the flows works run on, each kept once as the engine starts with it: its files, each file once by
//! the digest of its text, and its outline (decision `a-version-is-kept-as-its-files-and-its-outline`). The store keeps
//! the outline as the text it is given, and reads nothing of it.

use std::collections::BTreeMap;

use rusqlite::{Connection, params};

use crate::Fault;

/// A version of a flow as the store keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptFlow {
    /// The digest of its files, which names it.
    pub version: String,
    /// The flow's name.
    pub name: String,
    /// Each of its files by its path from the flow's directory, with the digest of its text and its text.
    pub files: BTreeMap<String, (String, String)>,
    /// Its outline, as JSON.
    pub outline: String,
}

/// A version kept, as reading a work on it needs it: its name, its flow's name and its outline, as JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptOutline {
    pub version: String,
    pub name: String,
    pub outline: String,
}

/// Keeps the version where it is not kept already, and each of its files where no version holds it already.
pub(crate) fn keep(db: &Connection, kept: &KeptFlow) -> Result<(), Fault> {
    for (digest, text) in kept.files.values() {
        db.execute(
            "INSERT OR IGNORE INTO flow_files (digest, text) VALUES (?1, ?2)",
            params![digest, text],
        )?;
    }
    let files: BTreeMap<&String, &String> = (kept.files.iter()).map(|(path, (digest, _))| (path, digest)).collect();
    let files = serde_json::to_string(&files).map_err(|e| Fault::Value {
        table: "flows",
        message: e.to_string(),
    })?;
    db.execute(
        "INSERT OR IGNORE INTO flows (version, name, files, outline) VALUES (?1, ?2, ?3, ?4)",
        params![kept.version, kept.name, files, kept.outline],
    )?;
    Ok(())
}

/// Every version kept, in the order of their digests.
pub(crate) fn outlines(db: &Connection) -> Result<Vec<KeptOutline>, Fault> {
    let mut query = db.prepare("SELECT version, name, outline FROM flows ORDER BY version")?;
    let rows = query.query_map([], |row| {
        Ok(KeptOutline {
            version: row.get(0)?,
            name: row.get(1)?,
            outline: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
