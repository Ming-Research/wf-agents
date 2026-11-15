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
//! acts = [
//!     { at = { waits = "plan", for = "answer", question = "named" }, do = "answer", question = "named", option = 1 },
//! ]
//! # The steps its work takes, in order; the last is its end, where the work stands as the play ends.
//! path = [
//!     { runs = "plan" },
//!     { waits = "plan", for = "answer", question = "named" },
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
//! - `waits`: the work waits at the cell, `for` what: `tool`, a tool's result; `retry`, the engine's wait before a
//!   round that failed in passing is tried again; `answer`, the owner's answer to a question, the one named `question`
//!   where it names one; `outside`, a new version of a block a source outside the work writes, where only cells write
//!   what keeps a key of the cell's preconditions from holding and the latest run of each said that what it wrote waits
//!   for that version (decision what-a-run-wrote-may-wait-for-the-outside), the block it wrote being `block` where it
//!   names one; `limit`, the owner's word on the cell at its limit of a sender, the cell after it named `by`, or, where
//!   it names none, the engine, which sent the cell's returns back; `precondition`, a key of the cell's preconditions
//!   that does not hold, the one named `key` (a named test, or the block a key reads) where it names one; `result`, the
//!   result of what the cell asked for in a record; `slot`, a free slot; `turn`, the turn to check and merge, which
//!   another work holds; `shared`, the wait every work shares.
//! - `paused`: the work is paused, by the `owner` or the `engine`.
//! - `let_on = true`: the owner lets the work on.
//! - `let_go`: the owner lets the cell go on at its limit.
//! - `reopened = true`: the work's item, cancelled, is reopened.
//! - `stalled`: the work stands at the cell and can go on by itself no further.
//! - `ends`: the work ends `merged` or `cancelled`.
//! - `everything`: the pause of everything holds, set by the `owner` or the `engine`.
//!
//! A key that says more of a step, left out, matches whatever it would have said, but for a limit's `by`, which names
//! the engine where it is left out. What a work stands at now, and no step
//! before it, is a path's end only: a round under way, and what no record holds, which is read from the engine's view
//! of the work: a stall, a wait for a slot, for the turn or for what every work shares, and the pause of everything.
//! The result of what a cell asked for in a record is no step of a path: the runs of the cells that give it are.
//!
//! An act's `at` is a state of the work as the engine shows it, in the same words: a round under way, a wait at a cell
//! (but for the turn and what every work shares), paused, stalled or ended. Its `do` is one of what people do to a
//! work: `write` the `block` with `content` and a `value` where it is a choice; `let_go` the `cell` at its limit, which
//! is the owner's word on the engine's call at a run's limit too; `pause`; `resume`; `cancel`, for a reason `why` where
//! one is given, which is the change of the item whoever plans the work makes too, refused while an item that depends
//! on it has not ended; `say` the `words` to a `line`, or about a `question`; `answer` the `question`, taking the
//! `option` counted from one, for a reason `why` where one is given; and `new_session` of a `line`. Or one of what
//! whoever plans the work does to its item, for a reason `why`: give it a `title`; make it depend on the `item` of a
//! place among the items (`depends_on`) or no longer (`no_longer_depends_on`); `tag` or `untag` it with a `tag`;
//! `reopen` it. Or one of what is done to the engine as a whole: `pause_everything`; `resume_everything`; `restart`,
//! stopping the engine and opening it again on its store; and `main`, main moving to a head that says `content`, which
//! the engine writes into every going work of the project whose flow has its block, and which a play gives once every
//! item with a `main` act still to give stands at its next one, so that the one write is each work's own. The head is
//! no act, nor the tree: they are written by the round that reads the work's branch, as the real carrier writes them.

use std::collections::BTreeMap;
use std::fmt;

use decide::run::Failure;
use decide::{BlockId, CellId, Condition, End, Pauser};
use flow::{Compiled, Kind};
use serde::{Deserialize, Serialize};

use super::{QuestionScript, Script};

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
    /// A round of the cell failed, of this kind where it is given.
    Fails {
        cell: CellId,
        failure: Option<Failure>,
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
            (Step::Fails { cell, failure }, Step::Fails { cell: c, failure: f }) => cell == c && or(failure, f),
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

    /// Whether the step is a state a work stands at, which an act may wait for.
    fn state(&self) -> bool {
        match self {
            Step::UnderWay { .. } | Step::Paused(_) | Step::Stalled(_) | Step::Ends(_) => true,
            Step::Waits { on, .. } => !matches!(on, Wait::Turn | Wait::Shared),
            _ => false,
        }
    }

    /// The cells it names.
    fn cells(&self) -> Vec<&CellId> {
        match self {
            Step::Runs(cell)
            | Step::UnderWay { cell, .. }
            | Step::SentBack(cell)
            | Step::Fails { cell, .. }
            | Step::LetGo(cell)
            | Step::Stalled(cell) => vec![cell],
            Step::Waits {
                cell,
                on: Wait::Limit(Some(by)),
            } => vec![cell, by],
            Step::Waits { cell, .. } => vec![cell],
            Step::Paused(_) | Step::LetOn | Step::Reopened | Step::Ends(_) | Step::Everything(_) => Vec::new(),
        }
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
            Step::Fails { cell, failure: None } => write!(f, "{{ fails = \"{cell}\" }}"),
            Step::Fails {
                cell,
                failure: Some(failure),
            } => write!(f, "{{ fails = \"{cell}\", failure = \"{}\" }}", named(*failure)),
            Step::Waits { cell, on } => {
                write!(f, "{{ waits = \"{cell}\", for = ")?;
                match on {
                    Wait::Tool => f.write_str("\"tool\"")?,
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
                failure: self.failure,
            },
            "waits" => {
                let Some(on) = self.on else {
                    return Err("says waits and not for what".to_string());
                };
                let on = match on {
                    Waiting::Tool => Wait::Tool,
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

impl ActScript {
    /// The keys it gives beside `at` and `do`, by name.
    fn given(&self) -> Vec<&'static str> {
        let keys = [
            ("block", self.block.is_some()),
            ("content", self.content.is_some()),
            ("value", self.value.is_some()),
            ("cell", self.cell.is_some()),
            ("why", self.why.is_some()),
            ("line", self.line.is_some()),
            ("question", self.question.is_some()),
            ("words", self.words.is_some()),
            ("option", self.option.is_some()),
            ("title", self.title.is_some()),
            ("item", self.item.is_some()),
            ("tag", self.tag.is_some()),
        ];
        keys.into_iter()
            .filter(|(_, given)| *given)
            .map(|(key, _)| key)
            .collect()
    }
}

impl Does {
    /// The keys an act that does this must give, and those it may give besides.
    fn keys(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            Does::Write => (&["block", "content"], &["value"]),
            Does::LetGo => (&["cell"], &[]),
            Does::Cancel => (&[], &["why"]),
            Does::Say => (&["words"], &["line", "question"]),
            Does::Answer => (&["question", "option"], &["why"]),
            Does::NewSession => (&["line"], &[]),
            Does::Title => (&["title", "why"], &[]),
            Does::DependsOn | Does::NoLongerDependsOn => (&["item", "why"], &[]),
            Does::Tag | Does::Untag => (&["tag", "why"], &[]),
            Does::Reopen => (&["why"], &[]),
            Does::Main => (&["content"], &[]),
            Does::Pause | Does::Resume | Does::PauseEverything | Does::ResumeEverything | Does::Restart => (&[], &[]),
        }
    }
}

/// Everything wrong with the script's items for these flows, each in words with where it is: an item of a flow there is
/// not, of a blank title, depending on an item that does not come before it, or written a block people do not write or
/// a value not its block's; a step or a state that says other than one thing, or names a cell, a block, a key, a
/// question or a sender its flow does not have; a wait for the outside by a block no cell writes, for a slot at a cell
/// that takes none, for the turn at an agent's cell, or for the result of a record at a cell that asks for nothing in
/// one; a path whose step that only the engine's view shows is not its end, or that names the result of what a cell
/// asked for; an act's state that is no state of a work; an act that leaves out a key what it does needs, gives one it
/// does not take, or names what its flow or the script does not have; and items whose main moves at the same place to
/// heads that differ.
pub(super) fn check(script: &Script, flows: &[Compiled]) -> Vec<String> {
    let mut faults = Vec::new();
    let items = &script.items;
    for (k, item) in items.iter().enumerate() {
        let at = format!("items[{k}]");
        if item.title.trim().is_empty() {
            faults.push(format!(
                "{at}.title: a title says what the item is to deliver: give it words"
            ));
        }
        for place in &item.depends_on {
            if *place == 0 || *place > k {
                faults.push(format!(
                    "{at}.depends_on: {place} is no place of an item before it, counted from 1"
                ));
            }
        }
        let Some(flow) = flows.iter().find(|flow| flow.name == item.flow) else {
            faults.push(format!("{at}.flow: no flow is named {}", item.flow));
            continue;
        };
        let names = Names { flow, script };
        for (j, write) in item.writes.iter().enumerate() {
            let written = names.written(&write.block, write.value.as_deref());
            faults.extend(written.into_iter().map(|fault| format!("{at}.writes[{j}]: {fault}")));
        }
        for (j, act) in item.acts.iter().enumerate() {
            let at = format!("{at}.acts[{j}]");
            match act.at.step() {
                Err(fault) => faults.push(format!("{at}.at: {fault}")),
                Ok(state) if !state.state() => faults.push(format!("{at}.at: {state} is no state of a work")),
                Ok(state) => faults.extend(names.step(&state).into_iter().map(|fault| format!("{at}.at: {fault}"))),
            }
            let (needs, takes) = act.does.keys();
            let given = act.given();
            for key in needs.iter().filter(|key| !given.contains(key)) {
                faults.push(format!("{at}: an act that does {} gives {key}", named(act.does)));
            }
            for key in given.iter().filter(|key| !needs.contains(key) && !takes.contains(key)) {
                faults.push(format!("{at}: an act that does {} takes no {key}", named(act.does)));
            }
            if act.does == Does::Say && act.line.is_some() == act.question.is_some() {
                faults.push(format!(
                    "{at}: words are said to a line or about a question, one of them"
                ));
            }
            faults.extend(
                names
                    .act(act, items.len(), k)
                    .into_iter()
                    .map(|fault| format!("{at}: {fault}")),
            );
        }
        let last = item.path.len().saturating_sub(1);
        for (j, step) in item.path.iter().enumerate() {
            let at = format!("{at}.path[{j}]");
            let step = match step.step() {
                Ok(step) => step,
                Err(fault) => {
                    faults.push(format!("{at}: {fault}"));
                    continue;
                }
            };
            if step.ends_only() && j != last {
                faults.push(format!(
                    "{at}: {step} is read from the engine's view of the work now, and so only a path's end"
                ));
            }
            if matches!(step, Step::Waits { on: Wait::Result, .. }) {
                faults.push(format!(
                    "{at}: the result of what a cell asked for is no step of a path: the runs of the cells that \
                     give it are"
                ));
            }
            faults.extend(names.step(&step).into_iter().map(|fault| format!("{at}: {fault}")));
        }
    }
    // Main moves once for every item whose act moves it, so the acts that move it together say one head.
    let mut heads: BTreeMap<usize, Vec<(usize, &str)>> = BTreeMap::new();
    for (k, item) in items.iter().enumerate() {
        let moves = (item.acts.iter()).filter(|act| act.does == Does::Main);
        for (n, act) in moves.enumerate() {
            heads
                .entry(n)
                .or_default()
                .push((k, act.content.as_deref().unwrap_or_default()));
        }
    }
    for (n, moved) in heads {
        if moved.iter().any(|(_, head)| *head != moved[0].1) {
            let items: Vec<String> = moved.iter().map(|(k, _)| format!("items[{k}]")).collect();
            faults.push(format!(
                "{}: main moves once for all of them at their main act {}, and they say other heads",
                items.join(", "),
                n + 1
            ));
        }
    }
    faults
}

/// What the names a script's item gives are checked against: its flow, and the script's rounds on it.
struct Names<'a> {
    flow: &'a Compiled,
    script: &'a Script,
}

impl Names<'_> {
    /// What is wrong with a cell's name: none where the flow has the cell.
    fn cell(&self, cell: &CellId) -> Option<String> {
        let has = self.flow.cells.iter().any(|c| c.id == *cell);
        (!has).then(|| format!("the flow {} has no cell {cell}", self.flow.name))
    }

    /// Everything wrong with a block people are to write with this value: a block of no such name, one people do not
    /// write, a choice written with no value or with one that is not its own, and a value for a block that is no
    /// choice.
    fn written(&self, block: &str, value: Option<&str>) -> Vec<String> {
        let name = &self.flow.name;
        let Some(b) = self.flow.blocks.get(&BlockId::from(block)) else {
            return vec![format!("the flow {name} has no block {block}")];
        };
        let mut faults = Vec::new();
        if !b.writers.people {
            faults.push(format!("people do not write the block {block}"));
        }
        match (&b.values, value) {
            (Some(values), None) => faults.push(format!(
                "the block {block} is a choice, written with one of its values: {}",
                values.join(", ")
            )),
            (Some(values), Some(value)) if !values.iter().any(|v| v == value) => {
                faults.push(format!("{value} is not one of the values of {block}"));
            }
            (None, Some(value)) => faults.push(format!("the block {block} is no choice, which {value} would be")),
            _ => {}
        }
        faults
    }

    /// What is wrong with a question's name: none where a round of the script on this flow asks it, or asks the
    /// engine's own question, which the words beside the flows name and the check does not read.
    fn question(&self, question: &str) -> Option<String> {
        let rounds = (self.script.flows.get(&self.flow.name)).map_or(&[][..], |script| script.rounds.as_slice());
        let named = |asked: &QuestionScript| match asked {
            QuestionScript::Asked(asked) => asked.name == question,
            QuestionScript::Withdrawn(withdrawn) => withdrawn.name == question,
        };
        let asked = (rounds.iter()).any(|round| round.questions.iter().flatten().any(named));
        let engine = rounds.iter().any(|round| round.engine.is_some());
        (!asked && !engine).then(|| format!("no round of the script asks a question named {question}"))
    }

    /// Everything wrong with what the step names.
    fn step(&self, step: &Step) -> Vec<String> {
        let mut faults: Vec<String> = step.cells().into_iter().filter_map(|cell| self.cell(cell)).collect();
        if !faults.is_empty() {
            return faults;
        }
        let place = |cell: &CellId| self.flow.flow.place(cell);
        let Step::Waits { cell, on } = step else {
            return faults;
        };
        let spec = self.flow.cells.iter().find(|c| c.id == *cell);
        let i = place(cell).expect("the cell is in the flow");
        match on {
            Wait::Limit(Some(by)) if place(by).is_none_or(|j| j <= i) => {
                faults.push(format!("{by} is no cell after {cell}, which sends a cell back"));
            }
            Wait::Precondition(Some(key)) => {
                let named = |c: &Condition| match c {
                    Condition::Test(test) => test.as_str() == key,
                    Condition::Is { block, .. } => block.as_str() == key,
                };
                if !self.flow.flow.cells()[i].requires.iter().any(named) {
                    faults.push(format!("{key} is no key of the preconditions of {cell}"));
                }
            }
            Wait::Outside(Some(block)) if !self.flow.blocks.contains_key(block) => {
                faults.push(format!("the flow {} has no block {block}", self.flow.name));
            }
            Wait::Outside(Some(block)) if self.flow.flow.writers(block).next().is_none() => {
                faults.push(format!("no cell writes {block}, which a run waits for the outside by"));
            }
            Wait::Slot if !self.flow.flow.cells()[i].slot => {
                faults.push(format!("{cell} takes no slot, as an act of the engine's takes none"));
            }
            Wait::Result if self.flow.flow.cells()[i].asks.is_empty() => {
                faults.push(format!("{cell} asks for nothing in a record"));
            }
            Wait::Answer(Some(question)) => faults.extend(self.question(question)),
            Wait::Turn if spec.is_some_and(|spec| spec.kind == Kind::Agent) => {
                faults.push(format!(
                    "{cell} is no act of the engine, which alone waits for the turn"
                ));
            }
            _ => {}
        }
        faults
    }

    /// Everything wrong with what the act names, among `items` items, it being of the item at place `k`.
    fn act(&self, act: &ActScript, items: usize, k: usize) -> Vec<String> {
        let mut faults = Vec::new();
        let lines = |line: &str| self.flow.lines.contains_key(line);
        match act.does {
            Does::Write => faults.extend(
                act.block
                    .as_deref()
                    .map(|block| self.written(block, act.value.as_deref()))
                    .unwrap_or_default(),
            ),
            Does::LetGo => faults.extend(act.cell.as_deref().and_then(|cell| self.cell(&CellId::from(cell)))),
            Does::Say | Does::NewSession => {
                if let Some(line) = act.line.as_deref().filter(|line| !lines(line)) {
                    faults.push(format!("the flow {} has no line {line}", self.flow.name));
                }
                faults.extend(act.question.as_deref().and_then(|question| self.question(question)));
            }
            Does::Answer => faults.extend(act.question.as_deref().and_then(|question| self.question(question))),
            Does::DependsOn | Does::NoLongerDependsOn => {
                if let Some(item) = act.item.filter(|item| *item == 0 || *item > items || *item == k + 1) {
                    faults.push(format!(
                        "{item} is no place of another of the script's items, counted from 1"
                    ));
                }
            }
            Does::Main
                if !self
                    .flow
                    .blocks
                    .values()
                    .any(|b| b.writers.outside.as_deref() == Some(driver::MAIN)) =>
            {
                faults.push(format!("the flow {} has no block of {}", self.flow.name, driver::MAIN));
            }
            _ => {}
        }
        if act.does == Does::Answer && act.option == Some(0) {
            faults.push("an option is counted from 1".to_string());
        }
        faults
    }
}
