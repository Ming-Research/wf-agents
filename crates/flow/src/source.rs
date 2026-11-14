//! The definition of a flow as it is written: the text of its files, and the shape each file must have.
//!
//! A flow is a directory. Laid out one thing to a file, it holds `flow.toml` for what is the flow's as a whole, and one
//! file to each cell, line, block and named test, in `cells/`, `lines/`, `blocks/` (in directories of their own, as
//! deep as is useful) and `tests/`, each named by the thing it defines. What every flow shares is beside the flows,
//! defined once: the engine's own data in `../data/`, which a flow names among those it uses; what runs a cell, in
//! `../triggers/`; and the sets of rules for the agents that a line names, in `../agents/rules/rules.toml`. Written
//! before that layout, a flow is `flow.toml` holding its lines, blocks and named tests, one `<cell id>.toml` beside it
//! for each cell, and in `rules/` one `<name>.md` for each set of rules for the agents that a line's system prompt
//! names. A field a file does not know is refused, so that every new kind of field is added on purpose.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use decide::TestKind;
use serde::Deserialize;

use crate::agents;

/// The name of the file that holds what belongs to the flow as a whole.
pub const FLOW_FILE: &str = "flow.toml";

/// The name of the directory, beside the files of a flow written before it was laid out one thing to a file, that
/// holds its sets of rules for the agents.
pub const RULES_DIR: &str = "rules";

/// The directories of a flow laid out one thing to a file, each holding the files of one kind of thing: its cells, its
/// lines, its blocks, as deep as is useful, and its named tests. A flow is laid out so where it has `cells/`.
pub const CELLS_DIR: &str = "cells";
pub const LINES_DIR: &str = "lines";
pub const BLOCKS_DIR: &str = "blocks";
pub const TESTS_DIR: &str = "tests";

/// The extension of a Markdown file in the directory of a flow laid out one thing to a file, as deep as is useful, which
/// a cell's task may be written in (`tasks/<name>.md`).
pub const TASK_EXTENSION: &str = "md";

/// The directory beside the flows, from a flow's directory, that holds the engine's own data, as deep as is useful:
/// each defined once for every flow, and used by a flow that names it.
pub const DATA_DIR: &str = "../data";

/// The directory beside the flows, from a flow's directory, that holds what runs a cell: in it, as deep as is useful,
/// each a trigger standing on a block that stands only for it, which it declares; in `of_a_cell/`, each a trigger that every
/// cell naming it has its own of; in `of_a_line/`, each a trigger that every line has its own of, which each cell of the
/// line naming it shares; in `of_a_datum/`, each a rule that makes a trigger of each datum of a flow; in `of_a_group/`,
/// each a rule that makes a trigger of each group of a flow's blocks.
pub const TRIGGERS_DIR: &str = "../triggers";
pub const OF_A_CELL: &str = "of_a_cell";
pub const OF_A_LINE: &str = "of_a_line";
pub const OF_A_DATUM: &str = "of_a_datum";
pub const OF_A_GROUP: &str = "of_a_group";

/// The path, from a flow's directory, of the file beside the flows that names the sets of rules for the agents a line
/// may take: [`agents::RULES_FILE`].
pub fn rules_file() -> String {
    format!("../{}", agents::RULES_FILE)
}

/// The text of a flow's files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    /// The flow is laid out one thing to a file; else it is written as it was before that layout.
    pub laid_out: bool,
    /// The text of `flow.toml`.
    pub flow: String,
    /// The text of each cell's file, by the cell's id: the file's name without `.toml`.
    pub cells: BTreeMap<String, String>,
    /// Of a flow written before it was laid out one thing to a file, the text of each set of rules for the agents, by
    /// its name: the name of its file in `rules/` without `.md`.
    pub rules: BTreeMap<String, String>,
    /// Of a flow laid out one thing to a file, the text of each of its files but `flow.toml` and its cells', and of
    /// each file beside the flows that it may use, by its path from the flow's directory: `lines/<name>.toml`,
    /// `blocks/.../<name>.toml`, `tests/<name>.toml`, `../data/.../<name>.toml`, `../triggers/.../<name>.toml` and
    /// [`rules_file`].
    pub files: BTreeMap<String, String>,
}

/// Reads the flow in this directory. Laid out one thing to a file where it has `cells/`: `flow.toml`; each `.toml` file
/// directly in `cells/`, `lines/` and `tests/`; each in `blocks/`, `../data/` and `../triggers/` or in a directory in
/// them; and [`rules_file`]. Else as it was written before: `flow.toml`, every other `.toml` file directly in it as the
/// file of the cell its name gives, and every `.md` file directly in its `rules/`, where it has one, as the set of rules
/// its name gives. Other files and directories are not read.
pub fn read(dir: &Path) -> io::Result<Source> {
    let mut source = Source {
        laid_out: dir.join(CELLS_DIR).is_dir(),
        flow: fs::read_to_string(dir.join(FLOW_FILE))?,
        ..Source::default()
    };
    if source.laid_out {
        for (sub, deep) in [
            (CELLS_DIR, false),
            (LINES_DIR, false),
            (BLOCKS_DIR, true),
            (TESTS_DIR, false),
            (DATA_DIR, true),
            (TRIGGERS_DIR, true),
        ] {
            let at = dir.join(sub);
            if at.is_dir() {
                for (path, text) in files(&at, "toml", deep)? {
                    source.add(&format!("{sub}/{path}"), text);
                }
            }
        }
        let rules = dir.join(rules_file());
        if rules.is_file() {
            source.add(&rules_file(), fs::read_to_string(rules)?);
        }
        for (path, text) in files(dir, TASK_EXTENSION, true)? {
            source.add(&path, text);
        }
        return Ok(source);
    }
    for (name, text) in files(dir, "toml", false)? {
        source.add(&name, text);
    }
    let rules_dir = dir.join(RULES_DIR);
    if rules_dir.is_dir() {
        for (name, text) in files(&rules_dir, "md", false)? {
            source.add(&format!("{RULES_DIR}/{name}"), text);
        }
    }
    Ok(source)
}

/// The text of each file in this directory whose name ends in this extension, and, where `deep`, of each in the
/// directories in it, by its path from it.
fn files(dir: &Path, extension: &str, deep: bool) -> io::Result<Vec<(String, String)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let within = deep && path.is_dir();
        if !within && !(path.is_file() && path.extension().is_some_and(|e| e == extension)) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("a file's name in {} is not UTF-8", dir.display()),
            ));
        };
        if within {
            for (inner, text) in self::files(&path, extension, true)? {
                files.push((format!("{name}/{inner}"), text));
            }
        } else {
            files.push((name.to_string(), fs::read_to_string(&path)?));
        }
    }
    Ok(files)
}

impl Source {
    /// The flow these files make, each by its path from the flow's directory and its text, as [`read`] takes the files
    /// of a directory: laid out one thing to a file where a file of `cells/` is among them, else as it was written
    /// before. Other files are left out. None where there is no `flow.toml` among them.
    pub fn of_files(files: impl IntoIterator<Item = (String, String)>) -> Option<Source> {
        let files: Vec<(String, String)> = files.into_iter().collect();
        let mut source = Source {
            laid_out: (files.iter()).any(|(path, _)| matches!(laid_out(path), Some(Kept::Cell(_)))),
            ..Source::default()
        };
        let mut flow = None;
        for (path, text) in files {
            if path == FLOW_FILE {
                flow = Some(text);
            } else {
                source.add(&path, text);
            }
        }
        source.flow = flow?;
        Some(source)
    }

    /// Keeps a file, by its path from the flow's directory, where it is one that a flow of its layout is made of.
    fn add(&mut self, path: &str, text: String) {
        let kept = if self.laid_out {
            laid_out(path)
        } else {
            written_before(path)
        };
        match kept {
            Some(Kept::Cell(id)) => {
                self.cells.insert(id.to_string(), text);
            }
            Some(Kept::Rules(name)) => {
                self.rules.insert(name.to_string(), text);
            }
            Some(Kept::File) => {
                self.files.insert(path.to_string(), text);
            }
            None => {}
        }
    }
}

/// What a file of a flow is kept as.
enum Kept<'a> {
    /// The file of the cell of this id.
    Cell(&'a str),
    /// The set of rules of this name, of a flow written before it was laid out one thing to a file.
    Rules(&'a str),
    /// Another file of a flow laid out one thing to a file, kept by its path.
    File,
}

/// What a file of a flow laid out one thing to a file is kept as, by its path from the flow's directory; none where it
/// is none that the flow is made of.
fn laid_out(path: &str) -> Option<Kept<'_>> {
    // A Markdown file in the flow's directory, as deep as it is, which a task may be written in.
    if !path.starts_with("../") && path.ends_with(&format!(".{TASK_EXTENSION}")) {
        return Some(Kept::File);
    }
    let (dir, name) = path.rsplit_once('/')?;
    let stem = name.strip_suffix(".toml")?;
    let under = |root: &str| dir == root || dir.starts_with(&format!("{root}/"));
    if dir == CELLS_DIR {
        Some(Kept::Cell(stem))
    } else if dir == LINES_DIR || dir == TESTS_DIR || under(BLOCKS_DIR) || under(DATA_DIR) || under(TRIGGERS_DIR) {
        Some(Kept::File)
    } else {
        (path == rules_file()).then_some(Kept::File)
    }
}

/// What a file of a flow written before it was laid out one thing to a file is kept as, by its path from the flow's
/// directory: every `.toml` file directly in it but `flow.toml` as a cell's, and every `.md` file directly in `rules/`
/// as a set of rules; none where it is none that the flow is made of.
fn written_before(path: &str) -> Option<Kept<'_>> {
    match path.split_once('/') {
        None if path == FLOW_FILE => None,
        None => path.strip_suffix(".toml").map(Kept::Cell),
        Some((RULES_DIR, name)) if !name.contains('/') => name.strip_suffix(".md").map(Kept::Rules),
        Some(_) => None,
    }
}

/// `flow.toml` of a flow written before it was laid out one thing to a file: what belongs to the flow as a whole, its
/// lines, its blocks and its named tests among it.
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
    /// The groups of the flow's blocks, by name, each with its blocks: a cell names a group's trigger
    /// (`../triggers/of_a_group/`), which fires for a new version of any of them.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
}

/// `flow.toml` of a flow laid out one thing to a file: what is the flow's as a whole. Its lines, blocks and named tests
/// are each in a file of its own.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaidOutFlowFile {
    /// The flow's name.
    pub name: String,
    /// The ids of the cells, in the order the walk takes them.
    pub order: Vec<String>,
    /// The limits of everything that repeats, for each cell that sets none of its own.
    pub limits: LimitsFile,
    /// The engine's own data the flow uses, each by its name: a file of `../data/` of that name defines it, and none of
    /// the flow's own blocks has the name.
    #[serde(default)]
    pub data: Vec<String>,
    /// The tools of the engine's that every agent cell of the flow may ask for in its return, by name, each a tool the
    /// engine has: the form of a return is the same for every cell of the flow, so a cell of a flow that names them
    /// names none of its own.
    #[serde(default)]
    pub tools: Vec<String>,
    /// The groups of the flow's blocks, by a name no block, datum or other group has, each with its blocks as the flow
    /// names them: a cell names a group's trigger (`../triggers/of_a_group/`), which fires for a new version of any of
    /// them.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
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
    /// by its name, in the order they stand in it; none where it is left out. The other parts of the system prompt, and
    /// where these stand among them, are those `agents/session.toml` beside the flows names.
    #[serde(default)]
    pub system: Vec<String>,
}

/// The file of a line, in a flow laid out one thing to a file.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaidOutLineFile {
    /// The line's agents only read: they edit no file and change nothing of the repository. A line that writes code,
    /// tests or anything else it commits leaves it out.
    #[serde(default)]
    pub reads_only: bool,
    /// How many levels above its item's the line's rounds run at, none above the highest an item may be at; none
    /// where it is left out.
    #[serde(default)]
    pub levels_above: u32,
    /// The sets of rules for the agents that the system prompt of the line's session holds, each by its name in
    /// [`agents::RULES_FILE`], in the order they stand in it; none where it is left out. The rest of the system prompt is the
    /// same for every line, and is said beside the flows.
    #[serde(default)]
    pub rules: Vec<String>,
}

/// The value of `from` that says the owner writes a block, by the owner's acts on the page; any other value names a
/// source outside the work. The two stay apart because a paused work still lets through what the owner writes, and
/// nothing a source outside writes.
pub const OWNER: &str = "owner";

/// A block as written in the `flow.toml` of a flow written before it was laid out one thing to a file.
#[derive(Clone, Debug, Default, Deserialize)]
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
    /// Of a choice, the value it counts as before anyone has written it, wherever a cell's `when` or `requires` or a
    /// named test reads it: one of its values. None where it has no value until it is written.
    pub default: Option<String>,
    /// What the block is, in a sentence, for whoever reads the flow: the owner's page shows it with the block and with
    /// each cell that reads or writes it.
    pub what: Option<String>,
    /// The field of a return that a cell writes the block through, where it writes it through one and not as a block
    /// under the block's name: a name of a field of the engine's.
    pub field: Option<String>,
    /// Of a block a source of the engine's writes, what it writes: each text by its name, in the order written, a
    /// `{name}` in it a place the engine fills in a run.
    #[serde(default)]
    pub writes: Texts,
}

/// A block as written in a file of a flow's `blocks/`, or, of the engine's own data, in a file of `../data/`: as in
/// `flow.toml` before, but that who writes it from outside the cells is said by one key, `from`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaidOutBlockFile {
    /// The values of a block that is a choice; a block with none is not one.
    pub values: Option<Vec<String>>,
    /// Who writes the block from outside the cells, where anyone does: [`OWNER`], or a name of a source outside the
    /// work that the engine has.
    pub from: Option<String>,
    /// The block is a note: what the owner writes in it are their words to the cells that read it, which a paused
    /// work still answers. Only a block the owner writes, and no choice, is one.
    #[serde(default)]
    pub note: bool,
    /// The owner's page reads the block, so it needs no other reader.
    #[serde(default)]
    pub shown: bool,
    /// Of a choice, the value it counts as before anyone has written it, wherever a cell's `when` or `requires` or a
    /// named test reads it: one of its values. None where it has no value until it is written.
    pub default: Option<String>,
    /// What the block is, in a sentence, for whoever reads the flow: the owner's page shows it with the block and with
    /// each cell that reads or writes it.
    pub what: Option<String>,
    /// The field of a return that a cell writes the block through, where it writes it through one and not as a block
    /// under the block's name: a name of a field of the engine's.
    pub field: Option<String>,
    /// Of a block a source of the engine's writes, what it writes: each text by its name, in the order written, a
    /// `{name}` in it a place the engine fills in a run. The words are the flow's, so the code holds none of them.
    #[serde(default)]
    pub writes: Texts,
}

/// Who writes a block from outside the cells, as `from` says it: whether the owner does, and the source outside the
/// work that does, where one does.
pub fn who_writes(from: Option<&str>) -> (bool, Option<String>) {
    match from {
        Some(OWNER) => (true, None),
        Some(source) => (false, Some(source.to_string())),
        None => (false, None),
    }
}

impl From<LaidOutBlockFile> for BlockFile {
    fn from(block: LaidOutBlockFile) -> Self {
        let (people, outside) = who_writes(block.from.as_deref());
        BlockFile {
            values: block.values,
            people,
            note: block.note,
            outside,
            shown: block.shown,
            default: block.default,
            what: block.what,
            field: block.field,
            writes: block.writes,
        }
    }
}

/// A trigger as written in a file of `../triggers/`, by the name of its file: one standing on a block that stands only for
/// it, which it declares, which the owner or a source outside the work writes; in `of_a_cell/`, one that every cell
/// naming it has its own of, `{cell}` standing for the id of the cell in small letters in `block`, which must hold it,
/// and in `read`, `detail` and `do`; or, in `of_a_line/`, one that every line has its own of, `{line}` standing so for
/// the name of the line of the cell naming it, so that the cells of one line share it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerFile {
    /// What it is, in a sentence: when it fires. The owner's page shows it with each cell it runs.
    pub what: Option<String>,
    /// The block it stands on.
    pub block: String,
    /// Who writes the block: [`OWNER`], or a name of a source outside the work that the engine has.
    pub from: Option<String>,
    /// What the engine says of what happened, where it says something: the name of a place it fills in a run.
    pub detail: Option<String>,
    /// The data the agent reads for it, each among those its cell is given.
    #[serde(default)]
    pub read: Vec<String>,
    /// What a cell does for it, where the cell says nothing of its own.
    #[serde(rename = "do")]
    pub does: Option<String>,
    /// What the source writes into the block it stands on, where it is one of the engine's: as a block's `writes`.
    #[serde(default)]
    pub writes: Texts,
}

/// A rule as written in a file of `../triggers/of_a_datum/`, which makes a trigger of each datum of a flow: a cell names
/// it by `name`, `{datum}` there standing for the datum's name, and it stands on the datum.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfADatumFile {
    /// The name of a datum's trigger, which must hold `{datum}`, standing for the datum's name.
    pub name: String,
    /// What a datum's trigger is, in a sentence, `{datum}` standing for the datum's name: when it fires. The owner's
    /// page shows it with each cell it runs.
    pub what: Option<String>,
    /// The data the agent reads for it, `{datum}` standing for the datum's name.
    #[serde(default)]
    pub read: Vec<String>,
    /// What a cell does for it, where the cell says nothing of its own, `{datum}` standing for the datum's name.
    #[serde(rename = "do")]
    pub does: Option<String>,
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
    /// What an agent cell's agent is told to do: the cell's task, written in place or in a file of its own.
    pub task: Option<Task>,
    /// The act of the engine's own that carries out an engine cell: a name of an act the engine has.
    pub act: Option<String>,
    /// When the cell runs at all: every key holds; with none it always may.
    pub when: Option<Keys>,
    /// What must hold before the cell: every key; the work waits before the cell while one does not.
    pub requires: Option<Keys>,
    /// What runs the cell, written `triggered_by`: each a trigger by its name, in the order it names them, a trigger of
    /// `../triggers/` or of its `of_a_cell/` or `of_a_line/` by the name of its file, or one a rule of
    /// `../triggers/of_a_datum/` makes of a datum of the flow. A file names these and its `context`, or one of the forms before them, not two.
    #[serde(default, rename = "triggered_by")]
    pub triggers: Vec<String>,
    /// What the cell does for a trigger, by the trigger's name: each among `triggered_by`.
    #[serde(default, rename = "do")]
    pub does: BTreeMap<String, String>,
    /// The data the cell's session must hold, its context, in the order it names them: each filled in by its kind.
    /// Only an agent cell has a session to fill.
    #[serde(default)]
    pub context: Vec<String>,
    /// The kind the cell is given each datum of its `context` in, by the datum's name: each among `context`, and each
    /// datum of `context` has one.
    #[serde(default)]
    pub kinds: BTreeMap<String, InputKind>,
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
    /// given it. A file names this list, the two, or `triggered_by` and `context`: one of the three forms.
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

/// A cell's task as written: its text in place (`task = '''...'''`), or a Markdown file whose whole text it is
/// (`task = { file = "tasks/better_way.md" }`, from the flow's directory), so that cells told the same are told it from
/// one place.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Task {
    Text(String),
    File(TaskFile),
}

/// A task written in a file of its own.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFile {
    /// The file, from the flow's directory, which it does not leave.
    pub file: String,
}

impl Task {
    /// The file it is written in, where it is not written in place.
    pub fn file(&self) -> Option<&str> {
        match self {
            Task::Text(_) => None,
            Task::File(file) => Some(&file.file),
        }
    }
}

/// A cell's `when` or `requires` as written: a table of one key or more, every one of which holds, in the order
/// written: a block's or a datum's name, with the value it must have (`when = { deep_needed = "yes" }`, `requires = {
/// dependencies = "merged" }`); or [`WHEN_TEST`], a named test that holds (`requires = { test = "settled" }`). So no
/// block is named `test`.
pub type Keys = Texts;

/// The key of a cell's `when` or `requires` that names a named test, which no block's name may be.
pub const WHEN_TEST: &str = "test";

/// Texts by their names, in the order the file writes them: a table of texts, read keeping its order, which this
/// crate's `toml` keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Texts(pub Vec<(String, String)>);

impl Texts {
    /// The text of this name, where there is one.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, text)| text.as_str())
    }

    /// Whether it holds no text.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for Texts {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct InOrder;
        impl<'de> serde::de::Visitor<'de> for InOrder {
            type Value = Texts;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a table of texts")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Texts, A::Error> {
                let mut texts = Vec::new();
                while let Some(entry) = map.next_entry::<String, String>()? {
                    texts.push(entry);
                }
                Ok(Texts(texts))
            }
        }
        deserializer.deserialize_map(InOrder)
    }
}
