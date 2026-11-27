//! The versions of the flows works run on: each kept once, each file once, however many versions hold it; a work's
//! entries in `on_flow` and what its time went to read back as written; the records keep every name as recorded, and a
//! work read through its moves saves as it was read (decision `a-move-is-read-through-the-renumberings`).

use decide::{BlockId, CellId, Reading};
use store::{Divergence, Entry, KeptFlow, KeptOutline, SCHEMA_VERSION};
use work::{KeptTime, OnFlow, Stretch, TurnSpan, Went};

use super::diverged::{Change, refused};
use super::opening::{
    after_reads_were_kept, before_held_calls, before_jobs_said_how_long_they_queued, before_pauses_waited_for_items,
    before_unpriced, columns, versions,
};
use super::*;

/// A version of the judged flow as the store is given it: these files, each with a digest of its own making, and an
/// outline that is the version's name in words.
fn kept(version: &str, files: &[(&str, &str)]) -> KeptFlow {
    KeptFlow {
        version: version.to_string(),
        name: "judged".to_string(),
        files: (files.iter())
            .map(|(path, text)| (path.to_string(), (format!("digest of {text}"), text.to_string())))
            .collect(),
        outline: format!("{{\"of\": \"{version}\"}}"),
    }
}

/// The rows of a table of the file.
fn count(path: &Path, table: &str) -> u64 {
    let db = Connection::open(path).expect("the database opens");
    (db.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0))).expect("the rows are counted")
}

#[test]
fn a_version_kept_twice_is_kept_once_and_a_file_two_versions_hold_once() {
    let db = Db::new();
    let mut store = db.open();
    let a = kept("a", &[("flow.toml", "one"), ("cells/S1.toml", "start")]);
    let b = kept("b", &[("flow.toml", "two"), ("cells/S1.toml", "start")]);
    store.keep_flow(&a).expect("a is kept");
    store.keep_flow(&a).expect("a is kept again, as nothing");
    store.keep_flow(&b).expect("b is kept");
    assert_eq!(count(&db.path, "flows"), 2);
    assert_eq!(count(&db.path, "flow_files"), 3, "the file both hold is kept once");
    let outline = |version: &str| KeptOutline {
        version: version.to_string(),
        name: "judged".to_string(),
        outline: format!("{{\"of\": \"{version}\"}}"),
    };
    assert_eq!(db.open().outlines(), Ok(vec![outline("a"), outline("b")]));
}

#[test]
fn a_work_s_moves_and_what_its_time_went_to_read_back_as_written_its_records_as_recorded() {
    let db = Db::new();
    let mut store = db.open();
    store.keep_flow(&kept("a", &[("flow.toml", "one")])).expect("a is kept");
    store.keep_flow(&kept("b", &[("flow.toml", "two")])).expect("b is kept");
    let number = first_work(&mut store);
    let mut d = Driver::new();
    d.work.onto(Time(START), Some("a".to_string()), Reading::default());
    d.wrote("request", Outsider::Person, None);
    d.end(hands_in(&[("text", None)]));
    // Saved as it was recorded, before the move; the move is then saved as the work's only new entry.
    store.save(number, &d.work).expect("the work is saved as recorded");
    let reading = Reading {
        cells: [(CellId::from("write"), CellId::from("drafting"))]
            .into_iter()
            .collect(),
        blocks: [(BlockId::from("text"), BlockId::from("draft"))].into_iter().collect(),
    };
    d.work.onto(Time(START + 60_000), Some("b".to_string()), reading);
    store.save(number, &d.work).expect("the work is saved with its move");
    let stretch = Stretch {
        from: Time(START),
        until: Time(START + 1000),
        went: Went::Model,
    };
    let at = d.work.tick();
    d.work.kept_time.push(KeptTime {
        at,
        stretches: vec![stretch],
        turn: Some(vec![TurnSpan {
            from: Time(START),
            until: Some(Time(START + 500)),
        }]),
        separated: true,
    });
    // A work that held the turn nowhere keeps none held, apart from one kept before they were kept, which keeps none.
    for turn in [Some(Vec::new()), None] {
        let at = d.work.tick();
        d.work.kept_time.push(KeptTime {
            at,
            stretches: Vec::new(),
            turn,
            separated: false,
        });
    }
    store
        .save(number, &d.work)
        .expect("the work read through its move saves again, with what came after");
    let loaded = db.open().load(number).expect("the work loads");
    assert_eq!(loaded.flows, d.work.flows);
    assert_eq!(
        loaded.flows,
        [
            OnFlow {
                at: Seq(1),
                version: Some("a".to_string())
            },
            OnFlow {
                at: d.work.flows[1].at,
                version: Some("b".to_string())
            }
        ]
    );
    assert_eq!(loaded.kept_time, d.work.kept_time);
    // The store keeps every name as it was recorded; read through the move, the records are the work's.
    assert!(loaded.records.started.iter().any(|s| s.cell.as_str() == "write"));
    assert!(loaded.contents.keys().any(|(block, _)| block.as_str() == "text"));
    let mut read = loaded.clone();
    read.read_through(&d.work.readings);
    assert_eq!(read.records, d.work.records);
    assert_eq!(read.contents, d.work.contents);
    assert!(read.records.started.iter().all(|s| s.cell.as_str() != "write"));
}

#[test]
fn a_file_of_version_21_is_migrated_with_no_version_kept_no_work_on_one_and_no_time_kept() {
    let mut d = Driver::new();
    history(&mut d);
    let mut work = d.work;
    before_pauses_waited_for_items(&mut work);
    before_held_calls(&mut work);
    before_unpriced(&mut work);
    let (old, new) = kept_in_21(&work);
    let store = old.open();
    // The file ends at the schema of this build, whichever migrations follow the one that makes the tables below.
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    // A file of version 21 kept no times of the runs of the CI, which version 30 keeps.
    work.timed.clear();
    assert_eq!(store.load(WorkNumber(1)), Ok(after_reads_were_kept(work)));
    for table in ["flow_files", "flows", "on_flow", "kept_time"] {
        assert_eq!(count(&old.path, table), 0, "{table} holds rows");
    }
    assert_eq!(store.outlines(), Ok(Vec::new()));
}

/// The work as a file of version 21 of the schema holds it, and a file of this version that holds it.
fn kept_in_21(work: &Work) -> (Db, Db) {
    super::opening::kept_in(21, work)
}

// A file of version 32 kept no stretches a work held the turn of its project: what it kept of the time of its works is
// read as it was, with none held, which is not the same as a work that held it nowhere.
#[test]
fn a_file_of_version_32_is_migrated_with_its_kept_times_read_as_holding_no_turn() {
    let mut d = Driver::new();
    history(&mut d);
    let mut work = d.work;
    before_held_calls(&mut work);
    before_unpriced(&mut work);
    before_jobs_said_how_long_they_queued(&mut work);
    let at = work.tick();
    work.kept_time.push(KeptTime {
        at,
        stretches: vec![Stretch {
            from: Time(START),
            until: Time(START + 1000),
            went: Went::Owner,
        }],
        turn: None,
        separated: false,
    });
    let (old, new) = super::opening::kept_in(32, &work);
    let store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    assert_eq!(store.load(WorkNumber(1)), Ok(after_reads_were_kept(work)));
    assert_eq!(count(&old.path, "kept_time"), 1);
}

// A file of version 38 told no wait for a slot apart from the others in what it kept of the time of its works, and kept
// no time a job of a run of the CI waited for a runner: what it kept is read as it was, each kept time as one that does
// not tell a wait for a slot apart, which is not the same as one that does and holds none, and each job as one that
// said nothing of it.
#[test]
fn a_file_of_version_38_is_migrated_with_its_kept_times_not_telling_a_slot_apart_and_its_jobs_not_saying_how_long_they_queued()
 {
    let mut d = Driver::new();
    history(&mut d);
    let mut work = d.work;
    let at = work.tick();
    work.kept_time.push(KeptTime {
        at,
        stretches: vec![Stretch {
            from: Time(START),
            until: Time(START + 1000),
            went: Went::Other,
        }],
        turn: Some(Vec::new()),
        separated: true,
    });
    for timed in &mut work.timed {
        for job in &mut timed.reading.jobs {
            job.queued_millis = Some(4_000);
        }
    }
    let mut kept_before = work.clone();
    kept_before.kept_time[0].separated = false;
    before_jobs_said_how_long_they_queued(&mut kept_before);
    let (old, new) = super::opening::kept_in(38, &work);
    let store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    assert_eq!(store.load(WorkNumber(1)), Ok(kept_before));
    assert_eq!(count(&old.path, "slots_set"), 0, "no slots were kept before");
}

#[test]
fn a_file_of_version_23_is_migrated_with_no_fork_kept() {
    let mut d = Driver::new();
    history(&mut d);
    let mut work = d.work;
    before_pauses_waited_for_items(&mut work);
    before_held_calls(&mut work);
    before_unpriced(&mut work);
    let (old, new) = super::opening::kept_in(23, &work);
    let store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    // A file of version 23 kept no times of the runs of the CI, which version 30 keeps.
    work.timed.clear();
    assert_eq!(store.load(WorkNumber(1)), Ok(after_reads_were_kept(work)));
    assert_eq!(count(&old.path, "forked"), 0, "a work kept before has no fork");
}

#[test]
fn the_forks_of_a_work_read_back_as_written_each_with_its_round_and_what_it_took() {
    let db = Db::new();
    let mut store = db.open();
    let number = first_work(&mut store);
    let mut d = Driver::new();
    history(&mut d);
    let run = d.work.records.rounds[0].run;
    let spent = |usd_micros| Spent {
        usd_micros,
        uncached_tokens: 120,
        cache_written_tokens: Some(100),
        cached_tokens: 18_100,
        output_tokens: 30,
        ..Spent::default()
    };
    d.on(Event::Forked {
        run,
        round: 1,
        spent: spent(400_000),
    });
    store.save(number, &d.work).expect("the work is saved with a fork");
    d.on(Event::Forked {
        run,
        round: 1,
        spent: spent(250_000),
    });
    store
        .save(number, &d.work)
        .expect("the work is saved with a second fork");
    assert_eq!(d.work.forked.len(), 2);
    let loaded = db.open().load(number).expect("the work loads");
    assert_eq!(loaded.forked, d.work.forked);
    assert_eq!(
        (loaded.forked[0].usd_micros, loaded.forked[1].usd_micros),
        (400_000, 250_000)
    );
    assert_eq!(loaded, d.work);
    assert_eq!(count(&db.path, "forked"), 2);
}

// A table the store holds is only added to: the moves a work made onto the versions of its flow, what its time went to
// and the forks of its sessions are as refused where a save changes or lacks an entry as those of the records.
#[test]
fn a_work_changed_at_or_lacking_an_entry_of_its_moves_its_time_or_its_forks_is_refused() {
    let db = Db::new();
    let mut store = db.open();
    store.keep_flow(&kept("a", &[("flow.toml", "one")])).expect("a is kept");
    store.keep_flow(&kept("b", &[("flow.toml", "two")])).expect("b is kept");
    let number = first_work(&mut store);
    let mut d = Driver::new();
    d.work.onto(Time(START), Some("a".to_string()), Reading::default());
    history(&mut d);
    let run = d.work.records.rounds[0].run;
    d.on(Event::Forked {
        run,
        round: 1,
        spent: Spent {
            usd_micros: 400_000,
            ..Spent::default()
        },
    });
    d.work
        .onto(Time(START + 60_000), Some("b".to_string()), Reading::default());
    let stretch = Stretch {
        from: Time(START),
        until: Time(START + 1000),
        went: Went::Model,
    };
    let at = d.work.tick();
    d.work.kept_time.push(KeptTime {
        at,
        stretches: vec![stretch],
        turn: None,
        separated: false,
    });
    store.save(number, &d.work).expect("the work is saved");
    let kept = d.work;
    assert_eq!((kept.flows.len(), kept.kept_time.len(), kept.forked.len()), (2, 1, 1));

    let changes: [Change; 3] = [
        |w| {
            let on = &mut w.flows[0];
            on.version = Some("b".to_string());
            ("on_flow", Entry::At(on.at))
        },
        |w| {
            let k = &mut w.kept_time[0];
            k.stretches.pop();
            ("kept_time", Entry::At(k.at))
        },
        |w| {
            let f = &mut w.forked[0];
            f.usd_micros += 1;
            ("forked", Entry::At(f.at))
        },
    ];
    for change in changes {
        let mut given = kept.clone();
        let (table, entry) = change(&mut given);
        refused(&db, &mut store, number, &given, table, entry, Divergence::Changed);
    }
    let removals: [Change; 3] = [
        |w| ("on_flow", Entry::At(w.flows.pop().expect("a move").at)),
        |w| ("kept_time", Entry::At(w.kept_time.pop().expect("a time kept").at)),
        |w| ("forked", Entry::At(w.forked.pop().expect("a fork").at)),
    ];
    for removal in removals {
        let mut given = kept.clone();
        let (table, entry) = removal(&mut given);
        refused(&db, &mut store, number, &given, table, entry, Divergence::Missing);
    }
    assert_eq!(store.load(number), Ok(kept));
}

/// A version of the judged flow whose outline says that the cell `write` has this block as its words, and that any
/// other block, named like the words of no cell or not, is its own: a block is a cell's words by what the outline says
/// of it and not by its name.
fn declaring_words(version: &str, block: &str) -> KeptFlow {
    KeptFlow {
        outline: format!(
            r#"{{"name": "judged", "cells": [{{"id": "write", "words": {{"block": "{block}", "writes": []}}}}, {{"id": "judge"}}]}}"#
        ),
        ..kept(version, &[("flow.toml", version)])
    }
}

/// The versions of the work that a run wrote to this block: each by the block and the place of the run's finish.
fn written_by_runs(work: &Work, block: &str) -> BTreeSet<(BlockId, Seq)> {
    (work.records.finished.iter())
        .flat_map(|finished| {
            (finished.writes.iter())
                .filter(|write| write.block.as_str() == block)
                .map(|write| (write.block.clone(), finished.at))
        })
        .collect()
}

// The migration to version 37 gives a work kept before one place of reads after its latest entry. It holds every version
// of the work but those a run wrote to a block that a cell of a version of the flow the work ran on declares as its
// words: those are the cells' replies, and stay unread. A block is known so by the outline alone, whatever its name; a
// work that ran on no version that was kept has no outline, and everything it has is read.
#[test]
fn a_file_of_version_36_is_migrated_with_every_version_read_but_the_cells_replies_its_outlines_declare() {
    let (flow, later) = (declaring_words("a", "text"), declaring_words("b", "verdict"));
    let mut ran_on_it = Driver::new();
    ran_on_it
        .work
        .onto(Time(START), Some("a".to_string()), Reading::default());
    history(&mut ran_on_it);
    let mut ran_on_none = Driver::new();
    history(&mut ran_on_none);
    // A work that went on with another version of the flow, which declares another block as words, has the words of both.
    let mut ran_on_two = Driver::new();
    ran_on_two
        .work
        .onto(Time(START), Some("a".to_string()), Reading::default());
    history(&mut ran_on_two);
    ran_on_two
        .work
        .onto(Time(START + 1_000_000), Some("b".to_string()), Reading::default());
    let (replies, others) = (
        written_by_runs(&ran_on_it.work, "text"),
        written_by_runs(&ran_on_it.work, "verdict"),
    );
    assert!(
        !replies.is_empty() && !others.is_empty(),
        "the history has no run that wrote the words block or a block that is not"
    );
    let (old, new) = super::opening::kept_in_flows(
        36,
        &[&ran_on_it.work, &ran_on_none.work, &ran_on_two.work],
        &[flow, later],
    );
    let store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    let loaded = store.load(WorkNumber(1)).expect("the work loads");
    // A file of version 36 holds no read, whatever the history the work was made from has: what it holds is what the
    // migration writes.
    let mut expected = ran_on_it.work.clone();
    expected.records.owner_reads.clear();
    let at = expected.tick();
    let read: Vec<(BlockId, Seq)> = (expected.contents.keys())
        .filter(|version| !replies.contains(*version))
        .cloned()
        .collect();
    assert!(
        others.iter().all(|version| read.contains(version)),
        "a block no outline declares as words is left unread"
    );
    expected
        .records
        .owner_reads
        .push(decide::OwnerRead { versions: read, at });
    assert_eq!(loaded, expected);
    // What the migration left unread are the replies and nothing else, and every other version is a row of the table.
    assert_eq!(
        expected.contents.len() - replies.len(),
        expected.records.owner_reads[0].versions.len()
    );
    // The work that ran on both versions of the flow has the replies of both unread: the block each declares as words.
    let mut both = ran_on_two.work.clone();
    both.records.owner_reads.clear();
    let at = both.tick();
    let unread: BTreeSet<(BlockId, Seq)> = written_by_runs(&both, "text")
        .into_iter()
        .chain(written_by_runs(&both, "verdict"))
        .collect();
    assert!(
        unread.len() > replies.len(),
        "the history has no run that wrote the block only the later version declares"
    );
    let read: Vec<(BlockId, Seq)> = (both.contents.keys())
        .filter(|version| !unread.contains(*version))
        .cloned()
        .collect();
    both.records.owner_reads.push(decide::OwnerRead { versions: read, at });
    assert_eq!(store.load(WorkNumber(3)), Ok(both.clone()));
    let other = store.load(WorkNumber(2)).expect("the other work loads");
    assert_eq!(
        count(&old.path, "owner_read") as usize,
        expected.records.owner_reads[0].versions.len()
            + other.records.owner_reads[0].versions.len()
            + both.records.owner_reads[0].versions.len()
    );
    // The work that ran on no version that was kept has no outline: all it has is read, in one entry after its last.
    let mut everything = ran_on_none.work.clone();
    everything.records.owner_reads.clear();
    let at = everything.tick();
    let versions: Vec<(BlockId, Seq)> = everything.contents.keys().cloned().collect();
    everything.records.owner_reads.push(decide::OwnerRead { versions, at });
    assert_eq!(store.load(WorkNumber(2)), Ok(everything));
}
