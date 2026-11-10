//! Whether a round read from the prompt cache what its session's previous round left there. A small change of what is
//! sent to a session breaks the cache silently, and every later round of it then pays for writing it again; the
//! first round that misses is caught here, and its work waits for the owner (principle `no-unseen-spending-loop`).
//!
//! Measured with the command line (version 2.1.321, `--output-format stream-json --verbose`): a round that goes on in
//! its session makes one request to the model for each turn, and its first request reads from the cache everything
//! the session's previous round's last request read and wrote, then writes what was added since; in three resumed
//! rounds it read exactly that (4,620 of 4,620; 4,776 of 4,776; 4,932 of 4,932 tokens). A round whose form of return
//! had changed read nothing where 5,190 were expected. The round's totals hide it: that round still read 5,238 tokens
//! in all, its second request reading what its first had written again. So the first request is judged, against the
//! previous round's last.
//!
//! What a round's first and last requests read and wrote, and the judgement, are kept beside its files
//! ([`Kept`], in `.cache.json`): the next round of the session reads the previous round's numbers there, and a page
//! may count the hits. The records hold only a round's totals, which do not say what its last request left.

use serde::{Deserialize, Serialize};
use work::Time;

/// The extension, after the round's own name, of the file that keeps its [`Kept`].
pub const EXTENSION: &str = "cache.json";

/// The tokens of one request a round made to the model, as the command line printed its usage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// The model it was made to.
    pub model: String,
    /// The tokens sent that were neither read from the cache nor written to it.
    pub input: u64,
    /// The tokens read from the cache.
    pub read: u64,
    /// The tokens written to the cache.
    pub written: u64,
}

/// What is kept of a round that made at least one request, beside its files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// When its program was started.
    pub began: Time,
    /// When its program had ended.
    pub ended: Time,
    /// Its first request.
    pub first: Request,
    /// Its last request: what it left in the cache for the session's next round.
    pub last: Request,
    /// Whether its first request read what it should have.
    pub judged: Judged,
}

impl Kept {
    /// What the session's next round should read from the cache: all that this round's last request read and wrote.
    pub fn left(&self) -> u64 {
        self.last.read + self.last.written
    }
}

/// Whether a round's first request read from the cache what the session's previous round left there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "judged", rename_all = "snake_case")]
pub enum Judged {
    /// It read what was expected, or no less than the tolerance allows.
    Hit { expected: u64, read: u64 },
    /// It read less than was expected, by more than the tolerance allows.
    Miss { expected: u64, read: u64 },
    /// No judgement is honest, for this reason.
    NotJudged { why: Unjudged },
}

/// Why a round's read from the cache is not judged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Unjudged {
    /// The round began its session: nothing of the session was in the cache.
    Begins,
    /// The session's previous round ended this many minutes before the round began, longer than the cache keeps
    /// what it holds.
    Expired { minutes: u64 },
    /// The model changed: what one model's requests left in the cache is not read by another's.
    ModelChanged { before: String, now: String },
    /// What the session's previous round left in the cache is not known: it kept no numbers, having failed with
    /// nothing that could be read, or having been run before they were kept.
    Unknown,
}

/// What came before a round in its session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Before {
    /// Nothing: the round begins the session.
    Nothing,
    /// A round whose numbers are not known.
    Unknown,
    /// A round whose numbers were kept.
    Round(Kept),
}

/// How a round's read is judged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Check {
    /// How long, in minutes, what was left in the cache is taken to be there still. The command line writes the cache
    /// to be kept an hour (`ephemeral_1h`, measured), each read keeping it an hour more; the default leaves a few
    /// minutes of that hour for a round's own length.
    pub life_minutes: u64,
    /// The share, in hundredths of what was expected, a round may read less than expected and still hit.
    pub shortfall_percent: f64,
    /// The tokens a round may read less than expected and still hit, where that is more than the share.
    pub shortfall_tokens: u64,
}

/// Judges a round's first request against what the session's previous round left in the cache, the round having
/// begun at `began`.
pub fn judge(before: &Before, began: Time, first: &Request, check: &Check) -> Judged {
    let not = |why| Judged::NotJudged { why };
    let previous = match before {
        Before::Nothing => return not(Unjudged::Begins),
        Before::Unknown => return not(Unjudged::Unknown),
        Before::Round(previous) => previous,
    };
    let since = began.0.saturating_sub(previous.ended.0);
    if since > check.life_minutes.saturating_mul(60_000) {
        return not(Unjudged::Expired {
            minutes: since / 60_000,
        });
    }
    if previous.last.model != first.model {
        return not(Unjudged::ModelChanged {
            before: previous.last.model.clone(),
            now: first.model.clone(),
        });
    }
    let expected = previous.left();
    let read = first.read;
    if read.saturating_add(allowed(expected, check)) >= expected {
        Judged::Hit { expected, read }
    } else {
        Judged::Miss { expected, read }
    }
}

/// How many tokens a round may read less than expected and still hit: the share of what was expected, or the number
/// of tokens, whichever is more.
fn allowed(expected: u64, check: &Check) -> u64 {
    let share = (expected as f64 * check.shortfall_percent / 100.0).ceil();
    // The share was checked to be a finite number of no less than nothing; one larger than a u64 holds allows all.
    (share as u64).max(check.shortfall_tokens)
}

/// Why a work waits for the owner after its round of this number of this run missed the cache, the session's previous
/// round having ended this many minutes before.
pub fn missed(run: u64, number: usize, expected: u64, read: u64, minutes: u64) -> String {
    format!(
        "round {number} of run {run} read {read} tokens from the cache where {expected} were expected (the session's \
         previous round ended {minutes} minutes ago): what is sent to this session changed, or the cache was lost"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECK: Check = Check {
        life_minutes: 55,
        shortfall_percent: 2.0,
        shortfall_tokens: 512,
    };

    fn request(model: &str, read: u64, written: u64) -> Request {
        Request {
            model: model.to_string(),
            input: 3,
            read,
            written,
        }
    }

    /// A round of haiku that ended at ten minutes past the start, its last request having read 40,000 tokens and
    /// written 10,000.
    fn previous() -> Before {
        let last = request("claude-haiku-5-5", 40_000, 10_000);
        Before::Round(Kept {
            began: Time(0),
            ended: Time(10 * 60_000),
            first: request("claude-haiku-5-5", 30_000, 2_000),
            judged: Judged::NotJudged { why: Unjudged::Begins },
            last,
        })
    }

    /// The judgement of a round of haiku begun this many minutes after the previous one ended, its first request
    /// reading this.
    fn after(minutes: u64, read: u64) -> Judged {
        let began = Time((10 + minutes) * 60_000);
        judge(&previous(), began, &request("claude-haiku-5-5", read, 500), &CHECK)
    }

    #[test]
    fn a_round_that_reads_what_was_left_hits() {
        assert_eq!(
            after(3, 50_000),
            Judged::Hit {
                expected: 50_000,
                read: 50_000
            }
        );
        assert_eq!(
            after(3, 60_000),
            Judged::Hit {
                expected: 50_000,
                read: 60_000
            },
            "more than was left, as after a round whose numbers were lost"
        );
    }

    #[test]
    fn a_round_that_reads_nothing_misses() {
        assert_eq!(
            after(3, 0),
            Judged::Miss {
                expected: 50_000,
                read: 0
            }
        );
    }

    #[test]
    fn a_shortfall_within_the_tolerance_hits_and_beyond_it_misses() {
        // Two hundredths of 50,000 is 1,000, more than 512 tokens.
        assert!(matches!(after(3, 49_000), Judged::Hit { .. }));
        assert!(matches!(after(3, 48_999), Judged::Miss { .. }));
        // Of 10,000 it is 200, and the 512 tokens hold.
        let small = Before::Round(Kept {
            last: request("claude-haiku-5-5", 9_000, 1_000),
            ..match previous() {
                Before::Round(kept) => kept,
                _ => unreachable!(),
            }
        });
        let began = Time(11 * 60_000);
        let read = |read| judge(&small, began, &request("claude-haiku-5-5", read, 0), &CHECK);
        assert!(matches!(read(9_488), Judged::Hit { .. }));
        assert!(matches!(read(9_487), Judged::Miss { .. }));
    }

    #[test]
    fn a_round_is_not_judged_where_no_judgement_is_honest() {
        let first = request("claude-haiku-5-5", 0, 5_000);
        let not = |why| Judged::NotJudged { why };
        assert_eq!(judge(&Before::Nothing, Time(0), &first, &CHECK), not(Unjudged::Begins));
        assert_eq!(judge(&Before::Unknown, Time(0), &first, &CHECK), not(Unjudged::Unknown));
        assert_eq!(
            after(55, 0),
            Judged::Miss {
                expected: 50_000,
                read: 0
            },
            "the last minute of the life"
        );
        assert_eq!(after(56, 0), not(Unjudged::Expired { minutes: 56 }));
        let began = Time(11 * 60_000);
        assert_eq!(
            judge(&previous(), began, &request("claude-opus-5-5", 0, 5_000), &CHECK),
            not(Unjudged::ModelChanged {
                before: "claude-haiku-5-5".to_string(),
                now: "claude-opus-5-5".to_string()
            })
        );
    }

    #[test]
    fn what_is_kept_reads_back_as_it_was_written() {
        let Before::Round(mut kept) = previous() else {
            unreachable!()
        };
        for judged in [
            Judged::Miss { expected: 9, read: 0 },
            Judged::NotJudged {
                why: Unjudged::Expired { minutes: 70 },
            },
        ] {
            kept.judged = judged;
            let text = serde_json::to_string(&kept).unwrap();
            assert_eq!(serde_json::from_str::<Kept>(&text).unwrap(), kept, "{text}");
        }
    }

    #[test]
    fn the_reason_says_what_was_read_and_expected() {
        assert_eq!(
            missed(7, 2, 50_000, 0, 3),
            "round 2 of run 7 read 0 tokens from the cache where 50000 were expected (the session's previous round \
             ended 3 minutes ago): what is sent to this session changed, or the cache was lost"
        );
    }
}
