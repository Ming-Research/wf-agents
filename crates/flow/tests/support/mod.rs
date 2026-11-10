//! A small valid flow, written as its files would be, for the tests of the compile to break one place at a time.

use std::collections::BTreeSet;

use decide::TestName;
use flow::{Compiled, FLOW_FILE, Fault, FaultKind, Source};

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
outside = true

[blocks.ci]
values = ["green", "red"]

[tests]
has_comment = "The item has a comment"
plan_confirmed = "The owner confirmed the plan"
may_merge = "The work may merge"
"#;

/// A cell that reads what people asked for and the opinions on its plan, and writes the plan.
const PLAN: &str = r#"
title = "Plan"
kind = "agent"
line = "engineer"
inputs = ["request", "opinions"]
outputs = ["plan"]
"#;

/// A cell that judges the plan: it writes a choice and its opinions, which the planning cell reads.
const JUDGE: &str = r#"
title = "Judge the plan"
kind = "agent"
line = "review"
inputs = ["plan"]
outputs = ["needs_design", "opinions"]
"#;

/// A cell that runs when the judge chose yes.
const DESIGN: &str = r#"
title = "Design"
kind = "agent"
line = "engineer"
when = { block = "needs_design", is = "yes" }
inputs = ["plan"]
outputs = ["design"]
"#;

/// A cell that runs when a test holds, waits on another and sets a limit of its own.
const ANSWER: &str = r#"
title = "Answer comments"
kind = "agent"
line = "engineer"
when = { test = "has_comment" }
requires = ["plan_confirmed"]
inputs = ["comments", "plan"]
outputs = ["reply"]
limit = 5
"#;

/// An engine cell that reads what the outside writes.
const CHECK: &str = r#"
title = "Check with the latest main"
kind = "engine"
inputs = ["main", "design"]
outputs = ["ci"]
"#;

/// The last cell, which the engine carries out.
const MERGE: &str = r#"
title = "Merge"
kind = "engine"
requires = ["may_merge"]
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

/// The tests the engine has: those the small flow declares.
pub fn known() -> BTreeSet<TestName> {
    ["has_comment", "plan_confirmed", "may_merge"]
        .into_iter()
        .map(TestName::from)
        .collect()
}

/// The text of a file of the source, by the file's name.
fn text<'a>(source: &'a mut Source, file: &str) -> &'a mut String {
    if file == FLOW_FILE {
        return &mut source.flow;
    }
    let id = file.strip_suffix(".toml").expect("a file of a flow ends in .toml");
    source.cells.entry(id.to_string()).or_default()
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

/// The faults of a definition against the tests the small flow declares.
pub fn faults(source: &Source) -> Vec<Fault> {
    faults_with(source, &known())
}

/// The faults of a definition against these tests.
pub fn faults_with(source: &Source, known: &BTreeSet<TestName>) -> Vec<Fault> {
    match flow::compile(source, known) {
        Ok(_) => panic!("the definition should not compile"),
        Err(faults) => faults,
    }
}

/// The definition compiled against the tests the small flow declares.
pub fn compiled(source: &Source) -> Compiled {
    match flow::compile(source, &known()) {
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
