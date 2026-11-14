//! The engineer's flow as the repository defines it (`flow/engineer`), explored: every way a work on it can go within a
//! bound, the walk being the deciding code's own (`decide::decide`), a return taken or sent back as the engine's check
//! of a return says (`decide::run::next`), and the properties the flow must keep checked at each decision by the
//! histories' own reading of the records (crate `decide`, `tests/histories/reading.rs`), never by the walk's (principle
//! flow-tests-formal). The work's records are kept by the helper the deciding code's tests share (`tests/support`).
//!
//! What happens next, at each state: the engine does what the walk says (starts a run, takes a return, goes on with a
//! run, abandons one); a round under way returns; the outside writes. What a round returns (`model::Variant`): the
//! return of a cell content with its work; or, spending one of the path's two, any other value of a choice it writes,
//! its choices alone, its content with its opinions for the cells before it, a judge's refusal with its reasons, a
//! return that is sent back, its content with a question that waits for the owner, and of the check's act a red check
//! or a merge conflict. A return that sends work round (opinions, a refusal, a send-back, a red check) may be given at
//! every round of its cell from then on, for the same one spent: the flow's limits must stop it. What the outside
//! writes, spending one: the owner speaks to a cell, changes the background, answers a question or takes another option
//! for one, lets a cell go on at its limit; main moves, a session is begun anew, the tree changes. It writes while the
//! work waits or rests, on a path that spent at most one before, or while a round is under way on a path that spent
//! nothing (the background, the owner's words to the cell, what runs it from outside); past a write from outside the
//! cells give their content alone. The items the work depends on merging costs nothing.
//!
//! Two states that would go on alike are one (`explore::Explorer::key`), and one reached having spent no more is kept.
//! A path of more than 200 decisions with nothing written from outside, or that comes back to a state it was in, breaks
//! property 4.
//!
//! What it costs: the search is run once by each test's process, as nextest runs every test in a process of its own;
//! its time is in each test's failure and in the coverage test's output.

#[allow(dead_code)]
#[path = "../../../decide/tests/support/mod.rs"]
mod support;

#[allow(dead_code)]
#[path = "../../../decide/tests/histories/reading.rs"]
mod reading;

mod explore;
mod model;

use std::path::Path;
use std::sync::OnceLock;

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
        sources: names(&["dependencies", "main", "session", "start", "tree"]),
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        fields: names(&["questions", "reply"]),
        details: names(&["files"]),
        ..Default::default()
    };
    compile(&source, &has).expect("the engineer's flow compiles")
}

fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| Model::new(engineer()))
}

fn explored() -> &'static Explored {
    static EXPLORED: OnceLock<Explored> = OnceLock::new();
    EXPLORED.get_or_init(|| explore(model(), BOUNDS))
}

fn stats(e: &Explored) -> String {
    let s = &e.stats;
    format!(
        "{} states, {} decisions of the walk, {} readings of what a round was told, {} runs, at most {} decisions in a \
         row, in {:.2?}",
        s.states, s.decisions, s.told, s.runs, s.longest, s.took
    )
}

/// The property holds on every path explored, but for the breaches [`KNOWN`] gives it, each of which is found.
fn holds(property: Property) {
    let e = explored();
    let found: Vec<_> = e.breaches.iter().filter(|((p, _), _)| *p == property).collect();
    let known = |name: &str| KNOWN.iter().any(|(p, n, _)| *p == property && *n == name);
    let new: Vec<String> = (found.iter())
        .filter(|((_, name), _)| !known(name))
        .map(|((_, name), b)| format!("{name}\n{}", b.show()))
        .collect();
    let gone: Vec<&str> = (KNOWN.iter())
        .filter(|(p, n, _)| *p == property && !found.iter().any(|((_, name), _)| name == n))
        .map(|(_, n, _)| *n)
        .collect();
    let mut known_found = String::new();
    for ((_, name), b) in found.iter().filter(|((_, name), _)| known(name)) {
        known_found.push_str(&format!("{name} (known)\n{}", b.show()));
    }
    eprintln!("{}\n{known_found}", stats(e));
    assert!(
        new.is_empty() && gone.is_empty(),
        "{}\n\nbreached:\n{}\nknown and no longer found: {gone:?}",
        stats(e),
        new.join("\n")
    );
}

#[test]
fn a_cell_runs_only_on_a_version_of_its_own_trigger_that_it_has_not_consumed_and_did_not_write() {
    holds(Property::NewToIt);
}

#[test]
fn cells_whose_conditions_want_another_value_never_run_off_the_same_version() {
    holds(Property::Contradicting);
}

#[test]
fn every_state_has_a_next_step_or_waits_for_the_owner_or_has_merged() {
    holds(Property::NextStep);
}

#[test]
fn with_nothing_written_from_outside_the_walk_comes_to_a_stop_within_a_bound() {
    holds(Property::ComesToRest);
}

#[test]
fn a_group_runs_a_cell_only_for_a_change_of_a_member_and_the_cell_is_given_every_member() {
    holds(Property::Groups);
}

#[test]
fn a_cell_runs_only_through_its_gates_and_the_check_only_once_every_question_is_answered() {
    holds(Property::Gates);
}

#[test]
fn a_write_from_outside_runs_again_only_the_cells_its_change_reaches() {
    holds(Property::Incremental);
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

#[test]
fn the_exploration_reaches_every_rarely_taken_path_and_the_engine_takes_every_step() {
    let e = explored();
    let m = model();
    let missing: Vec<String> = rare().into_iter().filter(|r| !e.reached.contains_key(r)).collect();
    let never: Vec<String> = m
        .universe()
        .into_iter()
        .filter(|u| !e.reached.contains_key(u))
        .collect();
    let engine: Vec<String> = (e.breaches.iter())
        .filter(|((p, _), _)| *p == Property::Engine)
        .map(|((_, name), b)| format!("{name}\n{}", b.show()))
        .collect();
    let mut reruns = String::new();
    for (what, cells) in &e.reruns {
        let cells: Vec<&str> = cells.iter().map(CellId::as_str).collect();
        reruns.push_str(&format!("  {what}: {}\n", cells.join(", ")));
    }
    let mut reached = String::new();
    for (what, n) in &e.reached {
        reached.push_str(&format!("  {n:>6} {what}\n"));
    }
    let report = format!(
        "{}\n\nnever reached:\n  {}\n\nwhat each write from outside ran again:\n{reruns}\nreached:\n{reached}",
        stats(e),
        never.join("\n  ")
    );
    eprintln!("{report}");
    assert!(
        missing.is_empty() && engine.is_empty(),
        "{report}\nnot reached: {missing:?}\nsteps the engine refused:\n{}",
        engine.join("\n")
    );
}
