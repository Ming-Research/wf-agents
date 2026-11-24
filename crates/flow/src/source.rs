//! The definition of a flow as it is written: the text of its files, and the shape each file must have.
//!
//! A flow is a directory. Laid out one thing to a file, it holds `flow.toml` for what is the flow's as a whole, and one
//! file to each cell, line, block and named test, in `cells/`, `lines/`, `blocks/` (in directories of their own, as
//! deep as is useful) and `tests/`, each named by the thing it defines. What every flow shares is beside the flows,
//! defined once: the engine's own data in `../data/`, which a flow names among those it uses, and those each cell
//! has its own of, made where a cell names them; and the sets of rules for the agents that a line names, in
//! `../agents/rules/rules.toml`. Written
//! before that layout, a flow is `flow.toml` holding its lines, blocks and named tests, one `<cell id>.toml` beside it
//! for each cell, and in `rules/` one `<name>.md` for each set of rules for the agents that a line's system prompt
//! names. A field a file does not know is refused, so that every new kind of field is added on purpose.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use decide::TestKind;
use serde::{Deserialize, Serialize};

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
/// each defined once for every flow, and used by a flow that names it. In its `of_a_cell/` ([`OF_A_CELL`]), each a
/// datum every cell has its own of, its file's `name` holding `{cell}`, the cell's id in small letters: such a datum is
/// made where a cell names it.
pub const DATA_DIR: &str = "../data";
pub const OF_A_CELL: &str = "of_a_cell";

/// The path, from a flow's directory, of the file beside the flows that names the sets of rules for the agents a line
/// may take: [`agents::RULES_FILE`].
pub fn rules_file() -> String {
    format!("../{}", agents::RULES_FILE)
}

/// The path, from a flow's directory, of the file beside the flows that says why a work waits on the owner and why the
/// engine paused a work: [`agents::WAITS_FILE`] in `agents/`.
pub fn waits_file() -> String {
    format!("../{}/{}", agents::AGENTS_DIR, agents::WAITS_FILE)
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
    /// `blocks/.../<name>.toml`, `tests/<name>.toml`, `../data/.../<name>.toml`, [`rules_file`] and [`waits_file`].
    pub files: BTreeMap<String, String>,
}

/// Reads the flow in this directory. Laid out one thing to a file where it has `cells/`: `flow.toml`; each `.toml` file
/// directly in `cells/`, `lines/` and `tests/`; each in `blocks/` and `../data/` or in a directory in them;
/// [`rules_file`] and [`waits_file`]. Else as it was written before: `flow.toml`, every other `.toml` file
/// directly in it as the file of the cell its name gives, every `.md` file directly in its `rules/`, where it has one,
/// as the set of rules its name gives, and [`waits_file`]. Other files and directories are not read.
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
        ] {
            let at = dir.join(sub);
            if at.is_dir() {
                for (path, text) in files(&at, "toml", deep)? {
                    source.add(&format!("{sub}/{path}"), text);
                }
            }
        }
        for beside in [rules_file(), waits_file()] {
            let at = dir.join(&beside);
            if at.is_file() {
                source.add(&beside, fs::read_to_string(at)?);
            }
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
    let waits = dir.join(waits_file());
    if waits.is_file() {
        source.add(&waits_file(), fs::read_to_string(waits)?);
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

    /// The flow's files, each by its path from the flow's directory and its text, in the order of the paths: the files
    /// [`Source::of_files`] takes this flow back from.
    pub fn files(&self) -> Vec<(String, String)> {
        let cell = |id: &str| {
            if self.laid_out {
                format!("{CELLS_DIR}/{id}.toml")
            } else {
                format!("{id}.toml")
            }
        };
        let mut files: Vec<(String, String)> = std::iter::once((FLOW_FILE.to_string(), self.flow.clone()))
            .chain(self.cells.iter().map(|(id, text)| (cell(id), text.clone())))
            .chain((self.rules.iter()).map(|(name, text)| (format!("{RULES_DIR}/{name}.md"), text.clone())))
            .chain(self.files.iter().map(|(path, text)| (path.clone(), text.clone())))
            .collect();
        files.sort();
        files
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
    } else if dir == LINES_DIR || dir == TESTS_DIR || under(BLOCKS_DIR) || under(DATA_DIR) {
        Some(Kept::File)
    } else {
        (path == rules_file() || path == waits_file()).then_some(Kept::File)
    }
}

/// What a file of a flow written before it was laid out one thing to a file is kept as, by its path from the flow's
/// directory: every `.toml` file directly in it but `flow.toml` as a cell's, every `.md` file directly in `rules/` as
/// a set of rules, and [`waits_file`] beside the flows as it is; none where it is none that the flow is made of.
fn written_before(path: &str) -> Option<Kept<'_>> {
    match path.split_once('/') {
        None if path == FLOW_FILE => None,
        None => path.strip_suffix(".toml").map(Kept::Cell),
        Some((RULES_DIR, name)) if !name.contains('/') => name.strip_suffix(".md").map(Kept::Rules),
        Some(_) if path == waits_file() => Some(Kept::File),
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
    /// The groups of the flow's blocks, by name, each with its blocks: a cell watches or reads a group by its name, a
    /// new version of any of its blocks being new to a cell that watches it.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
    /// The cell in whose questions the engine asks what it asks of a work, as in a flow laid out one thing to a file;
    /// none where the flow names none.
    pub engine_asks_in: Option<String>,
    /// The table of earlier ids, as in a flow laid out one thing to a file.
    #[serde(default)]
    pub earlier_ids: Vec<RenumberingFile>,
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
    /// names them: a cell watches or reads a group by its name, a new version of any of its blocks being new to a cell
    /// that watches it.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
    /// What the item's card shows the owner of the work, in this order: data of the flow, each by its name. The page
    /// shows what this names, and names no datum of its own.
    #[serde(default)]
    pub card: Vec<String>,
    /// The cell in whose questions the engine asks what it asks of a work, such as whether a change of a file the
    /// project protects may merge: one that can change the work as the answer asks, and can be spoken to (design
    /// document `data-and-context`, 34). None where the flow names none.
    pub engine_asks_in: Option<String>,
    /// What every agent cell of the flow reads beside what its own file names, given it first.
    #[serde(default)]
    pub every_agent: EveryAgentFile,
    /// The table of earlier ids: each time cells were numbered anew, oldest first (principle `cell-numbering`). Only
    /// added to.
    #[serde(default)]
    pub earlier_ids: Vec<RenumberingFile>,
}

/// A renumbering of the table of earlier ids, as `flow.toml` writes it: each cell whose id changed, by the id it had,
/// with the id it took, and each cell taken out, by the id it had.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenumberingFile {
    #[serde(default)]
    pub ids: BTreeMap<String, String>,
    #[serde(default)]
    pub taken_out: Vec<String>,
}

/// What every agent cell of a flow reads, as its `flow.toml` writes it: each the engine's datum the flow names in
/// `data`, or `{each}_` and the rest of the name of a datum each cell has its own of (`../data/of_a_cell/`), standing
/// for that datum of each cell that has one. A cell watches or reads a datum this names only once, as its own file
/// names it where it does.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EveryAgentFile {
    /// The data every agent cell is given that run nothing, in the order named.
    #[serde(default)]
    pub reads: Vec<String>,
    /// The kind each is given in, by its name as written.
    #[serde(default)]
    pub kinds: BTreeMap<String, InputKind>,
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
    /// The line's agents change only the project's tests, the files `paths` of `[tests]` in the project's declaration
    /// matches: a round of the line is sent back for any other file its run changed. A line that writes anything else
    /// leaves it out.
    #[serde(default)]
    pub tests_only: bool,
    /// How many levels above its item's the line's rounds run at, none above the highest level the installation lets
    /// one round run at; none where it is left out.
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
    /// The line's agents change only the project's tests, the files `paths` of `[tests]` in the project's declaration
    /// matches: a round of the line is sent back for any other file its run changed. A line that writes anything else
    /// leaves it out.
    #[serde(default)]
    pub tests_only: bool,
    /// How many levels above its item's the line's rounds run at, none above the highest level the installation lets
    /// one round run at; none where it is left out.
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
    /// What a cell that writes the block is told of writing it, after the list of what it returns.
    pub words: Option<String>,
    /// What a cell that writes the block is told after those words, where it also has a block it must hand in every
    /// time.
    pub words_if_marked: Option<String>,
    /// The shape a return holds the block to, where it is neither a text nor a choice.
    pub shape: Option<toml::Table>,
    /// Of a block a source of the engine's writes, what it writes: each text by its name, in the order written, a
    /// `{name}` in it a place the engine fills in a run.
    #[serde(default)]
    pub writes: Texts,
    /// Of a block a source outside the work writes, who changes what it writes by their acts, where only one does.
    pub changed_by: Option<ChangedBy>,
    /// Of a choice, by a value of it, the other blocks a return that writes it with that value may hold, and no more
    /// of what its cell may write; none where it says none.
    #[serde(default)]
    pub with_nothing_but: BTreeMap<String, Vec<String>>,
    /// A return that writes it is sent back while a question of the work marked `tree` has the owner's answer to its
    /// latest version and the work's change against main changes nothing in the project's design tree.
    #[serde(default)]
    pub answers_in_the_tree: bool,
    /// Its lines name entries of the project's design tree, one to a line, as the tree's format (`agents/tree.toml`)
    /// refers to one: a return that writes it is sent back for a line that names none the tree on the work's branch
    /// holds, and a round is given each entry named whole in place of the names, as the tree's latest version has it.
    #[serde(default)]
    pub names_tree_entries: bool,
}

/// Who changes what a source outside the work writes of a block, by their acts: the owner, as the record of the
/// questions changes only as the owner answers them, so that a precondition that reads the block waits on the owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedBy {
    Owner,
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
    /// What a cell that writes the block is told of writing it, after the list of what it returns.
    pub words: Option<String>,
    /// What a cell that writes the block is told after those words, where it also has a block it must hand in every
    /// time.
    pub words_if_marked: Option<String>,
    /// The shape a return holds the block to, where it is neither a text nor a choice.
    pub shape: Option<toml::Table>,
    /// Of a block a source of the engine's writes, what it writes: each text by its name, in the order written, a
    /// `{name}` in it a place the engine fills in a run. The words are the flow's, so the code holds none of them.
    #[serde(default)]
    pub writes: Texts,
    /// Of a block a source outside the work writes, who changes what it writes by their acts, where only one does: the
    /// owner, so that a precondition that reads it waits on the owner.
    pub changed_by: Option<ChangedBy>,
    /// Of a choice, by a value of it, the other blocks a return that writes it with that value may hold, and no more
    /// of what its cell may write, its questions and its words among them; none where it says none.
    #[serde(default)]
    pub with_nothing_but: BTreeMap<String, Vec<String>>,
    /// A return that writes it is sent back while a question of the work marked `tree` has the owner's answer to its
    /// latest version and the work's change against main changes nothing in the project's design tree.
    #[serde(default)]
    pub answers_in_the_tree: bool,
    /// Its lines name entries of the project's design tree, one to a line, as the tree's format (`agents/tree.toml`)
    /// refers to one: a return that writes it is sent back for a line that names none the tree on the work's branch
    /// holds, and a round is given each entry named whole in place of the names, as the tree's latest version has it.
    #[serde(default)]
    pub names_tree_entries: bool,
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
            words: block.words,
            words_if_marked: block.words_if_marked,
            shape: block.shape,
            writes: block.writes,
            changed_by: block.changed_by,
            with_nothing_but: block.with_nothing_but,
            answers_in_the_tree: block.answers_in_the_tree,
            names_tree_entries: block.names_tree_entries,
        }
    }
}

/// A named test as written: a test of the blocks of the work itself.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestFile {
    /// The words shown for the test.
    pub says: String,
    /// How the blocks it reads make it hold: `all`, `any`, `none`, `none_standing` or `any_content`; another is
    /// refused.
    pub kind: TestKind,
    /// The blocks it reads: each a choice, or, of a test of content, any block.
    pub reads: Vec<String>,
    /// The value it looks for in each block it reads; none of a test of content, which looks for no value.
    pub is: Option<String>,
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
    /// What runs the cell, by its data: each datum, or group of the flow's blocks, whose version new to the cell runs
    /// it, by its name, in the order it names them (design document `data-and-context`, 5, 12).
    #[serde(default)]
    pub watches: Vec<String>,
    /// The data the cell is given that run nothing, each a datum or a group by its name, in the order it names them, none
    /// it watches. Only an agent cell is given data.
    #[serde(default)]
    pub reads: Vec<String>,
    /// What the cell does when a datum or group it watches is new, by its name: each among `watches`.
    #[serde(default, rename = "do")]
    pub does: BTreeMap<String, String>,
    /// The kind the cell is given each block it watches or reads in, by the block's name, a group's each by its own:
    /// what its session must hold of it.
    #[serde(default)]
    pub kinds: BTreeMap<String, InputKind>,
    /// The blocks the cell may write.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// The blocks among its outputs that every hand-in of the cell includes.
    #[serde(default)]
    pub must_write: Vec<String>,
    /// The blocks that say what the work is where the host of the project's repository shows the work, as the body of
    /// its pull request, whether or not they run the cell; an engine cell's act reads them.
    #[serde(default)]
    pub describes: Vec<String>,
    /// The block whose first line names the work's merge where the host shows it, as the title of its pull request
    /// and of the commit the merge makes; the act of an engine cell that merges reads it.
    pub named_by: Option<String>,
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

/// What the context of a cell's session must hold of a block it is given when the cell runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    /// Only the latest version.
    Latest,
    /// The version the cell last worked from and the latest, so that the agent sees what changed between them.
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

/// Where the flow's definition gives a cell a datum, as the `context` part of the message orders what a round is given
/// (design document `data-and-context`, 14, 15, 21).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Listed {
    /// What the flow gives every agent cell (`[every_agent]` of `flow.toml`).
    EveryAgent,
    /// What the cell only reads.
    Reads,
    /// What the cell watches.
    Watches,
    /// What the cell writes and is given by nothing else: its latest version, where the session lacks it, so that the
    /// cell brings its outputs up to date from it.
    Outputs,
}

impl Listed {
    /// Every list, as the files write them.
    pub const ALL: [Listed; 4] = [Listed::EveryAgent, Listed::Reads, Listed::Watches, Listed::Outputs];

    /// The list's name, as it is written.
    pub fn name(self) -> &'static str {
        match self {
            Listed::EveryAgent => "every_agent",
            Listed::Reads => "reads",
            Listed::Watches => "watches",
            Listed::Outputs => "outputs",
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
