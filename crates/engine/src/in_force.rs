//! A project's decisions in force: the files of its `decisions/` directory on main, one decision each, how the design
//! is done now and why, the options not taken and why not. Only what is in force is there: a decision overturned is
//! rewritten, one withdrawn deleted, and their earlier versions live in git alone, which the history gives a work that
//! might break one (the engine's input `decision_history`). The owner chooses each at the review of a design, and the
//! work that implements it writes it, so that it merges with the change. Each project keeps its own.

use std::path::Path;

use serde_json::{Value, json};

/// The directory of a project's repository that holds its decisions in force.
pub const DIR: &str = "decisions";

/// The files of the decisions in force at `commit` of the repository in `checkout`, by their paths from its root.
fn files(checkout: &Path, commit: &str) -> Result<Vec<String>, String> {
    let out = crate::git::git(checkout, &["ls-tree", "-r", "--name-only", commit, "--", DIR])?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// The engine's input `decisions_in_force`: each file of `decisions/` at `commit`, whole; none when there is none.
pub fn in_force(checkout: &Path, commit: &str) -> Result<Option<Value>, String> {
    let mut out = Vec::new();
    for file in files(checkout, commit)? {
        let text = crate::git::git(checkout, &["show", &format!("{commit}:{file}")])?;
        out.push(json!({ "file": file, "text": text }));
    }
    Ok((!out.is_empty()).then_some(Value::Array(out)))
}

/// The engine's input `decision_history`: each file of `decisions/` at `commit`, with the commits that wrote it, newest
/// first, each with its date and its whole message; none when there is none.
pub fn history(checkout: &Path, commit: &str) -> Result<Option<Value>, String> {
    let mut out = Vec::new();
    for file in files(checkout, commit)? {
        let log = crate::git::git(checkout, &["log", "--format=%H%x1f%as%x1f%B%x1e", commit, "--", &file])?;
        let versions: Vec<Value> = log
            .split('\u{1e}')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .filter_map(|x| {
                let mut parts = x.splitn(3, '\u{1f}');
                Some(json!({
                    "commit": parts.next()?,
                    "date": parts.next()?,
                    "message": parts.next().unwrap_or_default().trim(),
                }))
            })
            .collect();
        out.push(json!({ "file": file, "versions": versions }));
    }
    Ok((!out.is_empty()).then_some(Value::Array(out)))
}

/// Whether a path of the repository is one of the decisions.
pub fn holds(path: &str) -> bool {
    path.strip_prefix(DIR)
        .is_some_and(|rest| rest.starts_with('/') && rest.len() > 1)
}
