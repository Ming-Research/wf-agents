//! What a script says of the items it adds, for a play of it (crate module `player`): each item, what people and the
//! outside do at states of its work, and the path its work must take, which the judge reads its records against
//! (crate module `judge`).
//!
//! ```toml
//! # The items the script adds, in order, so that on an empty store the kth item's work is the kth work. Each is
//! # added at the one level of a script's engine, in the project the play names, and let on unless it is held.
//! [[items]]
//! title = "Greet the user"
//! flow = "trial"
//! # What people write for it as it is added.
//! writes = [{ block = "request", content = "Greet the user, please." }]
//! # What people and the outside do, each once the work stands at its state, in order.
//! acts = [{ at = { waits = "plan", for = "person" }, do = "result", result = "Yes, by name." }]
//! # The steps its work takes, in order; the last is its end, where the work stands as the play ends.
//! path = [
//!     { runs = "plan" },
//!     { waits = "plan", for = "person" },
//!     { runs = "plan" },
//!     { runs = "build" },
//!     { waits = "build", for = "tool" },
//!     { runs = "build" },
//!     { runs = "review" },
//!     { sent_back = "review" },
//!     { runs = "merge" },
//!     { ends = "merged" },
//! ]
//!
//! # An item that depends on the one before it, by its place among the items, counted from one, and that is never
//! # let on: it waits for that item to merge.
//! [[items]]
//! title = "Say goodbye"
//! flow = "trial"
//! depends_on = [1]
//! held = true
//! path = [{ waits = "plan", for = "precondition", key = "ready" }]
//! ```
//!
//! A step is a table that says one thing by its first key, naming the cell where it has one:
//!
//! - `runs`: a run of the cell starts.
//! - `under_way`: a round of the cell is under way, of its `run`th run and that run's `round`th round where they are
//!   given, each counted from one.
//! - `sent_back`: what a round of the cell returned is sent back.
//! - `fails`: a round of the cell fails, of the kind of cause `failure` names, as a round entry names it.
//! - `waits`: the work waits at the cell, `for` what: `tool`, a tool's result; `person`, a person's result to a call,
//!   `who` being the owner unless it says the architect; `retry`, the engine's wait before a round that failed in
//!   passing is tried again; `answer`, the owner's answer to a question, the one named `question` where it names one;
//!   `outside`, a new version from outside the work of the block a run of the cell wrote that what it wrote waits for,
//!   that `block` where it names one; `limit`, the owner's word on the cell at its limit of a sender, the cell after it
//!   named `by`, or, where it names none, the engine, which sent the cell's returns back; `precondition`, a key of the
//!   cell's preconditions that does not hold, the one named `key` (a named test, or the block a key reads) where it
//!   names one; `result`, the result of what the cell asked for in a record; `slot`, a free slot; `turn`, the turn to
//!   check and merge, which another work holds; `shared`, the wait every work shares.
//! - `paused`: the work is paused, by the `owner` or the `engine`.
//! - `let_on = true`: the owner lets the work on.
//! - `let_go`: the owner lets the cell go on at its limit.
//! - `reopened = true`: the work's item, cancelled, is reopened.
//! - `stalled`: the work stands at the cell and can go on by itself no further.
//! - `ends`: the work ends `merged` or `cancelled`.
//! - `everything`: the pause of everything holds, set by the `owner` or the `engine`.
//!
//! A step of a path left out of a key that names one thing of several matches any of them. What no record of a work
//! holds is read from the engine's view of it now, so that a path names it only as its end: a round under way, a stall,
//! a wait for a slot, the turn or what every work shares, and the pause of everything. The result of what a cell asked
//! for in a record is no step of a path: the runs of the cells that give it are.
//!
//! An act's `at` is a state of the work as the engine shows it, in the same words: a round under way, a wait at a cell
//! (but for the turn and what every work shares), paused, stalled or ended. Its `do` is one of what people do to a
//! work: `write` the `block` with `content` and a `value` where it is a choice; `let_go` the `cell`; `pause`; `resume`;
//! `cancel`, for a reason `why` where one is given; `result`, giving the person's call the work waits for its `result`;
//! `say` the `words` to a `line`, or about a `question`; `answer` the `question`, taking the `option` counted from one,
//! for a reason `why` where one is given; and `new_session` of a `line`. Or one of what whoever plans the work does to
//! its item, for a reason `why`: give it a `title`; make it depend on the `item` of a place among the items
//! (`depends_on`) or no longer (`no_longer_depends_on`); `tag` or `untag` it with a `tag`; `reopen` it. Or one of what
//! is done to the engine as a whole: `pause_everything`; `resume_everything`; `restart`, stopping the engine and
//! opening it again on its store; and `main`, main moving to a head that says `content`, which the engine writes into
//! every going work of the project whose flow has its block, and which a play gives once every item with a `main` act
//! still to give stands at its next one, so that the one write is each work's own. The head is no act, nor the tree:
//! they are written by the round that reads the work's branch, as the real carrier writes them.

use std::fmt;

use decide::run::Failure;
use decide::{BlockId, CellId, End, Pauser, Who};
use flow::Compiled;
use serde::{Deserialize, Serialize};

use super::{Person, Script};

/// The one level of the items of an engine whose agents follow a script, which run on no model: a play adds every
/// item at it, and a script's engine has no other.
pub const LEVEL: &str = "1";

/// An item a script adds.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemScript {
    pub title: String,
    /// The name of the flow its work runs on.
    pub flow: String,
    /// The items it depends on, each by its place among the script's items, counted from one: each comes before it.
    #[serde(default)]
    pub depends_on: Vec<usize>,
    /// What people write for it as it is added, in order.
    #[serde(default)]
    pub writes: Vec<WriteScript>,
    /// Whether it is never let on: it stays paused as it was added, but for what its acts do.
    #[serde(default)]
    pub held: bool,
    /// What people and the outside do, each at a state of its work, in order.
    #[serde(default)]
    pub acts: Vec<ActScript>,
    /// The steps its work takes, in order, the last its end.
    #[serde(default)]
    pub path: Vec<StepScript>,
}

/// A block people write for an item as it is added.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteScript {
    pub block: String,
    pub content: String,
    /// Its value, where it is a choice.
    pub value: Option<String>,
}

/// A step of a path, or a state of a work an act waits for, as the script writes it: one key says what it is, and the
/// others say more of it (the module's own words).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepScript {
    pub runs: Option<String>,
    pub under_way: Option<String>,
    pub run: Option<usize>,
    pub round: Option<usize>,
    pub sent_back: Option<String>,
    pub fails: Option<String>,
    pub failure: Option<Failure>,
    pub waits: Option<String>,
    #[serde(rename = "for")]
    pub on: Option<Waiting>,
    pub who: Option<Person>,
    pub question: Option<String>,
    pub block: Option<String>,
    pub by: Option<String>,
    pub key: Option<String>,
    pub paused: Option<By>,
    pub let_on: Option<bool>,
    pub let_go: Option<String>,
    pub reopened: Option<bool>,
    pub stalled: Option<String>,
    pub ends: Option<Ending>,
    pub everything: Option<By>,
}

/// What a work waits for, as a step names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Waiting {
    Tool,
    Person,
    Retry,
    Answer,
    Outside,
    Limit,
    Precondition,
    Result,
    Slot,
    Turn,
    Shared,
}

/// Who pauses a work, or the pause of everything, as a step names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    Owner,
    Engine,
}

/// How a work ends, as a step names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ending {
    Merged,
    Cancelled,
}

/// What an act does, at the state of the work it waits for.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActScript {
    pub at: StepScript,
    #[serde(rename = "do")]
    pub does: Does,
    pub block: Option<String>,
    pub content: Option<String>,
    pub value: Option<String>,
    pub cell: Option<String>,
    pub why: Option<String>,
    pub result: Option<String>,
    pub line: Option<String>,
    pub question: Option<String>,
    pub words: Option<String>,
    pub option: Option<u64>,
    pub title: Option<String>,
    pub item: Option<usize>,
    pub tag: Option<String>,
}

/// What an act does (the module's own words).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Does {
    Write,
    LetGo,
    Pause,
    Resume,
    Cancel,
    Result,
    Say,
    Answer,
    NewSession,
    Title,
    DependsOn,
    NoLongerDependsOn,
    Tag,
    Untag,
    Reopen,
    PauseEverything,
    ResumeEverything,
    Restart,
    Main,
}

/// A step of a path, or a state of a work, as the player and the judge read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Runs(CellId),
    /// A round of the cell under way: of its nth run and that run's nth round, where they are given.
    UnderWay {
        cell: CellId,
        run: Option<usize>,
        round: Option<usize>,
    },
    SentBack(CellId),
    Fails {
        cell: CellId,
        failure: Failure,
    },
    Waits {
        cell: CellId,
        on: Wait,
    },
    Paused(Pauser),
    LetOn,
    LetGo(CellId),
    Reopened,
    Stalled(CellId),
    Ends(End),
    /// The pause of everything holds, set by this pauser.
    Everything(Pauser),
}

/// What a work waits for at a cell; what is left out of one matches any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wait {
    Tool,
    Person(Who),
    Retry,
    Answer(Option<String>),
    Outside(Option<BlockId>),
    /// At the cell's limit of this sender, a cell after it; of the engine, which sent its returns back, where none.
    Limit(Option<CellId>),
    /// A key of the cell's preconditions, by its name: a named test's, or the block a key of a value reads.
    Precondition(Option<String>),
    Result,
    Slot,
    Turn,
    Shared,
}

impl Step {
    /// Whether this step, as a path or an act says it, says what was seen: what it leaves out matches anything.
    pub fn matches(&self, seen: &Step) -> bool {
        fn or<T: PartialEq>(said: &Option<T>, seen: &Option<T>) -> bool {
            said.is_none() || said == seen
        }
        match (self, seen) {
            (
                Step::UnderWay { cell, run, round },
                Step::UnderWay {
                    cell: c,
                    run: r,
                    round: n,
                },
            ) => cell == c && or(run, r) && or(round, n),
            (Step::Waits { cell, on }, Step::Waits { cell: c, on: o }) => {
                cell == c
                    && match (on, o) {
                        (Wait::Answer(q), Wait::Answer(p)) => or(q, p),
                        (Wait::Outside(b), Wait::Outside(c)) => or(b, c),
                        (Wait::Limit(b), Wait::Limit(c)) => b == c,
                        (Wait::Precondition(k), Wait::Precondition(l)) => or(k, l),
                        (on, o) => on == o,
                    }
            }
            (said, seen) => said == seen,
        }
    }

    /// Whether the step is one no record of the work holds, read from the engine's view of the work now, and so only a
    /// path's end.
    pub fn ends_only(&self) -> bool {
        matches!(
            self,
            Step::UnderWay { .. }
                | Step::Stalled(_)
                | Step::Everything(_)
                | Step::Waits {
                    on: Wait::Slot | Wait::Turn | Wait::Shared,
                    ..
                }
        )
    }
}

/// A step as a script writes it.
impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let by = |by: &Pauser| match by {
            Pauser::Owner => "owner",
            Pauser::Engine => "engine",
        };
        match self {
            Step::Runs(cell) => write!(f, "{{ runs = \"{cell}\" }}"),
            Step::UnderWay { cell, run, round } => {
                write!(f, "{{ under_way = \"{cell}\"")?;
                if let Some(run) = run {
                    write!(f, ", run = {run}")?;
                }
                if let Some(round) = round {
                    write!(f, ", round = {round}")?;
                }
                f.write_str(" }")
            }
            Step::SentBack(cell) => write!(f, "{{ sent_back = \"{cell}\" }}"),
            Step::Fails { cell, failure } => write!(f, "{{ fails = \"{cell}\", failure = \"{}\" }}", named(*failure)),
            Step::Waits { cell, on } => {
                write!(f, "{{ waits = \"{cell}\", for = ")?;
                match on {
                    Wait::Tool => f.write_str("\"tool\"")?,
                    Wait::Person(Who::Owner) => f.write_str("\"person\"")?,
                    Wait::Person(Who::Architect) => f.write_str("\"person\", who = \"architect\"")?,
                    Wait::Retry => f.write_str("\"retry\"")?,
                    Wait::Answer(None) => f.write_str("\"answer\"")?,
                    Wait::Answer(Some(question)) => write!(f, "\"answer\", question = \"{question}\"")?,
                    Wait::Outside(None) => f.write_str("\"outside\"")?,
                    Wait::Outside(Some(block)) => write!(f, "\"outside\", block = \"{block}\"")?,
                    Wait::Limit(None) => f.write_str("\"limit\"")?,
                    Wait::Limit(Some(by)) => write!(f, "\"limit\", by = \"{by}\"")?,
                    Wait::Precondition(None) => f.write_str("\"precondition\"")?,
                    Wait::Precondition(Some(key)) => write!(f, "\"precondition\", key = \"{key}\"")?,
                    Wait::Result => f.write_str("\"result\"")?,
                    Wait::Slot => f.write_str("\"slot\"")?,
                    Wait::Turn => f.write_str("\"turn\"")?,
                    Wait::Shared => f.write_str("\"shared\"")?,
                }
                f.write_str(" }")
            }
            Step::Paused(pauser) => write!(f, "{{ paused = \"{}\" }}", by(pauser)),
            Step::LetOn => f.write_str("{ let_on = true }"),
            Step::LetGo(cell) => write!(f, "{{ let_go = \"{cell}\" }}"),
            Step::Reopened => f.write_str("{ reopened = true }"),
            Step::Stalled(cell) => write!(f, "{{ stalled = \"{cell}\" }}"),
            Step::Ends(End::Merged) => f.write_str("{ ends = \"merged\" }"),
            Step::Ends(End::Cancelled) => f.write_str("{ ends = \"cancelled\" }"),
            Step::Everything(pauser) => write!(f, "{{ everything = \"{}\" }}", by(pauser)),
        }
    }
}

/// The name a script gives a value of one of its kinds: a kind of failure, or what an act does.
fn named<T: Serialize + fmt::Debug>(value: T) -> String {
    toml::Value::try_from(&value)
        .ok()
        .and_then(|named| named.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{value:?}"))
}

impl StepScript {
    /// The keys it gives, by name, in the order the module lists them.
    fn given(&self) -> Vec<&'static str> {
        let keys = [
            ("runs", self.runs.is_some()),
            ("under_way", self.under_way.is_some()),
            ("run", self.run.is_some()),
            ("round", self.round.is_some()),
            ("sent_back", self.sent_back.is_some()),
            ("fails", self.fails.is_some()),
            ("failure", self.failure.is_some()),
            ("waits", self.waits.is_some()),
            ("for", self.on.is_some()),
            ("who", self.who.is_some()),
            ("question", self.question.is_some()),
            ("block", self.block.is_some()),
            ("by", self.by.is_some()),
            ("key", self.key.is_some()),
            ("paused", self.paused.is_some()),
            ("let_on", self.let_on.is_some()),
            ("let_go", self.let_go.is_some()),
            ("reopened", self.reopened.is_some()),
            ("stalled", self.stalled.is_some()),
            ("ends", self.ends.is_some()),
            ("everything", self.everything.is_some()),
        ];
        keys.into_iter()
            .filter(|(_, given)| *given)
            .map(|(key, _)| key)
            .collect()
    }

    /// The step it says, or what is wrong with it in words: a step says one thing by one key, and only the keys that
    /// say more of that thing.
    pub fn step(&self) -> Result<Step, String> {
        const SAYS: [&str; 12] = [
            "runs",
            "under_way",
            "sent_back",
            "fails",
            "waits",
            "paused",
            "let_on",
            "let_go",
            "reopened",
            "stalled",
            "ends",
            "everything",
        ];
        let given = self.given();
        let says: Vec<&str> = given.iter().copied().filter(|key| SAYS.contains(key)).collect();
        let &[said] = says.as_slice() else {
            return Err(match says.len() {
                0 => format!("says nothing: a step says one of {}", SAYS.join(", ")),
                _ => format!("says {}: a step says one thing", says.join(" and ")),
            });
        };
        let more: &[&str] = match (said, self.on) {
            ("under_way", _) => &["run", "round"],
            ("fails", _) => &["failure"],
            ("waits", Some(Waiting::Person)) => &["for", "who"],
            ("waits", Some(Waiting::Answer)) => &["for", "question"],
            ("waits", Some(Waiting::Outside)) => &["for", "block"],
            ("waits", Some(Waiting::Limit)) => &["for", "by"],
            ("waits", Some(Waiting::Precondition)) => &["for", "key"],
            ("waits", _) => &["for"],
            _ => &[],
        };
        let stray: Vec<&str> = (given.iter().copied())
            .filter(|key| *key != said && !more.contains(key))
            .collect();
        if !stray.is_empty() {
            return Err(format!("says {} of a step that says {said}", stray.join(" and ")));
        }
        let cell = |name: &Option<String>| CellId::from(name.as_deref().unwrap_or_default());
        let by = |by: Option<By>| match by {
            Some(By::Engine) => Pauser::Engine,
            _ => Pauser::Owner,
        };
        let counted = |n: Option<usize>, what: &str| match n {
            Some(0) => Err(format!("{what} is counted from 1")),
            n => Ok(n),
        };
        Ok(match said {
            "runs" => Step::Runs(cell(&self.runs)),
            "under_way" => Step::UnderWay {
                cell: cell(&self.under_way),
                run: counted(self.run, "a run")?,
                round: counted(self.round, "a round")?,
            },
            "sent_back" => Step::SentBack(cell(&self.sent_back)),
            "fails" => Step::Fails {
                cell: cell(&self.fails),
                failure: self.failure.unwrap_or_default(),
            },
            "waits" => {
                let Some(on) = self.on else {
                    return Err("says waits and not for what".to_string());
                };
                let on = match on {
                    Waiting::Tool => Wait::Tool,
                    Waiting::Person => Wait::Person(match self.who {
                        Some(Person::Architect) => Who::Architect,
                        _ => Who::Owner,
                    }),
                    Waiting::Retry => Wait::Retry,
                    Waiting::Answer => Wait::Answer(self.question.clone()),
                    Waiting::Outside => Wait::Outside(self.block.as_deref().map(BlockId::from)),
                    Waiting::Limit => Wait::Limit(self.by.as_deref().map(CellId::from)),
                    Waiting::Precondition => Wait::Precondition(self.key.clone()),
                    Waiting::Result => Wait::Result,
                    Waiting::Slot => Wait::Slot,
                    Waiting::Turn => Wait::Turn,
                    Waiting::Shared => Wait::Shared,
                };
                Step::Waits {
                    cell: cell(&self.waits),
                    on,
                }
            }
            "paused" => Step::Paused(by(self.paused)),
            "let_on" if self.let_on == Some(true) => Step::LetOn,
            "reopened" if self.reopened == Some(true) => Step::Reopened,
            "let_on" | "reopened" => return Err(format!("says {said} = false, which says nothing: give true")),
            "let_go" => Step::LetGo(cell(&self.let_go)),
            "stalled" => Step::Stalled(cell(&self.stalled)),
            "ends" => Step::Ends(match self.ends {
                Some(Ending::Cancelled) => End::Cancelled,
                _ => End::Merged,
            }),
            _ => Step::Everything(by(self.everything)),
        })
    }
}

/// Everything wrong with the script's items for these flows: nothing of them is checked yet.
pub(super) fn check(script: &Script, flows: &[Compiled]) -> Vec<String> {
    let _ = (script, flows);
    Vec::new()
}
