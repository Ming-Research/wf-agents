//! What the owner read of the work's blocks is kept (decision the-engine-keeps-what-the-owner-read): each version read
//! is named by its block and the place of the entry that wrote it, once, and it changes nothing the work is to do.

use decide::OwnerRead;

use super::*;

#[test]
fn a_version_the_owner_read_is_kept_once_by_the_place_that_wrote_it_and_changes_nothing_the_work_does() {
    let (mut w, write, _) = W::requested();
    w.returns(write, hands_in(&[("text", None)]));
    let wrote = w.last_finished();
    let decided = w.decision();
    w.on(Event::Read {
        versions: vec![("text".into(), 1)],
    });
    assert_eq!(w.decision(), decided, "reading changed what the work does");
    let [kept] = w.work.records.owner_reads.as_slice() else {
        panic!("one read is kept: {:?}", w.work.records.owner_reads);
    };
    assert_eq!(kept.versions, vec![("text".into(), wrote)]);
    assert!(kept.at > wrote, "the read is an entry of its own, after what it read");
    // Read again, it is kept no more: what the owner read is read.
    w.on(Event::Read {
        versions: vec![("text".into(), 1)],
    });
    assert_eq!(w.work.records.owner_reads.len(), 1);
}

#[test]
fn a_read_of_nothing_is_taken_and_adds_no_entry_and_takes_no_place() {
    let (mut w, write, _) = W::requested();
    w.returns(write, hands_in(&[("text", None)]));
    let place = w.work.records.next_place();
    w.on(Event::Read { versions: Vec::new() });
    assert!(
        w.work.records.owner_reads.is_empty(),
        "a read of nothing kept an entry: {:?}",
        w.work.records.owner_reads
    );
    assert_eq!(w.work.records.next_place(), place, "a read of nothing took a place");
}

#[test]
fn versions_read_together_are_one_entry_in_the_order_the_store_reads_them_back() {
    let (mut w, write, first_request) = W::requested();
    w.returns(write, hands_in(&[("text", None)]));
    let (_, written) = w.person_writes("request");
    w.on(Event::Read {
        versions: vec![("text".into(), 1), ("request".into(), 2), ("request".into(), 1)],
    });
    let [kept] = w.work.records.owner_reads.as_slice() else {
        panic!("one read is kept");
    };
    let expected: Vec<(BlockId, Seq)> = vec![
        ("request".into(), first_request),
        ("request".into(), written),
        ("text".into(), w.last_finished()),
    ];
    assert_eq!(kept.versions, expected);
}

#[test]
fn a_read_of_a_version_the_work_has_not_is_refused_and_keeps_nothing_of_what_it_read() {
    let (mut w, write, _) = W::requested();
    w.returns(write, hands_in(&[("text", None)]));
    for (block, number) in [("text", 2), ("text", 0), ("elsewhere", 1)] {
        let why = w.refused(Event::Read {
            versions: vec![("text".into(), 1), (block.into(), number)],
        });
        assert_eq!(
            why,
            Refused::NoSuchBlockVersion {
                block: block.into(),
                version: number
            },
            "a read of {block} {number}"
        );
    }
    assert!(w.work.records.owner_reads.is_empty());
}

#[test]
fn the_owner_reads_a_work_that_has_ended_too() {
    let (mut w, write, _) = W::requested();
    w.returns(write, hands_in(&[("text", None)]));
    w.on(Event::Cancelled { why: None });
    assert!(w.work.records.cancelled().is_some());
    w.on(Event::Read {
        versions: vec![("text".into(), 1)],
    });
    let wrote = w.last_finished();
    let [OwnerRead { versions, .. }] = w.work.records.owner_reads.as_slice() else {
        panic!("one read is kept");
    };
    assert_eq!(*versions, vec![("text".into(), wrote)]);
}
