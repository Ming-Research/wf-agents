//! The named tests, asked of the work as it is now.

use decide::{Outcome, TestName, Tests};
use work::Work;

/// The named tests of a flow, as whatever drives the engine answers them. A named test reads the work's state and what
/// people and the outside did (entry `conditions-and-preconditions`), so it is asked with the work as it is at the walk
/// that asks: the loop adds to the work between two walks of one thing that happened, and a test answered from the
/// work as it was before would decide on what is no longer so.
pub trait NamedTests {
    fn test(&self, name: &TestName, work: &Work) -> Outcome;
}

/// Tests that read nothing of the work come out the same whatever it holds.
impl<T: Tests + ?Sized> NamedTests for T {
    fn test(&self, name: &TestName, _: &Work) -> Outcome {
        Tests::test(self, name)
    }
}

/// The named tests as they come out for a work as it is now: what the deciding code is given for one walk. Whoever
/// walks a work's records outside the loop, to show the work or to trace a decision, binds the tests the same way.
pub struct Now<'a> {
    pub tests: &'a dyn NamedTests,
    pub work: &'a Work,
}

impl Tests for Now<'_> {
    fn test(&self, name: &TestName) -> Outcome {
        self.tests.test(name, self.work)
    }
}
