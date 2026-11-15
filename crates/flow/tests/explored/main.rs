//! The flows as the repository defines them (`flow/engineer`, `flow/deliver`), explored by the crate `explore`: every
//! way a work on one can go within a bound, the work driven by the engine's own loop, which keeps its records, and the
//! properties the flow must keep checked as each run starts and at each stop by the records read as the design tree
//! says (crate `reading`), never by the walk's (design document `testing`, 5).
//!
//! What happens next, at each state: a round under way returns, and the loop takes it; or the outside writes. What a
//! round returns (`model::Variant`): the
//! return of a cell content with its work; or, spending one of the path's two, every other set of values of the choices
//! it writes, its choices alone, its content with its opinions for the cells before it (all, or each alone), a judge's
//! refusal with its reasons, of a cell that may ask the owner its content with a question that waits for the owner (with
//! the content's choices, or with choices that skip the cell itself) or one it goes on past, an entry asking for tests and nothing else, of the check's act a red check or a
//! merge conflict, and on a path that spent nothing a return that is sent back. The act that runs the tests hands in
//! the result of each entry it gives as its round ends. An agent's round that deals with what an act after its cell
//! wrote commits, moving the work's head as it ends, as does a round of such a cell that asks for tests. A return that
//! sends work round (opinions, a refusal, a send-back, a red check) may be given at every round of its cell from then
//! on, for the same one spent: the flow's limits must stop it. What the outside writes, spending one: the owner speaks
//! to a line, changes the background, answers a question as it recommends or takes another option, lets a cell go on at
//! its limit or gives its word at a run's limit; main moves, a line begins a new session, the tree changes, a commit no round made moves the work's head. It
//! writes while the work waits or is stalled, on a path that spent at most one before; while a round is under way, the
//! owner speaks to its cell on such a path, and on a path that spent nothing the background or what runs the cell from
//! outside is written too. The owner answers a question the work waits on whatever the path spent. Past a write from
//! outside the cells give their content alone. The items the work depends on merging costs nothing. A stalled work,
//! which the engine pauses for the owner, saying why, is a stop for the owner as a wait for the owner is.
//!
//! Two states that would go on alike are one (`explore::Explorer::key`), and one reached having spent no more is kept.
//! A path of more than 200 things given the loop with nothing written from outside, or that comes back to a state it was in, breaks
//! property 4.
//!
//! What it costs: a few thousand states, each a few decisions of the walk, in a few seconds; the test prints the
//! numbers. The search runs once, in one test that checks every property on what it found and names each that fails,
//! with every breach of it.

use std::path::Path;

use decide::{BlockId, CellId};
use explore::model::{Model, ran, ran_on, reply_loop};
use explore::{
    A_QUESTION_WAITED, Bounds, Explored, GAVE_RESULTS, MAIN_WHILE_THE_CHECK_WAITS, Property, RAN_FOR_A_WAIT,
    WAITED_FOR_THE_OWNER, explore, replay,
};
use flow::{Compiled, Has, compile, read};

/// The bound of the exploration.
const BOUNDS: Bounds = Bounds {
    spend: 2,
    outside_after: 1,
    steps: 200,
    states: 200_000,
};

/// The breaches the flow's own definition makes, each by its property and its name, with why it is the flow's and not
/// the engine's. Each is reported; none is mended here, since the flow changes only with the owner.
const KNOWN: &[(Property, &str, &str)] = &[];

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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow").join(name);
    let source = read(&dir).expect("the flow is read");
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
    compile(&source, &has).expect("the flow compiles")
}

fn stats(e: &Explored) -> String {
    let s = &e.stats;
    format!(
        "{} states, {} decisions of the walk, {} readings of what a round was told, {} runs, at most {} decisions in a \
         row, in {:.2?}; states by what their paths spent, and whether the outside wrote: {:?}",
        s.states, s.decisions, s.told, s.runs, s.longest, s.took, s.by_spent
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
        WAITED_FOR_THE_OWNER.to_string(),
        "stop: the work merged".to_string(),
    ]
}

/// What the coverage lacks: a rarely taken path not reached, and what was never reached against [`NEVER`].
fn coverage(e: &Explored, m: &Model) -> Vec<String> {
    let mut failures: Vec<String> = (rare().into_iter())
        .filter(|r| !e.reached.contains_key(r))
        .map(|r| format!("not reached: {r}"))
        .collect();
    let never: Vec<String> = (m.universe().into_iter())
        .filter(|u| !e.reached.contains_key(u))
        .collect();
    for n in never.iter().filter(|n| !NEVER.iter().any(|(item, _)| item == n)) {
        failures.push(format!("never reached, and not said why: {n}"));
    }
    for (item, _) in NEVER.iter().filter(|(item, _)| !never.iter().any(|n| n == item)) {
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
    let coverage = coverage(&e, &m);
    if !coverage.is_empty() {
        failed.push(format!("8. coverage:\n  {}", coverage.join("\n  ")));
    }
    // No property passes by being checked on nothing; the engineer's flow reaches no stall (`NEVER`).
    failed.extend(unchecked(&e, &[Property::Shown]));
    failed.extend(replays(&m, &e, &rare()));
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
    // The deliver flow has no condition, no group and no stall, so those properties are checked on nothing in it.
    failed.extend(unchecked(
        &e,
        &[Property::Contradicting, Property::Groups, Property::Shown],
    ));
    let way = [
        ran(&CellId::from("V1")),
        "the work waited at V1 on dependencies = merged".to_string(),
        "stop: the work merged".to_string(),
    ];
    for item in way.iter().filter(|item| !e.reached.contains_key(*item)) {
        failed.push(format!("not reached: {item}"));
    }
    eprintln!("{}\n\n{}", stats(&e), report(&e));
    assert!(failed.is_empty(), "{}\n\n{}", stats(&e), failed.join("\n\n"));
}

/// Each property but those given that was checked on no case, and each of those given that was checked on some.
fn unchecked(e: &Explored, none: &[Property]) -> Vec<String> {
    let mut wrong = Vec::new();
    for (property, name) in PROPERTIES.iter().filter(|(p, _)| *p != Property::Engine) {
        let checked = e.checked.get(property).copied().unwrap_or(0);
        if none.contains(property) && checked > 0 {
            wrong.push(format!(
                "property {name} was checked on {checked} cases, where it has none"
            ));
        } else if !none.contains(property) && checked == 0 {
            wrong.push(format!("property {name} was checked on no case"));
        }
    }
    wrong
}

/// Each of these items whose nearest path, given again to the engine's loop for a new work, does not give the records
/// the exploration had when it reached it.
fn replays(m: &Model, e: &Explored, items: &[String]) -> Vec<String> {
    let mut wrong = Vec::new();
    for item in items {
        let Some(path) = e.paths.get(item) else {
            continue;
        };
        match replay(m, &path.given) {
            Ok(work) if work.records == path.records => {}
            Ok(_) => wrong.push(format!("the path to {item}, given again, gives other records")),
            Err(why) => wrong.push(format!("the path to {item}, given again, is refused: {why:?}")),
        }
    }
    wrong
}
