//! The system prompt a session starts with: what every session of its line is given alike, byte for byte, in every
//! work of the project, which the engine works out from the flow, the line and the project alone (the forms every
//! return of the line shares, the project's rules and those every cell of the line has, the settings every cell of the
//! line reads, and for the architect's lines where they work). The command line appends it to its own system prompt,
//! which comes after the tools and before the session's messages, and puts a breakpoint of the prompt cache at its end;
//! the cache is shared by the sessions of one account and model for an hour, so a new session of a line reads from it
//! what another session of the line wrote, where its first message, which begins with what differs from session to
//! session (the worktree, the branch, the date), could share nothing.
//!
//! A session keeps the system prompt it started with to its end: each of its rounds and each keep-warm ping runs its
//! command line with the same text, and the command line keeps it through a compaction. The parts are recorded on the
//! round that started the session, and the session holds them as it holds a conditional input it was given
//! ([`crate::held`]), so that its messages refer to them by their addresses, and a part that changed since is given as
//! what changed, as any input of the standard context is.

use db::rusqlite::Connection;
use flow::formats::hash;
use serde_json::{Value, json};

use crate::give::render;
use crate::held::Version;
use crate::round::{Round, RoundId};

/// The flag that appends a text to the command line's own system prompt.
pub const FLAG: &str = "--append-system-prompt";

/// What the system prompt says it holds, before its parts.
const HEAD: &str = "# Given to every round of your line\n\nWhat the engine gives every round of your line, the same \
    in every work of the project, each under its address. The messages refer to each by its address, and give what \
    changed in one, if anything, under the same address.\n";

/// The text of a system prompt of parts, each an address and its content, in their order: what it holds, then each
/// part under its address, as a message gives a conditional input.
pub fn text(parts: &[(String, Value)]) -> String {
    let mut out = HEAD.to_string();
    for (address, content) in parts {
        out.push_str(&format!("\n### {address}\n\n{}\n", render(content)));
    }
    out
}

/// A part's content as `contents` keeps its whole version, so that a later round can give only what changed in it: a
/// text as it is, anything else as its JSON.
fn kept(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The parts as the round that starts a session records them (`system`): each by its address, the hash of its content
/// and the key `contents` keeps its whole version under; with the texts to keep there.
pub fn record(parts: &[(String, Value)]) -> (Value, Vec<(String, String)>) {
    let mut keep: Vec<(String, String)> = Vec::new();
    let recorded = parts
        .iter()
        .map(|(address, content)| {
            let whole = kept(content);
            let key = db::content_hash(&whole);
            if !keep.iter().any(|(k, _)| *k == key) {
                keep.push((key.clone(), whole));
            }
            json!({ "address": address, "hash": hash(content), "text": key })
        })
        .collect();
    (Value::Array(recorded), keep)
}

/// The parts as a session that starts with them in a round holds them.
pub fn versions(parts: &[(String, Value)], round: RoundId) -> Vec<Version> {
    parts
        .iter()
        .map(|(address, content)| Version {
            address: address.clone(),
            hash: hash(content),
            round,
            text: Some(db::content_hash(&kept(content))),
        })
        .collect()
}

/// The system prompt a session's command line carries, as its latest round that ran the command line gave it; none for
/// a session that started without one, which goes on without one.
pub fn of_session(conn: &Connection, rounds: &[Round]) -> Option<String> {
    let command = rounds
        .iter()
        .rev()
        .find_map(|r| r.input["call"]["command"].as_array())?;
    let at = command.iter().position(|a| a == FLAG)?;
    db::put_back(conn, command.get(at + 1)?).as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The text gives each part under its address, in order, a text as it is and anything else as compact JSON; the
    // record names each by its address, the hash of its content and the key of its whole version, kept once.
    #[test]
    fn a_system_prompt_gives_each_part_under_its_address() {
        let parts = vec![
            ("format:common".to_string(), json!("The forms.")),
            ("rule:general".to_string(), json!("Facts first.")),
            ("config:design_goal".to_string(), json!({ "goal": "fast" })),
        ];
        let t = text(&parts);
        assert!(t.starts_with("# Given to every round of your line\n"), "{t}");
        assert!(
            t.ends_with(
                "\n### format:common\n\nThe forms.\n\n### rule:general\n\nFacts first.\n\n### config:design_goal\n\n\
                 {\"goal\":\"fast\"}\n"
            ),
            "{t}"
        );
        let (recorded, keep) = record(&parts);
        assert_eq!(recorded[0]["address"], "format:common");
        assert_eq!(recorded[0]["hash"], hash(&json!("The forms.")));
        assert_eq!(recorded[0]["text"], db::content_hash("The forms."));
        assert_eq!(keep.len(), 3);
        assert_eq!(keep[2].1, "{\"goal\":\"fast\"}");
        let v = versions(&parts, RoundId::new(1, 4));
        assert_eq!(v[1].address, "rule:general");
        assert_eq!(v[1].text.as_deref(), Some(db::content_hash("Facts first.").as_str()));
    }
}
