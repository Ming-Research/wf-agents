//! What carries out the rounds, acts and calls of a case's work. The history's rounds are handed in as the case says,
//! each read as the engine reads an agent's return of the cell's form and, of a line that may change the work, with
//! what the round committed read of the branch as the agents' carrier reads it; the start of the work makes its branch
//! and worktree in the made project's repository, as the engine's act does on a host that is the repository itself;
//! and once the history is handed in, the rounds of the case's cell, and of the cell of its line that reads the tree
//! first, are the real agents' carrier's. A round the history does not give, before or after, is held, ending with
//! nothing if the engine stops it, and said to whoever runs the case, which judges what it means.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use decide::{BlockId, CellId, RunId};
use driver::Policy;
use engine::Came;
use flow::{CellSpec, Compiled};
use service::{CallReply, Doing, Release, ReleaseReply, Round, RoundReply, Rounds, Telling, Told, ToolCall, Tools};
use store::WorkNumber;
use tokio::sync::mpsc::UnboundedSender;
use work::{Handed, Spent};

use super::made;

/// What the carriers of a case say to whoever runs it, as it happens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    /// The history's round of this place was handed in, as the round of this run.
    Handed { index: usize, run: RunId },
    /// It could not be, for this reason.
    NotHanded { index: usize, why: String },
    /// A round no step of the history gives, of this cell, was given and is held.
    Held { cell: CellId, run: RunId },
    /// A held round was stopped by the engine.
    Stopped { run: RunId },
    /// A round of this cell was given to the real agents' carrier.
    Real { cell: CellId, run: RunId, number: usize },
    /// A call was made of this tool, which the case holds no result for.
    Called { tool: String },
}

/// Whether the history is still being handed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    History,
    Real,
}

/// What the carriers of a case share.
pub struct Shared {
    /// The history's rounds, in order, and how far it is handed in.
    rounds: Vec<cases::Round>,
    state: Mutex<State>,
    /// The case's cell.
    target: CellId,
    /// The made project's commits the history's rounds stand at, by their names.
    commits: BTreeMap<String, String>,
    /// What is written beside the flows, with the made project's declaration.
    beside: flow::agents::Agents,
    /// The project as the carriers carry its works out.
    project: carry::settings::Project,
    /// What the branch's data are read with.
    reading: carry::branch::Reading,
    /// The installation's data directory, and its tag.
    data: PathBuf,
    tag: String,
    /// The commit of main the work starts from.
    main: String,
    /// Where the stand-in for the agents' program reads what it returns, when one stands in.
    stand_in: Option<PathBuf>,
    notes: UnboundedSender<Note>,
}

struct State {
    phase: Phase,
    next: usize,
    /// Whether a round of the case's cell was given to the real carrier.
    began: bool,
    held: BTreeMap<(WorkNumber, RunId, usize), RoundReply>,
}

/// What a case's carriers are made with.
pub struct Making {
    pub rounds: Vec<cases::Round>,
    pub target: CellId,
    pub commits: BTreeMap<String, String>,
    pub beside: flow::agents::Agents,
    pub project: carry::settings::Project,
    pub data: PathBuf,
    pub tag: String,
    pub main: String,
    pub stand_in: Option<PathBuf>,
    pub notes: UnboundedSender<Note>,
}

impl Shared {
    pub fn new(making: Making) -> Arc<Shared> {
        let reading = carry::branch::Reading {
            tree: making.project.tree.clone(),
            main: super::installation::MAIN_REF.to_string(),
            guide: making.project.guide.clone(),
        };
        let phase = match making.rounds.is_empty() {
            true => Phase::Real,
            false => Phase::History,
        };
        Arc::new(Shared {
            rounds: making.rounds,
            state: Mutex::new(State {
                phase,
                next: 0,
                began: false,
                held: BTreeMap::new(),
            }),
            target: making.target,
            commits: making.commits,
            beside: making.beside,
            project: making.project,
            reading,
            data: making.data,
            tag: making.tag,
            main: making.main,
            stand_in: making.stand_in,
            notes: making.notes,
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn note(&self, note: Note) {
        let _ = self.notes.send(note);
    }

    /// The history's next round where it is of this cell, with its place; the history then goes on past it.
    fn next_of(&self, cell: &CellId) -> Option<(usize, cases::Round)> {
        let mut state = self.state();
        if state.phase != Phase::History {
            return None;
        }
        let round = self.rounds.get(state.next).filter(|r| &r.cell == cell)?.clone();
        let index = state.next;
        state.next += 1;
        if state.next == self.rounds.len() {
            state.phase = Phase::Real;
        }
        Some((index, round))
    }

    /// Holds the round, which no step of the history gives, until the engine stops it.
    fn hold(&self, round: &Round, reply: RoundReply) {
        self.state().held.insert((round.work, round.run, round.number), reply);
        self.note(Note::Held {
            cell: round.cell.id.clone(),
            run: round.run,
        });
    }

    /// Ends a held round with nothing, the engine having stopped it; whether it was held.
    fn stopped(&self, round: &Round) -> bool {
        let held = self.state().held.remove(&(round.work, round.run, round.number));
        match held {
            Some(reply) => {
                reply.ended(Came::hands_in(Vec::new()), Spent::default());
                self.note(Note::Stopped { run: round.run });
                true
            }
            None => false,
        }
    }

    /// The names of what ran the round, as the cell watches them.
    fn ran(round: &Round) -> Vec<String> {
        let told = decide::told(&round.flow.flow, &round.kept.records, round.run);
        let ran_by = told.map(|t| t.ran_by).unwrap_or_default();
        round.cell.ran(&ran_by).map(|w| w.name.clone()).collect()
    }
}

/// Whether the cell is one of the case's cell's line that reads the tree for the others, as the line's session block
/// runs it: the cell of the line that watches a block the driver writes as the line's session begins anew.
fn reads_for_the_line(flow: &Compiled, cell: &CellSpec, target: &CellId) -> bool {
    let line = (flow.cells.iter())
        .find(|c| &c.id == target)
        .and_then(|c| c.line.clone());
    let session = |block: &BlockId| {
        (flow.blocks.get(block)).and_then(|b| b.writers.outside.as_deref()) == Some(driver::sessions::SESSION)
    };
    line.is_some() && cell.line == line && cell.watches.iter().any(|w| w.blocks.iter().any(session))
}

/// The agents' rounds of a case.
pub struct Agents {
    pub shared: Arc<Shared>,
    pub real: carry::Agents,
}

impl Agents {
    /// Hands the history's round in, as the engine reads an agent's return, with what it committed read of the branch.
    fn hand_in(&self, index: usize, step: cases::Round, round: Round, reply: RoundReply) {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            match handed(&shared, &step, &round).await {
                Ok(came) => {
                    shared.note(Note::Handed { index, run: round.run });
                    reply.ended(came, Spent::default());
                }
                Err(why) => {
                    shared.note(Note::NotHanded {
                        index,
                        why: why.clone(),
                    });
                    reply.failed(why, Spent::default());
                }
            }
        });
    }
}

/// What the history's round returns: its blocks read as the engine reads an agent's return, and, of a line that may
/// change the work, what it committed, read of the branch moved to the commit it stands at.
async fn handed(shared: &Shared, step: &cases::Round, round: &Round) -> Result<Came, String> {
    let back = cases::back::outcome(step, &Shared::ran(round));
    let mut came = carry::returns::came(&back, &round.flow, &round.cell, &shared.beside)?.came;
    let place = carry::git::place(&shared.data, &shared.tag, round.work);
    if let Some(name) = &step.commit {
        let id = (shared.commits.get(name)).ok_or_else(|| format!("the made project has no commit {name}"))?;
        made::git(&place.dir, &["reset", "--quiet", "--hard", id.as_str()])?;
    }
    if !round.cell.reads_only {
        let mut looking = carry::branch::Looking::of(&round.flow, &round.kept, shared.reading.clone());
        came.read = looking.handed_in(&place).await?;
    }
    Ok(came)
}

impl Rounds for Agents {
    fn give(&self, round: Round, reply: RoundReply) {
        if let Some((index, step)) = self.shared.next_of(&round.cell.id) {
            return self.hand_in(index, step, round, reply);
        }
        let real = {
            let mut state = self.shared.state();
            let target = round.cell.id == self.shared.target;
            let reading = !state.began && reads_for_the_line(&round.flow, &round.cell, &self.shared.target);
            let real = state.phase == Phase::Real && (target || reading);
            state.began |= real && target;
            real
        };
        if !real {
            return self.shared.hold(&round, reply);
        }
        if let Some(at) = &self.shared.stand_in {
            // The stand-in returns what a round that deals with what ran it and hands in nothing returns.
            let nothing = cases::Round {
                cell: round.cell.id.clone(),
                hands_in: Vec::new(),
                commit: None,
            };
            let back = cases::back::outcome(&nothing, &Shared::ran(&round));
            if let Err(e) = std::fs::write(at, back.to_string()) {
                return reply.failed(format!("{}: {e}", at.display()), Spent::default());
            }
        }
        self.shared.note(Note::Real {
            cell: round.cell.id.clone(),
            run: round.run,
            number: round.number,
        });
        self.real.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        if !self.shared.stopped(&round) {
            self.real.stop(round, reply);
        }
    }

    fn tell(&self, round: Round, told: Telling) {
        self.real.tell(round, told);
    }

    fn told(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        self.real.told(work, run, round)
    }

    fn system(&self, work: WorkNumber, line: &str, number: u32) -> Result<Option<String>, String> {
        self.real.system(work, line, number)
    }

    fn doing(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Vec<Doing>, String> {
        self.real.doing(work, run, round)
    }

    fn policy(&self) -> Option<Policy> {
        self.real.policy()
    }

    fn release(&self, release: Release, reply: ReleaseReply) {
        self.real.release(release, reply);
    }
}

/// The engine's acts of a case: the start makes the work's branch and worktree at the commit of main the case's item
/// starts from; another act hands in what the history says it does, and is held where the history says nothing of it.
pub struct Acts {
    pub shared: Arc<Shared>,
}

impl Rounds for Acts {
    fn give(&self, round: Round, reply: RoundReply) {
        if round.cell.act.as_deref() == Some("start") {
            let shared = self.shared.clone();
            tokio::spawn(async move {
                let teller = reply.teller();
                let place = carry::git::place(&shared.data, &shared.tag, round.work);
                let begun = async {
                    carry::git::configuration(&shared.project, &place).await?;
                    carry::git::start(&shared.project, &place, &shared.main).await
                };
                match begun.await {
                    Ok(started) => {
                        let words = &shared.beside.acts;
                        let reading = shared.reading.clone();
                        let came = carry::begun(&round, &shared.project, &started, reading, &teller, words).await;
                        reply.ended(came, Spent::default());
                    }
                    Err(why) => reply.failed(why, Spent::default()),
                }
            });
            return;
        }
        match self.shared.next_of(&round.cell.id) {
            Some((index, step)) => {
                let handed = (step.hands_in.iter())
                    .map(|hand| Handed {
                        block: hand.block.clone(),
                        content: hand.content.clone(),
                        matters: !hand.unchanged,
                        value: hand.value.clone(),
                    })
                    .collect();
                self.shared.note(Note::Handed { index, run: round.run });
                reply.ended(Came::hands_in(handed), Spent::default());
            }
            None => self.shared.hold(&round, reply),
        }
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        if !self.shared.stopped(&round) {
            reply.ended(Came::hands_in(Vec::new()), Spent::default());
        }
    }
}

/// The calls of a case's work: each is said to whoever runs the case, which ends it where the case holds no result for
/// it (design document `testing`, 13).
pub struct Calls {
    pub shared: Arc<Shared>,
}

impl Tools for Calls {
    fn call(&self, call: ToolCall, reply: CallReply) {
        drop(reply);
        self.shared.note(Note::Called { tool: call.name });
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        let _ = call;
        reply.done(service::WITHDRAWN.to_string());
    }
}
