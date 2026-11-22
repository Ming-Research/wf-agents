//! What happens to a work, and what is to be done for it.

use std::collections::BTreeSet;

use decide::run::{Failure, Unmet};
use decide::{BlockId, CallId, CellId, End, Outsider, Pauser, RunId, Seq};
use flow::{Compiled, Kind};

use work::{Ask, Call, Fill, Given, Handed, Measures, Question, RanOn, Said, Spent, Time, Why};

use crate::spoken::{NoOne, Telling, To};

/// One thing that happened to a work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Nothing new is recorded: the work is looked at again. A new work, or a changed flow.
    Look,
    /// A person or the outside wrote a block.
    Wrote {
        block: BlockId,
        by: Outsider,
        content: String,
        /// Whether what the outside wrote changes the block's meaning; not read for a person's prose or a choice.
        matters: bool,
        value: Option<String>,
    },
    /// The outside wrote a block while the round of this run was under way, as its carrier's own doing for that round,
    /// as taking main in is: the version is written and put into the round, so that the round is told of it and the
    /// run consumes it. Where the block's latest version says it already, as the watch of main may have written the
    /// head a check reads, no version is written and that one is put in, unless the round was told of it already; a
    /// run abandoned while its round is under way consumes nothing, and the version is only written.
    WroteForRound {
        run: RunId,
        block: BlockId,
        content: String,
    },
    /// The owner let the cell go on at its limit.
    LetGo { cell: CellId },
    /// The work was paused: by the owner, who gives no reason, or by the engine itself, for this reason, as when it
    /// spent more than it may. A round that fails pauses its work by [`Event::Failed`] instead.
    Paused { by: Pauser, why: Option<String> },
    /// The work was let on again.
    Resumed,
    /// The work's item was cancelled, for this reason where whoever cancelled it gave one.
    Cancelled { why: Option<String> },
    /// The work's item, which was cancelled, was reopened, for this reason: the work no longer ends there, and is
    /// paused by the owner, as a work added is, until it is let on.
    Reopened { why: String },
    /// The round of the run that was under way ended, having taken this.
    Returned { run: RunId, came: Came, spent: Spent },
    /// The round of the run that was under way failed, for this reason, having taken this: nothing came back. The
    /// cause is of the kind `failure`, as whatever carried the round out told it apart, and `resets` says when the
    /// account's usage limit resets, where that is the cause and its report says when. Where the run is open, the
    /// engine handles the failure as its kind says (crate `decide`, module `run`): where that is to pause the work, it
    /// pauses it with the reason as it takes this; the run's next round is given what the failed one was given.
    Failed {
        run: RunId,
        why: String,
        failure: Failure,
        resets: Option<Time>,
        spent: Spent,
    },
    /// The call has its result.
    Done { call: CallId, result: String },
    /// The owner says these words to a cell of the work's flow, or to a line of it: to the cell the engine chooses
    /// (crate module `spoken`).
    Said { to: To, words: String },
    /// What the owner said reached the round of this run that is under way, after a use of one of its program's own
    /// tools: these versions, which the engine gave its carrier to tell the round ([`Effect::Tell`]), are put into it.
    SaidToRound { run: RunId, versions: Vec<(BlockId, Seq)> },
    /// The owner answers this version of the question, by its number among the versions of its cell's questions,
    /// taking this option, counted from one, and nothing else (design document `data-and-context`, 30).
    Answer {
        question: String,
        version: u64,
        option: u64,
    },
    /// A new session of the line began, or is to begin with the line's next round, for this reason.
    Anew { line: String, why: Why },
    /// A try of the round of the run that is under way began, on this: the try of this number in the round, the first
    /// being one.
    Ran { run: RunId, attempt: usize, on: RanOn },
    /// The message of the try of this number of the round of the run that is under way filled this into its session,
    /// as the context manager worked it out (crate `context`).
    Filled {
        run: RunId,
        attempt: usize,
        fills: Vec<Fill>,
    },
    /// What the calls to the model of the try of this number of the round of the run that is under way came to, as
    /// its carrier read them.
    Measured {
        run: RunId,
        attempt: usize,
        measures: Measures,
    },
    /// The program of the try of this number of the round of the run that is under way ended by itself and left these
    /// many processes running in its group, one or more, which its carrier stopped before it gave the try's end.
    LeftBehind {
        run: RunId,
        attempt: usize,
        processes: usize,
    },
    /// A keep-warm ping of the session of this number of the line ended, having taken this, or failed for this
    /// reason, as [`Effect::Ping`] asked.
    Pinged {
        line: String,
        session: u32,
        spent: Spent,
        failed: Option<String>,
    },
    /// The host of the work's repository found the work checked through the pull request of this number, at this
    /// address, opening it where the work had none: kept with the work where it is another than the latest kept.
    Pulled { number: u64, url: String },
    /// What the work held outside the engine was let go, as [`Effect::Release`] asked.
    Released,
    /// What the work held outside the engine could not be let go, for this reason, as whatever carried out
    /// [`Effect::Release`] says.
    NotReleased { why: String },
}

/// What came back at the end of a round: the return of the agent, or of the engine's act. A return either asks for
/// calls, and for nothing else, or does the cell's work (design document `data-and-context`, 30).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Came {
    /// Which of what ran the round the return dealt with, in the order it names them; `None` where it does not say, as
    /// the engine's acts and no return read in the form before a return named it do: such a return dealt with
    /// everything that ran the round, and the engine records that.
    pub dealt_with: Option<Vec<BlockId>>,
    /// The blocks it writes.
    pub writes: Vec<Handed>,
    /// The calls it asks for.
    pub asks: Vec<Ask>,
    /// What was found wrong with it as it was read against the cell it was entered for, which only its reading can
    /// see, in the order found: the run's check names these first, with everything else wrong with it.
    pub found: Vec<Unmet>,
    /// What it says beside its work.
    pub said: Said,
    /// What the engine itself asks the owner as the round ends, from what it found of the work then, such as a change
    /// of a path the project protects: no word of the agent's, and so no question the return is held to or let off for.
    /// It is recorded as the round's end is taken, whether or not the return is.
    pub engine: Vec<Question>,
    /// A block a source outside the work writes, which runs the cell, a new version of which what the return writes
    /// waits for, as the engine's own act says where what it found waits for that source; none of an agent's return.
    pub waits_for: Option<BlockId>,
    /// What the carrier read of the work's branch as the round ended, of the blocks a source outside the work writes
    /// from it, where it differs from what each says: what the round committed, which the run writes as it finishes,
    /// beside its own blocks; nothing of it is written where the return is sent back.
    pub read: Vec<Handed>,
}

impl Came {
    /// A return that does the cell's work, writing these blocks, without naming what it dealt with: it dealt with
    /// everything that ran its round, as the engine's acts and a return of the form before a return named it do.
    pub fn hands_in(writes: Vec<Handed>) -> Came {
        Came {
            writes,
            ..Came::default()
        }
    }

    /// A return that asks for these calls, and so deals with nothing.
    pub fn asks(asks: Vec<Ask>) -> Came {
        Came {
            dealt_with: Some(Vec::new()),
            asks,
            ..Came::default()
        }
    }

    /// A return that does the cell's work: it dealt with these, and writes these blocks.
    pub fn works(dealt_with: Vec<BlockId>, writes: Vec<Handed>) -> Came {
        Came {
            dealt_with: Some(dealt_with),
            writes,
            ..Came::default()
        }
    }
}

/// What is to be done for a work outside the engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// This round of the run is given to the cell's agent, or acted out where the cell is the engine's own.
    Round {
        run: RunId,
        cell: CellId,
        /// Which round of the run it is, the first being one.
        number: usize,
        given: Given,
    },
    /// This call is made.
    Call { call: CallId, run: RunId, what: Call },
    /// This call is withdrawn: its run was abandoned, or its work ended before its cell was entered for its result.
    /// It is to be done until the call is said to have its result, which is then that it was withdrawn, or whatever
    /// came back before it could be.
    Withdraw { call: CallId },
    /// What the owner said is to be told to the round of this run under way, after its next use of one of its
    /// program's own tools ([`Telling`]). Once the round was given it, its carrier says so with its `versions`
    /// ([`Event::SaidToRound`]); words that reached no round before it ended stay new to the cell, which runs again for
    /// them. It is not given again after the engine stops.
    Tell { run: RunId, telling: Box<Telling> },
    /// The round of this run that is under way is stopped: the run was abandoned, with its work cancelled or for a
    /// cell before it that the owner turned to. It is to be done until the round is said to have ended; what comes
    /// back from it is dropped.
    Stop { run: RunId },
    /// The work has ended so, and nothing of it is under way: what it holds outside the engine is let go (its
    /// worktree, and its pull request where it did not merge; its branch stays). It is to be done until the work is
    /// said to have released it.
    Release { end: End },
    /// The session of this number of the line, which has no round under way, is sent a keep-warm ping, so that the
    /// prompt cache does not let it go. Whatever drives the engine gives it, not the work's loop: it is to be done
    /// until the ping is said to have ended or failed, and given again where it is still due after a stop.
    Ping { line: String, session: u32 },
}

/// Which rounds may begin, of this work as of every other: a round that failed for a cause every work shares, such as
/// the account's usage limit, holds them, as whatever drives the engine works out from the records of every work. It
/// holds back nothing of a run but the beginning of its rounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gate {
    /// Every round may begin.
    #[default]
    Open,
    /// No round of an agent begins; the engine's own acts do.
    Agents,
    /// No round of the engine's own acts begins, their host being out of reach; the agents' do.
    Acts,
    /// No round begins, of an agent or an act.
    Every,
}

impl Gate {
    /// Whether it holds the agents' rounds.
    pub fn holds_agents(self) -> bool {
        matches!(self, Gate::Agents | Gate::Every)
    }

    /// Whether it holds the engine's own acts.
    pub fn holds_acts(self) -> bool {
        matches!(self, Gate::Acts | Gate::Every)
    }

    /// The gate that holds what either holds.
    pub fn and(self, other: Gate) -> Gate {
        match (self, other) {
            (Gate::Open, gate) | (gate, Gate::Open) => gate,
            (Gate::Agents, Gate::Agents) => Gate::Agents,
            (Gate::Acts, Gate::Acts) => Gate::Acts,
            _ => Gate::Every,
        }
    }
}

/// Which rounds of a work may begin: those the wait every work shares lets ([`Gate`]), and those of a cell in none of
/// the lines whose session a keep-warm ping of the work has under way, which the session's program runs once for each
/// at a time. Whatever drives the engine works it out from the records of every work and the pings it has given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hold {
    /// The rounds the wait every work shares holds.
    pub gate: Gate,
    /// The lines whose session is being pinged: no round of a cell of one of them begins.
    pub lines: BTreeSet<String>,
}

impl From<Gate> for Hold {
    fn from(gate: Gate) -> Hold {
        Hold {
            gate,
            lines: BTreeSet::new(),
        }
    }
}

impl Hold {
    /// The hold that holds what this does and what the gate does.
    pub fn and(mut self, gate: Gate) -> Hold {
        self.gate = self.gate.and(gate);
        self
    }

    /// Whether the rounds of the cell are held: by the gate, which holds every round, or an agent's and the cell is
    /// one an agent carries out, or an act's and the cell is one the engine carries out; or by its line being pinged.
    pub fn holds(&self, flow: &Compiled, cell: &CellId) -> bool {
        let spec = (flow.cells.iter()).find(|spec| spec.id == *cell);
        let by_gate = match self.gate {
            Gate::Open => false,
            Gate::Every => true,
            Gate::Agents => spec.is_some_and(|spec| spec.kind == Kind::Agent),
            Gate::Acts => spec.is_some_and(|spec| spec.kind == Kind::Engine),
        };
        by_gate
            || spec
                .and_then(|spec| spec.line.as_ref())
                .is_some_and(|line| self.lines.contains(line))
    }
}

/// Why what happened was not taken: it cannot have happened to this work, or nothing can be decided of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Doing what the deciding code decides on the work's records leads it to decide again without end: the loop
    /// stopped at its bound, deciding this last. What the work was given meanwhile is let go with what happened.
    WithoutEnd(decide::Decision),
    /// The flow has no such block.
    NoSuchBlock(BlockId),
    /// The flow does not let this writer write the block.
    NotTheirs { block: BlockId, by: Outsider },
    /// The block is a choice, and was written with no value.
    NoValue { block: BlockId },
    /// The block is a choice, and this is not one of its values.
    NotAValue { block: BlockId, value: String },
    /// The block is no choice, and was written with a value.
    NotAChoice { block: BlockId },
    /// The flow has no such cell.
    NoSuchCell(CellId),
    /// No round of the run is under way.
    NoRoundUnderWay(RunId),
    /// The work has no such call.
    NoSuchCall(CallId),
    /// The call has its result already.
    AlreadyDone(CallId),
    /// The flow has no cell on such a line.
    NoSuchLine(String),
    /// The work has not ended, or something of it is still under way: it has nothing to let go yet.
    NotToRelease,
    /// The work has no question of this name.
    NoSuchQuestion(String),
    /// The question has no version of this number that asks it: none of its versions has that number among the
    /// versions of its cell's questions, or the one that has withdrew the question.
    NoSuchVersion { question: String, version: u64 },
    /// The version of the question has no option of this number.
    NotAnOption { question: String, option: u64 },
    /// The work's item is not cancelled: there is no cancel to reopen.
    NotCancelled,
    /// No cell can take the owner's words, for this reason: they go to no other.
    NoOneSpokenTo(NoOne),
    /// The work ended on a version of its flow that is not installed, or on none that was kept, and no flow decides it
    /// now: what happened would be decided by one.
    NotInstalled,
}

#[cfg(test)]
mod tests {
    //! Which rounds a hold holds back.

    use flow::{Has, Source, compile};

    use super::*;

    /// Two agent cells on two lines, `write` on `main` and `judge` on `second`, and the merge, an act of the engine.
    fn flow() -> Compiled {
        let text = r#"
            name = "lines"
            order = ["write", "judge", "merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [lines.second]
            [blocks.request]
            people = true
            [blocks.text]
            [blocks.verdict]
            [blocks.merged]
            shown = true
        "#;
        let cell = |line: &str, watches: &str, outputs: &str| {
            format!(
                "title = \"A cell\"\nkind = \"agent\"\neffort = \"high\"\nline = \"{line}\"\ntask = \"Do it.\"\n\
                 watches = [\"{watches}\"]\noutputs = [\"{outputs}\"]\n[kinds]\n{watches} = \"last_and_latest\"\n"
            )
        };
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"verdict\"]\noutputs = [\"merged\"]\n";
        let source = Source {
            flow: text.to_string(),
            cells: [
                ("write", cell("main", "request", "text")),
                ("judge", cell("second", "text", "verdict")),
                ("merge", merge.to_string()),
            ]
            .into_iter()
            .map(|(id, text)| (id.to_string(), text))
            .collect(),
            files: [(
                flow::waits_file(),
                include_str!("../../../flow/agents/waits.toml").to_string(),
            )]
            .into(),
            ..Source::default()
        };
        let has = Has {
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    // The gate holds the agents' rounds, the acts' or every one, and a pinged line holds the rounds of its cells and no
    // other's, each as the table says: whether the rounds of `write`, `judge` and `merge` are held. Fails where the
    // gate's agents and acts are taken the other way round, where a line holds the cells of another, or where a hold
    // by line holds an act. Microseconds.
    #[test]
    fn a_hold_holds_the_rounds_its_gate_holds_and_those_of_the_cells_of_a_pinged_line() {
        let flow = flow();
        let table: [(Gate, Vec<&str>, [bool; 3]); 12] = [
            (Gate::Open, vec![], [false, false, false]),
            (Gate::Open, vec!["main"], [true, false, false]),
            (Gate::Open, vec!["second"], [false, true, false]),
            (Gate::Agents, vec![], [true, true, false]),
            (Gate::Agents, vec!["main"], [true, true, false]),
            (Gate::Agents, vec!["second"], [true, true, false]),
            (Gate::Acts, vec![], [false, false, true]),
            (Gate::Acts, vec!["main"], [true, false, true]),
            (Gate::Acts, vec!["second"], [false, true, true]),
            (Gate::Every, vec![], [true, true, true]),
            (Gate::Every, vec!["main"], [true, true, true]),
            (Gate::Every, vec!["second"], [true, true, true]),
        ];
        for (gate, lines, held) in table {
            let hold = Hold {
                gate,
                lines: lines.iter().map(|line| line.to_string()).collect(),
            };
            for (cell, held) in ["write", "judge", "merge"].into_iter().zip(held) {
                assert_eq!(
                    hold.holds(&flow, &CellId::from(cell)),
                    held,
                    "{gate:?} and the lines {lines:?} for {cell}"
                );
            }
        }
    }
}
