//! The engineer's flow as the repository defines it (`flow/engineer`), explored: every way a work on it can go within a
//! bound, the walk being the deciding code's own (`decide::decide`), a return taken or sent back as the engine's check
//! of a return says (`decide::run::next`), and the properties the flow must keep checked at each decision by the
//! histories' own reading of the records (crate `reading`), never by the walk's (design document `testing`, 5). The
//! work's records are kept by the helper the deciding code's tests share (`tests/support`).
//!
//! What happens next, at each state: the engine does what the walk says (starts a run, takes a return, goes on with a
//! run, abandons one); a round under way returns; the outside writes. What a round returns (`model::Variant`): the
//! return of a cell content with its work; or, spending one of the path's two, every other set of values of the choices
//! it writes, its choices alone, its content with its opinions for the cells before it (all, or each alone), a judge's
//! refusal with its reasons, its content with a question that waits for the owner (with the content's choices, or with
//! choices that skip the cell itself), of the check's act a red check or a merge conflict, and on a path that spent
//! nothing a return that is sent back. An agent's round that deals with what an act after its cell wrote commits,
//! moving the work's head as it ends. A return that sends work round (opinions, a refusal, a send-back, a red check)
//! may be given at every round of its cell from then on, for the same one spent: the flow's limits must stop it. What
//! the outside writes, spending one: the owner speaks to a cell, changes the background, answers a question or takes
//! another option for one, lets a cell go on at its limit; main moves, a line begins a new session, the tree changes, a
//! commit no round made moves the work's head. It writes while the work waits or is stalled, on a path that spent at
//! most one before; while a round is under way, the owner speaks to its cell on such a path, and on a path that spent
//! nothing the background or what runs the cell from outside is written too. The owner answers a question the work
//! waits on whatever the path spent. Past a write from outside the cells give their content alone. The items the work
//! depends on merging costs nothing. A stalled work, which the engine pauses for the owner, saying why, is a stop for
//! the owner as a wait for the owner is.
//!
//! Two states that would go on alike are one (`explore::Explorer::key`), and one reached having spent no more is kept.
//! A path of more than 200 decisions with nothing written from outside, or that comes back to a state it was in, breaks
//! property 4.
//!
//! What it costs: a few thousand states, each a few decisions of the walk, in a few seconds; the test prints the
//! numbers. The search runs once, in one test that checks every property on what it found and names each that fails,
//! with every breach of it.

#[allow(dead_code)]
#[path = "../../../decide/tests/support/mod.rs"]
mod support;

mod explore;
mod model;

use std::path::Path;

use decide::{BlockId, CellId};
use explore::{
    A_QUESTION_WAITED, Bounds, Explored, MAIN_WHILE_THE_CHECK_WAITS, Property, WAITED_FOR_THE_OWNER, explore,
};
use flow::{Compiled, Has, compile, read};
use model::{Model, ran, ran_on, reply_loop};

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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = read(&dir).expect("the engineer's flow is read");
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
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        details: names(&["files"]),
        ..Default::default()
    };
    compile(&source, &has).expect("the engineer's flow compiles")
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
        A_QUESTION_WAITED.to_string(),
        ran_on(&cell("R0"), "on_session"),
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
        "D3 ran back on plan, written by a cell after it",
        "I1's opinion in acceptance reaches D2 first, and D2's plan reaches D3 with it, so D3 runs for D2's version \
         too. That it runs for I1's alone takes D2 handing in no plan: a third return besides the content, past the \
         bound.",
    ),
    (
        "the work waited at M2 on checked = yes",
        "The merge waits on checked = yes only where nothing runs the check again after it said no, a stall, which no \
         path of the flow reaches: the implementation's return to a red check leaves what runs the check again, its \
         commit, a question with an answer to come or what is new to the check already, or is sent back (decision \
         an-act-is-answered-by-what-runs-it), which the engineer's histories hold with questions in the records \
         (crates/engine/tests/engineer). How a stalled work is shown is the engine's test of stalling \
         (crates/engine/tests/loop/stalling.rs).",
    ),
    (
        "D2 ran on r2_questions",
        "R2 asks the owner only where the research wrote the plan, deep_needed = no, and D2 runs only where it is \
         yes: an answer to R2's question reaches D2 only once the research has written yes since R2 asked, which takes \
         R2's question, a write from outside that runs the research again and a return of it with yes, past the bound. \
         That D1 runs on an answer to R1's question, which the work waits for at R1, skipped once deep_needed is yes, \
         is reached.",
    ),
    (
        "the work waited at M1 on asked = answered",
        "Every question the exploration asks waits, and a question that waits holds the work at its cell, before M1, \
         whether or not the cell applies, so asked is waiting only while nothing after that cell runs. That the merge \
         never begins while a question that does not wait stands unanswered is the histories of questions' \
         (crates/engine/tests/questions.rs).",
    ),
];
