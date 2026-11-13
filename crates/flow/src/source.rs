//! The definition of a flow as it is written: the text of its files, and the shape each file must have.
//!
//! A flow is a directory holding `flow.toml`, one `<cell id>.toml` for each cell, and in `rules/` one `<name>.md` for
//! each set of rules for the agents that a line's system prompt names. A field a file does not know is refused, so that
//! every new kind of field is added on purpose.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use decide::TestKind;
use serde::Deserialize;

/// The name of the file that holds what belongs to the flow as a whole.
pub const FLOW_FILE: &str = "flow.toml";

/// The name of the directory, beside the flow's files, that holds its sets of rules for the agents.
pub const RULES_DIR: &str = "rules";

/// The text of a flow's files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    /// The text of `flow.toml`.
    pub flow: String,
    /// The text of each cell's file, by the cell's id: the file's name without `.toml`.
    pub cells: BTreeMap<String, String>,
    /// The text of each set of rules for the agents, by its name: the name of its file in `rules/` without `.md`.
    pub rules: BTreeMap<String, String>,
}

/// Reads the flow in this directory: `flow.toml`, every other `.toml` file directly in it as the file of the cell its
/// name gives, and every `.md` file directly in its `rules/`, where it has one, as the set of rules its name gives.
/// Other files and directories are not read.
pub fn read(dir: &Path) -> io::Result<Source> {
    let flow = fs::read_to_string(dir.join(FLOW_FILE))?;
    let cells = files(dir, "toml", |name| name != FLOW_FILE)?;
    let rules_dir = dir.join(RULES_DIR);
    let rules = if rules_dir.is_dir() {
        files(&rules_dir, "md", |_| true)?
    } else {
        BTreeMap::new()
    };
    Ok(Source { flow, cells, rules })
}

/// The text of each file directly in this directory whose name ends in this extension and is taken, by its name
/// without the extension.
fn files(dir: &Path, extension: &str, taken: impl Fn(&str) -> bool) -> io::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != extension) {
            continue;
        }
        let (Some(name), Some(stem)) = (
            path.file_name().and_then(|n| n.to_str()),
            path.file_stem().and_then(|s| s.to_str()),
        ) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("a file's name in {} is not UTF-8", dir.display()),
            ));
        };
        if taken(name) {
            files.insert(stem.to_string(), fs::read_to_string(&path)?);
        }
    }
    Ok(files)
}

impl Source {
    /// The flow these files make, each by its path within the flow's directory and its text, as [`read`] takes the
    /// files of a directory: `flow.toml`, every other file directly in it whose name ends in `.toml` as the file of the
    /// cell its name gives, and every file directly in `rules/` whose name ends in `.md` as the set of rules its name
    /// gives. Other files are left out. None where there is no `flow.toml` among them.
    pub fn of_files(files: impl IntoIterator<Item = (String, String)>) -> Option<Source> {
        let mut flow = None;
        let mut cells = BTreeMap::new();
        let mut rules = BTreeMap::new();
        for (path, text) in files {
            match path.split_once('/') {
                None if path == FLOW_FILE => flow = Some(text),
                None => {
                    if let Some(id) = path.strip_suffix(".toml") {
                        cells.insert(id.to_string(), text);
                    }
                }
                Some((RULES_DIR, name)) if !name.contains('/') => {
                    if let Some(name) = name.strip_suffix(".md") {
                        rules.insert(name.to_string(), text);
                    }
                }
                Some(_) => {}
            }
        }
        Some(Source {
            flow: flow?,
            cells,
            rules,
        })
    }
}

/// `flow.toml`: what belongs to the flow as a whole.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowFile {
    /// The flow's name.
    pub name: String,
    /// The ids of the cells, in the order the walk takes them.
    pub order: Vec<String>,
    /// The limits of everything that repeats, for each cell that sets none of its own.
    pub limits: LimitsFile,
    /// The lines: the cells of a work that share one session.
    #[serde(default)]
    pub lines: BTreeMap<String, LineFile>,
    /// Every block of a work's state.
    #[serde(default)]
    pub blocks: BTreeMap<String, BlockFile>,
    /// The named tests the cells use, by name.
    #[serde(default)]
    pub tests: BTreeMap<String, TestFile>,
}

/// The limits as written. They are read as any whole number so that one that is not positive is a fault of the
/// definition, not of its shape.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsFile {
    /// How many runs of a cell in a row may be ways round.
    pub runs: i64,
    /// How often a return may be sent back within one run.
    pub returns: i64,
    /// How many calls one run may make.
    pub calls: i64,
}

/// A line as written.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineFile {
    /// The line's agents only read: they edit no file and change nothing of the repository. A line that writes code,
    /// tests or anything else it commits leaves it out.
    #[serde(default)]
    pub reads_only: bool,
    /// How many levels above its item's the line's rounds run at, none above the highest an item may be at; none
    /// where it is left out.
    #[serde(default)]
    pub levels_above: u32,
    /// The flow's sets of rules for the agents that the system prompt of the line's session holds, each `rules/<name>.md`
    /// by its name, in the order they stand in it; none where it is left out. The engine places its own parts around
    /// them, the same for every line: the project's guide for its agents, the constitution and the rules of the design
    /// tree before them, its own words on working in the engine after them.
    #[serde(default)]
    pub system: Vec<String>,
}

/// A block as written.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockFile {
    /// The values of a block that is a choice; a block with none is not one.
    pub values: Option<Vec<String>>,
    /// People write the block.
    #[serde(default)]
    pub people: bool,
    /// The block is a note: what people write in it are their words to the cells that read it, which a paused work
    /// still answers. Only a block people write, and no choice, is one.
    #[serde(default)]
    pub note: bool,
    /// The source outside the work that writes the block, where one does: a name of a source the engine has.
    pub outside: Option<String>,
    /// The owner's page reads the block, so it needs no other reader.
    #[serde(default)]
    pub shown: bool,
    /// What the block is, in a sentence, for whoever reads the flow: the owner's page shows it with the block and with
    /// each cell that reads or writes it.
    pub what: Option<String>,
}

/// A named test as written: a test of the values of blocks of the work itself.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestFile {
    /// The words shown for the test.
    pub says: String,
    /// How the blocks it reads make it hold: `all`, `any`, `none` or `none_standing`; another is refused.
    pub kind: TestKind,
    /// The blocks it reads, each a choice.
    pub reads: Vec<String>,
    /// The value it looks for in each block it reads.
    pub is: String,
}

/// The file of one cell.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellFile {
    /// What the cell is called on the chart.
    pub title: String,
    /// Who carries the cell out.
    pub kind: Kind,
    /// The line whose session an agent cell runs in.
    pub line: Option<String>,
    /// What an agent cell's agent is told to do: the cell's task.
    pub task: Option<String>,
    /// The act of the engine's own that carries out an engine cell: a name of an act the engine has.
    pub act: Option<String>,
    /// When the cell runs at all; with none it always may.
    pub when: Option<When>,
    /// The named tests that must hold before the cell.
    #[serde(default)]
    pub requires: Vec<String>,
    /// The blocks whose new version runs the cell, in the order it names them.
    #[serde(default)]
    pub runs_on: Vec<String>,
    /// Why a block runs the cell, by the block's name: each a block among `runs_on`.
    #[serde(default)]
    pub run: BTreeMap<String, RunFile>,
    /// The blocks the cell is given: its session is kept supplied with their content, each by its kind, in the order
    /// it names them. Only an agent cell has a session to supply.
    #[serde(default)]
    pub given: Vec<String>,
    /// How the cell is given a block and why, by the block's name: each a block among `given`, and each block of
    /// `given` has one that names its kind.
    #[serde(default)]
    pub give: BTreeMap<String, GiveFile>,
    /// The old form of `runs_on` and `given`, one list for both: each block of it runs the cell, and an agent cell is
    /// given it. A file names this list or the two, not both.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// The old form of `give`, by the block's name: each a block among `inputs`. Of an engine cell, which is given
    /// nothing, it says only why the block runs the cell.
    #[serde(default)]
    pub input: BTreeMap<String, GiveFile>,
    /// The blocks the cell may write.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// The blocks among its outputs that every hand-in of the cell includes.
    #[serde(default)]
    pub must_write: Vec<String>,
    /// The blocks among those that run it that say what the work is where the host of the project's repository shows
    /// the work, as the body of its pull request; an engine cell's act reads them.
    #[serde(default)]
    pub describes: Vec<String>,
    /// How many runs of the cell in a row may be ways round, if not the flow's limit.
    pub limit: Option<i64>,
    /// The reasoning effort an agent cell's rounds run at, whatever the model; every agent cell sets one.
    pub effort: Option<Effort>,
    /// Whether the work's branch takes the latest main in as each run of the agent cell starts: a cell that changes
    /// the code.
    #[serde(default)]
    pub takes_main: bool,
    /// The tools of the engine's that an agent cell may ask for in its return, by name, each a tool the engine has.
    #[serde(default)]
    pub tools: Vec<String>,
}

/// Why a block runs a cell, as written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunFile {
    /// What the cell's agent does when the block is new, in a sentence.
    pub why: Option<String>,
}

/// How a cell is given a block, as written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GiveFile {
    /// What the context of the cell's session must hold of the block when the cell runs.
    pub kind: Option<InputKind>,
    /// Why the cell is given the block: what its agent does with it, in a sentence.
    pub why: Option<String>,
}

/// What the context of a cell's session must hold of a block it is given when the cell runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    /// Only the latest version.
    Latest,
    /// The version the cell last consumed and the latest, so that the agent sees what changed between them.
    LastAndLatest,
    /// Every version.
    All,
}

impl InputKind {
    /// The kind's name, as it is written.
    pub fn name(self) -> &'static str {
        match self {
            InputKind::Latest => "latest",
            InputKind::LastAndLatest => "last_and_latest",
            InputKind::All => "all",
        }
    }
}

/// The reasoning efforts an agent's round may run at, as the command line names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Effort {
    /// The effort's name, as it is written and as the command line takes it.
    pub fn name(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

/// Who carries a cell out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An agent, in the session of the cell's line.
    Agent,
    /// The engine itself.
    Engine,
}

/// A cell's condition as written: on the value of a block, or on a named test.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum When {
    Is(WhenIs),
    Test(WhenTest),
}

/// The cell runs when the block `block` has the value `is`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhenIs {
    pub block: String,
    pub is: String,
}

/// The cell runs when the named test holds.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhenTest {
    pub test: String,
}
