//! Which version of its flow a work runs on (principle `the-past-as-it-ran`), and how what it recorded before it was
//! put on another reads (decision `a-move-is-read-through-the-renumberings`).
//!
//! A work is added on the version of its flow installed then, its first entry, and is put on another as the engine
//! starts with one, or as it is reopened, while it has not ended: each is an entry of its own, [`OnFlow`], which records
//! the version alone. What the records hold before an entry reads on its version through the renumberings of the table
//! of earlier ids after the version before, which the outlines kept of the two say ([`decide::Reading`]); the records
//! keep each name as it was recorded, and whoever holds a work reads them through each entry in turn ([`Work::read`]).

use std::collections::BTreeMap;

use decide::{Reading, Seq};
use serde::{Deserialize, Serialize};

use crate::{Moment, Time, Work, digest};

/// From this place on, the work runs on this version of its flow: none where the version it ran on was not kept, as of
/// a work that had ended as the engine first kept versions (decision `a-work-kept-before-the-versions-is-on-none`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnFlow {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    /// The version: the digest of the files of its definition.
    pub version: Option<String>,
}

/// How the records before this place were read, as whoever holds the work read them through its entry there: what
/// saving the work compares the store's records with, read the same way. It is no record, and the store keeps none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub at: Seq,
    pub reading: Reading,
}

/// A version of a flow, as it is kept: the digest of each of its files, by its path from the flow's directory, and the
/// digest of those, which names the version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub version: String,
    pub files: BTreeMap<String, String>,
}

/// The version these files make, each by its path from the flow's directory and its text: each file's digest, and the
/// digest of the paths with their files' digests, one to a line, in the order of the paths.
pub fn version_of(files: &[(String, String)]) -> Version {
    let files: BTreeMap<String, String> = (files.iter())
        .map(|(path, text)| (path.clone(), digest(text)))
        .collect();
    let listed: String = (files.iter()).map(|(path, file)| format!("{path} {file}\n")).collect();
    Version {
        version: digest(&listed),
        files,
    }
}

impl Work {
    /// The version of its flow the work runs on now: its latest entry's; none where it was put on none, or has no
    /// entry, as a work the store kept before versions were.
    pub fn version(&self) -> Option<&OnFlow> {
        self.flows.last()
    }

    /// Puts the work, as it is added, on this version of its flow from its start: an entry at the place before every
    /// other, which no other entry is given, so that it moves none of the places the work's entries take after it.
    pub fn added_on(&mut self, version: String) {
        self.flows.push(OnFlow {
            at: Seq(0),
            version: Some(version),
        });
    }

    /// Reads what the work recorded before this place through this reading: the names its records hold, and the
    /// blocks its contents are kept by. What passed between the engine and an agent's session, what a round was given,
    /// what came back and what was filled into it, is read as it was recorded. A reading that renames nothing is not
    /// kept among the work's readings.
    pub fn read(&mut self, at: Seq, reading: Reading) {
        if reading.is_empty() {
            return;
        }
        reading.read(&mut self.records, at);
        if !reading.blocks.is_empty() {
            let contents = std::mem::take(&mut self.contents);
            self.contents = (contents.into_iter())
                .map(|((block, version), text)| {
                    let block = if version < at { reading.block(&block) } else { block };
                    ((block, version), text)
                })
                .collect();
        }
        self.readings.push(Read { at, reading });
    }

    /// Reads what the work recorded through each of these readings in turn.
    pub fn read_through(&mut self, readings: &[Read]) {
        for read in readings {
            self.read(read.at, read.reading.clone());
        }
    }

    /// Puts the work on this version of its flow at this time, from its next place on, what it recorded before reading
    /// through this reading. Nothing else is added to the work before it is saved, so that what the store holds of it
    /// before the entry is what it held already.
    pub fn onto(&mut self, now: Time, version: Option<String>, reading: Reading) {
        let at = self.tick();
        self.read(at, reading);
        self.flows.push(OnFlow { at, version });
        self.moments.push(Moment { from: at, at: now });
    }
}

#[cfg(test)]
mod tests {
    use decide::{
        BlockId, Cause, CellId, Finished, LetGo, PutIn, Questioned, RunId, Started, Write, Written, records::Outsider,
    };

    use super::*;

    fn started(at: u64, cell: &str, block: &str) -> Started {
        Started {
            run: RunId(at),
            cell: cell.into(),
            at: Seq(at),
            because: vec![Cause::Changed {
                block: block.into(),
                was: None,
                behind: false,
            }],
            basis: [(BlockId::from(block), None)].into_iter().collect(),
        }
    }

    fn finished(at: u64, run: u64, block: &str) -> Finished {
        Finished {
            run: RunId(run),
            at: Seq(at),
            writes: vec![Write {
                block: block.into(),
                meaning_unchanged: false,
                value: None,
                empty: false,
            }],
            dealt_with: Some(vec![block.into()]),
            waits_for: Some(block.into()),
        }
    }

    fn reading(cells: &[(&str, &str)], blocks: &[(&str, &str)]) -> Reading {
        Reading {
            cells: cells
                .iter()
                .map(|(a, b)| (CellId::from(*a), CellId::from(*b)))
                .collect(),
            blocks: blocks
                .iter()
                .map(|(a, b)| (BlockId::from(*a), BlockId::from(*b)))
                .collect(),
        }
    }

    /// Every name the records hold, by the kind of entry, in the order the records hold them.
    fn names(work: &Work) -> Vec<String> {
        let r = &work.records;
        let mut names = Vec::new();
        for s in &r.started {
            names.push(format!(
                "started {} {:?} {:?}",
                s.cell,
                s.basis.keys().collect::<Vec<_>>(),
                s.because
            ));
        }
        for f in &r.finished {
            names.push(format!("finished {:?} {:?} {:?}", f.writes, f.dealt_with, f.waits_for));
        }
        for p in &r.put_in {
            names.push(format!("put_in {}", p.block));
        }
        for w in &r.written {
            names.push(format!("written {} {:?}", w.block, w.said_to));
        }
        for q in &r.questions {
            names.push(format!("question {}", q.cell));
        }
        for l in &r.let_go {
            names.push(format!("let_go {}", l.cell));
        }
        names.extend(
            work.contents
                .keys()
                .map(|(block, at)| format!("content {block} {}", at.0)),
        );
        names
    }

    fn work() -> Work {
        let mut work = Work::default();
        let r = &mut work.records;
        r.started.push(started(1, "T2", "t2_words"));
        r.finished.push(finished(2, 1, "t2_words"));
        r.put_in.push(PutIn {
            run: RunId(1),
            block: "t2_words".into(),
            version: Seq(2),
            at: Seq(3),
        });
        r.written.push(Written {
            block: "t2_words".into(),
            by: Outsider::Person,
            matters: true,
            value: None,
            empty: false,
            said_to: Some("T2".into()),
            at: Seq(4),
        });
        r.questions.push(Questioned {
            name: "q".into(),
            cell: "T2".into(),
            run: RunId(1),
            engine: false,
            waits: false,
            withdrawn: false,
            at: Seq(5),
        });
        r.let_go.push(LetGo {
            cell: "T2".into(),
            at: Seq(6),
        });
        work.contents
            .insert((BlockId::from("t2_words"), Seq(2)), "words".into());
        work
    }

    // Every name an entry recorded before a move holds reads through the move's reading, and through each move after it
    // in turn; what was recorded after a move reads as it was recorded; and what passed with a session stays as it was.
    #[test]
    fn what_was_recorded_before_a_move_reads_through_it_and_each_move_after_it_in_turn() {
        let mut work = work();
        work.given.insert((RunId(1), 1), crate::Given::Task);
        work.onto(
            Time(10),
            Some("b".into()),
            reading(&[("T2", "T4")], &[("t2_words", "t4_words")]),
        );
        assert_eq!(
            work.flows,
            [OnFlow {
                at: Seq(7),
                version: Some("b".into())
            }]
        );
        // Recorded after the first move: a new T2's run.
        work.records.started.push(started(8, "T2", "t2_words"));
        work.onto(
            Time(20),
            Some("c".into()),
            reading(&[("T4", "T5")], &[("t4_words", "t5_words")]),
        );
        let read = names(&work);
        let once = |s: &str| read.iter().filter(|n| n.contains(s)).count();
        // Each of the first run's names went to T4 and then to T5; the run recorded between the moves went only to T4's
        // reading after it, which does not touch T2.
        assert_eq!(once("T5"), 4, "{read:#?}");
        assert_eq!(once("t5_words"), 5, "{read:#?}");
        assert!(read.contains(&"started T2 [BlockId(\"t2_words\")] [Changed { block: BlockId(\"t2_words\"), was: None, behind: false }]".to_string()), "{read:#?}");
        assert!(
            !read.iter().any(|n| n.contains("T4") || n.contains("t4_words")),
            "{read:#?}"
        );
        assert_eq!(work.given.get(&(RunId(1), 1)), Some(&crate::Given::Task));
        // Read from the records as recorded, through the readings kept, the same.
        let mut recorded = self::work();
        recorded.records.started.push(started(8, "T2", "t2_words"));
        recorded.read_through(&work.readings);
        assert_eq!(names(&recorded), read);
        assert_eq!(work.version().and_then(|on| on.version.as_deref()), Some("c"));
    }

    // A version is named by its files alone: the same files make the same version, and a change of any file another.
    #[test]
    fn a_version_is_named_by_the_digest_of_its_files() {
        let files = vec![("flow.toml".to_string(), "name = \"e\"".to_string())];
        let one = version_of(&files);
        assert_eq!(one, version_of(&files));
        assert_eq!(one.files["flow.toml"], digest("name = \"e\""));
        let other = version_of(&[("flow.toml".to_string(), "name = \"f\"".to_string())]);
        assert_ne!(one.version, other.version);
    }
}
