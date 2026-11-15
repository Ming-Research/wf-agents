//! A small valid flow, written as its files would be, for the tests of the compile to break one place at a time; and
//! its twin laid out one thing to a file, in `tests/laid_out/`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use flow::{CELLS_DIR, Compiled, FLOW_FILE, Fault, FaultKind, Has, Source};

const FLOW: &str = r#"
name = "small"
order = ["plan", "judge", "design", "answer", "check", "merge"]

[limits]
runs = 3
returns = 2
calls = 40

[lines.engineer]
[lines.review]

[blocks.request]
people = true

[blocks.plan]

[blocks.opinions]
people = true

[blocks.needs_design]
values = ["yes", "no"]

[blocks.design]

[blocks.comments]
people = true

[blocks.reply]
shown = true

[blocks.main]
outside = "main"

[blocks.ci]
values = ["green", "red"]

[blocks.commented]
values = ["yes", "no"]
people = true

[blocks.confirmed]
values = ["yes", "no"]
people = true

[tests.has_comment]
says = "The item has a comment"
kind = "any"
reads = ["commented"]
is = "yes"

[tests.plan_confirmed]
says = "The owner confirmed the plan"
kind = "all"
reads = ["confirmed"]
is = "yes"

[tests.may_merge]
says = "The work may merge"
kind = "none"
reads = ["ci"]
is = "red"
"#;

/// A cell that reads what people asked for and the opinions on its plan, and writes the plan.
const PLAN: &str = r#"
title = "Plan"
kind = "agent"
effort = "high"
line = "engineer"
task = "Write the plan from the request."
inputs = ["request", "opinions"]
outputs = ["plan"]
"#;

/// A cell that judges the plan: it writes a choice and its opinions, which the planning cell reads.
const JUDGE: &str = r#"
title = "Judge the plan"
kind = "agent"
effort = "high"
line = "review"
task = "Judge the plan, and say whether it needs a design."
inputs = ["plan"]
outputs = ["needs_design", "opinions"]
"#;

/// A cell that runs when the judge chose yes.
const DESIGN: &str = r#"
title = "Design"
kind = "agent"
effort = "high"
line = "engineer"
task = "Design what the plan asks for."
when = { needs_design = "yes" }
inputs = ["plan"]
outputs = ["design"]
"#;

/// A cell that runs when a test holds, waits on another and sets a limit of its own; its task has more than one line.
const ANSWER: &str = r#"
title = "Answer comments"
kind = "agent"
effort = "high"
line = "engineer"
task = """
Answer the comments on the plan.
Say what you changed for each.
"""
when = { test = "has_comment" }
requires = { test = "plan_confirmed" }
inputs = ["comments", "plan"]
outputs = ["reply"]
limit = 5
"#;

/// An engine cell that reads what the outside writes, carried out by the engine's act `check`.
const CHECK: &str = r#"
title = "Check with the latest main"
kind = "engine"
act = "check"
inputs = ["main", "design"]
outputs = ["ci"]
"#;

/// The last cell, which the engine carries out by its act `merge`.
const MERGE: &str = r#"
title = "Merge"
kind = "engine"
act = "merge"
requires = { test = "may_merge" }
inputs = ["ci"]
"#;

/// The small flow's files.
pub fn valid() -> Source {
    let mut source = Source {
        flow: FLOW.to_string(),
        ..Source::default()
    };
    for (id, text) in [
        ("plan", PLAN),
        ("judge", JUDGE),
        ("design", DESIGN),
        ("answer", ANSWER),
        ("check", CHECK),
        ("merge", MERGE),
    ] {
        source.cells.insert(id.to_string(), text.to_string());
    }
    source
}

/// What the engine has: the source outside a work and the acts the small flow names, two tools, two parts of a line's
/// system prompt that it fills in itself, two fields of a return a datum may be written through, and one place it fills
/// with what happened.
pub fn has() -> Has {
    Has {
        sources: names(&["main"]),
        acts: names(&["check", "merge"]),
        tools: names(&["query", "run_tests"]),
        system: names(&["guide", "tree_rules"]),
        fields: names(&["reply", "questions"]),
        details: names(&["files"]),
    }
}

/// A set of names.
pub fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// The directory of the small flow laid out one thing to a file, beside what it shares with any flow.
pub fn laid_out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/laid_out/small")
}

/// The small flow laid out one thing to a file: the twin of [`valid`], its cells naming their triggers and context.
pub fn laid_out() -> Source {
    flow::read(&laid_out_dir()).expect("the small flow laid out one thing to a file is read")
}

/// The text of a file of the source, by its path from the flow's directory.
fn text<'a>(source: &'a mut Source, file: &str) -> &'a mut String {
    if file == FLOW_FILE {
        return &mut source.flow;
    }
    let cell = if source.laid_out {
        file.strip_prefix(&format!("{CELLS_DIR}/"))
    } else {
        Some(file)
    };
    match cell.and_then(|name| name.strip_suffix(".toml")) {
        Some(id) => source.cells.entry(id.to_string()).or_default(),
        None => source.files.entry(file.to_string()).or_default(),
    }
}

/// The flow with `from` replaced by `to` in one file; `from` must be there.
pub fn edit(source: Source, file: &str, from: &str, to: &str) -> Source {
    let mut source = source;
    let text = text(&mut source, file);
    assert!(text.contains(from), "{file} has no {from:?}");
    *text = text.replacen(from, to, 1);
    source
}

/// The flow with this file's text set, the file being added if it was not there.
pub fn set(source: Source, file: &str, to: &str) -> Source {
    let mut source = source;
    *text(&mut source, file) = to.to_string();
    source
}

/// The faults of a definition against what the small flow names.
pub fn faults(source: &Source) -> Vec<Fault> {
    faults_with(source, &has())
}

/// The faults of a definition against what this engine has.
pub fn faults_with(source: &Source, has: &Has) -> Vec<Fault> {
    match flow::compile(source, has) {
        Ok(_) => panic!("the definition should not compile"),
        Err(faults) => faults,
    }
}

/// The definition compiled against what the small flow names.
pub fn compiled(source: &Source) -> Compiled {
    match flow::compile(source, &has()) {
        Ok(compiled) => compiled,
        Err(faults) => panic!("the definition should compile: {faults:#?}"),
    }
}

/// A fault in this file and field.
pub fn at(file: &str, field: &str, what: FaultKind) -> Fault {
    Fault {
        file: file.to_string(),
        field: field.to_string(),
        what,
    }
}

/// The parser's words, for a definition that has exactly one fault and it is that a file could not be read.
pub fn unreadable(source: &Source, file: &str) -> String {
    let faults = faults(source);
    match faults.as_slice() {
        [
            Fault {
                file: f,
                field,
                what: FaultKind::Unreadable { message },
            },
        ] if f == file && field.is_empty() => message.clone(),
        other => panic!("{file} alone should be unreadable: {other:#?}"),
    }
}
