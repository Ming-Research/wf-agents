//! The definition checked and compiled: the flow the deciding code walks, and what the rest of the engine needs to
//! know of each cell and each block.
//!
//! Every fault a definition has is found at once and given as data, with the file and the field it is in, so that the
//! one who wrote the definition can mend all of them before compiling again.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Cell, CellId, Condition, Flow, FlowFault, Outsiders, Test, TestKind, TestName};

use crate::source::{BlockFile, CellFile, Effort, FLOW_FILE, FlowFile, Kind, Source, When, WhenIs, WhenTest};

/// A flow's definition compiled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compiled {
    /// The flow's name.
    pub name: String,
    /// The cells as the deciding code walks them, with the named tests: what each tests.
    pub flow: Flow,
    /// What the engine needs to know of each cell beyond what the walk reads, in the order of the walk.
    pub cells: Vec<CellSpec>,
    /// Every block of a work's state, with who writes and who reads it.
    pub blocks: BTreeMap<BlockId, Block>,
    /// The words shown for each named test the cells use, by name; what each tests is in `flow`.
    pub tests: BTreeMap<TestName, String>,
    /// The limits of the flow as a whole.
    pub limits: Limits,
}

/// The limits of everything that repeats, for a cell that sets none of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// How many runs of a cell in a row may be ways round before the work waits for the owner.
    pub runs: u32,
    /// How often a return may be sent back within one run.
    pub returns: u32,
    /// How many calls one run may make.
    pub calls: u32,
}

/// What the engine that runs a flow has, by name. A definition that names another source or another act is refused.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Has {
    /// The sources outside a work that write blocks.
    pub sources: BTreeSet<String>,
    /// The acts of the engine's own that carry out an engine cell.
    pub acts: BTreeSet<String>,
}

/// What the engine needs to know of a cell beyond what the walk reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellSpec {
    pub id: CellId,
    /// What the cell is called on the chart.
    pub title: String,
    /// Who carries the cell out.
    pub kind: Kind,
    /// The line whose session an agent cell runs in; an engine cell has none.
    pub line: Option<String>,
    /// The agents of the cell's line only read, as the line says; an engine cell's do not.
    pub reads_only: bool,
    /// What an agent cell's agent is told to do; an engine cell has none.
    pub task: Option<String>,
    /// The act of the engine's own that carries out an engine cell; an agent cell has none.
    pub act: Option<String>,
    /// The blocks among its outputs that every hand-in of the cell includes, in the order written: a hand-in that lacks
    /// one is sent back.
    pub must_write: Vec<BlockId>,
    /// How often a return of the cell may be sent back within one run.
    pub returns: u32,
    /// How many calls one run of the cell may make.
    pub calls: u32,
    /// The reasoning effort an agent cell's rounds run at; an engine cell has none.
    pub effort: Option<Effort>,
    /// How many levels above its item's the cell's rounds run at, as its line says; none for an engine cell.
    pub levels_above: u32,
}

/// A block of a work's state, as the engine needs to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The values of a block that is a choice; a block with none is not one.
    pub values: Option<Vec<String>>,
    /// Who writes it.
    pub writers: Writers,
    /// Who reads it.
    pub readers: Readers,
}

/// Who writes a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Writers {
    /// The cells that have it among their outputs, in the order of the walk.
    pub cells: Vec<CellId>,
    /// People write it.
    pub people: bool,
    /// What people write in it is a note: their words to the cells that read it, which a paused work still answers.
    pub note: bool,
    /// The source outside the work that writes it, where one does.
    pub outside: Option<String>,
}

/// Who reads a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readers {
    /// The cells that have it among their inputs, in the order of the walk.
    pub cells: Vec<CellId>,
    /// The cells whose condition reads its value, in the order of the walk.
    pub conditions: Vec<CellId>,
    /// The named tests that read its value and that a cell uses, by name.
    pub tests: Vec<TestName>,
    /// The owner's page reads it.
    pub shown: bool,
}

/// Something wrong with a definition, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The file it is in: `flow.toml` or `<cell id>.toml`.
    pub file: String,
    /// The path of the field at fault within the file, such as `inputs[2]` or `blocks.plan.values`; empty when the
    /// whole file is.
    pub field: String,
    /// What is wrong there.
    pub what: FaultKind,
}

/// What can be wrong with a definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaultKind {
    /// The file is not TOML, or does not have the shape its kind of file must have; the parser's words say how.
    Unreadable { message: String },
    /// The order names no cell.
    OrderEmpty,
    /// The order names a cell that has no file.
    NoCellFile { cell: String },
    /// A cell's file is not named in the order.
    NotInOrder { cell: String },
    /// The name is given a second time in the same list.
    Twice { name: String },
    /// A limit is not a whole number from one to the largest count the engine keeps.
    LimitOutOfRange { value: i64 },
    /// An agent cell names no line to run in.
    NoLine,
    /// An engine cell names a line: it runs in no session.
    EngineHasLine { line: String },
    /// The line is not declared in `flow.toml`.
    NoSuchLine { line: String },
    /// An agent cell has no task, or one of no words: its agent would be told nothing to do.
    NoTask,
    /// An engine cell has a task: no agent reads it.
    EngineHasTask,
    /// An engine cell names no act to carry it out.
    NoAct,
    /// An agent cell names an act: an agent carries the cell out.
    AgentHasAct { act: String },
    /// An engine cell names a reasoning effort: no agent runs it.
    EngineHasEffort,
    /// An agent cell names no reasoning effort: how hard its rounds think is the flow's to say.
    NoEffort,
    /// The cell names an act the engine does not have.
    UnknownAct { act: String },
    /// A choice has no values to choose from.
    ValuesEmpty,
    /// The block is a note, and people do not write it: a note is their words to the cells that read it.
    NoteNotByPeople { block: String },
    /// The block is a note, and a choice: a note is words to a cell, which a choice is not.
    NoteIsAChoice { block: String },
    /// The block is not declared in `flow.toml`.
    NoSuchBlock { block: String },
    /// The cell must write the block each time, and it is not among its outputs.
    NotAnOutput { block: String },
    /// The block has no writer: no cell of the order, no person and nothing outside writes it.
    NoWriter { block: String },
    /// The block has no reader: no cell of the order reads it, no condition and no named test reads its value, and
    /// the owner's page does not show it.
    NoReader { block: String },
    /// The block names a source outside the work that the engine does not have.
    UnknownSource { block: String, source: String },
    /// A condition or a named test reads a block that is not a choice.
    NotAChoice { block: String },
    /// A condition or a named test waits for a value the choice does not offer.
    NotAValue { block: String, is: String },
    /// A condition reads a block its own cell writes: the cell would decide by what it writes itself whether it runs.
    ConditionOnOwnOutput { block: String },
    /// A condition reads a block that neither a cell before it, people nor the outside write, so that it is decided
    /// by nothing that comes before the cell.
    ConditionNotWrittenBefore { block: String },
    /// The last cell has a condition: a work ends when its last cell has a record, so the last cell always runs.
    LastHasCondition,
    /// A cell uses a test that `flow.toml` does not declare.
    UndeclaredTest { test: String },
    /// A declared test reads no block.
    ReadsEmpty { test: String },
    /// A declared test is used by no cell.
    UnusedTest { test: String },
    /// A cell's condition or precondition is a test that reads a block the cell itself writes: the cell would decide by
    /// what it writes itself whether it runs, or whether the work waits before it.
    TestOnOwnOutput { test: String, block: String },
    /// A cell's condition or precondition is a test that reads a block that neither a cell before it, people nor the
    /// outside write, so that it is decided by nothing that comes before the cell.
    TestNotWrittenBefore { test: String, block: String },
    /// A cell's condition or precondition is a test of what stands that reads a block this cell, after it in the
    /// order, writes. Whether a version stands is whether its writer is skipped, which the walk decides from the place
    /// of the cell asking: the versions such a test reads are to be written before that cell.
    TestWrittenAfter { test: String, block: String, cell: String },
    /// A test of what stands reads a block that this cell writes as well as another before it. Such a test reads the
    /// current version alone, and a skipped cell's write would hide the other's word.
    StandingWrittenByTwo { test: String, block: String, cell: String },
    /// The deciding code refuses the cells as a flow.
    Flow(FlowFault),
}

/// Checks a flow's definition against what the engine has, by name, and compiles it; or gives every fault it has, in
/// the order of the files (`flow.toml`, then the cells in the order of the walk, then any other) and, within a file, in
/// the order they were found. When a file cannot be read, only the files that cannot be read are reported.
pub fn compile(source: &Source, has: &Has) -> Result<Compiled, Vec<Fault>> {
    let (flow_file, cell_files) = parse(source)?;
    let def = Definition::new(&flow_file, &cell_files);
    let mut faults = Vec::new();

    def.check_order(&mut faults);
    let mut limit = |field: &str, value: i64| {
        count(value).unwrap_or_else(|what| {
            faults.push(fault(FLOW_FILE, field, what));
            0
        })
    };
    let limits = Limits {
        runs: limit("limits.runs", flow_file.limits.runs),
        returns: limit("limits.returns", flow_file.limits.returns),
        calls: limit("limits.calls", flow_file.limits.calls),
    };
    def.check_blocks(&has.sources, &mut faults);
    def.check_tests(&mut faults);

    let mut cells = Vec::new();
    let mut specs = Vec::new();
    for (place, id, file) in def.ordered() {
        let (cell, spec) = def.cell(place, id, file, &limits, &has.acts, &mut faults);
        cells.push(cell);
        specs.push(spec);
    }

    if !faults.is_empty() {
        sort(&mut faults, Some(&def.place));
        return Err(faults);
    }
    let tests = (flow_file.tests.iter())
        .map(|(name, test)| {
            let test = Test {
                kind: test.kind,
                reads: test.reads.iter().map(|b| BlockId(b.clone())).collect(),
                is: test.is.clone(),
            };
            (TestName(name.clone()), test)
        })
        .collect();
    let outsiders = (flow_file.blocks.iter())
        .filter(|(_, b)| b.people || b.outside.is_some())
        .map(|(name, b)| {
            let outsiders = Outsiders {
                people: b.people,
                note: b.note,
                outside: b.outside.clone(),
            };
            (BlockId(name.clone()), outsiders)
        })
        .collect();
    let flow = Flow::new(cells, tests, outsiders).map_err(|f| vec![fault(FLOW_FILE, "order", FaultKind::Flow(f))])?;
    Ok(Compiled {
        name: flow_file.name.clone(),
        flow,
        cells: specs,
        blocks: def.blocks(),
        tests: (flow_file.tests.iter())
            .map(|(name, test)| (TestName(name.clone()), test.says.clone()))
            .collect(),
        limits,
    })
}

/// Reads each file into its shape, or gives the faults of every file that does not fit.
fn parse(source: &Source) -> Result<(FlowFile, BTreeMap<String, CellFile>), Vec<Fault>> {
    let mut faults = Vec::new();
    let flow = toml::from_str::<FlowFile>(&source.flow)
        .map_err(|e| faults.push(unreadable(FLOW_FILE.to_string(), &e)))
        .ok();
    let mut cells = BTreeMap::new();
    for (id, text) in &source.cells {
        match toml::from_str::<CellFile>(text) {
            Ok(cell) => {
                cells.insert(id.clone(), cell);
            }
            Err(e) => faults.push(unreadable(cell_file(id), &e)),
        }
    }
    match flow {
        Some(flow) if faults.is_empty() => Ok((flow, cells)),
        flow => {
            let place = flow.as_ref().map(|f| first_places(&f.order, &source.cells));
            sort(&mut faults, place.as_ref());
            Err(faults)
        }
    }
}

fn unreadable(file: String, error: &toml::de::Error) -> Fault {
    Fault {
        file,
        field: String::new(),
        what: FaultKind::Unreadable {
            message: error.to_string(),
        },
    }
}

fn fault(file: &str, field: impl Into<String>, what: FaultKind) -> Fault {
    Fault {
        file: file.to_string(),
        field: field.into(),
        what,
    }
}

fn cell_file(id: &str) -> String {
    format!("{id}.toml")
}

/// The limit as a count, or why it is none.
fn count(value: i64) -> Result<u32, FaultKind> {
    match u32::try_from(value) {
        Ok(count) if count > 0 => Ok(count),
        _ => Err(FaultKind::LimitOutOfRange { value }),
    }
}

/// Where each cell first stands in the order, for the cells that have a file.
fn first_places<'a, T>(order: &'a [String], cells: &BTreeMap<String, T>) -> BTreeMap<&'a str, usize> {
    let mut place = BTreeMap::new();
    for (i, id) in order.iter().enumerate() {
        if cells.contains_key(id) {
            place.entry(id.as_str()).or_insert(i);
        }
    }
    place
}

/// Puts the faults in the order of their files, keeping the order they were found in within a file.
fn sort(faults: &mut [Fault], place: Option<&BTreeMap<&str, usize>>) {
    faults.sort_by_cached_key(|f| {
        if f.file == FLOW_FILE {
            return (0, 0, String::new());
        }
        let id = f.file.strip_suffix(".toml").unwrap_or(&f.file);
        match place.and_then(|p| p.get(id)) {
            Some(&i) => (1, i, String::new()),
            None => (2, 0, f.file.clone()),
        }
    });
}

/// The faults of a list of names: each name given twice is one, and each other name that `check` refuses.
fn check_list(
    names: &[String],
    list: &str,
    mut check: impl FnMut(&str) -> Option<FaultKind>,
) -> Vec<(String, FaultKind)> {
    let mut faults = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let what = if names[..i].contains(name) {
            Some(FaultKind::Twice { name: name.clone() })
        } else {
            check(name)
        };
        faults.extend(what.map(|what| (format!("{list}[{i}]"), what)));
    }
    faults
}

/// The cells of the order that write and read one block, each list in the order of the walk, and the tests that read
/// it and that a cell of the order uses, by name.
#[derive(Default)]
struct Uses<'a> {
    writers: Vec<(usize, &'a str)>,
    readers: Vec<&'a str>,
    conditions: Vec<&'a str>,
    tests: Vec<&'a str>,
}

/// Why a block a cell's condition or precondition reads is not decided before the cell.
enum NotBefore<'a> {
    /// The cell itself writes it.
    OwnOutput,
    /// This cell, after the cell in the order, writes it.
    After(&'a str),
    /// Nothing before the cell writes it: only cells after it do.
    Later,
}

/// The parsed files, with what the checks look up in them.
struct Definition<'a> {
    flow: &'a FlowFile,
    cells: &'a BTreeMap<String, CellFile>,
    /// Where each cell of the order that has a file first stands.
    place: BTreeMap<&'a str, usize>,
    /// Who of the order writes and reads each block that is declared.
    uses: BTreeMap<&'a str, Uses<'a>>,
    /// The tests a cell of the order uses, as its condition or a precondition.
    used: BTreeSet<&'a str>,
}

impl<'a> Definition<'a> {
    fn new(flow: &'a FlowFile, cells: &'a BTreeMap<String, CellFile>) -> Self {
        let mut def = Self {
            flow,
            cells,
            place: first_places(&flow.order, cells),
            uses: BTreeMap::new(),
            used: BTreeSet::new(),
        };
        let mut uses: BTreeMap<&'a str, Uses<'a>> = flow.blocks.keys().map(|b| (b.as_str(), Uses::default())).collect();
        for (place, id, file) in def.ordered() {
            for block in &file.outputs {
                if let Some(u) = uses.get_mut(block.as_str())
                    && !u.writers.contains(&(place, id))
                {
                    u.writers.push((place, id));
                }
            }
            for block in &file.inputs {
                if let Some(u) = uses.get_mut(block.as_str())
                    && !u.readers.contains(&id)
                {
                    u.readers.push(id);
                }
            }
            if let Some(When::Is(WhenIs { block, .. })) = &file.when
                && let Some(u) = uses.get_mut(block.as_str())
            {
                u.conditions.push(id);
            }
        }
        let used: BTreeSet<&'a str> = (def.ordered())
            .flat_map(|(_, _, file)| {
                let when = match &file.when {
                    Some(When::Test(WhenTest { test })) => Some(test.as_str()),
                    _ => None,
                };
                file.requires.iter().map(String::as_str).chain(when)
            })
            .collect();
        for (test, declared) in flow.tests.iter().filter(|(t, _)| used.contains(t.as_str())) {
            for block in &declared.reads {
                if let Some(u) = uses.get_mut(block.as_str())
                    && !u.tests.contains(&test.as_str())
                {
                    u.tests.push(test);
                }
            }
        }
        def.uses = uses;
        def.used = used;
        def
    }

    /// The cells of the order that have a file, each once, at its first place.
    fn ordered(&self) -> impl Iterator<Item = (usize, &'a str, &'a CellFile)> {
        self.flow.order.iter().enumerate().filter_map(|(i, id)| {
            let file = self.cells.get(id)?;
            (self.place.get(id.as_str()) == Some(&i)).then_some((i, id.as_str(), file))
        })
    }

    /// Whether a cell of the order, a person or the outside writes the block.
    fn written(&self, block: &str) -> bool {
        let declared = &self.flow.blocks[block];
        declared.people || declared.outside.is_some() || !self.uses[block].writers.is_empty()
    }

    /// Whether a cell of the order, a condition, a named test or the owner's page reads the block.
    fn read(&self, block: &str) -> bool {
        let uses = &self.uses[block];
        self.flow.blocks[block].shown
            || !uses.readers.is_empty()
            || !uses.conditions.is_empty()
            || !uses.tests.is_empty()
    }

    /// The faults of the declared blocks: values that make no choice, a source the engine does not have, a block with
    /// no writer, and one with no reader.
    fn check_blocks(&self, sources: &BTreeSet<String>, faults: &mut Vec<Fault>) {
        for (name, block) in &self.flow.blocks {
            if let Some(values) = &block.values {
                let field = format!("blocks.{name}.values");
                if values.is_empty() {
                    faults.push(fault(FLOW_FILE, field.clone(), FaultKind::ValuesEmpty));
                }
                for (field, what) in check_list(values, &field, |_| None) {
                    faults.push(fault(FLOW_FILE, field, what));
                }
            }
            if block.note && !block.people {
                let what = FaultKind::NoteNotByPeople { block: name.clone() };
                faults.push(fault(FLOW_FILE, format!("blocks.{name}.note"), what));
            } else if block.note && block.values.is_some() {
                let what = FaultKind::NoteIsAChoice { block: name.clone() };
                faults.push(fault(FLOW_FILE, format!("blocks.{name}.note"), what));
            }
            if let Some(source) = &block.outside
                && !sources.contains(source)
            {
                let what = FaultKind::UnknownSource {
                    block: name.clone(),
                    source: source.clone(),
                };
                faults.push(fault(FLOW_FILE, format!("blocks.{name}.outside"), what));
            }
            if !self.written(name) {
                let what = FaultKind::NoWriter { block: name.clone() };
                faults.push(fault(FLOW_FILE, format!("blocks.{name}"), what));
            }
            if !self.read(name) {
                let what = FaultKind::NoReader { block: name.clone() };
                faults.push(fault(FLOW_FILE, format!("blocks.{name}"), what));
            }
        }
    }

    /// The faults of the declared tests: one that reads no block; a block read that is not declared, is no choice or
    /// does not offer the value looked for; and a test that no cell of the order uses.
    fn check_tests(&self, faults: &mut Vec<Fault>) {
        for (test, declared) in &self.flow.tests {
            let field = format!("tests.{test}.reads");
            if declared.reads.is_empty() {
                let what = FaultKind::ReadsEmpty { test: test.clone() };
                faults.push(fault(FLOW_FILE, field.clone(), what));
            }
            for (field, what) in check_list(&declared.reads, &field, |block| self.check_read(block, &declared.is)) {
                faults.push(fault(FLOW_FILE, field, what));
            }
            if declared.kind == TestKind::NoneStanding {
                for (i, block) in declared.reads.iter().enumerate() {
                    let second = (self.uses.get(block.as_str())).and_then(|u| u.writers.get(1));
                    if let Some(&(_, cell)) = second.filter(|_| !declared.reads[..i].contains(block)) {
                        let what = FaultKind::StandingWrittenByTwo {
                            test: test.clone(),
                            block: block.clone(),
                            cell: cell.to_string(),
                        };
                        faults.push(fault(FLOW_FILE, format!("{field}[{i}]"), what));
                    }
                }
            }
            if !self.used.contains(test.as_str()) {
                let what = FaultKind::UnusedTest { test: test.clone() };
                faults.push(fault(FLOW_FILE, format!("tests.{test}"), what));
            }
        }
    }

    /// The fault of a block a named test reads, looking for this value: not declared, not a choice, or not offering
    /// the value.
    fn check_read(&self, block: &str, is: &str) -> Option<FaultKind> {
        let name = block.to_string();
        match self.flow.blocks.get(block).map(|b| &b.values) {
            None => Some(FaultKind::NoSuchBlock { block: name }),
            Some(None) => Some(FaultKind::NotAChoice { block: name }),
            Some(Some(values)) if !values.iter().any(|v| v == is) => Some(FaultKind::NotAValue {
                block: name,
                is: is.to_string(),
            }),
            Some(Some(_)) => None,
        }
    }

    /// The faults of the order: empty, naming a cell twice or one with no file, or leaving out a cell's file.
    fn check_order(&self, faults: &mut Vec<Fault>) {
        if self.flow.order.is_empty() {
            faults.push(fault(FLOW_FILE, "order", FaultKind::OrderEmpty));
        }
        for (i, id) in self.flow.order.iter().enumerate() {
            let field = format!("order[{i}]");
            if self.flow.order[..i].contains(id) {
                faults.push(fault(FLOW_FILE, field, FaultKind::Twice { name: id.clone() }));
            } else if !self.cells.contains_key(id) {
                faults.push(fault(FLOW_FILE, field, FaultKind::NoCellFile { cell: id.clone() }));
            }
        }
        for id in self.cells.keys() {
            if !self.flow.order.contains(id) {
                faults.push(fault(&cell_file(id), "", FaultKind::NotInOrder { cell: id.clone() }));
            }
        }
    }

    /// Checks one cell of the order and compiles it, as the walk reads it and as the rest of the engine does.
    fn cell(
        &self,
        place: usize,
        id: &str,
        file: &CellFile,
        limits: &Limits,
        acts: &BTreeSet<String>,
        faults: &mut Vec<Fault>,
    ) -> (Cell, CellSpec) {
        let name = cell_file(id);
        let mut found = Vec::new();

        match (file.kind, &file.line) {
            (Kind::Agent, None) => found.push(("line".to_string(), FaultKind::NoLine)),
            (Kind::Agent, Some(line)) if !self.flow.lines.contains_key(line) => {
                found.push(("line".to_string(), FaultKind::NoSuchLine { line: line.clone() }));
            }
            (Kind::Engine, Some(line)) => {
                found.push(("line".to_string(), FaultKind::EngineHasLine { line: line.clone() }));
            }
            _ => {}
        }
        match (file.kind, &file.task) {
            (Kind::Agent, None) => found.push(("task".to_string(), FaultKind::NoTask)),
            (Kind::Agent, Some(task)) if task.trim().is_empty() => {
                found.push(("task".to_string(), FaultKind::NoTask));
            }
            (Kind::Engine, Some(_)) => found.push(("task".to_string(), FaultKind::EngineHasTask)),
            _ => {}
        }
        match (file.kind, &file.act) {
            (Kind::Engine, None) => found.push(("act".to_string(), FaultKind::NoAct)),
            (Kind::Engine, Some(act)) if !acts.contains(act) => {
                found.push(("act".to_string(), FaultKind::UnknownAct { act: act.clone() }));
            }
            (Kind::Agent, Some(act)) => {
                found.push(("act".to_string(), FaultKind::AgentHasAct { act: act.clone() }));
            }
            _ => {}
        }
        match (file.kind, file.effort) {
            (Kind::Engine, Some(_)) => found.push(("effort".to_string(), FaultKind::EngineHasEffort)),
            (Kind::Agent, None) => found.push(("effort".to_string(), FaultKind::NoEffort)),
            _ => {}
        }
        let runs = match file.limit.map(count) {
            None => limits.runs,
            Some(Ok(runs)) => runs,
            Some(Err(what)) => {
                found.push(("limit".to_string(), what));
                0
            }
        };

        let applies = file.when.as_ref().map(|when| {
            if place + 1 == self.flow.order.len() {
                found.push(("when".to_string(), FaultKind::LastHasCondition));
            }
            match when {
                When::Is(WhenIs { block, is }) => {
                    found.extend(self.check_is(place, file, block, is));
                    Condition::Is {
                        block: BlockId(block.clone()),
                        value: is.clone(),
                    }
                }
                When::Test(WhenTest { test }) => {
                    if self.flow.tests.contains_key(test) {
                        found.extend(self.check_use(place, file, test, "when.test"));
                    } else {
                        let what = FaultKind::UndeclaredTest { test: test.clone() };
                        found.push(("when.test".to_string(), what));
                    }
                    Condition::Test(TestName(test.clone()))
                }
            }
        });

        found.extend(check_list(&file.requires, "requires", |test| {
            let what = FaultKind::UndeclaredTest { test: test.to_string() };
            (!self.flow.tests.contains_key(test)).then_some(what)
        }));
        for (i, test) in file.requires.iter().enumerate() {
            if !file.requires[..i].contains(test) && self.flow.tests.contains_key(test) {
                found.extend(self.check_use(place, file, test, &format!("requires[{i}]")));
            }
        }
        let declared = |block: &str| {
            let what = FaultKind::NoSuchBlock {
                block: block.to_string(),
            };
            (!self.flow.blocks.contains_key(block)).then_some(what)
        };
        found.extend(check_list(&file.inputs, "inputs", declared));
        found.extend(check_list(&file.outputs, "outputs", declared));
        // Whether a block is declared is said once, where the outputs name it.
        found.extend(check_list(&file.must_write, "must_write", |block| {
            let what = FaultKind::NotAnOutput {
                block: block.to_string(),
            };
            (!file.outputs.iter().any(|o| o == block)).then_some(what)
        }));
        faults.extend(found.into_iter().map(|(field, what)| fault(&name, field, what)));

        let cell = Cell {
            id: CellId(id.to_string()),
            applies,
            requires: file.requires.iter().map(|t| TestName(t.clone())).collect(),
            inputs: file.inputs.iter().map(|b| BlockId(b.clone())).collect(),
            outputs: file.outputs.iter().map(|b| BlockId(b.clone())).collect(),
            limit: runs,
            slot: file.kind == Kind::Agent,
        };
        let spec = CellSpec {
            id: CellId(id.to_string()),
            title: file.title.clone(),
            kind: file.kind,
            line: file.line.clone(),
            reads_only: (file.line.as_ref())
                .is_some_and(|line| self.flow.lines.get(line).is_some_and(|l| l.reads_only)),
            task: file.task.clone(),
            act: file.act.clone(),
            must_write: file.must_write.iter().map(|b| BlockId(b.clone())).collect(),
            returns: limits.returns,
            calls: limits.calls,
            effort: file.effort,
            levels_above: (file.line.as_ref())
                .and_then(|line| self.flow.lines.get(line))
                .map_or(0, |line| line.levels_above),
        };
        (cell, spec)
    }

    /// The faults of a condition on the value of a block, for the cell of this file at this place, each with its
    /// field.
    fn check_is(&self, place: usize, file: &CellFile, block: &str, is: &str) -> Vec<(String, FaultKind)> {
        let field = || "when.block".to_string();
        let name = block.to_string();
        let Some(declared) = self.flow.blocks.get(block) else {
            return vec![(field(), FaultKind::NoSuchBlock { block: name })];
        };
        let mut found = Vec::new();
        match &declared.values {
            None => found.push((field(), FaultKind::NotAChoice { block: name.clone() })),
            Some(values) if !values.iter().any(|v| v == is) => {
                let what = FaultKind::NotAValue {
                    block: name.clone(),
                    is: is.to_string(),
                };
                found.push(("when.is".to_string(), what));
            }
            Some(_) => {}
        }
        match self.not_before(place, file, block, declared, true) {
            Some(NotBefore::OwnOutput) => found.push((field(), FaultKind::ConditionOnOwnOutput { block: name })),
            Some(NotBefore::Later | NotBefore::After(_)) => {
                found.push((field(), FaultKind::ConditionNotWrittenBefore { block: name }));
            }
            None => {}
        }
        found
    }

    /// Why the block, which the condition or a precondition of the cell of this file at this place reads, is not
    /// decided before the cell; `None` where it is. A block the cell itself writes is not; nor is one that neither a
    /// cell before it, people nor the outside write. What people or the outside write can be written before any cell
    /// runs. A block a cell after it writes as well is, where `later_writes_allowed`: that cell's write decides again
    /// whether this one runs, a way back. A test of what stands allows none.
    fn not_before(
        &self,
        place: usize,
        file: &CellFile,
        block: &str,
        declared: &BlockFile,
        later_writes_allowed: bool,
    ) -> Option<NotBefore<'a>> {
        let later = self.uses[block].writers.iter().find(|&&(p, _)| p > place);
        if file.outputs.iter().any(|o| o == block) {
            Some(NotBefore::OwnOutput)
        } else if let Some(&(_, cell)) = later.filter(|_| !later_writes_allowed) {
            Some(NotBefore::After(cell))
        } else if !declared.people
            && declared.outside.is_none()
            && !self.uses[block].writers.iter().any(|&(p, _)| p < place)
        {
            Some(NotBefore::Later)
        } else {
            None
        }
    }

    /// The faults of a declared test used by the cell of this file at this place, as its condition or a precondition,
    /// each at the field that names the test, by [`Self::not_before`]. A block the test reads that is not
    /// declared is the test's fault, not the cell's.
    fn check_use(&self, place: usize, file: &CellFile, test: &str, field: &str) -> Vec<(String, FaultKind)> {
        let mut found = Vec::new();
        let declared_test = &self.flow.tests[test];
        let later_writes_allowed = declared_test.kind != TestKind::NoneStanding;
        let reads = &declared_test.reads;
        for (i, block) in reads.iter().enumerate() {
            let Some(declared) = self.flow.blocks.get(block).filter(|_| !reads[..i].contains(block)) else {
                continue;
            };
            let (test, block) = (test.to_string(), block.clone());
            let what = match self.not_before(place, file, &block, declared, later_writes_allowed) {
                Some(NotBefore::OwnOutput) => FaultKind::TestOnOwnOutput { test, block },
                Some(NotBefore::After(cell)) => FaultKind::TestWrittenAfter {
                    test,
                    block,
                    cell: cell.to_string(),
                },
                Some(NotBefore::Later) => FaultKind::TestNotWrittenBefore { test, block },
                None => continue,
            };
            found.push((field.to_string(), what));
        }
        found
    }

    /// Every block, with its values and who writes and reads it.
    fn blocks(&self) -> BTreeMap<BlockId, Block> {
        let ids = |cells: &[&str]| cells.iter().map(|c| CellId(c.to_string())).collect();
        self.flow
            .blocks
            .iter()
            .map(|(name, declared)| {
                let uses = &self.uses[name.as_str()];
                let writers: Vec<&str> = uses.writers.iter().map(|&(_, id)| id).collect();
                let block = Block {
                    values: declared.values.clone(),
                    writers: Writers {
                        cells: ids(&writers),
                        people: declared.people,
                        note: declared.note,
                        outside: declared.outside.clone(),
                    },
                    readers: Readers {
                        cells: ids(&uses.readers),
                        conditions: ids(&uses.conditions),
                        tests: uses.tests.iter().map(|t| TestName(t.to_string())).collect(),
                        shown: declared.shown,
                    },
                };
                (BlockId(name.clone()), block)
            })
            .collect()
    }
}
