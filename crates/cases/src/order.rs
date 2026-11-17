//! When each step of a case's history is put in, as the program that runs the case drives the engine's loop: each round
//! is handed in as the engine gives it, and each act of the owner's, and main moving, as the loop takes the round's
//! end before or after it. What a round's end makes the engine do next is decided as the loop takes it, so an act goes
//! in before the end of the round it follows wherever it can, and the walk after that end sees it: the item's
//! background written anew and main moving. What turns the owner to a cell, words to it or an answer, goes in right
//! after: an answer answers what that round asked, and words to a cell before the round's would stop the round. The
//! session of the case's cell's line, where the history holds a round of that line, is begun anew as the history's
//! last round ends, before its end is taken, so that the cell's line goes on in a new session (design document
//! `testing`, 12).

use decide::CellId;

use crate::checked::{Case, Round, Step};

/// An act the program puts in through the engine's loop.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// The owner writes the item's background anew.
    Background(String),
    /// The owner says words to the cell.
    Words { to: CellId, text: String },
    /// The owner answers the question of this name, taking this option.
    Answer { question: String, option: u64 },
    /// Main moves to this commit of the made project.
    Main(String),
    /// The line begins a new session at its next round.
    NewSession(String),
}

/// A round of the history, with the acts that go in before its end is taken and right after.
#[derive(Clone, Debug, PartialEq)]
pub struct Due {
    pub round: Round,
    pub before: Vec<Act>,
    pub after: Vec<Act>,
}

/// The history in the order it is put in: the acts before any round, as the work starts, and each round with its acts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Order {
    pub start: Vec<Act>,
    pub rounds: Vec<Due>,
}

/// The order of the case's history; `line` gives the line of a cell, so that the case's cell's line is begun anew where
/// the history holds a round of it.
pub fn order(case: &Case, line: impl Fn(&CellId) -> Option<String>) -> Order {
    let mut order = Order::default();
    for step in &case.history {
        let act = match step {
            Step::Round(round) => {
                order.rounds.push(Due {
                    round: round.clone(),
                    before: Vec::new(),
                    after: Vec::new(),
                });
                continue;
            }
            Step::Background(text) => Act::Background(text.clone()),
            Step::Words { to, text } => Act::Words {
                to: to.clone(),
                text: text.clone(),
            },
            Step::Answer { question, option } => Act::Answer {
                question: question.clone(),
                option: *option,
            },
            Step::Main(commit) => Act::Main(commit.clone()),
        };
        let turns = matches!(act, Act::Words { .. } | Act::Answer { .. });
        match order.rounds.last_mut() {
            None => order.start.push(act),
            Some(due) if turns => due.after.push(act),
            Some(due) => due.before.push(act),
        }
    }
    let own = line(&case.cell);
    let holds = (order.rounds.iter()).any(|due| own.is_some() && line(&due.round.cell) == own);
    let anew = own.filter(|_| holds);
    if let (Some(line), Some(last)) = (anew, order.rounds.last_mut()) {
        last.before.push(Act::NewSession(line));
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(cell: &str) -> Step {
        Step::Round(Round {
            cell: CellId::from(cell),
            hands_in: Vec::new(),
            commit: None,
        })
    }

    fn case(cell: &str, history: Vec<Step>) -> Case {
        Case {
            id: "X-01".into(),
            cell: CellId::from(cell),
            item: 1,
            history,
            ran_before: true,
            new: Vec::new(),
            results: None,
            owes: Default::default(),
            judge: None,
        }
    }

    fn line(cell: &CellId) -> Option<String> {
        Some(
            if cell.as_str().starts_with('P') {
                "review"
            } else {
                "make"
            }
            .to_string(),
        )
    }

    #[test]
    fn what_turns_the_owner_to_a_cell_goes_in_after_the_round_s_end_and_the_rest_before_it() {
        let words = Step::Words {
            to: "A1".into(),
            text: "Why?".into(),
        };
        let history = vec![
            Step::Background("First.".into()),
            round("A1"),
            Step::Answer {
                question: "q".into(),
                option: 2,
            },
            Step::Main("guide".into()),
            round("A1"),
            Step::Background("Anew.".into()),
            words.clone(),
        ];
        let order = order(&case("A1", history), line);
        assert_eq!(order.start, [Act::Background("First.".into())]);
        assert_eq!(order.rounds[0].before, [Act::Main("guide".into())]);
        let answer = Act::Answer {
            question: "q".into(),
            option: 2,
        };
        assert_eq!(order.rounds[0].after, [answer]);
        // The last round of the cell's line ends the line's session.
        let anew = Act::NewSession("make".into());
        assert_eq!(order.rounds[1].before, [Act::Background("Anew.".into()), anew]);
        let words = Act::Words {
            to: "A1".into(),
            text: "Why?".into(),
        };
        assert_eq!(order.rounds[1].after, [words]);
    }

    #[test]
    fn a_line_the_history_holds_no_round_of_goes_on_in_its_first_session() {
        let order = order(&case("P1", vec![round("A1"), round("A2")]), line);
        assert!(
            order
                .rounds
                .iter()
                .all(|due| due.before.is_empty() && due.after.is_empty())
        );
        // The session of the line is begun anew as the history's last round ends, whatever that round's line.
        let order = super::order(&case("P1", vec![round("P1"), round("A1")]), line);
        assert_eq!(order.rounds[1].before, [Act::NewSession("review".into())]);
    }
}
