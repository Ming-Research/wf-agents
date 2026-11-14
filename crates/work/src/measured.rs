//! What a work's agents' calls to the model came to, as its records keep it: each try of a round, and each keep-warm
//! ping of a line's session. A later round is judged by what the rounds before it came to (principle
//! `judged-by-own-history`), and a round that missed its session's prompt cache is told by them.

use std::collections::BTreeMap;

use decide::{RunId, Seq};
use serde::{Deserialize, Serialize};

use crate::Spent;

/// What the calls of a try of an agent's round came to, as its carrier read them from the agents' program while the
/// try ran, at a place of its own in the order of what happened to the work. The try is one [`Ran`](crate::Ran)
/// records; a round's measures are its last try's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measured {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub run: RunId,
    /// The round's number in its run, the first being one.
    pub round: usize,
    /// The try's number in its round, the first being one.
    pub attempt: usize,
    pub measures: Measures,
}

/// What the calls of a try came to: the tokens of its prompts by how the prompt cache took them, and what it thought, as
/// the agents' program said them; and what the calls of the session's own agent came to, of which only they count, a
/// subagent's context not being the session's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Measures {
    /// The tokens of its prompts sent fresh, neither read from the cache nor written to it.
    pub input_tokens: u64,
    /// The tokens of its prompts read from the cache.
    pub cache_read_tokens: u64,
    /// The tokens of its prompts written to the cache.
    pub cache_written_tokens: u64,
    /// The tokens its model thought in, of those that came back, where the program said.
    pub thinking_tokens: Option<u64>,
    /// How many turns it took, as the program counts them, where it said; else its own agent's calls.
    pub turns: u64,
    /// How many calls the session's own agent made.
    pub calls: u32,
    /// What its first call read from the prompt cache, in tokens: a session gone on in within the cache's lifetime
    /// reads there what it held, and shows in this alone whether the cache held it. None where it made no call.
    pub first_read: Option<u64>,
    /// The whole prompt of its first call, in tokens, read from the cache or not; none where it made no call.
    pub first_context: Option<u64>,
    /// The tokens its context grew by from its first call to its last, each rise from one call's prompt to the next
    /// counted, so that a compaction takes none back.
    pub added: u64,
    /// The characters of the message it was given.
    pub given: u64,
    /// What came before the session's messages in its prompt, which the cache reads only where it is what the call
    /// before had: each part by its name, with its value, or the digest of a long one (`model`, `effort`, `program`,
    /// `system`, `schema`, `tools`). A part the try did not say is missing.
    pub before: BTreeMap<String, String>,
    /// Whether its program said, as its stream began, that it had taken the message in: it is then in the session,
    /// whatever came of the try.
    pub received: bool,
    /// Whether its context went past the lines of a signal the run guard stops a round on, stopped or let run: such a
    /// try is no part of the history later rounds are judged by.
    pub crossed: bool,
    /// Whether what its round took was worked out from its calls at the models' prices, its program having printed no
    /// result to say it: an estimate.
    pub priced: bool,
}

impl Measures {
    /// How much its context grew for each call after the first; none for fewer than two calls.
    pub fn rate(&self) -> Option<u64> {
        (self.calls >= 2).then(|| self.added / u64::from(self.calls - 1))
    }
}

/// A keep-warm ping of a line's session, which only keeps the session's prompt cache from letting it go while the line
/// waits, at a place of its own in the order of what happened to the work; its time is the time of that place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pinged {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub line: String,
    /// Which of the line's sessions it was sent, counted from none as [`crate::sessions::number_at`] counts them.
    pub session: u32,
    /// What it took, as the agents' program said; nothing where it said nothing.
    pub spent: Spent,
    /// Why it failed, where it did: the session's cache was not read, and a ping is tried again a while after.
    pub failed: Option<String>,
}

/// The measures of the round of this number of the run: its last try's; none where no try of it said them.
pub fn of_round(measured: &[Measured], run: RunId, round: usize) -> Option<&Measures> {
    (measured.iter().rev())
        .find(|m| m.run == run && m.round == round)
        .map(|m| &m.measures)
}
