//! The sessions of a work's lines, as its records say them.
//!
//! A line's rounds run in one session of the agents' program, its first, until something makes the line begin a new
//! one; each such beginning is an entry of the work, [`Session`], with why it began. A session the program lost is
//! begun again under the same id; every other new session has an id of its own, so that a round finds its session by
//! counting the line's new sessions recorded before it began ([`of_round`]), and needs nothing stored but the records.
//! What each try of a round ran on is recorded as the try begins ([`RanOn`]), so that what a session was begun on is
//! the records' too ([`began_on`]).

use decide::{RunId, Seq};
use serde::{Deserialize, Serialize};

use crate::{Spent, Work};

/// A new session of a line of the work, as it is recorded: the line's rounds that begin after it run in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub line: String,
    pub why: Why,
}

/// A try of an agent's round began, on this: the try of its round counted by the tries recorded before it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ran {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub run: RunId,
    /// The round's number in its run, the first being one.
    pub round: usize,
    pub on: RanOn,
}

/// What a try of an agent's round ran on, as its carrier chose it as the try began: the level, the model of that
/// level, the reasoning effort, and the account the program makes its calls under, which is the directory of the
/// program's settings and sign-in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RanOn {
    pub level: String,
    pub model: String,
    /// None where the cell sets none, and the program ran at its own.
    pub effort: Option<String>,
    /// None where the program ran with its own default directory.
    pub account: Option<String>,
}

/// Why a line began a new session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Why {
    /// The context of the line's session, as its last round ended, was past the installation's limit, in tokens.
    Limit { context: u64, limit: u64 },
    /// The line's rounds run on another model than the one its session was begun on, whose prompt cache holds
    /// nothing of it.
    Model { was: String },
    /// The line's rounds run under another account than its session's, whose directory does not hold the session.
    Account { was: Option<String> },
    /// The owner asked for one.
    Owner,
    /// The service's safety check flagged the line's last round, which is tried again in a session that does not give
    /// the check the same context.
    Flagged,
    /// The agents' program had no session under the line's id: the round that found it begins one under that id.
    Lost,
}

impl Why {
    /// Whether the session begins under an id of its own: every new session but one begun again under the id of the
    /// one the program lost.
    pub fn own_id(&self) -> bool {
        !matches!(self, Why::Lost)
    }

    /// Why, in words, for whoever reads it: an agent reading a block that says the session is new, or the owner.
    pub fn words(&self, line: &str) -> String {
        let why = match self {
            Why::Limit { context, limit } => {
                format!("its context was {context} tokens as its last round ended, past the limit of {limit}")
            }
            Why::Model { was } => format!("its rounds now run on another model than {was}, which it was begun on"),
            Why::Account { .. } => "its rounds now run under another account than it was begun under".to_string(),
            Why::Owner => "the owner asked for one".to_string(),
            Why::Flagged => "the service's safety check flagged its last round".to_string(),
            Why::Lost => "the agents' program no longer had the session it was in".to_string(),
        };
        format!("A new session of the line `{line}`: {why}.")
    }
}

/// Where the round of this number of the run began; none where the records hold no such round.
pub fn began_at(work: &Work, run: RunId, number: usize) -> Option<Seq> {
    let mut rounds = work.records.rounds.iter().filter(|r| r.run == run);
    rounds.nth(number.checked_sub(1)?).map(|r| r.at)
}

/// The line's sessions, in the order they were recorded.
pub fn of_line<'a>(work: &'a Work, line: &'a str) -> impl Iterator<Item = &'a Session> {
    work.sessions.iter().filter(move |s| s.line == line)
}

/// Which of the line's sessions a round that began at this place runs in, counted from none: how many of the line's
/// sessions with an id of their own were recorded before it.
pub fn number_at(work: &Work, line: &str, at: Seq) -> u32 {
    let own = of_line(work, line).filter(|s| s.why.own_id() && s.at < at).count();
    u32::try_from(own).unwrap_or(u32::MAX)
}

/// Of the round of this number of the run, on the line whose runs `on_line` tells: which of the line's sessions it runs
/// in, and whether it begins that session, which it does where no round of the line that began before it in the same
/// session spent tokens, as a round that never reached the model has not.
pub fn of_round(work: &Work, line: &str, on_line: impl Fn(RunId) -> bool, run: RunId, number: usize) -> (u32, bool) {
    let at = began_at(work, run, number).unwrap_or_else(|| work.tick());
    let session = number_at(work, line, at);
    let spent = |s: &Spent| s.input_tokens + s.cached_tokens + s.output_tokens > 0;
    let went_on = (work.spent.iter())
        .filter(|((r, _), s)| on_line(*r) && spent(s))
        .filter_map(|((r, n), _)| began_at(work, *r, *n))
        .any(|b| b < at && number_at(work, line, b) == session);
    (session, !went_on)
}

/// Whether an earlier round of the run of this round ran in another of the line's sessions than the round does: the
/// session the round runs in never saw what the run was doing, so the round is told its task and what each earlier
/// round of the run was given, as a round that begins a session is, even where it goes on in a session another cell's
/// round began. A session begun again under the id of the one the program lost is the same session.
pub fn moved(work: &Work, line: &str, run: RunId, number: usize) -> bool {
    let at = began_at(work, run, number).unwrap_or_else(|| work.tick());
    let session = number_at(work, line, at);
    (1..number)
        .filter_map(|earlier| began_at(work, run, earlier))
        .any(|b| number_at(work, line, b) != session)
}

/// What the line's session of this number was begun on: what the first try of its first round recorded ran on; none
/// where none of its rounds recorded it.
pub fn began_on<'a>(work: &'a Work, line: &str, on_line: impl Fn(RunId) -> bool, session: u32) -> Option<&'a RanOn> {
    let mut rounds: Vec<(Seq, RunId, usize)> = Vec::new();
    for round in &work.records.rounds {
        let number = rounds.iter().filter(|(_, run, _)| *run == round.run).count() + 1;
        rounds.push((round.at, round.run, number));
    }
    (rounds.into_iter())
        .filter(|(at, run, _)| on_line(*run) && number_at(work, line, *at) == session)
        .find_map(|(_, run, number)| work.ran.iter().find(|r| r.run == run && r.round == number))
        .map(|r| &r.on)
}

#[cfg(test)]
mod tests {
    use decide::{Returned, Round};

    use super::*;

    /// A work whose one run has rounds begun at these places, each ended after it with these tokens, and these
    /// sessions recorded.
    fn work(rounds: &[(u64, u64)], sessions: &[(u64, Why)]) -> Work {
        let mut work = Work::default();
        for (n, (at, tokens)) in (1..).zip(rounds) {
            work.records.rounds.push(Round {
                at: Seq(*at),
                run: RunId(1),
            });
            work.records.returned.push(Returned {
                at: Seq(at + 1),
                run: RunId(1),
            });
            let spent = Spent {
                input_tokens: *tokens,
                ..Spent::default()
            };
            work.spent.insert((RunId(1), n), spent);
        }
        for (at, why) in sessions {
            work.sessions.push(Session {
                at: Seq(*at),
                line: "engineer".to_string(),
                why: why.clone(),
            });
        }
        work
    }

    fn of(work: &Work, number: usize) -> (u32, bool) {
        of_round(work, "engineer", |_| true, RunId(1), number)
    }

    #[test]
    fn a_line_goes_on_in_its_session_once_a_round_of_it_spent_tokens() {
        let w = work(&[(1, 0), (3, 10), (5, 10)], &[]);
        assert_eq!(of(&w, 1), (0, true));
        assert_eq!(of(&w, 2), (0, true), "the round before reached no model");
        assert_eq!(of(&w, 3), (0, false));
    }

    #[test]
    fn a_new_session_with_an_id_of_its_own_is_begun_by_the_next_round() {
        let limit = Why::Limit { context: 9, limit: 5 };
        let w = work(&[(1, 10), (4, 10), (6, 10)], &[(3, limit)]);
        assert_eq!(of(&w, 1), (0, true));
        assert_eq!(of(&w, 2), (1, true));
        assert_eq!(of(&w, 3), (1, false));
    }

    #[test]
    fn a_session_the_program_lost_is_begun_again_under_its_id() {
        let w = work(&[(1, 10), (3, 10), (6, 10)], &[(4, Why::Lost)]);
        assert_eq!(
            of(&w, 2),
            (0, false),
            "the round that found it lost was given as going on"
        );
        assert_eq!(
            of(&w, 3),
            (0, false),
            "the next goes on in the session that round began"
        );
    }

    #[test]
    fn a_round_whose_run_ran_in_another_session_before_is_told_what_the_run_was_doing() {
        let limit = Why::Limit { context: 9, limit: 5 };
        let w = work(&[(1, 10), (3, 10), (6, 10)], &[(5, limit)]);
        assert!(!moved(&w, "engineer", RunId(1), 1), "a run's first round");
        assert!(
            !moved(&w, "engineer", RunId(1), 2),
            "in the same session as the round before"
        );
        assert!(
            moved(&w, "engineer", RunId(1), 3),
            "the round before ran in the session before"
        );
        let lost = work(&[(1, 10), (3, 10), (6, 10)], &[(5, Why::Lost)]);
        assert!(
            !moved(&lost, "engineer", RunId(1), 3),
            "a session begun again under its id is the same"
        );
    }

    #[test]
    fn a_session_was_begun_on_what_the_first_try_of_its_first_round_ran_on() {
        let limit = Why::Limit { context: 9, limit: 5 };
        let mut w = work(&[(1, 10), (4, 10), (7, 10)], &[(3, limit)]);
        let on = |model: &str| RanOn {
            level: "1".to_string(),
            model: model.to_string(),
            effort: None,
            account: None,
        };
        for (at, round, model) in [(10, 1, "a"), (11, 2, "b"), (12, 2, "c"), (13, 3, "d")] {
            w.ran.push(Ran {
                at: Seq(at),
                run: RunId(1),
                round,
                on: on(model),
            });
        }
        let began = |session| began_on(&w, "engineer", |_| true, session).map(|on| on.model.as_str());
        assert_eq!(began(0), Some("a"));
        assert_eq!(began(1), Some("b"), "the first try of the session's first round");
        assert_eq!(began(2), None, "a session none of whose rounds began");
    }
}
