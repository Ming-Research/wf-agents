//! A work that is not what the store holds with more added is refused, saying at which entry of which table, and
//! the database is left as it was.

use decide::run::Limit;
use decide::{BlockId, Seq};
use engine::{Back, Call, Given, Moment};
use store::{Divergence, Entry, Fault};

use super::*;

/// A store holding the work of the history, and that work.
fn held() -> (Db, Store, WorkNumber, Work) {
    let mut d = Driver::new();
    history(&mut d);
    held_as(d.work)
}

/// A store holding the work, and the work.
fn held_as(kept: Work) -> (Db, Store, WorkNumber, Work) {
    let db = Db::new();
    let mut store = db.open();
    let work = first_work(&mut store);
    store.save(work, &kept).expect("the work is saved");
    (db, store, work, kept)
}

/// Each way of changing an entry the store holds: the table that holds it, and the entry it changes.
type Change = fn(&mut Work) -> (&'static str, Entry);

fn changes() -> Vec<Change> {
    vec![
        |w| {
            let e = &mut w.records.started[0];
            e.cell = "judge".into();
            ("started", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.started[0];
            e.basis.insert("elsewhere".into(), None);
            ("started", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.rounds[0];
            e.run = RunId(999);
            ("rounds", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.returned[0];
            e.run = RunId(999);
            ("returned", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.called[0];
            e.on = if e.on.is_some() { None } else { Some(Who::Owner) };
            ("called", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.done[0];
            e.call = CallId(999);
            ("done", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.finished[0];
            e.writes[0].meaning_unchanged = !e.writes[0].meaning_unchanged;
            ("finished", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.finished[0];
            e.dealt_with = Some(vec![]);
            ("finished", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.abandoned[0];
            e.run = RunId(999);
            ("abandoned", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.written[0];
            e.matters = !e.matters;
            ("written", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.questions[0];
            e.waits = !e.waits;
            ("questions", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.answers[0];
            e.option += 1;
            ("answers", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.let_go[0];
            e.cell = "judge".into();
            ("let_go", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.pauses[0];
            e.paused = !e.paused;
            ("pauses", Entry::At(e.at))
        },
        |w| {
            let at = w.records.cancelled.expect("cancelled");
            w.records.cancelled = Some(Seq(at.0 + 1000));
            ("cancelled", Entry::At(at))
        },
        |w| {
            let at = w.records.released.expect("released");
            w.records.released = Some(Seq(at.0 + 1000));
            ("released", Entry::At(at))
        },
        |w| {
            let ((block, at), content) = w.contents.iter_mut().next().expect("a content");
            content.push_str(", changed");
            let entry = Entry::Version {
                block: block.clone(),
                at: *at,
            };
            ("contents", entry)
        },
        |w| {
            let (&(run, round), given) = w.given.iter_mut().next().expect("a round given");
            *given = Given::Results(vec![]);
            ("given", Entry::Round { run, round })
        },
        |w| {
            let (&(run, round), back) = w.back.iter_mut().next().expect("a round come back");
            *back = match back {
                Back::Asked(_) => Back::HandedIn(vec![]),
                Back::HandedIn(_) | Back::Failed { .. } | Back::Returned { .. } => Back::Asked(vec![]),
            };
            ("back", Entry::Round { run, round })
        },
        |w| {
            let (call, what) = w.calls.iter_mut().next().expect("a call");
            *what = Call::AtLimit(Limit::Calls { made: 99 });
            ("calls", Entry::Call(*call))
        },
        |w| {
            let (call, result) = w.results.iter_mut().next().expect("a result");
            result.push_str(", changed");
            ("results", Entry::Call(*call))
        },
        |w| {
            let (&(run, round), spent) = w.spent.iter_mut().next().expect("a cost");
            spent.usd_micros += 1;
            ("spent", Entry::Round { run, round })
        },
        |w| {
            let m = &mut w.moments[0];
            m.at = Time(0);
            ("moments", Entry::At(m.from))
        },
        |w| {
            let s = &mut w.sessions[0];
            s.line = "other".to_string();
            ("sessions", Entry::At(s.at))
        },
        |w| {
            let r = &mut w.ran[0];
            r.on.model = "another model".to_string();
            ("ran", Entry::At(r.at))
        },
    ]
}

/// Each way of taking out an entry the store holds: the table that holds it, and the entry taken out.
fn removals() -> Vec<Change> {
    vec![
        |w| ("started", Entry::At(w.records.started.pop().expect("a run").at)),
        |w| ("rounds", Entry::At(w.records.rounds.pop().expect("a round").at)),
        |w| ("returned", Entry::At(w.records.returned.pop().expect("a return").at)),
        |w| ("called", Entry::At(w.records.called.pop().expect("a call").at)),
        |w| ("done", Entry::At(w.records.done.pop().expect("a result").at)),
        |w| ("finished", Entry::At(w.records.finished.pop().expect("a finish").at)),
        |w| {
            (
                "abandoned",
                Entry::At(w.records.abandoned.pop().expect("an abandoning").at),
            )
        },
        |w| ("written", Entry::At(w.records.written.pop().expect("a write").at)),
        |w| {
            (
                "questions",
                Entry::At(w.records.questions.pop().expect("a question").at),
            )
        },
        |w| ("answers", Entry::At(w.records.answers.pop().expect("an answer").at)),
        |w| ("let_go", Entry::At(w.records.let_go.pop().expect("a let-go").at)),
        |w| ("pauses", Entry::At(w.records.pauses.pop().expect("a pause").at)),
        |w| ("cancelled", Entry::At(w.records.cancelled.take().expect("cancelled"))),
        |w| ("released", Entry::At(w.records.released.take().expect("released"))),
        |w| {
            let ((block, at), _) = w.contents.pop_last().expect("a content");
            ("contents", Entry::Version { block, at })
        },
        |w| {
            let ((run, round), _) = w.given.pop_last().expect("a round given");
            ("given", Entry::Round { run, round })
        },
        |w| {
            let ((run, round), _) = w.back.pop_last().expect("a round come back");
            ("back", Entry::Round { run, round })
        },
        |w| ("calls", Entry::Call(w.calls.pop_last().expect("a call").0)),
        |w| ("results", Entry::Call(w.results.pop_last().expect("a result").0)),
        |w| {
            let ((run, round), _) = w.spent.pop_last().expect("a cost");
            ("spent", Entry::Round { run, round })
        },
        |w| ("moments", Entry::At(w.moments.pop().expect("a moment").from)),
        |w| ("sessions", Entry::At(w.sessions.pop().expect("a session").at)),
        |w| ("ran", Entry::At(w.ran.pop().expect("a try").at)),
    ]
}

/// Saving the work is refused as diverged at the entry, and the database is as it was.
fn refused(
    db: &Db,
    store: &mut Store,
    work: WorkNumber,
    given: &Work,
    table: &'static str,
    entry: Entry,
    how: Divergence,
) {
    let before = dump(&db.path);
    let fault = Fault::Diverged { table, entry, how };
    assert_eq!(store.save(work, given), Err(fault));
    assert_eq!(dump(&db.path), before, "a refused save changed the database ({table})");
}

#[test]
fn a_work_changed_at_an_entry_the_store_holds_is_refused_for_every_table_and_nothing_is_saved() {
    let (db, mut store, work, kept) = held();
    let changes = changes();
    let tables: BTreeSet<&str> = changes.iter().map(|change| change(&mut kept.clone()).0).collect();
    assert_eq!(tables.len(), 23, "a table has no change: {tables:?}");
    for change in changes {
        let mut given = kept.clone();
        let (table, entry) = change(&mut given);
        // What is new beside the change is not saved either.
        given
            .contents
            .insert((BlockId::from("text"), Seq(1_000_000)), "new".into());
        given.moments.push(Moment {
            from: Seq(1_000_000),
            at: Time(START * 2),
        });
        refused(&db, &mut store, work, &given, table, entry, Divergence::Changed);
    }
    assert_eq!(store.load(work), Ok(kept));
}

#[test]
fn a_work_lacking_an_entry_the_store_holds_is_refused_for_every_table() {
    let (db, mut store, work, kept) = held();
    for removal in removals() {
        let mut given = kept.clone();
        let (table, entry) = removal(&mut given);
        refused(&db, &mut store, work, &given, table, entry, Divergence::Missing);
    }
    let first = Entry::At(kept.records.started[0].at);
    refused(
        &db,
        &mut store,
        work,
        &Work::default(),
        "started",
        first,
        Divergence::Missing,
    );
    assert_eq!(store.load(work), Ok(kept));
}

#[test]
fn a_work_the_store_does_not_have_is_neither_saved_nor_loaded() {
    let mut store = Store::in_memory().expect("the store opens");
    first_work(&mut store);
    let none = WorkNumber(2);
    assert_eq!(store.save(none, &Work::default()), Err(Fault::NoSuchWork(none)));
    assert_eq!(store.load(none), Err(Fault::NoSuchWork(none)));
}

#[test]
fn a_work_of_the_new_form_changed_at_or_lacking_an_entry_the_store_holds_is_refused() {
    let (db, mut store, work, kept) = held_as(of_the_new_form());
    let changes: Vec<Change> = vec![
        |w| {
            let e = &mut w.records.finished[0];
            e.dealt_with = None;
            ("finished", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.finished[0];
            e.writes[0].meaning_unchanged = !e.writes[0].meaning_unchanged;
            ("finished", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.records.put_in[0];
            e.run = RunId(999);
            ("put_in", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.filled[0];
            e.fills.pop();
            ("filled", Entry::At(e.at))
        },
        |w| {
            let e = &mut w.filled[0];
            e.attempt += 1;
            ("filled", Entry::At(e.at))
        },
    ];
    for change in changes {
        let mut given = kept.clone();
        let (table, entry) = change(&mut given);
        refused(&db, &mut store, work, &given, table, entry, Divergence::Changed);
    }
    let removals: Vec<Change> = vec![
        |w| {
            (
                "put_in",
                Entry::At(w.records.put_in.pop().expect("a version put in").at),
            )
        },
        |w| ("filled", Entry::At(w.filled.pop().expect("a filling").at)),
    ];
    for removal in removals {
        let mut given = kept.clone();
        let (table, entry) = removal(&mut given);
        refused(&db, &mut store, work, &given, table, entry, Divergence::Missing);
    }
    assert_eq!(store.load(work), Ok(kept));
}
