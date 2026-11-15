//! The versions of the flows works run on (principles `the-past-as-it-ran` and `cell-numbering`): a work is added on
//! the version installed and keeps it; as the driver opens on another, a work going on is put on it, each run it
//! recorded read as the cell it ran at through the table of earlier ids, and a work that ended is read and drawn by
//! the version it ran on, its rounds judged as the cells they are on the flow installed; a work reopened goes on on
//! the flow installed; and a flow installed that does not keep the table of a version a work ran on is refused.

use std::collections::BTreeMap;
use std::path::Path;

use decide::{CellId, Seq};
use driver::Asked;
use engine::Work;
use flow::Compiled;
use rusqlite::Connection;
use store::Storage;
use work::{Measures, OnFlow};

use super::*;

/// The flow, its cells numbered anew: each of `ids`, an id and the one it takes, takes it with its file and its place
/// in the order, and the table of earlier ids gains the renumbering.
fn renumbered(flow: &Compiled, ids: &[(&str, &str)]) -> Compiled {
    let mut source = flow.source.clone();
    for (was, now) in ids {
        let text = source.cells.remove(*was).expect("the cell is there");
        source.cells.insert(now.to_string(), text);
        source.flow = source.flow.replace(&format!("\"{was}\""), &format!("\"{now}\""));
    }
    let table: Vec<String> = ids.iter().map(|(was, now)| format!("{now} = \"{was}\"")).collect();
    source
        .flow
        .push_str(&format!("\n[[earlier_ids]]\n{}\n", table.join("\n")));
    flow.again(&source).expect("the renumbered flow compiles")
}

/// The judged flow with its writer and its judge numbered anew.
fn judged_anew() -> Compiled {
    renumbered(&judged(), &[("write", "compose"), ("judge", "review")])
}

/// The plain flow with its writer numbered anew.
fn plain_anew() -> Compiled {
    renumbered(&plain(), &[("write", "compose")])
}

/// The cell of each run the work's records name, as they read.
fn cells(work: &Work) -> Vec<String> {
    work.records.started.iter().map(|s| s.cell.0.clone()).collect()
}

/// The cell of each run of the work, as the file keeps it.
fn recorded(path: &Path, work: WorkNumber) -> Vec<String> {
    let db = Connection::open(path).expect("the database opens");
    let mut query = (db.prepare("SELECT cell FROM started WHERE work = ?1 ORDER BY at")).expect("the runs are read");
    let rows = query.query_map([work.0], |row| row.get(0)).expect("the runs are read");
    rows.collect::<Result<_, _>>().expect("the runs are read")
}

/// The driver of the harness stopped, and one opened on its file with these flows.
fn opened_on(h: &mut H, flows: Vec<Compiled>) -> (Driver, Vec<Todo>) {
    h.driver = None;
    let (opened, _) = open_on_flows(&h.db, Plan::Never, flows, Time(h.clock + 1000));
    opened.expect("the driver opens")
}

#[test]
fn a_work_is_added_on_the_version_installed_from_its_start_and_the_store_keeps_each_version_once() {
    let mut h = H::new();
    let (_, a) = h.add(new_item("a", &[], "plain"));
    let on = |h: &H, work| h.d().work(work).expect("the work is there").work.flows.clone();
    let start = OnFlow {
        at: Seq(0),
        version: plain().version,
        ids: BTreeMap::new(),
    };
    assert_eq!(on(&h, a), std::slice::from_ref(&start));
    h.arm(Plan::Never);
    let (_, b) = h.add(new_item("b", &[], "plain"));
    assert_eq!(on(&h, b), [start]);
    assert!(
        !h.ops().contains(&Op::KeepVersion),
        "a version kept already is kept again"
    );
    let kept: u64 = (Connection::open(&h.db.path).expect("the database opens"))
        .query_row("SELECT count(*) FROM flows", [], |row| row.get(0))
        .expect("the versions are counted");
    assert_eq!(kept, 1);
    assert_eq!(
        h.reader
            .version(&plain().version)
            .expect("the version is read")
            .map(|v| v.files),
        Some(plain().source.files())
    );
}

// A work going on as the driver opens on a version that numbers its cells anew is put on it from its next place: its
// runs read as the cells they ran at, the round under way is given again as the cell it runs at now, and the store
// loads it so, keeping each run as it was recorded.
#[test]
fn a_work_going_on_is_put_on_the_version_installed_and_its_runs_read_as_the_cells_they_ran_at() {
    let mut h = H::new();
    let (_, work) = h.add_let_on(new_item("a", &[], "judged"));
    h.hands_in(work, &[("text", None)]);
    let anew = judged_anew();
    let (driver, todos) = opened_on(&mut h, vec![anew.clone(), short(), plain()]);
    let kept = driver.work(work).expect("the work is there");
    assert_eq!(kept.flow.version, anew.version);
    assert_eq!(cells(kept.work), ["compose", "review"]);
    let moved = kept.work.flows.last().expect("the work is on a version");
    assert_eq!(moved.version, anew.version);
    assert!(kept.work.records.started.iter().all(|s| s.at < moved.at));
    let review = (todos.iter()).any(|t| matches!(&t.effect, Effect::Round { cell, .. } if cell.as_str() == "review"));
    assert!(
        review,
        "the judge's round is not given as the cell it runs at now: {todos:?}"
    );
    assert_eq!(h.db.open().load(work).as_ref(), Ok(kept.work));
    assert_eq!(recorded(&h.db.path, work), ["write", "judge"]);
    assert!(kept.ids.is_empty());
}

// A work that ended is read by the version it ran on, though the one installed numbers its cells anew; the rounds of
// its cells are judged as the cells they are on the flow installed.
#[test]
fn a_work_that_ended_is_read_by_the_version_it_ran_on_and_its_rounds_judged_as_the_cells_installed() {
    let mut h = H::new();
    let (_, work) = h.add_let_on(new_item("a", &[], "judged"));
    let (run, ..) = h.round(work);
    let measures = Measures {
        calls: 1,
        ..Measures::default()
    };
    h.on(
        work,
        Event::Measured {
            run,
            attempt: 1,
            measures,
        },
    );
    h.through(work);
    let (driver, _) = opened_on(&mut h, vec![judged_anew(), short(), plain()]);
    let kept = driver.work(work).expect("the work is there");
    assert_eq!(kept.flow.version, judged().version);
    assert_eq!(kept.flow.flow, judged().flow);
    assert_eq!(kept.work.flows.len(), 1, "a work that ended is put on no other version");
    assert_eq!(cells(kept.work), ["write", "judge", "merge"]);
    assert_eq!(
        driver.view(work).map(|v| v.decision),
        Some(Decision::Ended(End::Merged))
    );
    let ids = BTreeMap::from([
        (CellId::from("write"), CellId::from("compose")),
        (CellId::from("judge"), CellId::from("review")),
    ]);
    assert_eq!(kept.ids, &ids);
    let judged: Vec<String> = driver.history("judged").into_iter().map(|s| s.cell.0).collect();
    assert_eq!(judged, ["compose"]);
}

// A work that ended on a version and is reopened goes on on the flow installed, its runs read as the cells they ran
// at there.
#[test]
fn a_work_reopened_goes_on_on_the_flow_installed() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    h.let_on(wa);
    let (run, ..) = h.round(wa);
    h.changes(vec![(a, Asked::Cancel)]).expect("a is cancelled");
    let spent = h.spent();
    let todos = h.on(
        wa,
        Event::Returned {
            run,
            came: Came::default(),
            spent,
        },
    );
    h.released(todos);
    let flows = vec![judged(), short(), plain_anew()];
    let (driver, todos) = opened_on(&mut h, flows.clone());
    assert_eq!(todos, []);
    assert_eq!(driver.work(wa).expect("the work").flow.version, plain().version);
    h.driver = Some(driver);
    h.flows = flows;
    h.changes(vec![(a, Asked::Reopen)]).expect("a is reopened");
    let kept = h.d().work(wa).expect("the work is there");
    assert_eq!(kept.flow.version, plain_anew().version);
    assert_eq!(cells(kept.work), ["compose"]);
    let todos = h.let_on(wa);
    assert_eq!(rounds(&todos), [(wa, "compose".to_string(), 1)]);
}

// A work kept before the versions were is put on the version installed as the driver first opens on it, everything it
// recorded read as recorded before any renumbering.
#[test]
fn a_work_kept_before_the_versions_were_is_put_on_the_version_installed() {
    let mut h = H::new();
    let (_, work) = h.add_let_on(new_item("a", &[], "plain"));
    h.driver = None;
    (Connection::open(&h.db.path).expect("the database opens"))
        .execute("DELETE FROM on_flow", [])
        .expect("the work is made one kept before the versions were");
    let anew = plain_anew();
    let (driver, _) = opened_on(&mut h, vec![judged(), short(), anew.clone()]);
    let kept = driver.work(work).expect("the work is there");
    assert_eq!(kept.flow.version, anew.version);
    let on: Vec<&str> = kept.work.flows.iter().map(|f| f.version.as_str()).collect();
    assert_eq!(on, [anew.version.as_str()]);
    assert_eq!(cells(kept.work), ["compose"]);
    assert_eq!(h.db.open().load(work).as_ref(), Ok(kept.work));
}

// The table of earlier ids is only added to: a flow installed whose table does not begin with that of the version a
// work ran on is refused, and so is a version kept that does not compile now.
#[test]
fn a_flow_that_rewrites_the_table_of_a_version_a_work_ran_on_or_a_version_that_does_not_compile_is_refused() {
    let anew = plain_anew();
    let mut h = H::with(vec![judged(), short(), anew.clone()]);
    let (_, work) = h.add_let_on(new_item("a", &[], "plain"));
    h.through(work);
    h.driver = None;
    let mut source = anew.source.clone();
    source.flow = source.flow.replace("compose = \"write\"", "compose = \"draft\"");
    let rewritten = anew.again(&source).expect("the rewritten flow compiles");
    let (opened, _) = open_on_flows(&h.db, Plan::Never, vec![judged(), short(), rewritten], Time(h.clock));
    assert_eq!(
        opened.err(),
        Some(Fault::EarlierIdsRewritten {
            work,
            flow: "plain".to_string(),
            version: anew.version.clone(),
        })
    );
    // Ended on its version, which the store keeps and which does not compile now, it cannot be read.
    (Connection::open(&h.db.path).expect("the database opens"))
        .execute(
            "UPDATE flows SET files = '{\"flow.toml\": \"name = 1\"}' WHERE version = ?1",
            [&anew.version],
        )
        .expect("the version kept is made one that does not compile");
    let (opened, _) = open_on_flows(&h.db, Plan::Never, vec![judged(), short(), plain()], Time(h.clock));
    assert!(
        matches!(opened.err(), Some(Fault::VersionUnreadable { work: w, .. }) if w == work),
        "a version that does not compile is not refused"
    );
}
