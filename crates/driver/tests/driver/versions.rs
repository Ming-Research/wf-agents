//! The versions of a flow the works run on: each work is added on the version installed, a work that ended stays on
//! the version it ended on, every other is put on the version installed as the engine starts, what it recorded before
//! read through the renumberings between the two, and the engine does not start on a table that would misread a version
//! a work is on (design tree, `decisions/engine/flow-versions.md`).

use std::collections::BTreeMap;

use decide::{BlockId, Seq};
use driver::{Asked, Misread};
use engine::Why;
use flow::Compiled;
use rusqlite::Connection;
use store::Storage;
use work::Measures;

use super::*;

/// The writer of the flow `staged`, run by the owner's start and a request.
const WRITE: &str = r#"
    title = "Write"
    kind = "agent"
    effort = "high"
    line = "main"
    task = "Write the text the request asks for."
    watches = ["start", "request"]
    outputs = ["text"]
    [kinds]
    start = "last_and_latest"
    request = "last_and_latest"
"#;

/// Its review of the text.
const REVIEW: &str = r#"
    title = "Review"
    kind = "agent"
    effort = "high"
    line = "main"
    task = "Review the text."
    watches = ["text"]
    outputs = ["reviewed"]
    [kinds]
    text = "last_and_latest"
"#;

/// A check of the text, which a later version adds before the review.
const CHECK: &str = r#"
    title = "Check"
    kind = "agent"
    effort = "high"
    line = "main"
    task = "Check the text."
    watches = ["text"]
    outputs = ["checked"]
    [kinds]
    text = "last_and_latest"
"#;

/// The merge, once the review has said it reviewed the text.
const MERGE: &str = r#"
    title = "Merge"
    kind = "engine"
    act = "merge"
    watches = ["reviewed"]
    outputs = ["merged"]
"#;

/// The merge of a version with no review, once the text is written.
const MERGE_TEXT: &str = r#"
    title = "Merge"
    kind = "engine"
    act = "merge"
    watches = ["text"]
    outputs = ["merged"]
"#;

/// A note after the merge, on a line of its own, which a later version adds after the last cell.
const NOTE: &str = r#"
    title = "Note"
    kind = "agent"
    effort = "high"
    line = "other"
    task = "Note what merged."
    watches = ["merged"]
    outputs = ["note"]
    [kinds]
    merged = "last_and_latest"
"#;

/// A review that runs only until it has handed its work in, as the datum `../data/of_a_cell/handed_in.toml` makes for
/// its cell says: `{cell}` stands for the cell's id in small letters.
const REVIEW_ONCE: &str = r#"
    title = "Review"
    kind = "agent"
    effort = "high"
    line = "main"
    task = "Review the text."
    when = { {cell}_handed_in = "no" }
    watches = ["text"]
    outputs = ["reviewed"]
    [kinds]
    text = "last_and_latest"
"#;

/// A version of the flow `staged` with this table of earlier ids, these lines and these blocks beside the request, the
/// start and what merged, each shown, so that one no cell reads has a reader, and these cells in this order.
fn staged(table: &str, lines: &[&str], blocks: &[&str], cells: &[(&str, &str)]) -> Compiled {
    let order: Vec<String> = cells.iter().map(|(id, _)| format!("\"{id}\"")).collect();
    let lines: String = lines.iter().map(|l| format!("[lines.{l}]\n")).collect();
    let blocks: String = blocks.iter().map(|b| format!("[blocks.{b}]\nshown = true\n")).collect();
    let flow = format!(
        "name = \"staged\"\norder = [{}]\nearlier_ids = {table}\n[limits]\nruns = 3\nreturns = 2\ncalls = 3\n{lines}\
         [blocks.request]\npeople = true\n[blocks.merged]\nshown = true\n[blocks.start]\noutside = \"start\"\n\
         writes = {{ text = \"The owner started the work.\" }}\n{blocks}",
        order.join(", ")
    );
    compiled(&flow, cells)
}

/// The first version: a writer, its review and the merge.
fn first() -> Compiled {
    staged(
        "[]",
        &["main"],
        &["text", "reviewed"],
        &[("T1", WRITE), ("T2", REVIEW), ("M1", MERGE)],
    )
}

/// A version that renumbers the review T2 to T3, a new T2, a check, standing before it.
fn renumbered() -> Compiled {
    staged(
        "[{ ids = { T2 = \"T3\" } }]",
        &["main"],
        &["text", "reviewed", "checked"],
        &[("T1", WRITE), ("T2", CHECK), ("T3", REVIEW), ("M1", MERGE)],
    )
}

/// A proofing of the checked text, which a third version adds after the check.
const PROOF: &str = r#"
    title = "Proof"
    kind = "agent"
    effort = "high"
    line = "main"
    task = "Proof the checked text."
    watches = ["checked"]
    outputs = ["proofed"]
    [kinds]
    checked = "last_and_latest"
"#;

/// A third version, after [`renumbered`]: its table holds that version's renumbering and one after it, which moves the
/// review T3 on to T4, a new T3, a proofing, standing before it; the check stays T2.
fn renumbered_twice() -> Compiled {
    staged(
        "[{ ids = { T2 = \"T3\" } }, { ids = { T3 = \"T4\" } }]",
        &["main"],
        &["text", "reviewed", "checked", "proofed"],
        &[
            ("T1", WRITE),
            ("T2", CHECK),
            ("T3", PROOF),
            ("T4", REVIEW),
            ("M1", MERGE),
        ],
    )
}

/// The first version with a note after its last cell, on a line of its own.
fn followed() -> Compiled {
    staged(
        "[]",
        &["main", "other"],
        &["text", "reviewed", "note"],
        &[("T1", WRITE), ("T2", REVIEW), ("M1", MERGE), ("N1", NOTE)],
    )
}

/// A version that takes the review T2 out, the merge taking the text as it is written.
fn shortened() -> Compiled {
    staged(
        "[{ taken_out = [\"T2\"] }]",
        &["main"],
        &["text"],
        &[("T1", WRITE), ("M1", MERGE_TEXT)],
    )
}

/// A version of the flow `staged` laid out one thing to a file, as the repository's flows are, so that a cell's
/// condition may name the datum `../data/of_a_cell/handed_in.toml` makes for it: with this table of earlier ids, the
/// engine's start and the owner's request, these blocks, each shown, and these cells in this order, on the line `main`.
fn laid_out(table: &str, blocks: &[&str], cells: &[(&str, &str)]) -> Compiled {
    let order: Vec<String> = cells.iter().map(|(id, _)| format!("\"{id}\"")).collect();
    let flow = format!(
        "name = \"staged\"\norder = [{}]\ndata = [\"start\", \"request\"]\nearlier_ids = {table}\n\
         [limits]\nruns = 3\nreturns = 2\ncalls = 3\n",
        order.join(", ")
    );
    let mut files: BTreeMap<String, String> = [
        ("lines/main.toml", ""),
        ("../data/start.toml", include_str!("../../../../flow/data/start.toml")),
        ("../data/request.toml", "from = \"owner\"\n"),
        (
            "../data/of_a_cell/handed_in.toml",
            include_str!("../../../../flow/data/of_a_cell/handed_in.toml"),
        ),
    ]
    .iter()
    .map(|(path, text)| (path.to_string(), text.to_string()))
    .collect();
    files.insert(
        flow::waits_file(),
        include_str!("../../../../flow/agents/waits.toml").to_string(),
    );
    for block in blocks {
        files.insert(format!("blocks/{block}.toml"), "shown = true\n".to_string());
    }
    let source = flow::Source {
        laid_out: true,
        flow,
        cells: (cells.iter())
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        files,
        ..Default::default()
    };
    let mut has = has();
    has.sources.insert("handed_in".to_string());
    flow::compile(&source, &has).unwrap_or_else(|faults| panic!("the definition compiles: {faults:#?}"))
}

/// A laid-out version whose review T2 runs only until it has handed its work in.
fn handing() -> Compiled {
    let review = REVIEW_ONCE.replace("{cell}", "t2");
    laid_out(
        "[]",
        &["text", "reviewed", "merged"],
        &[("T1", WRITE), ("T2", &review), ("M1", MERGE)],
    )
}

/// That version with its review renumbered to T3, which runs only until it has handed its work in as T3, and a new
/// T2, a check, before it.
fn handing_renumbered() -> Compiled {
    let review = REVIEW_ONCE.replace("{cell}", "t3");
    laid_out(
        "[{ ids = { T2 = \"T3\" } }]",
        &["text", "reviewed", "checked", "merged"],
        &[("T1", WRITE), ("T2", CHECK), ("T3", &review), ("M1", MERGE)],
    )
}

/// The round of the work under way hands in these blocks, its one try having run on a model and said what its calls
/// came to: a round that is history.
fn measured(h: &mut H, work: WorkNumber, blocks: &[(&str, Option<&str>)]) {
    let (run, ..) = h.round(work);
    let on = engine::RanOn {
        level: "1".to_string(),
        model: "opus".to_string(),
        effort: None,
        directory: None,
        account: None,
    };
    h.on(work, Event::Ran { run, attempt: 1, on });
    let measures = Measures {
        calls: 3,
        added: 100,
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
    h.hands_in(work, blocks);
}

/// The cell of each round of the history of the flow `staged`, in order, as the flow installed names it.
fn history_cells(h: &H) -> Vec<Option<String>> {
    (h.d().history("staged").iter())
        .map(|s| s.cell.as_ref().map(|c| c.to_string()))
        .collect()
}

/// The version a work is on, as the driver holds it.
fn version_of(h: &H, work: WorkNumber) -> String {
    let kept = h.d().work(work).expect("the work is there");
    kept.on.version().expect("the work is on a version kept").to_string()
}

/// The ids of the cells the work's runs started at, as the driver reads them.
fn ran_at(h: &H, work: WorkNumber) -> Vec<String> {
    let kept = h.d().work(work).expect("the work is there");
    (kept.work.records.started.iter()).map(|s| s.cell.to_string()).collect()
}

/// The policy of the lines' sessions under which a new session of a line would begin otherwise than the line's first
/// session began only on the version [`renumbered`] makes: the carriers say so of that session alone, and only where
/// they are asked with that version's flow.
fn changed_on_the_renumbered_version() -> driver::Policy {
    let renumbered = work::version_of(&renumbered().files).version;
    driver::Policy {
        samples: 1,
        model: Box::new(|_, _| Some("opus".to_string())),
        changed: Some(Box::new(move |flow, _, _, _, session| {
            session == 0 && work::version_of(&flow.files).version == renumbered
        })),
        warm: None,
    }
}

/// Why the engine refuses to start on these flows, over the store of the harness.
fn refused_on(h: &H, flows: Vec<Compiled>) -> Fault {
    match open_on_flows(&h.db, Plan::Never, flows, Time(h.clock)).0 {
        Ok(_) => panic!("the engine started"),
        Err(fault) => fault,
    }
}

// Acceptance 1 and 10: a work that ended stays on the version it ended on, which is never compiled again.
#[test]
fn a_work_that_merged_stays_ended_on_its_version_when_a_cell_follows_its_last_and_its_files_no_longer_compile() {
    let mut h = H::with(vec![first()]);
    let (_, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.through(work);
    let version = version_of(&h, work);
    // Its version's files are made so that they no longer compile: nothing compiles a kept version again.
    let db = Connection::open(&h.db.path).expect("the database opens");
    db.execute("UPDATE flow_files SET text = 'not a flow'", [])
        .expect("the files are spoilt");
    let todos = h.install(vec![followed()]);
    assert!(todos.is_empty(), "{todos:?}");
    assert_eq!(h.decision(work), Decision::Ended(End::Merged));
    assert!(h.d().everything_paused().is_none(), "no work goes on");
    let kept = h.d().work(work).expect("the work is there");
    assert!(!kept.on.installed());
    assert_eq!(version_of(&h, work), version);
}

// Acceptance 2: a work going on is put on the version installed, what it recorded read through the renumbering.
#[test]
fn a_work_going_on_is_put_on_the_version_installed_its_runs_read_as_the_cells_they_ran_at_there() {
    let mut h = H::with(vec![first()]);
    let (_, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.hands_in(work, &[("text", None)]);
    assert_eq!(h.round(work).1, "T2");
    let todos = h.install(vec![renumbered()]);
    let kept = h.d().work(work).expect("the work is there");
    assert!(kept.on.installed());
    assert_eq!(ran_at(&h, work), ["T1", "T3"]);
    // The store keeps every name as recorded.
    let stored = h.reader.load(work).expect("the work loads");
    assert_eq!(stored.records.started[1].cell.as_str(), "T2");
    // The round under way is given again at the cell it is at now.
    let at = |t: &Todo| matches!(&t.effect, Effect::Round { cell, .. } if cell.as_str() == "T3");
    assert!(todos.iter().any(at), "{todos:?}");
    h.lift_the_starts_pause();
    h.hands_in(work, &[("reviewed", None)]);
    // The new T2 has no record, and runs.
    assert_eq!(h.round(work).1, "T2");
}

// Acceptance 12: a work put on the version installed is asked, with that version's flow, whether a new session of each
// of its lines would begin otherwise than the line's session began, and a line of which the carriers say so begins one
// before the work's round under way is given again (decision `a-changed-system-prompt-or-form-begins-a-new-session`).
// The tests of sessions show that check on a work that did not move; this one fails where a moved work is asked with
// another flow, of another session, or not at all. Runs in memory in milliseconds.
#[test]
fn a_work_put_on_the_version_installed_begins_anew_a_line_whose_session_began_otherwise_before_its_round_goes_on() {
    let mut h = H::with_policy(vec![first()], SLOTS, Some(changed_on_the_renumbered_version));
    let (_, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.hands_in(work, &[("text", None)]);
    assert_eq!(h.round(work).1, "T2");
    let kept = h.d().work(work).expect("the work is there");
    assert!(kept.work.sessions.is_empty(), "{:?}", kept.work.sessions);
    let todos = h.install(vec![renumbered()]);
    let kept = h.d().work(work).expect("the work is there");
    let moved = kept.work.flows.last().expect("the work was moved").at;
    let begun: Vec<(&str, bool)> = (kept.work.sessions.iter())
        .map(|s| (s.line.as_str(), matches!(s.why, Why::Changed { .. }) && s.at > moved))
        .collect();
    assert_eq!(begun, [("main", true)]);
    let at = |t: &Todo| matches!(&t.effect, Effect::Round { cell, .. } if cell.as_str() == "T3");
    assert!(todos.iter().any(at), "{todos:?}");
}

// Acceptance 17: the engine does not start on a table that would misread a version a work is on.
#[test]
fn the_engine_does_not_start_on_a_table_that_misreads_a_version_a_work_is_on() {
    let mut h = H::with(vec![renumbered()]);
    let (_, work) = h.add(new_item("a", &[], "staged"));
    let version = version_of(&h, work);
    // The renumbering the version holds is rewritten.
    let rewritten = staged(
        "[{ ids = { T2 = \"T4\" } }]",
        &["main"],
        &["text", "reviewed", "checked"],
        &[("T1", WRITE), ("T2", CHECK), ("T4", REVIEW), ("M1", MERGE)],
    );
    let Fault::EarlierIds {
        flow,
        version: of,
        works,
        why,
    } = refused_on(&h, vec![rewritten])
    else {
        panic!("another fault")
    };
    assert_eq!(
        (flow.as_str(), of.as_str(), works, *why),
        ("staged", version.as_str(), vec![work], Misread::Rewritten)
    );

    let mut h = H::with(vec![first()]);
    let (_, work) = h.add(new_item("a", &[], "staged"));
    // T1 is given the id T2 where a T2 stands still: two of the version's cells would read as one.
    let two = staged(
        "[{ ids = { T1 = \"T2\" } }]",
        &["main"],
        &["text", "reviewed"],
        &[("T1", WRITE), ("T2", REVIEW), ("M1", MERGE)],
    );
    let Fault::EarlierIds { works, why, .. } = refused_on(&h, vec![two]) else {
        panic!("another fault")
    };
    assert_eq!(works, [work]);
    assert_eq!(
        *why,
        Misread::TwoAsOne {
            cells: vec!["T1".into(), "T2".into()],
            reads_as: "T2".into()
        }
    );
    // T2 is gone with no renumbering taking it out: it would read as a cell the flow has not.
    let lost = staged("[]", &["main"], &["text"], &[("T1", WRITE), ("M1", MERGE_TEXT)]);
    let Fault::EarlierIds { why, .. } = refused_on(&h, vec![lost]) else {
        panic!("another fault")
    };
    assert_eq!(
        *why,
        Misread::Lost {
            cell: "T2".into(),
            reads_as: "T2".into()
        }
    );
}

// Acceptance 5: works kept before the versions are put on one at the first start, and a work added after is on the
// version installed from its first entry.
#[test]
fn a_work_kept_before_the_versions_is_put_on_none_where_it_ended_and_on_the_version_installed_otherwise() {
    let mut h = H::with(vec![first()]);
    let (_, merged) = h.add_let_on(new_item("a", &[], "staged"));
    h.through(merged);
    let (_, going) = h.add_let_on(new_item("b", &[], "staged"));
    // A work is added on the version installed, its first entry.
    let installed = version_of(&h, going);
    let flows = h.reader.load(going).expect("the work loads").flows;
    assert_eq!(flows.len(), 1);
    assert_eq!(
        (flows[0].at, flows[0].version.as_deref()),
        (Seq(0), Some(installed.as_str()))
    );
    // As kept before the versions: no work says which version it is on.
    let db = Connection::open(&h.db.path).expect("the database opens");
    db.execute("DELETE FROM on_flow", [])
        .expect("the entries are taken out");
    h.install(vec![first()]);
    let on = |h: &H, work| {
        h.reader
            .load(work)
            .expect("the work loads")
            .flows
            .last()
            .map(|on| on.version.clone())
    };
    assert_eq!(on(&h, merged), Some(None));
    assert_eq!(on(&h, going), Some(Some(installed.clone())));
    assert!(!h.d().work(merged).expect("the work is there").on.installed());
    // A later start with a cell after the last leaves the work on no version ended.
    h.install(vec![followed()]);
    assert_eq!(h.decision(merged), Decision::Ended(End::Merged));
    assert!(h.d().work(going).expect("the work is there").on.installed());
}

// Acceptance 6: a work reopened is put on the version installed, its records read through the renumberings since its
// version, or as recorded where it was on none.
#[test]
fn a_work_reopened_on_an_earlier_version_or_on_none_is_put_on_the_version_installed() {
    let mut h = H::with(vec![first()]);
    // Each ran at T1, and was at T2 as it was cancelled.
    let (earlier, on_earlier) = h.add_let_on(new_item("a", &[], "staged"));
    h.hands_in(on_earlier, &[("text", None)]);
    let (unkept, on_none) = h.add_let_on(new_item("b", &[], "staged"));
    h.hands_in(on_none, &[("text", None)]);
    for (item, work) in [(earlier, on_earlier), (unkept, on_none)] {
        let (run, ..) = h.round(work);
        h.changes(vec![(item, Asked::Cancel)]).expect("the item is cancelled");
        // The round under way, stopped, ends, and the work lets go of what it held.
        let spent = h.spent();
        let came = Came::default();
        let todos = h.on(work, Event::Returned { run, came, spent });
        h.released(todos);
    }
    let db = Connection::open(&h.db.path).expect("the database opens");
    db.execute("UPDATE on_flow SET version = NULL WHERE work = ?1", [on_none.0])
        .expect("the work is put on none");
    h.install(vec![renumbered()]);
    for work in [on_earlier, on_none] {
        assert!(!h.d().work(work).expect("the work is there").on.installed());
        assert_eq!(h.decision(work), Decision::Ended(End::Cancelled));
    }
    for item in [earlier, unkept] {
        h.changes(vec![(item, Asked::Reopen)]).expect("the item is reopened");
    }
    let installed = version_of(&h, on_earlier);
    // What the work on the earlier version recorded reads through the renumbering; what the work on none recorded, as
    // recorded, the nearest reading there is.
    assert_eq!(ran_at(&h, on_earlier)[..2], ["T1", "T3"]);
    assert_eq!(ran_at(&h, on_none)[..2], ["T1", "T2"]);
    for work in [on_earlier, on_none] {
        assert!(h.d().work(work).expect("the work is there").on.installed());
        let stored = h.reader.load(work).expect("the work loads");
        assert_eq!(
            stored.flows.last().and_then(|on| on.version.clone()),
            Some(installed.clone())
        );
    }
}

// Acceptance 13: a work keeps what its time went to as it ends, and a later install leaves it as it was.
#[test]
fn a_work_that_ends_keeps_what_its_time_went_to_which_a_later_install_leaves_as_it_was() {
    let mut h = H::with(vec![first()]);
    let (_, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.through(work);
    let kept = h.d().work(work).expect("the work is there").work.kept_time.clone();
    assert_eq!(kept.len(), 1, "kept once, as it ended");
    assert!(!kept[0].stretches.is_empty());
    assert!(
        kept[0].separated,
        "what is kept now tells a wait for a slot apart from the others"
    );
    h.install(vec![followed()]);
    assert_eq!(h.d().work(work).expect("the work is there").work.kept_time, kept);
}

// A work cancelled as its round runs has ended, but keeps what its time went to only once that round, stopped, has come
// back, so that what it keeps reaches the round's end; kept as the work ended, it would stop at the cancel. Runs in
// memory in milliseconds.
#[test]
fn a_work_cancelled_as_its_round_runs_keeps_what_its_time_went_to_once_the_round_has_come_back() {
    let mut h = H::with(vec![first()]);
    let (item, work) = h.add_let_on(new_item("a", &[], "staged"));
    let (run, ..) = h.round(work);
    h.changes(vec![(item, Asked::Cancel)]).expect("the item is cancelled");
    assert_eq!(h.decision(work), Decision::Ended(End::Cancelled));
    let kept = |h: &H| h.d().work(work).expect("the work is there").work.kept_time.clone();
    assert!(kept(&h).is_empty(), "nothing is kept while the round is still stopping");
    let back = h.now();
    let returned = Event::Returned {
        run,
        came: Came::default(),
        spent: h.spent(),
    };
    let Outcome::Done(todos) = h.try_on(back, work, returned) else {
        panic!("the driver stopped")
    };
    h.released(todos);
    let kept = kept(&h);
    assert_eq!(kept.len(), 1, "kept once, as the round came back");
    assert_eq!(kept[0].stretches.iter().map(|s| s.until).max(), Some(back));
}

// A cell a work ran at, which the version it is on has taken out since, is read as the version the work was on before
// has it, so that whoever shows the work can title it; the version installed has no such cell. Fails where the cell
// taken out is looked for in the version the work is on alone, or in no version the work passed through. Runs in memory
// in milliseconds.
#[test]
fn a_cell_taken_out_since_a_work_ran_at_it_is_read_as_the_version_it_ran_on_has_it() {
    let mut h = H::with(vec![first()]);
    let (item, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.hands_in(work, &[("text", None)]);
    let (run, ..) = h.round(work);
    h.changes(vec![(item, Asked::Cancel)]).expect("the item is cancelled");
    let spent = h.spent();
    let todos = h.on(
        work,
        Event::Returned {
            run,
            came: Came::default(),
            spent,
        },
    );
    h.released(todos);
    h.install(vec![shortened()]);
    h.changes(vec![(item, Asked::Reopen)]).expect("the item is reopened");
    assert_eq!(ran_at(&h, work)[..2], ["T1", "T2@1"]);
    let kept = h.d().work(work).expect("the work is there");
    assert!(kept.on.installed());
    let taken_out: decide::CellId = "T2@1".into();
    assert!(
        kept.cell(&taken_out).is_none(),
        "the version installed has no such cell"
    );
    let was = kept.came_from(&taken_out).expect("the cell taken out is read");
    assert_eq!(
        (was.id.as_str(), was.title.as_str(), was.line.as_deref(), was.agent),
        ("T2", "Review", Some("main"), true)
    );
}

// What every work shares is read from the works on an earlier version too, each by the outline of its own version: a
// usage limit met by a work that has ended on one still holds its account once another version is installed. Fails
// where a work no flow decides is left out of the shared waits. Runs in memory in milliseconds.
#[test]
fn a_usage_limit_met_by_a_work_that_ended_on_an_earlier_version_still_stands() {
    let mut h = H::with(vec![first()]);
    let (item, a) = h.add_let_on(new_item("a", &[], "staged"));
    let (_, b) = h.add_let_on(new_item("b", &[], "staged"));
    h.add_account("first");
    h.take_account("first");
    h.ran_under(a, "first");
    h.ran_under(b, "first");
    let resets = Time(h.clock + 3_600_000);
    let (run, ..) = h.round(a);
    let spent = h.spent();
    let failed = Event::Failed {
        run,
        why: "You've hit your limit".to_string(),
        failure: decide::run::Failure::UsageLimit,
        resets: Some(resets),
        spent,
    };
    h.on(a, failed);
    let (_, todos) = h.changes(vec![(item, Asked::Cancel)]).expect("the item is cancelled");
    h.released(todos);
    assert_eq!(h.decision(a), Decision::Ended(End::Cancelled));
    let limit = h.d().limits().get("first").copied().expect("the limit stands");
    assert_eq!(limit.resets, Some(resets));
    h.install(vec![followed()]);
    assert!(!h.d().work(a).expect("the work is there").on.installed());
    assert_eq!(h.d().limits().get("first"), Some(&limit));
    assert!(h.d().outage().expect("the wait stands").holds());
}

// Acceptance 4: a round counts in the history of the cell it is on the version installed, read through the
// renumberings since its own version; a round of a cell taken out since, or of a work on no version, counts in the
// flow's history alone (decision `a-round-is-judged-as-the-cell-it-is-now`).
#[test]
fn a_round_counts_in_the_history_of_the_cell_it_is_now_and_one_of_no_cell_there_in_the_flows_alone() {
    let mut h = H::with(vec![first()]);
    let mut works = Vec::new();
    for title in ["a", "b"] {
        let (_, work) = h.add_let_on(new_item(title, &[], "staged"));
        measured(&mut h, work, &[("text", None)]);
        measured(&mut h, work, &[("reviewed", None)]);
        h.through(work);
        works.push(work);
    }
    let t = |id: &str| Some(id.to_string());
    assert_eq!(history_cells(&h), [t("T1"), t("T2"), t("T1"), t("T2")]);
    // The second work is put on no version, as one kept before the versions that had ended.
    let db = Connection::open(&h.db.path).expect("the database opens");
    db.execute("UPDATE on_flow SET version = NULL WHERE work = ?1", [works[1].0])
        .expect("the work is put on none");
    h.install(vec![renumbered()]);
    assert_eq!(history_cells(&h), [t("T1"), t("T3"), None, None]);
    h.install(vec![shortened()]);
    assert_eq!(history_cells(&h), [t("T1"), None, None, None]);
}

// Acceptance 2: a datum the engine made for a cell follows its cell through a move, so that a condition on it reads on
// the version installed as it read on the version the work was on (decision `a-datum-made-for-a-cell-follows-its-cell`).
#[test]
fn a_datum_made_for_a_cell_follows_its_cell_through_the_renumbering_and_its_condition_reads_as_it_did() {
    let mut h = H::with(vec![handing()]);
    let (_, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.hands_in(work, &[("text", None)]);
    assert_eq!(h.round(work).1, "T2");
    // The owner asks for more while the review is under way, so that the writer runs again once it has handed in.
    h.on(work, wrote("request", Outsider::Person, None));
    h.hands_in(work, &[("reviewed", None)]);
    assert_eq!(h.round(work).1, "T1");
    h.install(vec![handing_renumbered()]);
    let kept = h.d().work(work).expect("the work is there");
    let says_yes = |name: &str| {
        (kept.work.records.written.iter()).any(|w| w.block == BlockId::from(name) && w.value.as_deref() == Some("yes"))
    };
    assert!(says_yes("t3_handed_in"), "the review's datum reads as T3's");
    assert!(!says_yes("t2_handed_in"), "the check has handed nothing in");
    // The store keeps the datum as it was written.
    let stored = h.reader.load(work).expect("the work loads");
    assert!((stored.records.written.iter()).any(|w| w.block == BlockId::from("t2_handed_in")));
    h.lift_the_starts_pause();
    h.hands_in(work, &[("text", None)]);
    // The check, which has no record, runs on the new text.
    assert_eq!(h.round(work).1, "T2");
    h.hands_in(work, &[("checked", None)]);
    // The review handed its work in as T2, so it does not run again on the new text: the merge comes next.
    assert_eq!(h.round(work).1, "M1");
}

// A work moved at two starts in turn reads what it recorded on each version through the renumberings after that
// version's own, and only those: what it recorded on the first through both, what it recorded on the second through the
// second alone (decision `a-move-is-read-through-the-renumberings`). Fails where the moves kept in the store are not
// read as the work loads, or where a move is read from a version other than the one the work was on before it: the
// check, recorded at T2 of the second version, would then read as T4. Runs in memory in milliseconds.
#[test]
fn a_work_moved_at_two_starts_reads_what_it_recorded_on_each_version_through_the_renumberings_after_that_version() {
    let mut h = H::with(vec![first()]);
    let (_, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.hands_in(work, &[("text", None)]);
    assert_eq!(h.round(work).1, "T2");
    h.install(vec![renumbered()]);
    h.lift_the_starts_pause();
    h.hands_in(work, &[("reviewed", None)]);
    // The check, new on the second version, runs at its T2.
    assert_eq!(h.round(work).1, "T2");
    assert_eq!(ran_at(&h, work), ["T1", "T3", "T2"]);
    let todos = h.install(vec![renumbered_twice()]);
    assert!(h.d().work(work).expect("the work is there").on.installed());
    // The review, recorded at T2 of the first version, reads as T4; the check, recorded at T2 of the second, as T2.
    assert_eq!(ran_at(&h, work), ["T1", "T4", "T2"]);
    let stored = h.reader.load(work).expect("the work loads");
    let recorded: Vec<&str> = stored.records.started.iter().map(|s| s.cell.as_str()).collect();
    assert_eq!(recorded, ["T1", "T2", "T2"], "the store keeps every name as recorded");
    assert_eq!(stored.flows.len(), 3, "added on the first version, moved twice");
    // The check's round under way is given again at the cell it is at now.
    let at = |t: &Todo| matches!(&t.effect, Effect::Round { cell, .. } if cell.as_str() == "T2");
    assert!(todos.iter().any(at), "{todos:?}");
    h.lift_the_starts_pause();
    h.hands_in(work, &[("checked", None)]);
    // The proofing, new at T3, has no record and runs on what the check wrote.
    assert_eq!(h.round(work).1, "T3");
}

// The start check reads every version a work is on, one a work ended on too, whose cells are read through the table
// installed wherever the work is shown and counted (decision `a-table-that-misreads-a-kept-version-is-refused`). Fails
// where only the works the start moves are checked. Runs in memory in milliseconds.
#[test]
fn the_engine_does_not_start_on_a_table_that_misreads_the_version_a_work_ended_on() {
    let mut h = H::with(vec![first()]);
    let (_, work) = h.add_let_on(new_item("a", &[], "staged"));
    h.through(work);
    let version = version_of(&h, work);
    h.install(vec![followed()]);
    assert!(!h.d().work(work).expect("the work is there").on.installed());
    // T1 is given the id T2 where a T2 stands still: the merged work's T1 and T2 would read as one.
    let two = staged(
        "[{ ids = { T1 = \"T2\" } }]",
        &["main"],
        &["text", "reviewed"],
        &[("T1", WRITE), ("T2", REVIEW), ("M1", MERGE)],
    );
    let Fault::EarlierIds {
        flow,
        version: of,
        works,
        why,
    } = refused_on(&h, vec![two])
    else {
        panic!("another fault")
    };
    assert_eq!(
        (flow.as_str(), of.as_str(), works),
        ("staged", version.as_str(), vec![work])
    );
    assert_eq!(
        *why,
        Misread::TwoAsOne {
            cells: vec!["T1".into(), "T2".into()],
            reads_as: "T2".into()
        }
    );
}
