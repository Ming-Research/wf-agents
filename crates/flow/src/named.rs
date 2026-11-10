//! The named tests of a compiled definition as they come out for one work's records.
//!
//! A named test is a test of the values of blocks of the work itself, so it comes out from the flow's definition and
//! the work's records alone (principle `decide-purely`): what is outside a work reaches it only as a block the outside
//! writes. The deciding code asks for the tests through [`decide::Tests`], and never sees how they are worked out.

use std::collections::BTreeMap;

use decide::{BlockId, Outcome, Records, TestName, Tests, Who, versions};

use crate::compile::{Compiled, Test};

/// The named tests of a definition, as they come out for the records they were made for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedTests<'a> {
    tests: &'a BTreeMap<TestName, Test>,
    /// The value each block has now, of the blocks whose current version has one.
    values: BTreeMap<BlockId, String>,
}

impl Compiled {
    /// The named tests as they come out for these records: each reads the value its blocks have now, worked out once,
    /// here, from the records alone. A test of records that were added to since must be asked for again.
    pub fn tests(&self, records: &Records) -> NamedTests<'_> {
        let values = (versions(records).into_iter())
            .filter_map(|(block, versions)| Some((block, versions.last()?.value.clone()?)))
            .collect();
        NamedTests {
            tests: &self.tests,
            values,
        }
    }
}

impl Test {
    /// Whether the test holds when each block has the value this gives it, `None` for a block that has none.
    pub fn holds<'v>(&self, value: impl Fn(&BlockId) -> Option<&'v str>) -> bool {
        self.kind
            .holds(self.reads.iter().map(|b| value(b) == Some(self.is.as_str())))
    }
}

impl Tests for NamedTests<'_> {
    /// Holds where the test holds, and otherwise waits for the owner. A name the definition does not declare, which no
    /// cell of a compiled flow uses, never holds.
    fn test(&self, name: &TestName) -> Outcome {
        let value = |block: &BlockId| self.values.get(block).map(String::as_str);
        match self.tests.get(name) {
            Some(test) if test.holds(value) => Outcome::Holds,
            _ => Outcome::Waits { who: Who::Owner },
        }
    }
}
