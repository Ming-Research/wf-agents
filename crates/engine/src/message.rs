//! The message an agent's round is sent, and what the round's session already holds.
//!
//! A message is the round's inputs, of two kinds. The always inputs are given every time the cell is entered: what to
//! do this round (the cell's task), the addresses of the conditional inputs to work from by the number of each of the
//! cell's inputs, why the round happens and which format to hand in. A conditional input
//! is anything worked on: a rule, the item, a setting, another cell's output, a tool's results, an interjection, a
//! return format, what each numbered input is, the worktree. Each has an address, and is given only when the session
//! does not hold it: one the session already holds, by its content, is only referred to; one that is new or changed
//! is given; after a compaction the session is taken to hold nothing. What a session holds is worked out from the
//! record of what each of its rounds was given and handed back. After a return sent back the message only says why,
//! and that the return is to be rewritten.
//!
//! The standard context is what every agent needs once in a session: the return formats, the lists of inputs, the
//! rules, the project guide, the project's settings and the worktree. When one of them changed since the version a
//! session holds, the session is given only what changed: the lines of a text that differ, the entries of an object.
//! The project guide is read from main as the work's branch merged it, so the work's own edits never change it.
//!
//! Before a message is sent, it is checked for what it would give whole that its session already holds: content the
//! session got before, under any address, or a whole new version of an input of the standard context. Such a message
//! is not sent; it is the engine's fault, and the work is paused for the owner.
//!
//! Addresses: a value a round produced is `<round>·<cell>·<path>`, the round named `R<work>-<n>`, by the earliest
//! round that produced the same content; a value the engine reads or computes for a round is `<round>·engine·<name>`,
//! by the earliest round it was the same for; and `rule:<name>`, `config:<setting>`, `item:<id>`, `design_card:<id>`,
//! `ask:<id>` (an ask of the owner), `ij:<id>`, `work:<id>` (`work:architect` for where the architect works),
//! `format:common`, `format:<cell>` and `inputs:<cell>`. A
//! round recorded before rounds were named by their work wrote a round's address as `R<number>·...`; such an address
//! names no round, and the engine never gives it again.

use std::collections::{BTreeSet, HashMap, HashSet};

pub use flow::formats::{cell_format, cell_inputs, common_format, hash};
use flow::{AskNeed, CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};

use crate::inputs::{GUIDE, Gathered, Origin, Part};
use crate::work::{Item, Round, RoundId, Work};
use crate::{ARCHITECT, Engine};

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

/// An agent round's message: its text, word for word, and every conditional input it gives or refers to.
#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    /// The text as the round's record keeps it: the text itself, or its parts, each large one (a conditional input
    /// given, the cell's task) a reference to what `kept` holds.
    pub stored: Value,
    /// The large parts, each by the hash `contents` keeps it under.
    pub kept: Vec<(String, String)>,
    pub conditional: Vec<Conditional>,
    /// The round a short message goes on from, a return sent back or a round cut off midway: the session holds
    /// everything that round was given, and only the interjections it was given are this round's.
    pub continues: Option<RoundId>,
    /// What the message would give whole that its session already holds, each said with the input's address, its
    /// size and the round the session got it or its earlier version in; a message with any is not sent.
    pub repeats: Vec<String>,
}

impl Message {
    /// The conditional inputs as a round records them: each by its address and hash, given or referred to, whether
    /// only what changed was given, and for an input of the standard context the key its whole version is kept under.
    pub fn record(&self) -> Value {
        Value::Array(
            self.conditional
                .iter()
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
}

/// The round an address names, for an address of a round's value.
pub fn round_of(address: &str) -> Option<RoundId> {
    RoundId::parse(address.split('·').next()?)
}

/// One conditional input a round recorded.
struct Recorded {
    address: String,
    hash: String,
    /// The key its whole version is kept under, for an input of the standard context recorded since rounds kept it.
    text: Option<String>,
}

/// The conditional inputs a round recorded; none for a round recorded before they were. A round recorded before they
/// were called so keeps them under `materials`.
fn recorded(input: &Value) -> Vec<Recorded> {
    input
        .get("conditional")
        .or_else(|| input.get("materials"))
        .unwrap_or(&Value::Null)
        .as_array()
        .map(|xs| {
            xs.iter()
                .filter_map(|m| {
                    Some(Recorded {
                        address: m["address"].as_str()?.to_string(),
                        hash: m["hash"].as_str()?.to_string(),
                        text: m["text"].as_str().map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// How the address of a version of the project guide ends, and the input of the standard context it is.
const GUIDE_ADDRESS: &str = "·engine·project_guide";
const GUIDE_INPUT: &str = "engine·project_guide";

/// Which input of the standard context an address is a version of: the project guide, whose versions have addresses
/// of their own, or a return format, a list of inputs, a rule, a setting or the worktree, whose versions share one;
/// none for an address of anything else.
pub fn standard(address: &str) -> Option<&str> {
    if address.ends_with(GUIDE_ADDRESS) {
        return Some(GUIDE_INPUT);
    }
    ["format:", "inputs:", "rule:", "config:", "work:"]
        .iter()
        .any(|p| address.starts_with(p))
        .then_some(address)
}

/// A version of an input of the standard context a session holds: its address, the hash of its content, the round
/// the session got it in, and the key `contents` keeps it under, none for a version recorded before rounds kept it.
#[derive(Debug, Clone)]
pub struct Version {
    pub address: String,
    pub hash: String,
    pub round: RoundId,
    pub text: Option<String>,
}

/// What a session already holds: what it was given and what it handed back since it was last compacted.
#[derive(Debug, Default)]
pub struct Held {
    /// The conditional inputs it was given or referred to, each as its address and the hash of its content.
    pub conditional: HashSet<(String, String)>,
    /// The rounds whose returns it holds: every value they produced is in the session under its address.
    pub rounds: BTreeSet<RoundId>,
    /// The last round in which it was compacted: it holds nothing from before that round's end.
    pub compacted: Option<RoundId>,
    /// Where it first got each content it holds, by the content's hash: the round, and the address it got it under.
    pub got: HashMap<String, (RoundId, String)>,
    /// The latest version it holds of each input of the standard context, by which input it is.
    pub standard: HashMap<String, Version>,
}

impl Held {
    /// What the session whose rounds these are, oldest first, holds now. A round counts once it handed in a return:
    /// a run that failed may never have reached the session.
    pub fn of(session_rounds: &[Round]) -> Held {
        let mut h = Held::default();
        for r in session_rounds {
            if r.compactions > 0 {
                h = Held {
                    compacted: Some(r.id),
                    ..Held::default()
                };
                continue;
            }
            if r.output.is_none() {
                continue;
            }
            h.rounds.insert(r.id);
            for x in recorded(&r.input) {
                let round = h.got.entry(x.hash.clone()).or_insert((r.id, x.address.clone())).0;
                if let Some(input) = standard(&x.address) {
                    let text = x.text.clone().or_else(|| {
                        h.standard
                            .get(input)
                            .filter(|v| v.hash == x.hash)
                            .and_then(|v| v.text.clone())
                    });
                    let v = Version {
                        address: x.address.clone(),
                        hash: x.hash.clone(),
                        round,
                        text,
                    };
                    h.standard.insert(input.to_string(), v);
                }
                h.conditional.insert((x.address, x.hash));
            }
        }
        h
    }

    /// Whether the session holds a conditional input: a value one of its own returns produced, or the same content under the
    /// same address as one it was given. One content has one address, so the same content is held however it came.
    pub fn holds(&self, address: &str, hash: &str) -> bool {
        let own = round_of(address).is_some_and(|r| self.rounds.contains(&r)) && !address.contains("·engine·");
        own || self.conditional.contains(&(address.to_string(), hash.to_string()))
    }
}

/// Every address each content was given or referred to by in the work, in the order they first appear, by its hash.
pub fn registry(work_rounds: &[Round]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for r in work_rounds {
        for x in recorded(&r.input) {
            let seen = out.entry(x.hash).or_default();
            if !seen.contains(&x.address) {
                seen.push(x.address);
            }
        }
    }
    out
}

/// Whether a content is something one place copies from another: a list, an object, or a text of more than one word.
/// A word or a number two places give alike is not one thing, and keeps the address of where it came from.
fn copied(v: &Value) -> bool {
    match v {
        Value::Array(_) | Value::Object(_) => true,
        Value::String(s) => s.trim().contains(char::is_whitespace),
        _ => false,
    }
}

/// Whether a value has nothing to give.
fn empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(xs) => xs.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// Whether a value is the project's own context, which a message gives before anything else: the project's guide (its
/// AGENTS.md and design tree) and a setting of its configuration, such as its design goal.
fn project_own(o: &Origin) -> bool {
    matches!(o, Origin::Config(_)) || matches!(o, Origin::Engine(n) if n == GUIDE)
}

/// How a conditional input's content is written in a message: a text as it is, anything else as compact JSON.
fn render(v: &Value) -> String {
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
fn repeats(held: &Held, list: &[Conditional]) -> Vec<String> {
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

/// The kinds of question a project cannot meet, which its agents are not told of: those whose need it does not meet.
pub fn kinds_left_out<'g>(g: &'g CompiledGraph, project: Option<&config::Project>) -> Vec<&'g AskNeed> {
    g.ask_needs
        .iter()
        .filter(|k| !project.is_some_and(|p| p.has(&k.need).unwrap_or(false)))
        .collect()
}

/// A return format or a rule with the value of each setting it names written beside the setting's name, such as
/// the owner's language beside `owner.language`: the flow and the rules name a setting, never its value, which the
/// configuration holds. In a return format, which holds the schema as JSON, the value is written as JSON writes it
/// inside a string.
fn with_settings(e: &Engine, project: &str, text: String, in_json: bool) -> String {
    let mut out = text;
    // The fences around code are not names: without them, names are the text between pairs of backticks.
    for (name, value) in e.config.config.named_settings(project, &out.replace("```", "")) {
        let value = if in_json {
            let quoted = Value::String(value).to_string();
            quoted[1..quoted.len() - 1].to_string()
        } else {
            value
        };
        out = out.replace(&format!("`{name}`"), &format!("`{name}` ({value})"));
    }
    out
}

/// What a message is built from.
pub struct Context<'a> {
    pub e: &'a Engine,
    pub w: &'a Work,
    pub g: &'a CompiledGraph,
    pub cell: &'a CompiledCell,
    /// The item the work is on; none for a wake of the architect.
    pub item: Option<&'a Item>,
    /// This round.
    pub round: RoundId,
    /// Every round of the work so far, oldest first.
    pub rounds: &'a [Round],
    pub gathered: &'a [Gathered],
    /// The values the engine knows and rows recorded, which pick the lines the prompt adds.
    pub given: &'a Map<String, Value>,
    pub columns: &'a [(String, String)],
    /// The rows of the cell's numbered table, each with its number, as [`crate::inputs::table`] makes them.
    pub table: &'a [Value],
    pub held: &'a Held,
    pub registry: &'a HashMap<String, Vec<String>>,
}

/// The conditional inputs of a message as it is built: each content once, under its one address.
struct Conditionals<'a> {
    held: &'a Held,
    registry: &'a HashMap<String, Vec<String>>,
    list: Vec<Conditional>,
    /// The whole version of each input of the standard context the message gives or refers to, by the key `contents`
    /// keeps it under.
    kept: Vec<(String, String)>,
}

impl Conditionals<'_> {
    /// Adds a conditional input, and gives the address the message refers to it by: one already added with the same content
    /// keeps its address.
    fn add(&mut self, address: String, content: Value) -> String {
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
    fn earliest(&self, address: String, content: &Value) -> String {
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
}

impl Context<'_> {
    /// The address of what the engine read or computed for this round under a name: the earliest round it was the
    /// same for.
    fn computed(&self, m: &Conditionals<'_>, name: &str, content: &Value) -> String {
        let suffix = format!("·engine·{name}");
        m.registry
            .get(&hash(content))
            .and_then(|known| known.iter().find(|a| a.ends_with(&suffix) && round_of(a).is_some()))
            .cloned()
            .unwrap_or_else(|| format!("{}{suffix}", self.round))
    }

    /// Adds a conditional input: for an input of the standard context the session holds an earlier version of, only
    /// what changed in it; any other as [`Conditionals::add`] does.
    fn give(&self, m: &mut Conditionals<'_>, address: String, content: Value) -> String {
        match self.changes(&address, &content) {
            Some(changes) => m.add_changes(address, &content, changes),
            None => m.add(address, content),
        }
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
                crate::changes::lines(was, is)
            ),
            (Value::Object(_), Value::Object(_)) => format!(
                "What changed since {}, given in {}: each entry here takes the place of the one there, and null \
                 removes it. {}",
                held.address,
                held.round,
                crate::changes::entries(&earlier, content)
            ),
            _ => return None,
        };
        Some(Value::String(what))
    }

    /// A version of an input the session holds, read again as it was: from `contents`, or for a version recorded
    /// before rounds kept it there, from the message of the round that gave it whole; none when neither reads back to
    /// its hash.
    fn version(&self, v: &Version, like: &Value) -> Option<Value> {
        let conn = self.e.db.conn();
        let texts: Vec<String> = match &v.text {
            Some(key) => conn
                .query_row("SELECT text FROM contents WHERE hash = ?1", [key], |r| r.get(0))
                .ok()
                .into_iter()
                .collect(),
            None => {
                let r = self.rounds.iter().find(|r| r.id == v.round)?;
                let message = db::put_back(conn, &r.input["message"]);
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

    fn cell_of_round(&self, id: RoundId) -> Option<String> {
        self.rounds
            .iter()
            .find(|r| r.id == id)
            .and_then(|r| r.cell.clone())
            .or_else(|| {
                self.e
                    .db
                    .conn()
                    .query_row(
                        "SELECT cell FROM rounds WHERE work = ?1 AND n = ?2",
                        [id.work, id.n],
                        |r| r.get(0),
                    )
                    .ok()
            })
    }

    /// The conditional inputs one source of an input gave, each with its address.
    fn conditional_of(&self, m: &Conditionals<'_>, p: &Part) -> Vec<(String, Value)> {
        match &p.origin {
            Origin::Place { first, cell, path, .. } => {
                vec![(m.earliest(format!("{first}·{cell}·{path}"), &p.value), p.value.clone())]
            }
            Origin::Config(n) => vec![(format!("config:{n}"), p.value.clone())],
            Origin::Recorded(n) | Origin::Engine(n) => vec![(self.computed(m, n, &p.value), p.value.clone())],
            Origin::Db(rel) => {
                let rows = p.value.as_array().cloned().unwrap_or_default();
                match rel.as_str() {
                    "item_body" => rows.into_iter().map(|r| (format!("item:{}", r["item"]), r)).collect(),
                    "item_design_cards" | "parent_design_cards" | "work_design_card" => rows
                        .into_iter()
                        .map(|r| (format!("design_card:{}", r["design_card"]), r))
                        .collect(),
                    "item_asks" => rows.into_iter().map(|r| (format!("ask:{}", r["ask"]), r)).collect(),
                    "waiting_interjections" => rows
                        .into_iter()
                        .map(|r| {
                            let text = format!(
                                "From the {}: {}",
                                r["speaker"].as_str().unwrap_or("owner"),
                                r["text"].as_str().unwrap_or_default()
                            );
                            (format!("ij:{}", r["interjection"]), Value::String(text))
                        })
                        .collect(),
                    // The locked acceptance is what the round that locked it handed in.
                    "item_acceptance" => rows
                        .into_iter()
                        .filter_map(|r| {
                            let a = r.get("acceptance").filter(|a| !empty(a))?.clone();
                            let round = RoundId::new(r["acceptance_work"].as_i64()?, r["acceptance_round"].as_i64()?);
                            let path = match self.cell_of_round(round).as_deref() {
                                Some("12") => "12·reply.new_acceptance".to_string(),
                                Some(c) => format!("{c}·acceptance"),
                                None => "acceptance".to_string(),
                            };
                            Some((m.earliest(format!("{round}·{path}"), &a), a))
                        })
                        .collect(),
                    // The problem as the engineer understood it, once research handed it in.
                    "item_problem" => rows
                        .into_iter()
                        .map(|r| {
                            let id = r["item"].as_i64().unwrap_or_default();
                            let researched: Option<RoundId> = self
                                .e
                                .db
                                .conn()
                                .query_row(
                                    "SELECT r.work, r.n FROM rounds r JOIN works w ON w.id = r.work
                                     WHERE w.item = ?1 AND r.cell = '1' AND r.transition IS NOT NULL
                                       AND json_extract(r.output, '$.outcome.understanding') IS NOT NULL
                                     ORDER BY r.work DESC, r.n DESC LIMIT 1",
                                    [id],
                                    |x| Ok(RoundId::new(x.get(0)?, x.get(1)?)),
                                )
                                .ok();
                            let address = match researched {
                                Some(k) => format!("{k}·1·understanding.problem"),
                                None => format!("item:{id}·problem"),
                            };
                            (address, r["problem"].clone())
                        })
                        .collect(),
                    _ => vec![(self.computed(m, rel, &p.value), p.value.clone())],
                }
            }
        }
    }

    /// Why the cell's last return was sent back, when this round only rewrites it: the session holds everything else.
    fn sent_back(&self) -> Option<(RoundId, String)> {
        let r = self.rounds.last()?;
        if r.cell.as_deref() != Some(self.cell.id.as_str()) || r.transition.is_some() {
            return None;
        }
        let rej = r.rejected.as_ref()?;
        if ["interrupted", "failed", "error"].iter().any(|k| rej.get(k).is_some()) {
            return None;
        }
        let reason = rej
            .as_object()
            .and_then(|o| o.values().next())
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
            .unwrap_or_default();
        Some((r.id, reason))
    }

    /// The cell's last round, when the engine stopped in the middle of it and the session it ran in goes on: a
    /// session that handed in a return before is resumed, and holds what that round was given.
    fn cut_off(&self) -> Option<RoundId> {
        let r = self.rounds.last()?;
        let here = r.cell.as_deref() == Some(self.cell.id.as_str()) && r.transition.is_none();
        let cut = r.rejected.as_ref().is_some_and(|x| x.get("interrupted").is_some());
        (here && cut && !self.held.rounds.is_empty()).then_some(r.id)
    }

    /// Why this round happens, and the conditional inputs it says it with.
    fn why(&self, m: &mut Conditionals<'_>, interjections: &[String]) -> Vec<String> {
        let c = self.cell;
        let mut out = Vec::new();
        let here = |r: &&Round| r.cell.as_deref() == Some(c.id.as_str());
        let earlier = self.rounds.iter().rev().find(here);
        match self.rounds.last() {
            Some(r) if here(&r) && r.transition.is_none() => {
                let rej = r.rejected.clone().unwrap_or(Value::Null);
                if rej.get("interrupted").is_some() {
                    out.push(format!(
                        "{} was cut off when the engine stopped: do its work again.",
                        r.id
                    ));
                } else if let Some(f) = rej.get("failed").or(rej.get("error")) {
                    let what = f.get("what").and_then(Value::as_str).or(f.as_str()).unwrap_or_default();
                    out.push(format!("{} ended without a return ({what}): do its work again.", r.id));
                }
            }
            Some(r) => {
                let row = r.transition.as_deref().unwrap_or_default();
                let from = r.cell.as_deref().unwrap_or_default();
                let paused = here(&r) && c.rows.iter().any(|x| x.name == row && x.to == "@self");
                if paused {
                    out.push(format!("You paused in {}: carry on.", r.id));
                } else {
                    let title = self.g.cell(from).map(|x| x.title.as_str()).unwrap_or_default();
                    out.push(format!("Came from cell {from} ({title}) by its row `{row}`, {}.", r.id));
                    // Back from asking: the answer it brought.
                    if self.g.asking.iter().any(|a| a == from) {
                        let since = earlier.map(|e| e.id);
                        let after = |x: &&Round| since.is_none_or(|s| x.id > s);
                        let answer = self.rounds.iter().rev().take_while(after).find_map(|x| {
                            let cell = x.cell.as_deref()?;
                            let kind = self.g.cell(cell)?.kind.as_str();
                            let out = x.output.as_ref().filter(|_| x.transition.is_some())?;
                            let out = if kind == "agent" {
                                self.g.unwrap(out)
                            } else {
                                out.clone()
                            };
                            let reply = out.get("reply").filter(|v| !empty(v))?.clone();
                            Some((x.id, cell.to_string(), reply))
                        });
                        if let Some((id, cell, reply)) = answer {
                            let a = m.add(m.earliest(format!("{id}·{cell}·reply"), &reply), reply);
                            out.push(format!("The answer to what you asked: {a}."));
                        }
                    }
                }
            }
            None => {}
        }
        if let Some(k) = self.held.compacted
            && self.held.rounds.is_empty()
        {
            out.push(format!(
                "Your session was compacted in {k}: what this round needs is given again."
            ));
        }
        if !interjections.is_empty() {
            out.push(format!(
                "Said to you: {}. Answer in this round.",
                interjections.join(", ")
            ));
        }
        out
    }

    /// The message. After a return sent back it only says why, and that the return is to be rewritten.
    pub fn build(&self) -> Message {
        let (e, w, g, c, item) = (self.e, self.w, self.g, self.cell, self.item);
        let short = |text: String, prev: RoundId| Message {
            stored: Value::String(text.clone()),
            text,
            kept: Vec::new(),
            conditional: Vec::new(),
            continues: Some(prev),
            repeats: Vec::new(),
        };
        if let Some((prev, reason)) = self.sent_back() {
            let text = format!(
                "# {} · cell {}\n\nYour return was sent back: {reason}\nRewrite it (format:{}).\n",
                self.round, c.id, c.id
            );
            return short(text, prev);
        }
        if let Some(prev) = self.cut_off() {
            let text = format!(
                "# {} · cell {}\n\n{prev} was cut off midway. First look at the worktree as it is now (files \
                 changed, commits made), then go on with its work, and hand it in as format:{} says.\n",
                self.round, c.id, c.id
            );
            return short(text, prev);
        }
        let mut m = Conditionals {
            held: self.held,
            registry: self.registry,
            list: Vec::new(),
            kept: Vec::new(),
        };
        // What every round needs from the start: the return formats (the query tool's description in them lists the
        // database's tables and views), what each numbered input of the cell is, the rules and the worktree; the value
        // of each setting the formats and the rules name is written beside its name, and the kinds of question the
        // project cannot meet are left out of the formats. A wake of the architect is on no project.
        let named = item.map(|i| i.project.as_str()).unwrap_or_default();
        let project = item.and_then(|i| e.config.config.projects.get(&i.project));
        let gone = kinds_left_out(g, project);
        self.give(
            &mut m,
            "format:common".into(),
            Value::String(with_settings(e, named, common_format(g, &gone), true)),
        );
        self.give(
            &mut m,
            format!("format:{}", c.id),
            Value::String(with_settings(e, named, cell_format(g, c, &gone), true)),
        );
        let numbered = cell_inputs(c);
        if !numbered.is_empty() {
            self.give(&mut m, format!("inputs:{}", c.id), Value::String(numbered));
        }
        // A project's own rules; the architect, over every project, is given each project's.
        let mut rules = Vec::new();
        let mut own: Vec<String> = Vec::new();
        match project {
            Some(p) => own.extend(p.rules.iter().cloned()),
            None => {
                for p in e.config.config.projects.values() {
                    for r in &p.rules {
                        if !own.contains(r) {
                            own.push(r.clone());
                        }
                    }
                }
            }
        }
        for name in own {
            let Some(text) = e.config.config.rules.get(&name) else {
                continue;
            };
            let text = with_settings(e, named, text.trim().to_string(), false);
            rules.push(self.give(&mut m, format!("rule:{name}"), Value::String(text)));
        }
        for r in &c.rules {
            if let Some(rule) = g.rules.iter().find(|x| x.name == r.name) {
                let lines: Vec<String> = rule.text.iter().map(|t| format!("- {t}")).collect();
                let text = with_settings(e, named, format!("{}\n\n{}", rule.desc, lines.join("\n")), false);
                rules.push(self.give(&mut m, format!("rule:{}", rule.name), Value::String(text)));
            }
        }
        // A line the configuration keeps from changing anything only reads the worktree. The architect works in a
        // directory of its own, with every project's checkout to read.
        let line = crate::line_of(g, c);
        let read_only = e.config.config.install.agents.permissions.read_only.contains(&line);
        let workspace = if g.name == ARCHITECT {
            Some(self.give(&mut m, "work:architect".into(), Value::String(architect_workspace(e))))
        } else {
            workspace(w, project, read_only)
                .map(|text| self.give(&mut m, format!("work:{}", w.id), Value::String(text)))
        };
        // The inputs, each by its number in `inputs:<cell>` and the addresses of what it holds; what the agent reads
        // itself is said in full.
        let mut work_from = Vec::new();
        let mut interjections = Vec::new();
        let mut first: Vec<String> = Vec::new();
        let mut number = 0;
        for x in self.gathered {
            if x.itself() {
                work_from.push(format!("Read yourself: {}", x.input.what));
                continue;
            }
            number += 1;
            // A numbered table is one input the engine computes from its sources, each row with its number.
            if x.input.numbered.is_some() {
                if !self.table.is_empty() {
                    let content = Value::Array(self.table.to_vec());
                    let address = self.computed(&m, "table", &content);
                    work_from.push(format!("{number}. {}", m.add(address, content)));
                }
                continue;
            }
            let mut addrs: Vec<String> = Vec::new();
            for p in &x.parts {
                for (address, content) in self.conditional_of(&m, p) {
                    if empty(&content) {
                        continue;
                    }
                    let a = self.give(&mut m, address, content);
                    if a.starts_with("ij:") && !interjections.contains(&a) {
                        interjections.push(a.clone());
                    }
                    if project_own(&p.origin) && !first.contains(&a) {
                        first.push(a.clone());
                    }
                    if !addrs.contains(&a) {
                        addrs.push(a);
                    }
                }
            }
            if !addrs.is_empty() {
                work_from.push(format!("{number}. {}", addrs.join(", ")));
            }
        }
        if !rules.is_empty() {
            work_from.push(format!("Rules: {}", rules.join(", ")));
        }
        if let Some(a) = workspace {
            work_from.push(format!("Worktree: {a}"));
        }
        let why = self.why(&mut m, &interjections);

        let mut out = Writer::default();
        out.line(&format!("# {} · cell {} · {}\n", self.round, c.id, c.title));
        if !why.is_empty() {
            out.line(&why.join("\n"));
            out.line("");
        }
        // The project's own context comes first: its guide and its settings, then the rest in the order added.
        let mut given: Vec<&Conditional> = m.list.iter().filter(|x| x.given).collect();
        given.sort_by_key(|x| first.iter().position(|a| *a == x.address).unwrap_or(first.len()));
        if !given.is_empty() {
            out.line("## Given\n");
            for x in given {
                out.line(&format!("### {}\n", x.address));
                out.keep(&render(&x.content));
                out.line("");
            }
        }
        let mut task: Vec<&str> = c.prompt.iter().map(String::as_str).collect();
        if let Some(pb) = &c.prompt_by
            && let Some(v) = self.given.get(&pb.by).and_then(Value::as_str)
            && let Some(lines) = pb.cases.get(v)
        {
            task.extend(lines.iter().map(String::as_str));
        }
        // A cell with no task of its own has no heading for one.
        if !task.is_empty() {
            out.line("## Task\n");
            out.keep(&task.join("\n"));
            out.line("");
        }
        if !work_from.is_empty() {
            out.line("## Work from\n");
            for l in &work_from {
                out.line(l);
            }
            out.line("");
        }
        if !self.columns.is_empty() {
            out.line("## Review table: answer each column by its number\n");
            for (id, col) in self.columns {
                out.line(&format!("{id}. {col}"));
            }
            out.line("");
        }
        out.line(&format!("Hand in one JSON object, as format:{} says.", c.id));
        let (text, stored, mut kept) = out.finish();
        for (key, whole) in m.kept {
            if !kept.iter().any(|(k, _)| *k == key) {
                kept.push((key, whole));
            }
        }
        Message {
            text,
            stored,
            kept,
            repeats: repeats(self.held, &m.list),
            conditional: m.list,
            continues: None,
        }
    }
}

/// A message as it is written, line by line, with the large parts the round's record keeps once.
#[derive(Default)]
struct Writer {
    text: String,
    /// The parts the record keeps, and the text written since the last large one.
    parts: Vec<Value>,
    since: String,
    kept: Vec<(String, String)>,
}

impl Writer {
    fn line(&mut self, s: &str) {
        self.text.push_str(s);
        self.text.push('\n');
        self.since.push_str(s);
        self.since.push('\n');
    }

    /// A line that may be large: the record refers to a large one by its hash, and keeps it once.
    fn keep(&mut self, s: &str) {
        if s.len() < db::STORED {
            return self.line(s);
        }
        self.text.push_str(s);
        self.text.push('\n');
        if !self.since.is_empty() {
            self.parts.push(Value::String(std::mem::take(&mut self.since)));
        }
        let h = db::content_hash(s);
        self.parts.push(json!({ "content": h }));
        self.since.push('\n');
        if !self.kept.iter().any(|(k, _)| *k == h) {
            self.kept.push((h, s.to_string()));
        }
    }

    /// The text, the text as the record keeps it, and what the record keeps once.
    fn finish(mut self) -> (String, Value, Vec<(String, String)>) {
        if self.parts.is_empty() {
            let text = self.text;
            return (text.clone(), Value::String(text), Vec::new());
        }
        if !self.since.is_empty() {
            self.parts.push(Value::String(std::mem::take(&mut self.since)));
        }
        (self.text, json!({ "parts": self.parts }), self.kept)
    }
}

/// Where the architect works, what `work:architect` gives: a directory of its own, in which it only reads, and every
/// project's checkout, which it may read, each with the branch the flow calls main there.
fn architect_workspace(e: &Engine) -> String {
    let dir = crate::architect::workdir(&e.data);
    let mut out = format!(
        "Your working directory is {}, a directory of your own: you only read, and change and commit nothing, there or \
         anywhere. You may read each project's checkout:",
        dir.display()
    );
    for (name, p) in &e.config.config.projects {
        out.push_str(&format!(
            "\n- {name}: {}, whose main is the branch {}",
            p.checkout.display(),
            p.main
        ));
    }
    out
}

/// Where the work's agents work: its worktree and branch, and the project's branch the flow calls main; what
/// `work:<id>` gives. An agent on a read-only line is told that it only reads the worktree. None for a work of no
/// project the configuration knows.
fn workspace(w: &Work, project: Option<&config::Project>, read_only: bool) -> Option<String> {
    let p = project?;
    let main = format!(
        "Main, wherever the flow says it, is the branch {}: the work branches from it, its PR merges into it, and a \
         diff against main is a diff against it.",
        p.main
    );
    Some(match (&w.worktree, &w.branch) {
        (Some(wt), Some(branch)) if read_only => format!(
            "Your working directory is the worktree {}, on the branch {branch}: you only read it, and change and \
             commit nothing there. {main}",
            wt.display()
        ),
        (Some(wt), Some(branch)) => format!(
            "Your working directory is the worktree {}, on the branch {branch}: commit there, and the engine pushes \
             when you ask it to with a tool. {main}",
            wt.display()
        ),
        _ => main,
    })
}

/// The columns the table review answers, each with its number, for the review of this round. A re-review that looks
/// at disputes only has none; one that looks at what changed has them all, since which columns a change concerns is
/// not worked out in this version. A work on no item reviews nothing.
pub fn columns(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
) -> Vec<(String, String)> {
    let Some(item) = item else { return Vec::new() };
    let rule = |name: &str| {
        g.rules
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    let mut cols: Vec<String> = Vec::new();
    match cell.id.as_str() {
        "5a" => {
            let review = crate::inputs::last_return(e, w, g, "18", "plan")
                .ok()
                .flatten()
                .and_then(|r| crate::inputs::at_path(&r, "plan.review"));
            if review.as_ref().and_then(Value::as_str) == Some("disputes") {
                return Vec::new();
            }
            let table = rule("review_table");
            let project = e.config.config.projects.get(&item.project);
            if item.deliver == "report" {
                cols.extend(table.get(16).cloned());
            } else {
                cols.extend(table.iter().skip(2).take(7).cloned());
                for entry in project.map(|p| p.review_list.clone()).unwrap_or_default() {
                    cols.push(format!("From the review list in the project's configuration: {entry}"));
                }
            }
            let acceptance = item.acceptance.clone().unwrap_or(Value::Null);
            for a in acceptance.as_array().cloned().unwrap_or_default() {
                cols.push(format!(
                    "Acceptance condition {}: {} Can it be checked; is the evidence there, does it come from the head to be merged, and does it hold?",
                    a["id"].as_str().unwrap_or("?"),
                    a["condition"].as_str().unwrap_or_default()
                ));
            }
            if item.deliver != "report" {
                cols.push("The research's judgment that the work stands: does its reason hold?".into());
                cols.push("The judgment that there is no better design: does its reason hold?".into());
                for (c, what) in [
                    ("3a", "a gap in what is under test"),
                    ("3b", "unsure facts"),
                    ("3c", "architectural burden"),
                    ("3d", "cross-project dependency"),
                    ("3e", "splitting"),
                ] {
                    // A check the project lacks what it needs for was answered by the engine, not judged: no column.
                    let not_asked = g
                        .cell(c)
                        .and_then(|x| x.needs.as_deref())
                        .is_some_and(|need| project.is_some_and(|p| !p.has(need).unwrap_or(false)));
                    if g.cell(c).is_some() && !not_asked {
                        cols.push(format!(
                            "The check of {what} answered \"no\" or not applicable: does its reason hold?"
                        ));
                    }
                }
                let changes = crate::inputs::last_return(e, w, g, "4", "deviations").ok().flatten();
                for d in changes
                    .and_then(|r| r["deviations"].as_array().cloned())
                    .unwrap_or_default()
                {
                    if d["kind"] != "surprise" {
                        cols.push(format!(
                            "The {} \"{}\": does its reason hold, and did the design change with it?",
                            d["kind"].as_str().unwrap_or_default().replace('_', " "),
                            d["what"].as_str().unwrap_or_default()
                        ));
                    }
                }
                // A return of a flow version before the answer was flat holds the test changes under end.
                let done = crate::inputs::last_return(e, w, g, "4", "test_changes").ok().flatten();
                for t in done
                    .and_then(|r| {
                        r.get("test_changes")
                            .or_else(|| r["end"].get("test_changes"))
                            .and_then(Value::as_array)
                            .cloned()
                    })
                    .unwrap_or_default()
                {
                    let name = t["test"].as_str().unwrap_or_default();
                    match t["change"].as_str() {
                        Some("added") => cols.push(format!(
                            "The new test {name}: take the other side on its running time. Do other tests already catch what it catches, could it be smaller or faster, could it be folded into another test?"
                        )),
                        Some(c) => cols.push(format!(
                            "The test {name}, {c}: compared with before, is there a technical reason?"
                        )),
                        None => {}
                    }
                }
            }
        }
        "5b" => {
            let review = rule("design_tree_review");
            for i in [1, 2, 3, 4, 10, 11] {
                cols.extend(review.get(i).cloned());
            }
        }
        _ => {}
    }
    cols.into_iter()
        .enumerate()
        .map(|(i, c)| ((i + 1).to_string(), c))
        .collect()
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

    fn graph() -> CompiledGraph {
        let relations = db::relations(db::SCHEMA).unwrap();
        let f = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations,
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        f.graph(crate::ENGINEER).unwrap().clone()
    }

    #[test]
    fn the_same_content_is_one_hash_however_its_keys_were_written() {
        assert_eq!(hash(&json!({ "a": 1, "b": [2] })), hash(&json!({ "b": [2], "a": 1 })));
        assert_ne!(hash(&json!({ "a": 1 })), hash(&json!({ "a": 2 })));
        assert_eq!(round_of("R5-28·4·end.test_changes[added]"), Some(RoundId::new(5, 28)));
        assert_eq!(RoundId::new(5, 28).to_string(), "R5-28");
        assert_eq!(round_of("rule:ground"), None);
        // An address written before rounds were named by their work names no round.
        assert_eq!(round_of("R28·4·end.test_changes[added]"), None);
    }

    // A session holds what it was given, referred to and handed back since its last compaction, and nothing from a
    // run that handed nothing in.
    #[test]
    fn a_session_holds_what_it_was_given_and_returned_since_its_last_compaction() {
        let given = |a: &str, v: &Value| json!({ "conditional": [{ "address": a, "hash": hash(v), "given": true }] });
        let rule = json!("Facts first.");
        let rounds = [
            round(
                2,
                given("rule:ground", &rule),
                Some(json!({ "outcome": {}, "progress": "p" })),
                0,
            ),
            round(3, json!({}), Some(json!({ "outcome": { "design": "d" } })), 0),
            round(4, given("ij:1", &json!("From the owner: x")), None, 0),
        ];
        let h = Held::of(&rounds);
        assert!(h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("rule:ground", &hash(&json!("Facts first, then the rest."))));
        assert!(h.holds("R1-3·2a·design", "any"));
        assert!(!h.holds("R2-3·2a·design", "any"));
        assert!(!h.holds("R1-3·engine·project_guide", "any"));
        assert!(!h.holds("ij:1", &hash(&json!("From the owner: x"))));
        let mut later = rounds.to_vec();
        later.push(round(5, json!({}), Some(json!({ "outcome": {} })), 1));
        let h = Held::of(&later);
        assert!(!h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("R1-3·2a·design", "any"));
        assert_eq!(h.compacted, Some(RoundId::new(1, 5)));
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
        let mut m = Conditionals {
            held: &held,
            registry: &registry,
            list: Vec::new(),
            kept: Vec::new(),
        };
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

    // The forms every return shares hold every tool and every role any cell has; a cell's own format names the ones
    // it has.
    #[test]
    fn the_common_format_holds_every_alternative_and_a_cell_says_which_it_has() {
        let g = graph();
        let common = common_format(&g, &[]);
        for t in [
            "\"ci\"",
            "\"ci_before\"",
            "\"gh\"",
            "\"query\"",
            "\"advisor\"",
            "\"architect\"",
            "\"owner\"",
        ] {
            assert!(common.contains(t), "{t}");
        }
        assert!(common.contains("leaves out `progress`"), "{common}");
        // The kinds of ask, the same under every role asked, are written once.
        assert_eq!(
            common.matches("The existing architecture or design has a flaw").count(),
            1,
            "{common}"
        );
        assert!(common.contains("\"$ref\":\"#/$defs/"), "{common}");
        let four = cell_format(&g, g.cell("4").unwrap(), &[]);
        assert!(
            four.contains("`tools` (of ci, ci_before, measure, gh, query)"),
            "{four}"
        );
        assert!(four.contains("`ask` (to advisor, architect, owner)"), "{four}");
        let twelve = cell_format(&g, g.cell("12").unwrap(), &[]);
        assert!(twelve.contains("`ask` (to architect, owner)"), "{twelve}");
        assert!(!twelve.contains("`redirect`"), "{twelve}");
    }

    // What the inputs every agent's cell has are is said once, in the common format; each cell's list of its inputs
    // names them only, and says in full what its own are.
    #[test]
    fn the_inputs_every_cell_has_are_described_once() {
        let g = graph();
        let common = common_format(&g, &[]);
        let four = g.cell("4").unwrap();
        let goal = four
            .inputs
            .iter()
            .find(|x| x.name.as_deref() == Some("Design goal"))
            .unwrap();
        assert_eq!(common.matches(goal.what.as_str()).count(), 1, "{common}");
        let listed = cell_inputs(four);
        assert!(!listed.contains(goal.what.as_str()), "{listed}");
        assert!(
            listed.contains("Design goal, which format:common describes"),
            "{listed}"
        );
        assert!(listed.contains(four.inputs[0].what.as_str()), "{listed}");
    }

    // A kind of question whose need the project does not meet is left out of the formats, value and meaning, and the
    // other kinds stay.
    #[test]
    fn a_kind_the_project_cannot_meet_is_left_out_of_the_formats() {
        let g = graph();
        let gone = kinds_left_out(&g, None);
        assert!(gone.iter().any(|k| k.kind == "gap_under_test"), "{gone:?}");
        let common = common_format(&g, &gone);
        assert!(!common.contains("gap_under_test"), "{common}");
        assert!(!common.contains("rejects a natural way of writing"), "{common}");
        assert!(common.contains("\"unsure\""), "{common}");
        assert!(common_format(&g, &[]).contains("gap_under_test"));
    }
}
