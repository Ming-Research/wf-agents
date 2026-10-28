//! Giving a session only what it lacks: each conditional input of a message is given when its session does not hold
//! it and referred to by its address when it does; an input of the standard context the session holds an earlier
//! version of is given as what changed in it; and before a message is sent, what it would give whole that its session
//! already holds is caught.

use std::collections::HashMap;

use db::rusqlite::Connection;
use flow::formats::hash;
use serde_json::{Value, json};

use crate::diff;
use crate::held::{Held, Version, round_of, standard};
use crate::round::{Round, RoundId};

/// One conditional input of a message.
#[derive(Debug, Clone, PartialEq)]
pub struct Conditional {
    pub address: String,
    /// What the message writes for it: a text, or JSON; for one given as what changed, that change.
    pub content: Value,
    /// The hash of its content: of the whole version the session holds once given it, even when only what changed
    /// in it is written.
    pub hash: String,
    /// Given in this message; otherwise the session already holds it, and the message only refers to it.
    pub given: bool,
    /// Given as what changed in the version the session held, not whole.
    pub changes: bool,
    /// For an input of the standard context, the key `contents` keeps its whole version under, so that a later round
    /// can give only what changed in it.
    pub text: Option<String>,
}

/// The conditional inputs as a round records them: each by its address and hash, given or referred to, whether only
/// what changed was given, and for an input of the standard context the key its whole version is kept under.
pub fn record(list: &[Conditional]) -> Value {
    Value::Array(
        list.iter()
            .map(|m| {
                let mut r = json!({ "address": m.address, "hash": m.hash, "given": m.given });
                if m.changes {
                    r["changes"] = Value::Bool(true);
                }
                if let Some(t) = &m.text {
                    r["text"] = Value::String(t.clone());
                }
                r
            })
            .collect(),
    )
}

/// Whether a content is something one place copies from another: a list, an object, or a text of more than one word.
/// A word or a number two places give alike is not one thing, and keeps the address of where it came from.
pub fn copied(v: &Value) -> bool {
    match v {
        Value::Array(_) | Value::Object(_) => true,
        Value::String(s) => s.trim().contains(char::is_whitespace),
        _ => false,
    }
}

/// Whether a value has nothing to give.
pub fn empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(xs) => xs.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// How a conditional input's content is written in a message: a text as it is, anything else as compact JSON.
pub fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim_end().to_string(),
        other => other.to_string(),
    }
}

/// What a message wrote under an address, as each text it may be: the message does not mark where a text that holds
/// headings of its own ends, so each heading after it is a place it may end, and a text may have ended in a line break
/// the message left out.
fn written_under(message: &str, address: &str) -> Vec<String> {
    let heading = format!("### {address}\n\n");
    let Some(at) = message.find(&heading) else {
        return Vec::new();
    };
    let rest = &message[at + heading.len()..];
    rest.match_indices("\n\n#")
        .map(|(i, _)| i)
        .chain(std::iter::once(rest.len()))
        .flat_map(|i| [rest[..i].to_string(), format!("{}\n", &rest[..i])])
        .collect()
}

/// Whether an address names a row of the database that is its own thing whatever it says: an interjection, an ask of
/// the owner, a design card or the item. One that says what another said is new at its place, not a repeat.
fn own_thing(address: &str) -> bool {
    ["ij:", "ask:", "design_card:", "item:"]
        .iter()
        .any(|p| address.starts_with(p))
}

/// What a message would give whole that its session already holds, each said with the input's address, its size and
/// the round the session got it, or its earlier version, in: content the session got before under any address (a
/// word or a number two places give alike is not one thing, and a row of the database that is its own thing is new
/// at its place), and a whole new version of an input of the standard context the session holds an earlier version
/// of, of which only what changed may be given. A value new at its place, such as another round's return or what the
/// engine reports of the work as it is now, is new content, and a session started afresh or just compacted holds
/// nothing.
pub fn repeats(held: &Held, list: &[Conditional]) -> Vec<String> {
    list.iter()
        .filter(|c| c.given && !c.changes)
        .filter_map(|c| {
            let size = render(&c.content).chars().count();
            if copied(&c.content)
                && !own_thing(&c.address)
                && let Some((round, at)) = held.got.get(&c.hash)
            {
                let under = if *at == c.address {
                    String::new()
                } else {
                    format!(" as {at}")
                };
                return Some(format!(
                    "{} ({size} characters) would be given whole again, though the session got the same content in \
                     {round}{under}",
                    c.address
                ));
            }
            let v = held.standard.get(standard(&c.address)?)?;
            (v.hash != c.hash).then(|| {
                format!(
                    "{} ({size} characters) would be given whole, though the session holds its earlier version {} \
                     from {}, and only what changed in it may be given",
                    c.address, v.address, v.round
                )
            })
        })
        .collect()
}

/// The conditional inputs of a message as it is built: each content once, under its one address, given when the
/// session lacks it.
pub struct Giver<'a> {
    pub held: &'a Held,
    pub registry: &'a HashMap<String, Vec<String>>,
    /// The database, where the whole version of each input of the standard context is kept.
    pub conn: &'a Connection,
    /// Every round of the work so far, oldest first.
    pub rounds: &'a [Round],
    pub list: Vec<Conditional>,
    /// The whole version of each input of the standard context the message gives or refers to, by the key `contents`
    /// keeps it under.
    pub kept: Vec<(String, String)>,
}

impl<'a> Giver<'a> {
    pub fn new(
        held: &'a Held,
        registry: &'a HashMap<String, Vec<String>>,
        conn: &'a Connection,
        rounds: &'a [Round],
    ) -> Giver<'a> {
        Giver {
            held,
            registry,
            conn,
            rounds,
            list: Vec::new(),
            kept: Vec::new(),
        }
    }

    /// Adds a conditional input, and gives the address the message refers to it by: one already added with the same content
    /// keeps its address.
    pub fn add(&mut self, address: String, content: Value) -> String {
        let h = hash(&content);
        let one = copied(&content);
        if let Some(m) = self.list.iter().find(|m| m.hash == h && (one || m.address == address)) {
            return m.address.clone();
        }
        let given = !self.held.holds(&address, &h);
        let text = self.keep(&address, &content);
        self.list.push(Conditional {
            address: address.clone(),
            content,
            hash: h,
            given,
            changes: false,
            text,
        });
        address
    }

    /// Adds a conditional input: for an input of the standard context the session holds an earlier version of, only
    /// what changed in it; any other as [`Giver::add`] does.
    pub fn give(&mut self, address: String, content: Value) -> String {
        match self.changes(&address, &content) {
            Some(changes) => self.add_changes(address, &content, changes),
            None => self.add(address, content),
        }
    }

    /// Adds an input of the standard context the session holds an earlier version of by giving only what this version
    /// changes in it: the message writes `changes` under the address, and the session then holds `content` under it.
    fn add_changes(&mut self, address: String, content: &Value, changes: Value) -> String {
        let text = self.keep(&address, content);
        self.list.push(Conditional {
            address: address.clone(),
            content: changes,
            hash: hash(content),
            given: true,
            changes: true,
            text,
        });
        address
    }

    /// Keeps the whole version of an input of the standard context, so that a later round can give only what changes
    /// in it, and gives the key it is kept under; none for any other input. A text is kept as it is, anything else as
    /// its JSON.
    fn keep(&mut self, address: &str, content: &Value) -> Option<String> {
        standard(address)?;
        let text = match content {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let key = db::content_hash(&text);
        if !self.kept.iter().any(|(k, _)| *k == key) {
            self.kept.push((key.clone(), text));
        }
        Some(key)
    }

    /// The address of a round's value: of the rounds that produced the same content in the work, wherever it was
    /// copied to, the earliest.
    pub fn earliest(&self, address: String, content: &Value) -> String {
        if !copied(content) {
            return address;
        }
        let known = self.registry.get(&hash(content)).into_iter().flatten();
        let first = known
            .filter(|a| !a.contains("·engine·"))
            .filter_map(|a| Some((round_of(a)?, a)))
            .min_by_key(|x| x.0);
        match first {
            Some((r, a)) if round_of(&address).is_some_and(|mine| r < mine) => a.clone(),
            _ => address,
        }
    }

    /// The address of what the engine read or computed for a round under a name: the earliest round it was the same
    /// for.
    pub fn computed(&self, round: RoundId, name: &str, content: &Value) -> String {
        let suffix = format!("·engine·{name}");
        self.registry
            .get(&hash(content))
            .and_then(|known| known.iter().find(|a| a.ends_with(&suffix) && round_of(a).is_some()))
            .cloned()
            .unwrap_or_else(|| format!("{round}{suffix}"))
    }

    /// What changed in an input of the standard context since the latest version the session holds, when that is
    /// another: the lines of a text that differ, as a unified diff, or the entries of an object that differ. None when
    /// the session holds no version of it or holds this one as its latest, and when the version it holds cannot be
    /// read again as it was, for which the check before sending stops the message.
    fn changes(&self, address: &str, content: &Value) -> Option<Value> {
        let held = self.held.standard.get(standard(address)?)?;
        if held.hash == hash(content) {
            return None;
        }
        let earlier = self.version(held, content)?;
        let what = match (&earlier, content) {
            (Value::String(was), Value::String(is)) => format!(
                "What changed since {}, given in {}, as a unified diff of its lines:\n{}",
                held.address,
                held.round,
                diff::lines(was, is)
            ),
            (Value::Object(_), Value::Object(is)) => {
                let mut changed = diff::entries(&earlier, content);
                if let Value::Object(o) = &mut changed {
                    for k in crate::held::INDEXES {
                        if let Some(v) = is.get(k) {
                            o.insert(k.to_string(), v.clone());
                        }
                    }
                }
                format!(
                    "What changed since {}, given in {}: each entry here takes the place of the one there, and null \
                     removes it. {}",
                    held.address, held.round, changed
                )
            }
            _ => return None,
        };
        Some(Value::String(what))
    }

    /// A version of an input the session holds, read again as it was: from `contents`, or for a version recorded
    /// before rounds kept it there, from the message of the round that gave it whole; none when neither reads back to
    /// its hash.
    fn version(&self, v: &Version, like: &Value) -> Option<Value> {
        let texts: Vec<String> = match &v.text {
            Some(key) => self
                .conn
                .query_row("SELECT text FROM contents WHERE hash = ?1", [key], |r| r.get(0))
                .ok()
                .into_iter()
                .collect(),
            None => {
                let r = self.rounds.iter().find(|r| r.id == v.round)?;
                let message = db::put_back(self.conn, &r.input["message"]);
                written_under(message.as_str()?, &v.address)
            }
        };
        texts.into_iter().find_map(|t| {
            let value = match like {
                Value::String(_) => Value::String(t),
                _ => serde_json::from_str(&t).ok()?,
            };
            (hash(&value) == v.hash).then_some(value)
        })
    }

    /// What the message would give whole that its session already holds; see [`repeats`].
    pub fn repeats(&self) -> Vec<String> {
        repeats(self.held, &self.list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(n: i64, input: Value, output: Option<Value>, compactions: i64) -> Round {
        Round {
            id: RoundId::new(1, n),
            cell: Some("1".into()),
            session: Some(1),
            input,
            output,
            transition: Some("done".into()),
            rejected: None,
            compactions,
            started_at: 0,
            ended_at: Some(0),
            usage: None,
        }
    }

    #[test]
    fn the_same_content_is_one_hash_however_its_keys_were_written() {
        assert_eq!(hash(&json!({ "a": 1, "b": [2] })), hash(&json!({ "b": [2], "a": 1 })));
        assert_ne!(hash(&json!({ "a": 1 })), hash(&json!({ "a": 2 })));
    }

    // The same content copied from one place to another is one conditional input under the earliest round's address; a word
    // two places give alike keeps each place's address.
    #[test]
    fn copied_content_has_one_address_and_a_word_keeps_its_own() {
        let held = Held::default();
        let acceptance = json!([{ "id": "A1", "condition": "It works." }]);
        let registry = HashMap::from([
            (
                hash(&acceptance),
                vec![
                    "R290·12·reply.new_acceptance".to_string(),
                    "R4-25·12·reply.new_acceptance".to_string(),
                ],
            ),
            (hash(&json!("no")), vec!["R4-7·engine·adversarial".to_string()]),
        ]);
        let conn = Connection::open_in_memory().unwrap();
        let mut m = Giver::new(&held, &registry, &conn, &[]);
        // An address written before rounds were named by their work is never given again.
        assert_eq!(
            m.earliest("R4-30·2a·acceptance".into(), &acceptance),
            "R4-25·12·reply.new_acceptance"
        );
        assert_eq!(
            m.earliest("R4-9·3b·answer.verdict".into(), &json!("no")),
            "R4-9·3b·answer.verdict"
        );
        assert_eq!(
            m.add("R4-9·engine·rethought".into(), json!("no")),
            "R4-9·engine·rethought"
        );
        assert_eq!(
            m.add("R4-9·3b·answer.verdict".into(), json!("no")),
            "R4-9·3b·answer.verdict"
        );
        assert_eq!(
            m.add("R4-30·2a·acceptance".into(), acceptance.clone()),
            "R4-30·2a·acceptance"
        );
        assert_eq!(
            m.add("R4-31·12·reply.new_acceptance".into(), acceptance),
            "R4-30·2a·acceptance"
        );
        assert_eq!(m.list.len(), 3);
        assert_eq!(
            m.computed(RoundId::new(4, 9), "adversarial", &json!("no")),
            "R4-7·engine·adversarial"
        );
        assert_eq!(
            m.computed(RoundId::new(4, 9), "other", &json!("no")),
            "R4-9·engine·other"
        );
    }

    // What a message would give whole that its session holds is caught: the same content under another address, and
    // a whole new version of an input of the standard context, each with the round the session got it in. Only what
    // changed passes, and so do values new at their place, a word two places give alike, an interjection that says
    // what another said, and anything given to a session that holds nothing.
    #[test]
    fn a_repeat_is_caught_and_what_is_new_passes() {
        let guide = json!({ "agents_md": "How to work.", "design_tree": { "design/a.md": "A" }, "cited_nodes": {} });
        let newer = json!({ "agents_md": "How to work.", "design_tree": { "design/a.md": "A", "design/b.md": "B" } });
        let results = json!({ "results": [{ "tool": "query", "ran": { "rows": [1] } }] });
        let findings = json!([{ "id": "1", "verdict": "findings", "findings": [{ "fid": "R1" }] }]);
        let ci = json!({ "fast": { "head": "a", "result": "red" } });
        let said = json!("From the owner: go on");
        let record = |xs: &[(&str, &Value)]| {
            let xs: Vec<Value> = xs
                .iter()
                .map(|(a, v)| json!({ "address": a, "hash": hash(v), "given": true }))
                .collect();
            json!({ "conditional": xs })
        };
        let session = [
            round(
                2,
                record(&[
                    ("R1-2·engine·project_guide", &guide),
                    ("rule:ground", &json!("Facts first.")),
                    ("R1-2·engine·rethought", &json!("no")),
                    ("ij:1", &said),
                ]),
                Some(json!({})),
                0,
            ),
            round(
                4,
                record(&[
                    ("R1-3·13·results", &results),
                    ("R1-4·5a·columns[findings]", &findings),
                    ("R1-4·engine·work_ci", &ci),
                ]),
                Some(json!({})),
                0,
            ),
        ];
        let held = Held::of(&session);
        let one = |address: &str, content: Value| Conditional {
            address: address.into(),
            hash: hash(&content),
            content,
            given: true,
            changes: false,
            text: None,
        };
        let caught = repeats(
            &held,
            &[
                one("rule:elsewhere", json!("Facts first.")),
                one("R1-7·engine·project_guide", newer.clone()),
            ],
        );
        assert_eq!(caught.len(), 2, "{caught:?}");
        assert!(
            caught[0].starts_with("rule:elsewhere (12 characters) would be given whole again")
                && caught[0].ends_with("in R1-2 as rule:ground"),
            "{}",
            caught[0]
        );
        assert!(
            caught[1].starts_with("R1-7·engine·project_guide (")
                && caught[1].contains("its earlier version R1-2·engine·project_guide from R1-2"),
            "{}",
            caught[1]
        );
        let changed = Conditional {
            hash: hash(&newer),
            changes: true,
            ..one(
                "R1-7·engine·project_guide",
                json!("What changed since R1-2·engine·project_guide"),
            )
        };
        let new = [
            changed,
            one(
                "R1-9·13·results",
                json!({ "results": [{ "tool": "query", "ran": { "rows": [2] } }] }),
            ),
            one(
                "R1-9·5a·columns[findings]",
                json!([{ "id": "1", "verdict": "findings", "findings": [{ "fid": "R2" }] }]),
            ),
            one(
                "R1-9·engine·work_ci",
                json!({ "fast": { "head": "b", "result": "green" } }),
            ),
            one("R1-9·3b·answer.verdict", json!("no")),
            one("ij:2", said.clone()),
        ];
        assert_eq!(repeats(&held, &new), Vec::<String>::new());
        let mut compacted = session.to_vec();
        compacted.push(round(5, json!({}), Some(json!({})), 1));
        let again = [
            one("rule:ground", json!("Facts first.")),
            one("R1-7·engine·project_guide", newer),
        ];
        assert_eq!(repeats(&Held::default(), &again), Vec::<String>::new());
        assert_eq!(repeats(&Held::of(&compacted), &again), Vec::<String>::new());
    }

    // A rule the session holds an earlier version of is given as the lines that changed, read back from `contents`;
    // the session then holds the new version.
    #[test]
    fn a_changed_input_of_the_standard_context_is_given_as_what_changed() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE contents (hash TEXT PRIMARY KEY, text TEXT NOT NULL)")
            .unwrap();
        let was = "Facts first.\nThen the rest.\n";
        let key = db::content_hash(was);
        conn.execute("INSERT INTO contents (hash, text) VALUES (?1, ?2)", [key.as_str(), was])
            .unwrap();
        let input = json!({ "conditional": [{
            "address": "rule:ground", "hash": hash(&json!(was)), "given": true, "text": key,
        }] });
        let session = [round(2, input, Some(json!({})), 0)];
        let held = Held::of(&session);
        let registry = HashMap::new();
        let mut m = Giver::new(&held, &registry, &conn, &session);
        let is = "Facts first.\nThen the rest, briefly.\n";
        assert_eq!(m.give("rule:ground".into(), json!(is)), "rule:ground");
        let c = &m.list[0];
        assert!(c.given && c.changes, "{c:?}");
        assert_eq!(c.hash, hash(&json!(is)));
        assert!(
            c.content
                .as_str()
                .unwrap()
                .contains("-Then the rest.\n+Then the rest, briefly."),
            "{c:?}"
        );
        assert!(m.repeats().is_empty());
        // The same version again is only referred to.
        let mut m = Giver::new(&held, &registry, &conn, &session);
        m.give("rule:ground".into(), json!(was));
        assert!(!m.list[0].given);
    }

    // The architect's queue, given as what changed, carries the numbers of the matters that can be picked whole every
    // time, changed or not, beside what changed.
    #[test]
    fn a_queue_given_as_what_changed_carries_every_matter_that_can_be_picked() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE contents (hash TEXT PRIMARY KEY, text TEXT NOT NULL)")
            .unwrap();
        let was = json!({ "now": "10:00", "pickable": [1, 2, 6], "matters": { "#1": {}, "#2": {}, "#6": {} } });
        let key = db::content_hash(&was.to_string());
        conn.execute(
            "INSERT INTO contents (hash, text) VALUES (?1, ?2)",
            [key.as_str(), was.to_string().as_str()],
        )
        .unwrap();
        let input = json!({ "conditional": [{
            "address": "R2-1·engine·queue", "hash": hash(&was), "given": true, "text": key,
        }] });
        let session = [round(2, input, Some(json!({})), 0)];
        let held = Held::of(&session);
        let registry = HashMap::new();
        let mut m = Giver::new(&held, &registry, &conn, &session);
        let is = json!({ "now": "10:05", "pickable": [1, 2, 6], "matters": { "#1": {}, "#2": {}, "#6": {} } });
        m.give("R2-3·engine·queue".into(), is);
        let c = &m.list[0];
        assert!(c.given && c.changes, "{c:?}");
        let text = c.content.as_str().unwrap();
        assert!(
            text.contains(r#""now":"10:05""#) && text.contains(r#""pickable":[1,2,6]"#),
            "{text}"
        );
        assert!(!text.contains("matters"), "{text}");
    }
}
