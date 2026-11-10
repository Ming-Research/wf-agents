//! The definition checked and compiled: the flow the deciding code walks, and what the rest of the engine needs to
//! know of each cell and each block.
//!
//! Every fault a definition has is found at once and given as data, with the file and the field it is in, so that the
//! one who wrote the definition can mend all of them before compiling again.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Cell, CellId, Condition, Flow, FlowFault, TestName};

use crate::source::{CellFile, FLOW_FILE, FlowFile, Kind, Source, When, WhenIs, WhenTest};

/// A flow's definition compiled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compiled {
    /// The flow's name.
    pub name: String,
    /// The cells as the deciding code walks them.
    pub flow: Flow,
    /// What the engine needs to know of each cell beyond what the walk reads, in the order of the walk.
    pub cells: Vec<CellSpec>,
    /// Every block of a work's state, with who writes and who reads it.
    pub blocks: BTreeMap<BlockId, Block>,
    /// The named tests the cells use, each with the words shown for it.
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
    /// How often a return of the cell may be sent back within one run.
    pub returns: u32,
    /// How many calls one run of the cell may make.
    pub calls: u32,
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
    /// The outside writes it.
    pub outside: bool,
}

/// Who reads a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readers {
    /// The cells that have it among their inputs, in the order of the walk.
    pub cells: Vec<CellId>,
    /// The cells whose condition reads its value, in the order of the walk.
    pub conditions: Vec<CellId>,
    /// The owner's page or a named test reads it.
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
    /// A choice has no values to choose from.
    ValuesEmpty,
    /// The block is not declared in `flow.toml`.
    NoSuchBlock { block: String },
    /// The block has no writer: no cell of the order, no person and nothing outside writes it.
    NoWriter { block: String },
    /// The block has no reader: no cell of the order reads it, no condition reads its value, and neither the owner's
    /// page nor a named test shows it.
    NoReader { block: String },
    /// A condition reads a block that is not a choice.
    NotAChoice { block: String },
    /// A condition waits for a value the choice does not offer.
    NotAValue { block: String, is: String },
    /// A condition reads a block its own cell writes: the cell could never come to run.
    ConditionOnOwnOutput { block: String },
    /// A condition reads a block that neither a cell before it nor people write, so that it is decided by nothing
    /// that comes before the cell.
    ConditionNotWrittenBefore { block: String },
    /// The last cell has a condition: a work ends when its last cell has a record, so the last cell always runs.
    LastHasCondition,
    /// A cell uses a test that `flow.toml` does not declare.
    UndeclaredTest { test: String },
    /// A declared test is not one the engine has.
    UnknownTest { test: String },
    /// A declared test is used by no cell.
    UnusedTest { test: String },
    /// The deciding code refuses the cells as a flow.
    Flow(FlowFault),
}

/// Checks a flow's definition against the tests the engine has, and compiles it; or gives every fault it has, in
/// the order of the files (`flow.toml`, then the cells in the order of the walk, then any other) and, within a file,
/// in the order they were found. When a file cannot be read, only the files that cannot be read are reported.
pub fn compile(source: &Source, known_tests: &BTreeSet<TestName>) -> Result<Compiled, Vec<Fault>> {
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
    def.check_blocks(&mut faults);
    def.check_tests(known_tests, &mut faults);

    let mut cells = Vec::new();
    let mut specs = Vec::new();
    for (place, id, file) in def.ordered() {
        let (cell, spec) = def.cell(place, id, file, &limits, &mut faults);
        cells.push(cell);
        specs.push(spec);
    }

    if !faults.is_empty() {
        sort(&mut faults, Some(&def.place));
        return Err(faults);
    }
    let flow = Flow::new(cells).map_err(|f| vec![fault(FLOW_FILE, "order", FaultKind::Flow(f))])?;
    Ok(Compiled {
        name: flow_file.name.clone(),
        flow,
        cells: specs,
        blocks: def.blocks(),
        tests: flow_file
            .tests
            .iter()
            .map(|(n, w)| (TestName(n.clone()), w.clone()))
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

/// The cells of the order that write and read one block, each list in the order of the walk.
#[derive(Default)]
struct Uses<'a> {
    writers: Vec<(usize, &'a str)>,
    readers: Vec<&'a str>,
    conditions: Vec<&'a str>,
}

/// The parsed files, with what the checks look up in them.
struct Definition<'a> {
    flow: &'a FlowFile,
    cells: &'a BTreeMap<String, CellFile>,
    /// Where each cell of the order that has a file first stands.
    place: BTreeMap<&'a str, usize>,
    /// Who of the order writes and reads each block that is declared.
    uses: BTreeMap<&'a str, Uses<'a>>,
}

impl<'a> Definition<'a> {
    fn new(flow: &'a FlowFile, cells: &'a BTreeMap<String, CellFile>) -> Self {
        let mut def = Self {
            flow,
            cells,
            place: first_places(&flow.order, cells),
            uses: BTreeMap::new(),
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
        def.uses = uses;
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
        declared.people || declared.outside || !self.uses[block].writers.is_empty()
    }

    /// Whether a cell of the order, a condition, the owner's page or a named test reads the block.
    fn read(&self, block: &str) -> bool {
        let uses = &self.uses[block];
        self.flow.blocks[block].shown || !uses.readers.is_empty() || !uses.conditions.is_empty()
    }

    /// The faults of the declared blocks: values that make no choice, a block with no writer, and one with no reader.
    fn check_blocks(&self, faults: &mut Vec<Fault>) {
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

    /// The faults of the declared tests: one the engine does not have, or one that no cell of the order uses.
    fn check_tests(&self, known: &BTreeSet<TestName>, faults: &mut Vec<Fault>) {
        let mut used = BTreeSet::new();
        for (_, _, file) in self.ordered() {
            used.extend(file.requires.iter().map(String::as_str));
            if let Some(When::Test(WhenTest { test })) = &file.when {
                used.insert(test.as_str());
            }
        }
        for test in self.flow.tests.keys() {
            if !known.contains(&TestName(test.clone())) {
                let what = FaultKind::UnknownTest { test: test.clone() };
                faults.push(fault(FLOW_FILE, format!("tests.{test}"), what));
            }
            if !used.contains(test.as_str()) {
                let what = FaultKind::UnusedTest { test: test.clone() };
                faults.push(fault(FLOW_FILE, format!("tests.{test}"), what));
            }
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
                    if !self.flow.tests.contains_key(test) {
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
        let declared = |block: &str| {
            let what = FaultKind::NoSuchBlock {
                block: block.to_string(),
            };
            (!self.flow.blocks.contains_key(block)).then_some(what)
        };
        found.extend(check_list(&file.inputs, "inputs", declared));
        found.extend(check_list(&file.outputs, "outputs", declared));
        faults.extend(found.into_iter().map(|(field, what)| fault(&name, field, what)));

        let cell = Cell {
            id: CellId(id.to_string()),
            applies,
            requires: file.requires.iter().map(|t| TestName(t.clone())).collect(),
            inputs: file.inputs.iter().map(|b| BlockId(b.clone())).collect(),
            outputs: file.outputs.iter().map(|b| BlockId(b.clone())).collect(),
            limit: runs,
        };
        let spec = CellSpec {
            id: CellId(id.to_string()),
            title: file.title.clone(),
            kind: file.kind,
            line: file.line.clone(),
            returns: limits.returns,
            calls: limits.calls,
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
        if file.outputs.iter().any(|o| o == block) {
            found.push((field(), FaultKind::ConditionOnOwnOutput { block: name }));
        } else if !declared.people && !self.uses[block].writers.iter().any(|&(p, _)| p < place) {
            found.push((field(), FaultKind::ConditionNotWrittenBefore { block: name }));
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
                        outside: declared.outside,
                    },
                    readers: Readers {
                        cells: ids(&uses.readers),
                        conditions: ids(&uses.conditions),
                        shown: declared.shown,
                    },
                };
                (BlockId(name.clone()), block)
            })
            .collect()
    }
}
