//! The questions put to the owner and the owner's answers, as they stand (design document `data-and-context`, 32 to
//! 36): what the walk holds a work at a cell for, what the engine says the merge waits for, and what the pages show.
//!
//! A question's versions are the entries written under its name, the latest being what it is now. Its answer is the
//! last answer given to that latest version: one given to an earlier version answers it no more. A question stands
//! until a version withdraws it, and holds its cell while it stands, its cell waits for it, and it has no answer.

use std::collections::BTreeMap;

use crate::records::{Answered, Questioned, Records};

/// A question as it stands now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Standing<'a> {
    /// Its latest version.
    pub latest: &'a Questioned,
    /// The last answer to its latest version, where it has one.
    pub answer: Option<&'a Answered>,
}

impl Standing<'_> {
    /// Whether it stands: no version withdrew it since it was last asked.
    pub fn stands(&self) -> bool {
        !self.latest.withdrawn
    }

    /// Whether it stands with no answer to its latest version.
    pub fn unanswered(&self) -> bool {
        self.stands() && self.answer.is_none()
    }

    /// Whether it holds the work at its cell: it stands unanswered, and its cell waits for the answer.
    pub fn holds(&self) -> bool {
        self.unanswered() && self.latest.waits
    }
}

/// Each question of the work as it stands now, by its name.
pub fn questions(records: &Records) -> BTreeMap<&str, Standing<'_>> {
    let mut latest: BTreeMap<&str, &Questioned> = BTreeMap::new();
    for q in &records.questions {
        let entry = latest.entry(q.name.as_str()).or_insert(q);
        if q.at > entry.at {
            *entry = q;
        }
    }
    (latest.into_iter())
        .map(|(name, latest)| {
            let answer = (records.answers.iter())
                .filter(|a| a.question == name && a.version == latest.at)
                .max_by_key(|a| a.at);
            (name, Standing { latest, answer })
        })
        .collect()
}
