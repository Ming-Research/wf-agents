//! The sessions of a work's lines, as its records say them.
//!
//! A line's rounds run in one session of the agents' program, its first, until something makes the line begin a new
//! one, or something makes its session hold only a summary: each is an entry of the work, [`Session`], with why. A
//! session the program lost is begun again under the same id, and a compacted one goes on under it; every other new
//! session has an id of its own, so that a round finds its session by counting the line's new sessions recorded before
//! its first try began ([`start`], [`of_round`]), and needs nothing stored but the records.
//! What each try of a round ran on is recorded as the try begins ([`RanOn`]), so that what a session was begun on is
//! the records' too ([`began_on`]).

use decide::{RunId, Seq};
use serde::{Deserialize, Serialize};

use crate::{Spent, Work};

/// A line's session reopened, as it is recorded: a new session the line's rounds that begin after it run in, or the
/// session compacted, which they go on in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub line: String,
    pub why: Why,
}

/// A try of an agent's round began, on this.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ran {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub run: RunId,
    /// The round's number in its run, the first being one.
    pub round: usize,
    /// The try's number in its round, the first being one, as its carrier numbers what it keeps of it: a try kept
    /// before tries were recorded has no record, so the records are not counted to find a try's.
    pub attempt: usize,
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

/// Why a line's session was reopened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Why {
    /// The context of the line's session, as its last round ended, was past the limit of its line, in tokens: the line
    /// its rounds' history gives, never more than the installation's limit.
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
    /// The run guard stopped the line's last round, its context having grown past what its cell's history explains:
    /// it is tried again in a session that does not give it the same context.
    Guarded,
    /// The line's last round failed for a prompt too long for the model: a session that holds it would be too long
    /// for every round after it, so the round is given again, once the work is let on, in a session that does not.
    TooLong,
    /// The agents' program had no session under the line's id: the round that found it begins one under that id.
    Lost,
    /// The agents' program compacted the line's session during a round: the session goes on under its id, holding a
    /// summary where it held what the line had read.
    Compacted,
}

impl Why {
    /// Whether the session begins under an id of its own: every new session but one begun again under the id of the
    /// one the program lost, and a session compacted, which the program goes on in under its id.
    pub fn own_id(&self) -> bool {
        !matches!(self, Why::Lost | Why::Compacted)
    }

    /// The names of its kinds, each the name of the words of it where the flow says what the engine writes of a new
    /// session: [`Why::named`] gives a kind's name from here, by [`Why::kind`].
    pub const KINDS: [&'static str; 9] = [
        "limit",
        "model",
        "account",
        "owner",
        "flagged",
        "guarded",
        "too_long",
        "lost",
        "compacted",
    ];

    /// Its kind's place among [`Why::KINDS`]. The match names every kind, so that a kind added to `Why` is given a
    /// name there before the engine builds.
    fn kind(&self) -> usize {
        match self {
            Why::Limit { .. } => 0,
            Why::Model { .. } => 1,
            Why::Account { .. } => 2,
            Why::Owner => 3,
            Why::Flagged => 4,
            Why::Guarded => 5,
            Why::TooLong => 6,
            Why::Lost => 7,
            Why::Compacted => 8,
        }
    }

    /// Its kind, by its name among [`Why::KINDS`], and the places the words of it fill in a run, each with its value.
    pub fn named(&self) -> (&'static str, Vec<(&'static str, String)>) {
        let places = match self {
            Why::Limit { context, limit } => vec![("context", context.to_string()), ("limit", limit.to_string())],
            Why::Model { was } => vec![("was", was.clone())],
            Why::Account { .. }
            | Why::Owner
            | Why::Flagged
            | Why::Guarded
            | Why::TooLong
            | Why::Lost
            | Why::Compacted => Vec::new(),
        };
        (Self::KINDS[self.kind()], places)
    }
}

/// Where the round of this number of the run began; none where the records hold no such round.
pub fn began_at(work: &Work, run: RunId, number: usize) -> Option<Seq> {
    let mut rounds = work.records.rounds.iter().filter(|r| r.run == run);
    rounds.nth(number.checked_sub(1)?).map(|r| r.at)
}

/// Where the round of this number of the run takes its session from: the line's sessions recorded before this place
/// are the ones it counts. A round runs in the session its line is in as its first try begins, so this is where its
/// first try's record is; where it has none and has ended (a round kept before tries were recorded, or one that failed
/// before its program ran), where it began; and where it has none and has not ended, or has not begun, the next place,
/// since its first try is yet to begin. A round under way as the engine opens and records a new session of its line
/// before giving it again, as for a flag of a store kept before sessions were recorded, so runs in that session.
pub fn start(work: &Work, run: RunId, number: usize) -> Seq {
    let first = (work.ran.iter()).find(|r| r.run == run && r.round == number);
    let ended = work.records.returned.iter().filter(|r| r.run == run).count() >= number;
    match (first, began_at(work, run, number)) {
        (Some(first), _) => first.at,
        (None, Some(began)) if ended => began,
        _ => work.tick(),
    }
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

/// Whether the round took tokens: it reached the model.
fn spent(s: &Spent) -> bool {
    s.input_tokens + s.cached_tokens + s.output_tokens > 0
}

/// Whether the round of this number of the run put its message into its session: it reached the model, or its last
/// try's program said, as its stream began, that it had taken the message in, which it writes into the session before
/// it calls the service (decision `no-round-tries-the-network`). What a round failed of says nothing of this: a crash
/// may come before the program read its input or after. A round that says neither is taken to have put nothing
/// there: a message given again where the session holds it costs tokens, while one referred to where the session
/// lacks it leaves the agent without it.
pub fn wrote_into(work: &Work, run: RunId, number: usize) -> bool {
    if work.spent.get(&(run, number)).is_some_and(spent) {
        return true;
    }
    crate::measured::of_round(&work.measured, run, number).is_some_and(|m| m.received)
}

/// Of the round of this number of the run, on the line whose runs `on_line` tells: which of the line's sessions it runs
/// in, and whether it begins that session, which it does where no round of the line that took its session before it,
/// from the same session, put its message into it ([`wrote_into`]).
pub fn of_round(work: &Work, line: &str, on_line: impl Fn(RunId) -> bool, run: RunId, number: usize) -> (u32, bool) {
    let at = start(work, run, number);
    let session = number_at(work, line, at);
    let went_on = (work.spent.keys().chain(work.back.keys()))
        .filter(|(r, n)| on_line(*r) && wrote_into(work, *r, *n))
        .map(|(r, n)| start(work, *r, *n))
        .any(|b| b < at && number_at(work, line, b) == session);
    (session, !went_on)
}

/// What the line's session of this number was begun on: what the first try of its first round recorded ran on; none
/// where none of its rounds recorded it.
pub fn began_on<'a>(work: &'a Work, line: &str, on_line: impl Fn(RunId) -> bool, session: u32) -> Option<&'a RanOn> {
    let mut rounds: Vec<(RunId, usize)> = Vec::new();
    for round in &work.records.rounds {
        let number = rounds.iter().filter(|(run, _)| *run == round.run).count() + 1;
        rounds.push((round.run, number));
    }
    (rounds.into_iter())
        .filter(|&(run, number)| on_line(run) && number_at(work, line, start(work, run, number)) == session)
        .find_map(|(run, number)| work.ran.iter().find(|r| r.run == run && r.round == number))
        .map(|r| &r.on)
}

#[cfg(test)]
mod tests {
    use decide::{Returned, Round};

    use super::*;

    #[test]
    fn each_kind_of_a_new_session_has_a_name_of_its_own_among_the_kinds() {
        let every = [
            Why::Limit { context: 2, limit: 1 },
            Why::Model { was: "m".to_string() },
            Why::Account { was: None },
            Why::Owner,
            Why::Flagged,
            Why::Guarded,
            Why::TooLong,
            Why::Lost,
            Why::Compacted,
        ];
        let names: Vec<&str> = every.iter().map(|w| w.named().0).collect();
        assert_eq!(names, Why::KINDS);
    }

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

    /// A try of the round of this number of the run, begun at this place.
    fn ran(at: u64, run: RunId, round: usize) -> Ran {
        Ran {
            at: Seq(at),
            run,
            round,
            attempt: 1,
            on: RanOn {
                level: "1".to_string(),
                model: "a".to_string(),
                effort: None,
                account: None,
            },
        }
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
    fn a_round_goes_on_in_a_session_another_run_found_lost_and_began_again_under_its_id() {
        // Run 1 moves to session 1 at its round 2; while it waits, run 2 of the line finds session 1 lost as its first
        // try begins, and begins it again under its id in its second try.
        let (one, two) = (RunId(1), RunId(2));
        let mut w = Work::default();
        for (at, run) in [(1, one), (5, one), (9, two), (15, one)] {
            w.records.rounds.push(Round { at: Seq(at), run });
        }
        for (at, run) in [(3, one), (7, one), (13, two)] {
            w.records.returned.push(Returned { at: Seq(at), run });
        }
        let tokens = Spent {
            input_tokens: 10,
            ..Spent::default()
        };
        for key in [(one, 1), (one, 2), (two, 1)] {
            w.spent.insert(key, tokens);
        }
        w.ran
            .extend([ran(2, one, 1), ran(6, one, 2), ran(10, two, 1), ran(12, two, 1)]);
        let session = |at, why| Session {
            at: Seq(at),
            line: "engineer".to_string(),
            why,
        };
        w.sessions.extend([session(4, Why::Owner), session(11, Why::Lost)]);
        assert_eq!(
            of_round(&w, "engineer", |_| true, one, 3),
            (1, false),
            "it goes on in that session"
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
        // Each try is put at the place of its round, the fixture leaving no place between a round and its end.
        for (at, round, attempt, model) in [(1, 1, 1, "a"), (4, 2, 1, "b"), (4, 2, 2, "c"), (7, 3, 1, "d")] {
            w.ran.push(Ran {
                at: Seq(at),
                run: RunId(1),
                round,
                attempt,
                on: on(model),
            });
        }
        let began = |session| began_on(&w, "engineer", |_| true, session).map(|on| on.model.as_str());
        assert_eq!(began(0), Some("a"));
        assert_eq!(began(1), Some("b"), "the first try of the session's first round");
        assert_eq!(began(2), None, "a session none of whose rounds began");
    }

    #[test]
    fn a_session_compacted_goes_on_under_its_id() {
        let w = work(&[(1, 10), (3, 10), (6, 10), (8, 10)], &[(5, Why::Compacted)]);
        assert_eq!(of(&w, 3), (0, false));
    }
}
