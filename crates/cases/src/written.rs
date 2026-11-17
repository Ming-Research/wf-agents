//! The cases' files as they are written: `common.toml` of a flow's cases, what its cells' cases share, and a file for
//! each agent's cell, `<cell>.toml`, holding its cases.

use std::collections::BTreeMap;

use serde::Deserialize;

/// `common.toml`: the made items, how the made project's tests are found, and the texts, sets of blocks and rules the
/// cases of the flow share.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Common {
    /// The made project the cases run in, by the name of its file in `projects/` beside the flows' cases.
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub tests: Option<Tests>,
    /// The made items, by their number.
    #[serde(default)]
    pub items: BTreeMap<String, Item>,
    #[serde(default)]
    pub texts: BTreeMap<String, String>,
    #[serde(default)]
    pub sets: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub results: BTreeMap<String, Rules>,
}

/// A made item.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// In the made installation owner's language.
    pub title: String,
    /// Its first background, as whoever made it wrote it.
    pub background: String,
    /// The commit of the made project, by a name of it, that main stands at as the item's work starts.
    pub main: String,
}

/// The file of a cell's cases. Texts, sets and rules it names besides those of `common.toml` are its own.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellFile {
    pub cell: String,
    #[serde(default)]
    pub texts: BTreeMap<String, String>,
    #[serde(default)]
    pub sets: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub results: BTreeMap<String, Rules>,
    #[serde(default)]
    pub cases: Vec<Case>,
}

/// A case: the data its cell is given, in the order they were written, the situation its cell is entered in, what its
/// calls get, and what its cell owes.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    /// The made item its work is done for, by number.
    pub item: u64,
    /// What happened in the work before the cell is entered, in order.
    #[serde(default)]
    pub history: Vec<Entry>,
    /// The cell ran before in the work: it worked from versions the history gave it.
    pub ran_before: bool,
    /// What of what the cell watches is new to it, by the names its definition watches them by.
    pub new: Vec<String>,
    /// The rules its calls' results are given by, by name; a call of any tool fails the case where none is named.
    #[serde(default)]
    pub results: Option<String>,
    pub owes: Owes,
    /// What only reading can judge of the return, for the reading judge; nothing is read where none is given.
    #[serde(default)]
    pub judge: Option<String>,
}

/// Something that happened in the work: a round of a cell, an act of the owner's, or main moving. Exactly one of
/// `cell`, `owner` and `main` is given.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// A round of this cell, which hands in `blocks` and the blocks of `set`, and whose branch stands at `commit` as it
    /// ends where it commits.
    #[serde(default)]
    pub cell: Option<String>,
    #[serde(default)]
    pub set: Option<String>,
    #[serde(default)]
    pub blocks: BTreeMap<String, Value>,
    #[serde(default)]
    pub commit: Option<String>,
    /// An act of the owner's: `background` (the item's background anew, `content` or `text`), `words` (to the cell
    /// `to`, `content` or `text`), `answer` (to the question `question`, taking `option`, counted from one) or
    /// `session` (the line of the cell `to` begins a new session).
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub option: Option<u64>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    /// Main moves to this commit of the made project, by a name of it.
    #[serde(default)]
    pub main: Option<String>,
}

/// A version of a block as a round hands it in: its content, or the named text that is its content; or a table saying
/// that, the choice's value, whether it is handed in as changing nothing in meaning, and, of a block whose form is a
/// shape, the shaped value.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Content(String),
    Table(Version),
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub unchanged: bool,
    #[serde(default)]
    pub shaped: Option<toml::Value>,
}

/// What the cell owes, judged after the engine's own checks of its return.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owes {
    /// Each block it writes: `written`, or the value of a choice.
    #[serde(default)]
    pub writes: BTreeMap<String, String>,
    /// Each block it does not write.
    #[serde(default)]
    pub not_writes: Vec<String>,
    /// Each datum or group it hands in as changing its meaning.
    #[serde(default)]
    pub changes: Vec<String>,
    /// Each datum or group it hands in nothing new of: written not at all, or as changing nothing in meaning.
    #[serde(default)]
    pub keeps: Vec<String>,
    /// Each dialogue it writes its answer in.
    #[serde(default)]
    pub answers: Vec<String>,
    /// Whether it asks the owner a question: `none`, `waits` (one the work waits for) or `any`.
    #[serde(default)]
    pub question: Option<String>,
    /// Whether its rounds commit to the work's branch.
    #[serde(default)]
    pub commits: Option<bool>,
}

/// The rules a case's calls of `run_tests` get their results by.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Each test that fails, and what it says; every other test of the commit passes.
    #[serde(default)]
    pub fails: Vec<Fails>,
}

/// How the made project's tests are found in a commit: a test is the name after `name_after` on the first line at or
/// after a line holding `mark`, in a file under one of `under`; its text runs to the next such line.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    pub mark: String,
    pub name_after: String,
    pub under: Vec<String>,
    /// The jobs of the made project's full workflows, each running every test.
    pub full: Vec<String>,
    /// The job of its fast workflow that runs the tests named; each check named is a job of its name.
    pub fast: String,
    /// How long each job takes, in seconds, as the run says.
    pub seconds: u64,
    /// Where the runs are said to be seen, each by its number after it.
    pub url: String,
}

/// The tests that fail where this holds of them and of the commit run.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fails {
    /// Their names, each a pattern where `*` stands for any text.
    pub names: Vec<String>,
    /// Only a test main does not have (true), or only one it has (false).
    #[serde(default)]
    pub new: Option<bool>,
    /// The commit run holds this file, holding (and not holding) these texts.
    #[serde(default, rename = "where")]
    pub at: Option<Where>,
    /// The test's own text holds each of these.
    #[serde(default)]
    pub holds: Vec<String>,
    /// What it says as it fails.
    pub said: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Where {
    pub file: String,
    #[serde(default)]
    pub has: Option<String>,
    #[serde(default)]
    pub lacks: Option<String>,
}
