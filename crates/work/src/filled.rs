//! What was filled into the sessions of a work's rounds, as its records keep it (design document `data-and-context`,
//! 13 to 15): what a session holds is read from here, and from what the rounds that ran in it handed in, never worked
//! out again.

use decide::{BlockId, CallId, RunId, Seq};
use serde::{Deserialize, Serialize};

use crate::Work;

/// What was filled into the session of a try of a round, at a place of its own in the order of what happened to the
/// work, so that what the session held at any place can be read. The try is one [`Ran`](crate::Ran) records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filled {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub run: RunId,
    /// The round's number in its run, the first being one.
    pub round: usize,
    /// The try's number in its round, the first being one.
    pub attempt: usize,
    /// What was filled, in the order it was filled; none where the session lacked nothing.
    pub fills: Vec<Fill>,
}

/// One thing filled into a session, because the round's cell is given it and the session lacked it (design document
/// `data-and-context`, 14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fill {
    /// Which work, and which item it is done for, the session is for, by their numbers.
    For { work: u64, item: u64 },
    /// The cell's task, as the flow the round ran on has it, by the digest of its text ([`Fill::task`]), by which a
    /// task the flow changed since is told apart.
    Task { digest: String },
    /// A version of a block, whole: the version is named by the place of the entry that wrote it.
    Whole { block: BlockId, version: Seq },
    /// A version of a block, as what changed in it since an earlier version, which the session held.
    Change { block: BlockId, version: Seq, since: Seq },
    /// The results of these calls: those the cell's last round asked for (design document `data-and-context`, 28).
    Results(Vec<CallId>),
}

impl Fill {
    /// The task of this text filled: its digest is the SHA-1 of the text, in hexadecimal. It tells two texts apart,
    /// and keeps no text of its own (principle `stored-content-is-kept-small`).
    pub fn task(text: &str) -> Fill {
        Fill::Task {
            digest: sha1_smol::Sha1::from(text).digest().to_string(),
        }
    }
}

/// What is wrong with what a work says was filled, each by the place of the filling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FillFault {
    /// The filling names a try of a round that no record of a try names.
    NoTry(Seq),
    /// The filling fills a block at a version that is no version of it.
    NoVersion(Seq),
    /// The filling fills a block as what changed since a version that is not earlier than the one filled.
    NotSince(Seq),
}

/// Whether what the work says was filled can be: each filling names a try its records hold, and each version it fills
/// is a version of its block, a change filled since an earlier one.
pub fn check(work: &Work) -> Result<(), FillFault> {
    let records = &work.records;
    for f in &work.filled {
        let tried = (work.ran.iter()).any(|r| r.run == f.run && r.round == f.round && r.attempt == f.attempt);
        if !tried {
            return Err(FillFault::NoTry(f.at));
        }
        for fill in &f.fills {
            let (block, version, since) = match fill {
                Fill::Whole { block, version } => (block, *version, None),
                Fill::Change { block, version, since } => (block, *version, Some(*since)),
                Fill::For { .. } | Fill::Task { .. } | Fill::Results(_) => continue,
            };
            if !std::iter::once(version).chain(since).all(|v| records.wrote(block, v)) {
                return Err(FillFault::NoVersion(f.at));
            }
            if since.is_some_and(|since| since >= version) {
                return Err(FillFault::NotSince(f.at));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use decide::{Outsider, Round, Started, Written};

    use super::*;
    use crate::{Ran, RanOn};

    /// A work in which a person wrote `p` twice, at 1 and 2, and a run began at 3 with its round at 4, whose first try
    /// began at 5, and its session was filled at 6 with these.
    fn work(fills: Vec<Fill>) -> Work {
        let mut w = Work::default();
        for at in [1, 2] {
            w.records.written.push(Written {
                block: "p".into(),
                by: Outsider::Person,
                matters: true,
                value: None,
                at: Seq(at),
            });
        }
        let run = RunId(1);
        w.records.started.push(Started {
            run,
            cell: "a".into(),
            at: Seq(3),
            because: vec![],
            basis: [("p".into(), Some(Seq(2)))].into(),
        });
        w.records.rounds.push(Round { run, at: Seq(4) });
        w.ran.push(Ran {
            at: Seq(5),
            run,
            round: 1,
            attempt: 1,
            on: RanOn {
                level: "2".into(),
                model: "a model".into(),
                effort: None,
                account: None,
            },
        });
        w.filled.push(Filled {
            at: Seq(6),
            run,
            round: 1,
            attempt: 1,
            fills,
        });
        w
    }

    fn whole(version: u64) -> Fill {
        Fill::Whole {
            block: "p".into(),
            version: Seq(version),
        }
    }

    fn change(version: u64, since: u64) -> Fill {
        Fill::Change {
            block: "p".into(),
            version: Seq(version),
            since: Seq(since),
        }
    }

    #[test]
    fn what_a_try_can_have_been_filled_with_is_no_fault() {
        let fills = vec![
            Fill::For { work: 1, item: 1 },
            Fill::task("Write the text."),
            whole(1),
            change(2, 1),
            Fill::Results(vec![]),
        ];
        assert_eq!(check(&work(fills)), Ok(()));
        assert_eq!(check(&work(vec![])), Ok(()));
    }

    #[test]
    fn a_filling_of_a_try_no_record_names_is_a_fault() {
        for (run, round, attempt) in [(2, 1, 1), (1, 2, 1), (1, 1, 2)] {
            let mut w = work(vec![whole(2)]);
            let f = &mut w.filled[0];
            (f.run, f.round, f.attempt) = (RunId(run), round, attempt);
            assert_eq!(check(&w), Err(FillFault::NoTry(Seq(6))), "{run} {round} {attempt}");
        }
    }

    #[test]
    fn a_version_filled_that_is_no_version_of_its_block_is_a_fault() {
        assert_eq!(check(&work(vec![whole(3)])), Err(FillFault::NoVersion(Seq(6))));
        let other = Fill::Whole {
            block: "q".into(),
            version: Seq(2),
        };
        assert_eq!(check(&work(vec![other])), Err(FillFault::NoVersion(Seq(6))));
        assert_eq!(check(&work(vec![change(3, 1)])), Err(FillFault::NoVersion(Seq(6))));
        assert_eq!(check(&work(vec![change(2, 4)])), Err(FillFault::NoVersion(Seq(6))));
    }

    #[test]
    fn a_change_filled_since_a_version_not_earlier_than_its_own_is_a_fault() {
        assert_eq!(check(&work(vec![change(1, 2)])), Err(FillFault::NotSince(Seq(6))));
        assert_eq!(check(&work(vec![change(2, 2)])), Err(FillFault::NotSince(Seq(6))));
    }

    #[test]
    fn a_task_is_filled_by_the_digest_of_its_text() {
        // The SHA-1 of "abc", as its standard gives it.
        assert_eq!(
            Fill::task("abc"),
            Fill::Task {
                digest: "a9993e364706816aba3e25717850c26c9cd0d89d".into()
            }
        );
        assert_eq!(Fill::task("Write the text."), Fill::task("Write the text."));
        assert_ne!(Fill::task("Write the text."), Fill::task("Write the text again."));
    }
}
