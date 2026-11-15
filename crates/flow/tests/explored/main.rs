//! The flows as the repository defines them (`flow/engineer`, `flow/deliver`), explored by the crate `explore`: every
//! way a work on one can go within a bound, the work driven by the engine's own loop, which keeps its records, and the
//! properties the flow must keep checked as each run starts and at each stop by the records read as the design tree
//! says (crate `reading`), never by the walk's (design document `testing`, 5).
//!
//! What happens next, at each state: a round under way returns, and the loop takes it; the engine gives a tool's
//! result; or the outside writes. What a round returns (`model::Variant`): the return of a cell content with its work;
//! or, spending one of the path's two, every other set of values of the choices it writes, its choices alone, its
//! content with its opinions for the cells before it (all, or each alone), a judge's refusal with its reasons, of a
//! cell that may ask the owner its content with a question that waits for the owner (with the content's choices, or
//! with choices that skip the cell itself) or one it goes on past, an entry asking for tests and nothing else, of the
//! check's act a red check, a merge conflict or a red check main fails too; and on a path that spent nothing a return
//! that is sent back, or a call of a tool. The act that runs the tests hands in the result of each entry it gives as
//! its round ends. An agent's round that deals with what an act after its cell wrote commits, moving the work's head
//! as it ends, as does a round of such a cell that asks for tests. A return that sends work round (opinions, a refusal,
//! a send-back, a red check) may be given at every round of its cell from then on, for the same one spent: the flow's
//! limits must stop it. What the outside writes, spending one: the owner speaks to a line, changes the background,
//! answers a question as it recommends or takes another option, lets a cell go on at its limit or gives its word at a
//! run's limit; main moves, a line begins a new session, the tree changes, a commit no round made moves the work's
//! head. It writes while the work waits or is stalled, on a path that spent at most one before; while a round is under
//! way, the owner speaks to its line or answers a question that stands on such a path, and on a path that spent
//! nothing the background or what runs the cell from outside is written too. The owner answers what the work waits on
//! whatever the path spent. Past a write from outside the cells give their content alone. The items the work depends
//! on merging costs nothing. A stalled work, which the engine pauses for the owner, saying why, is a stop for the owner
//! as a wait for the owner is.
//!
//! Two states that would go on alike are one (`explore::Explorer::key`), and one reached having spent no more is kept.
//! A path of more than 200 things given the loop with nothing written from outside, or that comes back to a state it
//! was in, breaks property 4.
//!
//! What it costs: a few thousand states, in about ten seconds in all; the test prints the numbers. The search runs
//! once, in one test that checks every property on what it found and names each that fails, with every breach of it;
//! the flow renamed is explored again at a smaller bound, with the flow as it is at that bound beside it.

use std::collections::BTreeMap;
use std::path::Path;

use decide::{BlockId, CellId};
use explore::model::{Model, ran, ran_on, reply_loop};
use explore::{
    A_QUESTION_WAITED, A_ROUND_STOPPED, A_TOOL_ANSWERED, Bounds, CHECKED_CONDITION, CHECKED_GATES,
    CHECKED_GROUP_DEFINED, CHECKED_GROUP_RUN, CHECKED_RERUN, CHECKED_REST, CHECKED_RUN, CHECKED_STALL, CHECKED_STOP,
    CHECKED_THE_CHECK, CHECKED_WRITE_AT_STOP, Explored, GAVE_RESULTS, MAIN_WHILE_THE_CHECK_WAITS, Property,
    RAN_FOR_A_WAIT, WAITED_FOR_MAIN, WAITED_FOR_THE_OWNER, explore, replay,
};
use flow::{Compiled, Has, Source, compile, read};

/// The bound of the exploration.
const BOUNDS: Bounds = Bounds {
    spend: 2,
    outside_after: 1,
    steps: 200,
    states: 200_000,
};

/// The bound the flow renamed is explored at, beside the flow as it is at that bound, in half a second each against ten
/// for the whole bound (measured in CI). That a name written into the code shows already on the ways within it is a
/// judgment, which the trigger's name tried at it bears out; it leaves unreached, on both sides alike, 61 of the items the whole
/// bound reaches: those that take two spent, as the deep research's runs past its first and every other return of a
/// path that spent one.
const RENAMED_BOUNDS: Bounds = Bounds {
    spend: 1,
    outside_after: 1,
    steps: 200,
    states: 200_000,
};

/// The breaches the exploration finds and the design chose, each by its property and its name, with the decision that
/// makes it and why. Each is reported; none is mended here, since each is a choice of the design.
const KNOWN: &[(Property, &str, &str)] = &[
    (
        Property::Incremental,
        "the owner changes the background did not reach R0, which it runs, before the work stopped",
        MERGE_UNSTOPPED,
    ),
    (
        Property::Incremental,
        "the owner changes the background did not reach R1, which it runs, before the work stopped",
        MERGE_UNSTOPPED,
    ),
];

/// Why a change of the background while the merge's round is under way reaches no cell.
const MERGE_UNSTOPPED: &str = "Decision a-return-that-fits-is-taken-before-the-walk: the owner changes the \
     background while the merge's round is under way, which does not stop it (only what a source that turns the work \
     writes does), and the merge's return, fitting the flow, is taken as the round ends, before the walk is asked what \
     follows: the work has ended once the merge has its record, and R0 and R1 never read the change. Whether such a \
     change should hold the merge is the owner's to decide.";

/// The engineer's flow as the repository defines it, compiled against what the engine has.
fn engineer() -> Compiled {
    defined("engineer")
}

/// The deliver flow as the repository defines it, compiled against what the engine has.
fn deliver() -> Compiled {
    defined("deliver")
}

/// A flow of the repository's, by its directory's name, compiled against what the engine has.
fn defined(name: &str) -> Compiled {
    compiled(&source(name))
}

/// The source of a flow of the repository's, by its directory's name.
fn source(name: &str) -> Source {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow").join(name);
    read(&dir).expect("the flow is read")
}

/// The flow of this source, compiled against what the engine has.
fn compiled(source: &Source) -> Compiled {
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "run_tests", "deliver"]),
        tools: names(&["query"]),
        details: names(&["files"]),
        ..Default::default()
    };
    compile(source, &has).expect("the flow compiles")
}

fn stats(e: &Explored) -> String {
    let s = &e.stats;
    format!(
        "{} states, {} things given the engine's loop, {} readings of what a round was told, {} runs, at most {} \
         things given in a row, in {:.2?}; states by what their paths spent, and whether the outside wrote: {:?}",
        s.states, s.given, s.told, s.runs, s.longest, s.took, s.by_spent
    )
}

/// Each property by its number and what it says, as a failure names it.
const PROPERTIES: &[(Property, &str)] = &[
    (
        Property::NewToIt,
        "1. a cell runs only on a version of its own trigger that it has not consumed and did not write",
    ),
    (
        Property::Contradicting,
        "2. cells whose conditions want another value never run off the same version",
    ),
    (
        Property::NextStep,
        "3. every state has a next step, or waits for the owner or the outside, or has merged",
    ),
    (
        Property::ComesToRest,
        "4. with nothing written from outside, the walk comes to a stop within a bound",
    ),
    (
        Property::Groups,
        "5. a group runs a cell only for a change of a member, and the cell is given every member",
    ),
    (
        Property::Gates,
        "6. a cell runs only through its gates, and the check only once every question is answered",
    ),
    (
        Property::Incremental,
        "7. a write from outside runs again only the cells its change reaches",
    ),
    (
        Property::Shown,
        "8. every stall is shown as waiting for the owner, with where it stands, what it waits for and why",
    ),
    (
        Property::Engine,
        "the engine takes every step the exploration asks of it",
    ),
];

/// What breaks the property on the paths explored: each breach [`KNOWN`] does not give it, and each [`KNOWN`] gives it
/// that was not found.
fn failures(e: &Explored, property: Property) -> Vec<String> {
    let found: Vec<_> = e.breaches.iter().filter(|((p, _), _)| *p == property).collect();
    let known = |name: &str| KNOWN.iter().any(|(p, n, _)| *p == property && *n == name);
    let mut failures: Vec<String> = (found.iter())
        .filter(|((_, name), _)| !known(name))
        .map(|((_, name), b)| format!("breached: {name}\n{}", b.show()))
        .collect();
    for (_, name, _) in KNOWN.iter().filter(|(p, _, _)| *p == property) {
        if !found.iter().any(|((_, n), _)| n == name) {
            failures.push(format!("known, and no longer found: {name}"));
        }
    }
    failures
}

/// The paths the owner's test plan names as rarely taken, each of which the exploration must reach.
fn rare() -> Vec<String> {
    let cell = |id: &str| CellId::from(id);
    let block = |id: &str| BlockId::from(id);
    vec![
        // The deep research, and the review of the tree.
        ran(&cell("D1")),
        ran(&cell("D4")),
        // The loops of a review and its reply.
        reply_loop(&block("plan_review"), &block("reply_plan_review")),
        reply_loop(&block("tree_review"), &block("reply_tree_review")),
        reply_loop(&block("plan_doubts"), &block("reply_plan_doubts")),
        reply_loop(&block("review"), &block("reply_review")),
        reply_loop(&block("testing"), &block("reply_testing")),
        "a return was sent back".to_string(),
        // A question that does not hold the work, which the merge waits on until the owner answers it.
        "the work waited at M1 on asked = answered".to_string(),
        ran_on(&cell("M1"), "on_asked"),
        A_QUESTION_WAITED.to_string(),
        ran_on(&cell("R0"), "on_session"),
        ran_on(&cell("D1"), "on_session"),
        // The tests a cell asks for, run by the engine's cell while the asking cell waits, and the asking cell entered
        // again with their result.
        ran(&cell("T3")),
        RAN_FOR_A_WAIT.to_string(),
        GAVE_RESULTS.to_string(),
        ran_on(&cell("I1"), "on_tested"),
        MAIN_WHILE_THE_CHECK_WAITS.to_string(),
        // A cell asks for a tool, waits for its result, and is entered again with it.
        A_TOOL_ANSWERED.to_string(),
        "R1 was entered with the answers it waited for".to_string(),
        // The check finds a red that main fails too, and waits for main to move.
        WAITED_FOR_MAIN.to_string(),
        WAITED_FOR_THE_OWNER.to_string(),
        // The owner answers a question that stands while a round of a cell after its cell is under way: the round is
        // stopped, and the asking cell runs first.
        A_ROUND_STOPPED.to_string(),
        "stop: the work merged".to_string(),
    ]
}

/// What the coverage lacks: a rarely taken path not reached, and what was never reached against what the flow is said
/// never to reach.
fn coverage(e: &Explored, m: &Model, rare: &[String], never: &[(&str, &str)]) -> Vec<String> {
    let mut failures: Vec<String> = (rare.iter())
        .filter(|r| !e.reached.contains_key(*r))
        .map(|r| format!("not reached: {r}"))
        .collect();
    let unreached: Vec<String> = (m.universe().into_iter())
        .filter(|u| !e.reached.contains_key(u))
        .collect();
    for n in unreached.iter().filter(|n| !never.iter().any(|(item, _)| item == n)) {
        failures.push(format!("never reached, and not said why: {n}"));
    }
    for (item, _) in never.iter().filter(|(item, _)| !unreached.iter().any(|n| n == item)) {
        failures.push(format!("said never reached, and reached: {item}"));
    }
    failures
}

/// What the exploration reached, and what each write from outside ran again.
fn report(e: &Explored) -> String {
    let mut s = String::from("what each write from outside ran again:\n");
    for (what, cells) in &e.reruns {
        let cells: Vec<&str> = cells.iter().map(CellId::as_str).collect();
        s.push_str(&format!("  {what}: {}\n", cells.join(", ")));
    }
    s.push_str("reached:\n");
    for (what, n) in &e.reached {
        s.push_str(&format!("  {n:>6} {what}\n"));
    }
    s
}

/// One search of the engineer's flow, and every property checked on what it found: each that fails is named with
/// every breach of it, all in one run.
#[test]
fn the_engineers_flow_keeps_every_property_on_every_path_explored_and_reaches_every_rare_path() {
    let m = Model::new(engineer());
    let e = explore(&m, BOUNDS);
    let mut failed = Vec::new();
    for (property, name) in PROPERTIES {
        let failures = failures(&e, *property);
        if !failures.is_empty() {
            failed.push(format!("property {name}:\n{}", failures.join("\n")));
        }
    }
    let coverage = coverage(&e, &m, &rare(), NEVER);
    if !coverage.is_empty() {
        failed.push(format!("8. coverage:\n  {}", coverage.join("\n  ")));
    }
    // No part of a property passes by being checked on nothing; the engineer's flow reaches no stall (`NEVER`).
    failed.extend(unchecked(&e, &[CHECKED_STALL]));
    failed.extend(replays(&m, &e));
    // The exploration names no cell, block or trigger: the flow with every one of them renamed alike goes every way
    // the flow does at a smaller bound, its names following.
    let (names, renamed) = renamed(&source("engineer"));
    let small = explore(&m, RENAMED_BOUNDS);
    let rm = Model::new(compiled(&renamed));
    let re = explore(&rm, RENAMED_BOUNDS);
    failed.extend(alike(&m, &small, &re, &names));
    let is_known = |p: &Property, n: &String| KNOWN.iter().any(|(kp, kn, _)| kp == p && kn == n);
    let mut known = String::new();
    for ((_, name), b) in e.breaches.iter().filter(|((p, n), _)| is_known(p, n)) {
        known.push_str(&format!("{name} (known)\n{}", b.show()));
    }
    eprintln!("{}\n\n{known}\n{}", stats(&e), report(&e));
    assert!(failed.is_empty(), "{}\n\n{}", stats(&e), failed.join("\n\n"));
}

/// What of the flow the exploration never reaches, each with why: what the flow's definition holds and no path can
/// reach, or what lies past the bound.
const NEVER: &[(&str, &str)] = &[
    (
        "D3 ran back on on_plan, written by a cell after it",
        "I1's opinion in acceptance reaches D2 first, and D2's plan reaches D3 with it, so D3 runs for D2's version \
         too. That it runs for I1's alone takes D2 handing in no plan: a third return besides the content, past the \
         bound.",
    ),
    (
        "the work waited at M2 on checked = yes",
        "The merge waits on checked = yes only where nothing runs the check again after it said no, a stall, which no \
         path of the flow reaches: the implementation's return to a red check leaves what runs the check again, its \
         commit or a question, or is sent back (decision an-act-is-answered-by-what-runs-it). How a stalled work is \
         shown is the engine's test of stalling (crates/engine/tests/loop/stalling.rs).",
    ),
];

/// The deliver flow explored as the engineer's is: every property kept on every path, and its way reached, the work
/// waiting for the items it is made of and delivered once they merge.
#[test]
fn the_deliver_flow_keeps_every_property_on_every_path_explored() {
    let m = Model::new(deliver());
    let e = explore(&m, BOUNDS);
    let mut failed = Vec::new();
    for (property, name) in PROPERTIES {
        let breaches: Vec<String> = (e.breaches.iter())
            .filter(|((p, _), _)| p == property)
            .map(|((_, n), b)| format!("breached: {n}\n{}", b.show()))
            .collect();
        if !breaches.is_empty() {
            failed.push(format!("property {name}:\n{}", breaches.join("\n")));
        }
    }
    // The deliver flow has no condition, no group, no check and no stall, so those parts are checked on nothing in it.
    failed.extend(unchecked(
        &e,
        &[
            CHECKED_CONDITION,
            CHECKED_GROUP_RUN,
            CHECKED_GROUP_DEFINED,
            CHECKED_THE_CHECK,
            CHECKED_STALL,
        ],
    ));
    let way = [
        ran(&CellId::from("V1")),
        "the work waited at V1 on dependencies = merged".to_string(),
        "stop: the work merged".to_string(),
    ];
    let coverage = coverage(&e, &m, &way, DELIVER_NEVER);
    if !coverage.is_empty() {
        failed.push(format!("coverage:\n  {}", coverage.join("\n  ")));
    }
    failed.extend(replays(&m, &e));
    eprintln!("{}\n\n{}", stats(&e), report(&e));
    assert!(failed.is_empty(), "{}\n\n{}", stats(&e), failed.join("\n\n"));
}

/// What of the deliver flow the exploration never reaches, each with why.
const DELIVER_NEVER: &[(&str, &str)] = &[];

/// Every part of the properties, as the exploration counts the cases each was checked on.
const PARTS: [&str; 11] = [
    CHECKED_RUN,
    CHECKED_CONDITION,
    CHECKED_STOP,
    CHECKED_REST,
    CHECKED_GROUP_RUN,
    CHECKED_GROUP_DEFINED,
    CHECKED_GATES,
    CHECKED_THE_CHECK,
    CHECKED_RERUN,
    CHECKED_WRITE_AT_STOP,
    CHECKED_STALL,
];

/// Each part of a property but those given that was checked on no case, and each of those given checked on some.
fn unchecked(e: &Explored, none: &[&str]) -> Vec<String> {
    let mut wrong = Vec::new();
    for part in PARTS {
        let checked = e.checked.get(part).copied().unwrap_or(0);
        if none.contains(&part) && checked > 0 {
            wrong.push(format!("{part} was checked on {checked} cases, where it has none"));
        } else if !none.contains(&part) && checked == 0 {
            wrong.push(format!("{part} was checked on no case"));
        }
    }
    wrong
}

/// Each item whose nearest path, given again to the engine's loop for a new work, does not give the work the
/// exploration had when it reached it, its records and the content they name.
fn replays(m: &Model, e: &Explored) -> Vec<String> {
    let mut wrong = Vec::new();
    for (item, path) in &e.paths {
        match replay(m, &path.given) {
            Ok(work) if work == path.work => {}
            Ok(_) => wrong.push(format!("the path to {item}, given again, gives another work")),
            Err(why) => wrong.push(format!("the path to {item}, given again, is refused: {why:?}")),
        }
    }
    wrong
}

/// The source with every cell, block, group, line, named test and trigger of the flow given another name, and the names
/// given, each by the one it replaces: the trigger of a block's or a group's new version (`on_` and its name) follows
/// its name, and a line's block of its session (`{line}_session`) the line's.
fn renamed(source: &Source) -> (BTreeMap<String, String>, Source) {
    let mut names = BTreeMap::new();
    // One prefix for every name, after `on_` for a trigger's, keeps every set of names in the order it had, so that the
    // exploration, which goes through them in that order, goes the same way.
    let new = |name: &str| match name.strip_prefix("on_") {
        Some(rest) => format!("on_x_{rest}"),
        None => format!("x_{name}"),
    };
    for cell in source.cells.keys() {
        names.insert(cell.clone(), new(cell));
    }
    for path in source.files.keys() {
        let stem = (Path::new(path).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if path.starts_with("blocks/") || path.starts_with("../data/") {
            names.insert(stem.to_string(), new(stem));
            names.insert(format!("on_{stem}"), format!("on_{}", new(stem)));
        } else if (path.starts_with("../triggers/") && !path.ends_with("/changed.toml"))
            || path.starts_with("lines/")
            || path.starts_with("tests/")
        {
            names.insert(stem.to_string(), new(stem));
        }
    }
    // A group's name, and its trigger, `on_` and its name.
    let mut groups = false;
    for line in source.flow.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            groups = line == "[groups]";
        } else if groups && let Some((group, _)) = line.split_once('=') {
            let group = group.trim();
            names.insert(group.to_string(), new(group));
            names.insert(format!("on_{group}"), new(&format!("on_{group}")));
        }
    }
    let file = |path: &String| {
        let stem = (Path::new(path).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        match names.get(stem) {
            Some(new) if path.ends_with(&format!("/{stem}.toml")) => {
                format!("{}/{new}.toml", &path[..path.len() - stem.len() - "/.toml".len()])
            }
            _ => path.clone(),
        }
    };
    let renamed = Source {
        laid_out: source.laid_out,
        flow: rename_text(&source.flow, &names),
        cells: (source.cells.iter())
            .map(|(id, text)| (names[id].clone(), rename_text(text, &names)))
            .collect(),
        rules: source.rules.clone(),
        files: (source.files.iter())
            .map(|(path, text)| (file(path), rename_text(text, &names)))
            .collect(),
    };
    (names, renamed)
}

/// The text with each word the names give renamed, but where the word names no cell, block or trigger: a key of the
/// file's own form (a key at the start of a line, but in a table whose keys are names: `kinds`, `do`, `groups`), a
/// source or an act (`outside`, `from`, `act`), a place a text fills (`{name}`), a value of a choice (a list of
/// `values`, or what a key of a block is set to), and the line of a table's name.
fn rename_text(text: &str, names: &BTreeMap<String, String>) -> String {
    const NAMED: [&str; 3] = ["kinds", "do", "groups"];
    let mut out = String::with_capacity(text.len());
    let mut table = String::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') && !trimmed.starts_with("[\"") {
            table = trimmed.trim_end().trim_matches(|c| c == '[' || c == ']').to_string();
            out.push_str(line);
            continue;
        }
        if trimmed.starts_with("values") {
            out.push_str(line);
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let word_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let mut i = 0;
        while i < chars.len() {
            if !word_char(chars[i]) {
                out.push(chars[i]);
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && word_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let before: String = chars[..start].iter().collect();
            let after: String = chars[i..].iter().collect();
            let key = after.trim_start().starts_with('=') && !after.trim_start().starts_with("==");
            let form = key && before.trim().is_empty() && !NAMED.contains(&table.as_str());
            let value_of_a_block = (before.strip_suffix("= \"").map(str::trim_end)).is_some_and(|key| {
                let at = key.rfind(|c: char| !word_char(c)).map_or(0, |at| at + 1);
                names.contains_key(&key[at..])
            });
            let kept = form
                || before.ends_with('{')
                || ["outside = \"", "from = \"", "act = \""]
                    .iter()
                    .any(|k| before.ends_with(k))
                || value_of_a_block;
            match names.get(&word) {
                Some(new) if !kept => out.push_str(new),
                _ => out.push_str(&word),
            }
        }
    }
    out
}

/// What differs between the exploration of the flow and that of the flow renamed: the states kept and the runs, how
/// often each property was checked and breached, how often each item was reached, and, by each cell's and each of its
/// triggers' names in each, how often each ran.
fn alike(m: &Model, e: &Explored, renamed: &Explored, names: &BTreeMap<String, String>) -> Vec<String> {
    let mut wrong = Vec::new();
    let (s, r) = (&e.stats, &renamed.stats);
    if (s.states, s.runs) != (r.states, r.runs) {
        wrong.push(format!(
            "renamed, it kept {} states and {} runs, against {} and {}",
            r.states, r.runs, s.states, s.runs
        ));
    }
    if e.checked != renamed.checked {
        wrong.push(format!(
            "renamed, its properties were checked {:?}, against {:?}",
            renamed.checked, e.checked
        ));
    }
    let breached = |x: &Explored| {
        let mut by: BTreeMap<Property, usize> = BTreeMap::new();
        for (p, _) in x.breaches.keys() {
            *by.entry(*p).or_default() += 1;
        }
        by
    };
    if breached(e) != breached(renamed) {
        wrong.push(format!(
            "renamed, its breaches were {:?}, against {:?}",
            breached(renamed),
            breached(e)
        ));
    }
    let counts = |x: &Explored| {
        let mut counts: Vec<u64> = x.reached.values().copied().collect();
        counts.sort_unstable();
        counts
    };
    if counts(e) != counts(renamed) {
        wrong.push("renamed, its items were reached other numbers of times".to_string());
    }
    let named = |name: &str| names.get(name).cloned().unwrap_or_else(|| name.to_string());
    for (i, cell) in m.cells().iter().enumerate() {
        let new = CellId::from(named(cell.id.as_str()).as_str());
        let mut items = vec![(ran(&cell.id), ran(&new))];
        for trigger in m.triggers[i].values() {
            items.push((ran_on(&cell.id, trigger), ran_on(&new, &named(trigger))));
        }
        for (was, is) in items {
            let (a, b) = (e.reached.get(&was), renamed.reached.get(&is));
            if a != b {
                wrong.push(format!("{was} was reached {a:?} times, and renamed, {is} {b:?}"));
            }
        }
    }
    wrong
}
