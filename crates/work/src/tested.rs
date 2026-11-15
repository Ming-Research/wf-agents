//! What a run of the project's tests that a call of `run_tests` asked for reported, as its host read it from the
//! run's results: the commit it ran on, and each test with what became of it. What a return says a run gave is held to
//! it, never to the words of the call's result (crate `carry`).

use serde::{Deserialize, Serialize};

/// What the runs a call asked for reported: the commit they ran on, and each test they reported, in the order their
/// results hold them. Only a host that reads each test's result gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tested {
    pub commit: String,
    pub cases: Vec<Case>,
}

/// A test a run reported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    /// The unit it is in, as its results name it: a test binary, a test file.
    pub unit: String,
    /// Its name, as its results give it, which an agent names it by.
    pub name: String,
    pub outcome: Outcome,
}

/// What became of a test in a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Passed,
    Failed,
    Skipped,
}

impl Tested {
    /// What became of the test of this name, as the runs a name is asked by tell it: failed where a test of the name
    /// failed, passed where one passed and none failed; none where no test of the name ran.
    pub fn became(&self, name: &str) -> Option<Outcome> {
        let of: Vec<Outcome> = (self.cases.iter())
            .filter(|c| c.name == name)
            .map(|c| c.outcome)
            .collect();
        if of.contains(&Outcome::Failed) {
            Some(Outcome::Failed)
        } else if of.contains(&Outcome::Passed) {
            Some(Outcome::Passed)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(name: &str, outcome: Outcome) -> Case {
        Case {
            unit: "unit".to_string(),
            name: name.to_string(),
            outcome,
        }
    }

    // A name is failed where any test of it failed, passed where one passed and none failed, and a test only skipped
    // or not there gave nothing.
    #[test]
    fn a_test_became_what_its_cases_of_the_name_tell() {
        let tested = Tested {
            commit: "c".to_string(),
            cases: vec![
                case("a", Outcome::Passed),
                case("b", Outcome::Passed),
                case("b", Outcome::Failed),
                case("c", Outcome::Skipped),
            ],
        };
        assert_eq!(tested.became("a"), Some(Outcome::Passed));
        assert_eq!(tested.became("b"), Some(Outcome::Failed));
        assert_eq!(tested.became("c"), None);
        assert_eq!(tested.became("d"), None);
    }
}
