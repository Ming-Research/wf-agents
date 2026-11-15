//! The versions of the flows works ran on: each kept once, by its digest; and a work's records, which keep each cell
//! as it was recorded and say from which place on the work ran on which version, read as the version it runs on now
//! names its cells (principles `the-past-as-it-ran` and `cell-numbering`).

use std::collections::BTreeMap;

use decide::{CellId, Seq};
use store::{Fault, FlowVersion};

use super::*;

/// A version of the judged flow, under this digest.
fn version(digest: &str) -> FlowVersion {
    FlowVersion {
        version: digest.to_string(),
        name: "judged".to_string(),
        files: [("flow.toml".to_string(), format!("name = \"judged\" # {digest}"))].into(),
    }
}

/// Each id of these pairs, by the id it is read as.
fn ids(pairs: &[(&str, &str)]) -> BTreeMap<CellId, CellId> {
    pairs
        .iter()
        .map(|(a, b)| (CellId::from(*a), CellId::from(*b)))
        .collect()
}

/// The cell of each run, as the file holds them.
fn recorded_cells(path: &Path) -> Vec<String> {
    let db = Connection::open(path).expect("the database opens");
    let mut query = db
        .prepare("SELECT cell FROM started ORDER BY at")
        .expect("the runs are read");
    let rows = query.query_map([], |row| row.get(0)).expect("the runs are read");
    rows.collect::<Result<_, _>>().expect("the runs are read")
}

#[test]
fn a_version_is_kept_once_and_read_back_as_it_was_kept() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.version("a"), Ok(None));
    store.keep_version(&version("a")).expect("the version is kept");
    let before = dump(&db.path);
    // Kept again, it is kept as it was: the first is the version.
    let again = FlowVersion {
        name: "other".to_string(),
        ..version("a")
    };
    store.keep_version(&again).expect("keeping it again is no fault");
    assert_eq!(dump(&db.path), before);
    assert_eq!(db.open().version("a"), Ok(Some(version("a"))));
}

// A work put on a version that numbers its cells anew reads every run recorded before as the cell of that version, in
// memory and as a store opened anew loads it, and so does an observation of one; the file keeps each as it was
// recorded.
#[test]
fn a_work_put_on_a_renumbered_version_reads_what_it_recorded_before_as_that_versions_cells() {
    let mut d = Driver::saved();
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.keep_version(&version("a")).expect("the version is kept");
    s.store.keep_version(&version("b")).expect("the version is kept");
    d.work.flows.push(work::OnFlow {
        at: Seq(0),
        version: "a".to_string(),
        ids: BTreeMap::new(),
    });
    d.wrote("request", Outsider::Person, None);
    d.end(Came {
        said: super::observations::observing(&["The writer's."]),
        ..hands_in(&[("text", None)])
    });
    d.end(Came::asks(vec![question(Who::Owner)]));
    d.work.onto(
        Time(d.clock),
        "b".to_string(),
        ids(&[("write", "draft"), ("judge", "review")]),
    );
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.save(s.work, &d.work).expect("the work is saved on the version");
    let cells: Vec<&str> = d.work.records.started.iter().map(|s| s.cell.as_str()).collect();
    assert_eq!(cells, ["draft", "review"]);
    assert_eq!(s.db.open().load(s.work), Ok(d.work.clone()));
    assert_eq!(recorded_cells(&s.db.path), ["write", "judge"]);
    let observed: Vec<String> = super::observations::kept(&s.db.path)
        .into_iter()
        .map(|o| o.cell)
        .collect();
    assert_eq!(observed, ["draft"]);
}

// A work given to be saved with a run it recorded before a version it was put on, neither of which the store holds,
// would be kept as the version reads it: it is refused, and nothing is kept.
#[test]
fn a_run_not_yet_kept_before_a_version_the_work_was_put_on_is_refused() {
    let mut d = Driver::saved();
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.keep_version(&version("b")).expect("the version is kept");
    d.wrote("request", Outsider::Person, None);
    let mut given = d.work.clone();
    let mut run = given.records.started[0].clone();
    run.at = given.tick();
    run.run = decide::RunId(99);
    given.records.started.push(run.clone());
    given.onto(Time(d.clock), "b".to_string(), ids(&[("write", "draft")]));
    let s = d.saving.as_ref().expect("saved after each event");
    let before = dump(&s.db.path);
    let mut store = s.db.open();
    assert_eq!(
        store.save(s.work, &given),
        Err(Fault::ReadAsMoved {
            table: "started",
            at: run.at
        })
    );
    assert_eq!(dump(&s.db.path), before);
}
