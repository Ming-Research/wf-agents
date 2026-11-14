//! What the engine says to every agent, whatever the flow, as it is written beside the flows: the project's files and
//! settings (`project.toml`), and in `agents/` what a session begins with, the rules for the agents, the engine's own
//! words, the return and its fields, the tools, and the message a cell is sent as it is entered (design document
//! `data-and-context`, 51). Every word an agent is sent that is not a flow's is in these files; the code holds none of
//! them, only how they are put together. A field a file does not know is refused, and what cannot be read is said with
//! the path of its file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// The directory beside the flows that holds what the engine says to every agent.
pub const AGENTS_DIR: &str = "agents";

/// The file beside the flows that names the project's files and settings.
pub const PROJECT_FILE: &str = "project.toml";

/// All of it, read from the directory that holds the flows.
#[derive(Clone, Debug)]
pub struct Agents {
    /// The directory read, which holds the flows.
    pub root: PathBuf,
    pub project: Project,
    pub session: Session,
    /// The text of each file of the engine's own words that the session names, by its name in `agents/`.
    pub words: BTreeMap<String, String>,
    /// The sets of rules for the agents, by name, as `agents/rules/rules.toml` names them, each with its text.
    pub rules: BTreeMap<String, RuleSet>,
    pub returns: Return,
    /// Each field of a return that is the engine's, by name: `agents/return/fields/<name>.toml`.
    pub fields: BTreeMap<String, Field>,
    /// Each tool of the engine's, by name: `agents/tools/<name>.toml`.
    pub tools: BTreeMap<String, Tool>,
    pub message: Message,
    pub given: Given,
}

/// `project.toml`: the project's files the engine gives every agent, and the settings the words sent to the agents may
/// name.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The project's files, by the name a part of a session gives them.
    pub files: BTreeMap<String, ProjectFile>,
    /// The installation's settings the words sent to the agents may name.
    #[serde(default)]
    pub installation: Installation,
    /// The project's settings, which the words sent to the agents name as `project.<name>`.
    #[serde(default)]
    pub project: BTreeMap<String, String>,
}

/// The installation's settings the words sent to the agents may name, whose values the engine fills in from the
/// installation.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub names: Vec<String>,
}

/// A file of the project.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    /// Its path from the root of the project's checkout.
    pub path: String,
}

/// `agents/session.toml`: what a line's session begins with, its system prompt.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    /// Its parts, in the order they stand in it.
    pub parts: Vec<SessionPart>,
    /// How a file of the project stands in it: `{path}` its path, `{text}` its text.
    pub heading: String,
}

/// A part of the system prompt.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum SessionPart {
    Project(ProjectPart),
    Line(LinePart),
    Words(WordsPart),
}

/// A file of the project, by its name in `project.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPart {
    pub project: String,
}

/// What the line names for its own system prompt.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinePart {
    pub line: OfTheLine,
}

/// What of a line stands in its system prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfTheLine {
    /// The sets of rules the line takes, in the order it names them.
    Rules,
}

/// A text of the engine's, by its file in `agents/`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordsPart {
    pub words: String,
}

/// A set of rules for the agents, as `agents/rules/rules.toml` names it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSet {
    /// Its file, in `agents/rules/`.
    pub file: String,
    /// The text of its file.
    #[serde(skip)]
    pub text: String,
}

/// `agents/return/return.toml`: the form of a return, and what holds together what a cell is told it returns.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Return {
    pub form: Form,
    pub told: Told,
}

/// The form: the order of the fields of each kind of return, and the shape of a block.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    /// The fields of a return that asks for calls, in order.
    pub calls: Vec<String>,
    /// The fields of a return that does the cell's work, in order.
    pub outcome: Vec<String>,
    /// The shape of a block that is a choice.
    pub choice: toml::Value,
    /// The shape of a block that is a text.
    pub text: toml::Value,
}

/// What holds together what a cell is told it returns.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Told {
    pub heading: String,
    pub blocks: String,
    pub block: String,
    /// What a block's line says after its name: that it is a choice, `{values}` its values; that the owner reads it;
    /// that the cell must hand it in every time.
    pub choice: String,
    pub shown: String,
    pub must_write: String,
    /// An item of a list of fields or tools, `{text}` its words.
    pub item: String,
    /// The fields told `listed`, in order.
    pub listed: Vec<String>,
    pub every_time: String,
    pub fields: String,
    pub owner_reads: String,
    /// The fields the owner reads, in the order `owner_reads` names them.
    pub reads: Vec<String>,
    pub and: String,
    pub calls: String,
    pub tools: String,
}

/// A field of a return that is the engine's: `agents/return/fields/<name>.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// The kinds of return it is a field of.
    #[serde(rename = "in")]
    pub kinds: Vec<ReturnKind>,
    /// Every return of those kinds holds it.
    #[serde(default)]
    pub required: bool,
    pub given_to: GivenTo,
    pub told: ToldAs,
    /// Of a list whose items are names, which names, by a name the engine knows.
    #[serde(rename = "enum")]
    pub names: Option<String>,
    pub words: String,
    /// What is told after the list of the fields, of a field told `listed`.
    pub also: Option<String>,
    pub shape: toml::Value,
}

/// A kind of return.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnKind {
    /// A return that asks the engine for calls.
    Calls,
    /// A return that does the cell's work.
    Outcome,
}

/// Which cells may use a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GivenTo {
    /// Every agent cell.
    Every,
    /// A cell that may write a block.
    Writing,
    /// A cell that may write a datum declaring this field as the one it is written through.
    WritesThrough,
}

/// Where a field's words go in what a cell is told it returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToldAs {
    /// A paragraph of its own.
    Alone,
    /// After the words on blocks.
    WithBlocks,
    /// In the list of the fields that are not blocks.
    Listed,
}

/// A tool of the engine's: `agents/tools/<name>.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub words: String,
    /// The shape of a request of it.
    pub request: toml::Value,
}

/// `agents/message/message.toml`: what the engine sends a cell's agent as it enters the cell, its parts in order.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub parts: Vec<MessagePart>,
}

/// A part of a message.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessagePart {
    /// What the part is, by a name the engine knows.
    pub part: String,
    pub sent: Sent,
    pub text: Option<String>,
    /// Of a part that is a list, each item of it.
    pub each: Option<String>,
    /// What is sent in its place where the session holds it already.
    pub held: Option<String>,
    /// Of the triggers: what a trigger's line says of what happened, `{detail}` the trigger's own place.
    pub detail: Option<String>,
    /// Of the triggers: what a trigger's line says the cell does, `{do}` its words.
    #[serde(rename = "do")]
    pub does: Option<String>,
    /// What follows the list.
    pub after: Option<String>,
}

/// When a part of a message is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sent {
    /// Each time the cell is entered.
    EveryTime,
    /// Where the session lacks it, and again when it changes.
    OnceASession,
    /// After a return that handed in blocks.
    AfterHandIn,
    /// Each datum the cell is given, as its kind says.
    ByKind,
}

/// `agents/message/given.toml`: how a datum a cell is given is put into its session, by its kind.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Given {
    /// What a text says of a datum that is a choice.
    pub choice: String,
    /// What stands in place of a datum's content where the session holds its latest version already.
    pub held: String,
    pub latest: GivenKind,
    pub last_and_latest: GivenKind,
    pub all: GivenKind,
}

/// How a datum given in one kind is put in.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GivenKind {
    pub text: String,
}

/// Reads it all from the directory that holds the flows, anew each time, so that an edit of a file shows; or says which
/// file cannot be read, and why.
pub fn read(root: &Path) -> Result<Agents, String> {
    let at = root.join(AGENTS_DIR);
    let session: Session = file(&at.join("session.toml"))?;
    let mut words = BTreeMap::new();
    for part in &session.parts {
        if let SessionPart::Words(WordsPart { words: name }) = part {
            words.insert(name.clone(), text(&at.join(name))?);
        }
    }
    let rules_dir = at.join("rules");
    let mut rules: BTreeMap<String, RuleSet> = file(&rules_dir.join("rules.toml"))?;
    for set in rules.values_mut() {
        set.text = text(&rules_dir.join(&set.file))?;
    }
    Ok(Agents {
        root: root.to_path_buf(),
        project: file(&root.join(PROJECT_FILE))?,
        session,
        words,
        rules,
        returns: file(&at.join("return").join("return.toml"))?,
        fields: each(&at.join("return").join("fields"))?,
        tools: each(&at.join("tools"))?,
        message: file(&at.join("message").join("message.toml"))?,
        given: file(&at.join("message").join("given.toml"))?,
    })
}

/// The text of a file.
fn text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A file read into its shape.
fn file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    toml::from_str(&text(path)?).map_err(|e| format!("{}: {}", path.display(), e.to_string().trim_end()))
}

/// Each `.toml` file directly in a directory, read into its shape, by its name without `.toml`.
fn each<T: DeserializeOwned>(dir: &Path) -> Result<BTreeMap<String, T>, String> {
    let mut out = BTreeMap::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = (path.file_name().and_then(|n| n.to_str())).and_then(|n| n.strip_suffix(".toml")) else {
            continue;
        };
        out.insert(name.to_string(), file(&path)?);
    }
    Ok(out)
}
