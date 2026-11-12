//! The definition of a flow as it is written: the text of its files, and the shape each file must have.
//!
//! A flow is a directory holding `flow.toml` and one `<cell id>.toml` for each cell. A field a file does not know is
//! refused, so that every new kind of field is added on purpose.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use decide::TestKind;
use serde::Deserialize;

/// The name of the file that holds what belongs to the flow as a whole.
pub const FLOW_FILE: &str = "flow.toml";

/// The text of a flow's files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    /// The text of `flow.toml`.
    pub flow: String,
    /// The text of each cell's file, by the cell's id: the file's name without `.toml`.
    pub cells: BTreeMap<String, String>,
}

/// Reads the flow in this directory: `flow.toml`, and every other `.toml` file directly in it as the file of the cell
/// its name gives. Other files and directories are not read.
pub fn read(dir: &Path) -> io::Result<Source> {
    let flow = fs::read_to_string(dir.join(FLOW_FILE))?;
    let mut cells = BTreeMap::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        if path.file_name().is_some_and(|n| n == FLOW_FILE) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a file's name in the flow's directory is not UTF-8",
            ));
        };
        cells.insert(stem.to_string(), fs::read_to_string(&path)?);
    }
    Ok(Source { flow, cells })
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
#[derive(Clone, Copy, Debug, Default, Deserialize)]
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
    /// The blocks the cell reads.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// How the cell takes a block it reads, by the block's name: each a block among its inputs.
    #[serde(default)]
    pub input: BTreeMap<String, InputFile>,
    /// The blocks the cell may write.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// The blocks among its outputs that every hand-in of the cell includes.
    #[serde(default)]
    pub must_write: Vec<String>,
    /// The blocks among its inputs that say what the work is where the host of the project's repository shows the
    /// work, as the body of its pull request; an engine cell's act reads them.
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
}

/// How a cell takes one of the blocks it reads, as written. The owner's page shows it; what a round is given does
/// not go by it yet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputFile {
    /// What the context of the cell's session must hold of the block when the cell runs.
    pub kind: Option<InputKind>,
    /// Why the cell takes the block: what its agent does with it, in a sentence.
    pub why: Option<String>,
}

/// What the context of a cell's session must hold of an input when the cell runs.
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
