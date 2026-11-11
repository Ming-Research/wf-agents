//! The engine as a process: one loop that drives every work, the interfaces of what carries out what is to be done,
//! and the test implementations of those, which follow a script.
//!
//! One task owns the [`Driver`] and takes what happens in order, from one channel (entry `one-loop-decides`): what
//! came of a round or a call, an act of the owner, an item to add, a read for the pages. It reads the clock and gives
//! the time to the driver; nothing else reads it. After each thing it hands each [`Todo`] to what carries it out, and
//! tells whoever listens which works changed. It holds nothing of a work between two things that happen (entry
//! `engine-holds-no-state`): what a carrier is given is read from the driver as it is handed out, and what was to be
//! done is given again by the driver when the process starts again (entry `a-stop-changes-nothing`).
//!
//! What carries a todo out is reached through interfaces (principle `outside-behind-interfaces`): [`Rounds`] for the
//! rounds of the cells an agent carries out and for those that are the engine's own acts, and [`Tools`] for the calls
//! a tool answers. A call for a person, and a run at a limit, are carried out by nobody: they wait for the owner's
//! answer, which comes as an act. A withdrawn call of a person is answered by the loop itself.
//!
//! A carrier that cannot carry a round out says the round failed, and one that finds as a round ends that its work is
//! to wait for the owner, as when it has spent more than it may, says so with the end: either way the engine pauses the
//! work with the reason, which is in its records and on the page. Nothing of it is held here: after a restart a work so
//! paused is handed out nothing but the rounds of a run that answers a note, and the round that failed
//! is not handed out again until the owner lets the work on.
//!
//! - `carriers`: the interfaces, what each is given, and how it says what came of it.
//! - `clock`: the clock, read by the loop alone.
//! - [`script`]: what the test carriers do, as a TOML file says.
//! - `scripted`: the test carriers, which follow a script.
//! - `words`: the faults and the refusals in words.

mod carriers;
mod clock;
pub mod script;
mod scripted;
mod words;

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use decide::{BlockId, CallId, CellId, Outsider, Pauser, RunId};
use driver::{Driver, Failed, NewItem, Todo};
use engine::{Effect, Event};
use flow::{CellSpec, Compiled, Kind};
use store::{ItemNumber, Storage, WorkNumber};
use tokio::sync::{broadcast, mpsc, oneshot};
use work::{Ask, Call, Time};

pub use carriers::{
    Back, CallReply, Carriers, Returned, Round, RoundReply, Rounds, Told, ToolCall, Tools, Try, WITHDRAWN,
};
pub use clock::{Clock, SystemClock, TokioClock};
pub use script::Script;
pub use scripted::Scripted;
pub use words::{record_fault_words, refused_words, store_fault_words, unmet_words};

/// How many changes are kept for a listener that has not read them yet. One that falls further behind is told that
/// everything may have changed.
const CHANGES: usize = 256;

/// The loop: the driver, and what carries out what it gives to be done.
pub struct Service {
    driver: Driver,
    /// The flows the driver was given, by name, to be handed to the carriers.
    flows: BTreeMap<String, Arc<Compiled>>,
    carriers: Carriers,
    clock: Arc<dyn Clock>,
    /// The way into the loop's own channel, which the carriers' replies are given. Held by the loop, so that the
    /// channel never closes while the loop runs.
    to: mpsc::UnboundedSender<Message>,
    changed: broadcast::Sender<Changed>,
}

/// What the loop takes, one at a time, in the order it came.
pub enum Message {
    /// Something happened to a work: what came of a round or a call, said by what carried it out.
    Happened { work: WorkNumber, event: Event },
    /// The owner acts on a work.
    Act {
        work: WorkNumber,
        act: Act,
        reply: oneshot::Sender<Result<(), Refusal>>,
    },
    /// An item is added, with its work.
    Add {
        item: NewItem,
        reply: oneshot::Sender<Result<(ItemNumber, WorkNumber), Refusal>>,
    },
    /// The works are read, as the driver holds them now.
    Read(Box<dyn FnOnce(&Driver) + Send>),
}

/// What the owner does to a work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// Writes a block people write.
    Write {
        block: BlockId,
        content: String,
        value: Option<String>,
    },
    /// Lets the cell go on at its limit.
    LetGo {
        cell: CellId,
    },
    Pause,
    Resume,
    Cancel,
    /// Gives a call that waits for a person its result.
    Answer {
        call: CallId,
        result: String,
    },
}

/// Something happened: these works grew, and an item was added where `items`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    pub works: Vec<WorkNumber>,
    pub items: bool,
}

/// What stops the loop. Started again, the process goes on from the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The driver stopped.
    Driver(driver::Fault),
    /// The clock could not be read, in its own words.
    Clock(String),
    /// What was said to have come of something to be done, by its carrier or by the loop itself, cannot have
    /// happened to the work.
    Answer { work: WorkNumber, failed: Failed },
    /// The driver gave something to be done that names what its work does not hold.
    Stray { todo: Todo, missing: Missing },
}

/// What a todo names that its work does not hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    Work,
    Flow(String),
    Cell(CellId),
    Run(decide::RunId),
    /// What the round under way of the run was given.
    Given(decide::RunId),
    Call(CallId),
}

/// Why what was asked of the loop was not done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The driver did not do it.
    Failed(Failed),
    /// The call is a tool's: the tool gives its result, not a person.
    NotAPersonsCall(CallId),
    /// The loop has ended.
    Gone,
}

/// The loop's channel, from which it takes what happens.
pub struct Inbox(mpsc::UnboundedReceiver<Message>);

impl Inbox {
    /// The next thing to take, once it has come.
    pub async fn recv(&mut self) -> Option<Message> {
        self.0.recv().await
    }
}

/// The way to the loop for whoever is not the loop: the pages and the commands.
#[derive(Clone)]
pub struct Handle {
    to: mpsc::UnboundedSender<Message>,
    changed: broadcast::Sender<Changed>,
    /// Asked, off the loop, what they kept of a round.
    carriers: Carriers,
}

impl Service {
    /// Opens the driver on the store with these flows, hands out everything that is to be done, and gives the loop
    /// with its channel.
    pub fn open(
        store: Box<dyn Storage + Send>,
        flows: Vec<Compiled>,
        carriers: Carriers,
        clock: Arc<dyn Clock>,
    ) -> Result<(Service, Inbox), Fault> {
        let now = clock.now().map_err(Fault::Clock)?;
        let by_name = (flows.iter())
            .map(|flow| (flow.name.clone(), Arc::new(flow.clone())))
            .collect();
        let (driver, todos) = Driver::open(store, flows, now).map_err(Fault::Driver)?;
        let (to, inbox) = mpsc::unbounded_channel();
        let (changed, _) = broadcast::channel(CHANGES);
        let mut service = Service {
            driver,
            flows: by_name,
            carriers,
            clock,
            to,
            changed,
        };
        service.hand(now, todos)?;
        Ok((service, Inbox(inbox)))
    }

    /// The way to the loop.
    pub fn handle(&self) -> Handle {
        Handle {
            to: self.to.clone(),
            changed: self.changed.clone(),
            carriers: self.carriers.clone(),
        }
    }

    /// The driver, as it is between two things the loop takes.
    pub fn driver(&self) -> &Driver {
        &self.driver
    }

    /// Takes what comes, in order, until something stops the loop.
    pub async fn run(mut self, mut inbox: Inbox) -> Fault {
        while let Some(message) = inbox.recv().await {
            if let Err(fault) = self.take(message) {
                return fault;
            }
        }
        unreachable!("the loop holds a way into its own channel, which therefore never closes")
    }

    /// Takes one thing: gives it to the driver, hands out what is to be done, and tells whoever listens what changed.
    pub fn take(&mut self, message: Message) -> Result<(), Fault> {
        let before = self.sizes();
        let mut items = false;
        match message {
            Message::Read(read) => {
                read(&self.driver);
                return Ok(());
            }
            Message::Happened { work, event } => {
                let now = self.now()?;
                let todos = self
                    .driver
                    .happened(now, work, event)
                    .map_err(|failed| answer(work, failed))?;
                self.hand(now, todos)?;
            }
            Message::Act { work, act, reply } => {
                let now = self.now()?;
                match self.act(now, work, act) {
                    Ok(todos) => {
                        let _ = reply.send(Ok(()));
                        self.hand(now, todos)?;
                    }
                    Err(refusal) => {
                        let stopped = stopped(&refusal);
                        let _ = reply.send(Err(refusal));
                        if let Some(fault) = stopped {
                            return Err(fault);
                        }
                    }
                }
            }
            Message::Add { item, reply } => {
                let now = self.now()?;
                match self.driver.add(now, item) {
                    Ok((item, work, todos)) => {
                        items = true;
                        let _ = reply.send(Ok((item, work)));
                        // The work is added paused, so nothing is to be done yet; whatever the driver gives is handed
                        // out all the same.
                        self.hand(now, todos)?;
                    }
                    Err(failed) => {
                        let refusal = Refusal::Failed(failed);
                        let stopped = stopped(&refusal);
                        let _ = reply.send(Err(refusal));
                        if let Some(fault) = stopped {
                            return Err(fault);
                        }
                    }
                }
            }
        }
        self.tell(&before, items);
        Ok(())
    }

    fn now(&self) -> Result<Time, Fault> {
        self.clock.now().map_err(Fault::Clock)
    }

    /// The owner's act as what happened to the work, given to the driver.
    fn act(&mut self, now: Time, work: WorkNumber, act: Act) -> Result<Vec<Todo>, Refusal> {
        let event = match act {
            Act::Write { block, content, value } => Event::Wrote {
                block,
                by: Outsider::Person,
                content,
                matters: true,
                value,
            },
            Act::LetGo { cell } => Event::LetGo { cell },
            Act::Pause => Event::Paused {
                by: Pauser::Owner,
                why: None,
            },
            Act::Resume => Event::Resumed,
            Act::Cancel => Event::Cancelled,
            Act::Answer { call, result } => {
                // A call the engine makes has its result from what makes it: a person's answer to it would be a
                // second one.
                let by_engine = (self.driver.work(work)).is_some_and(|kept| {
                    kept.work
                        .records
                        .called
                        .iter()
                        .any(|c| c.call == call && c.on.is_none())
                });
                if by_engine {
                    return Err(Refusal::NotAPersonsCall(call));
                }
                Event::Done { call, result }
            }
        };
        self.driver.happened(now, work, event).map_err(Refusal::Failed)
    }

    /// Hands out what is to be done, and gives the driver what the loop answers itself, until nothing is left.
    fn hand(&mut self, now: Time, todos: Vec<Todo>) -> Result<(), Fault> {
        let mut queue = VecDeque::from(todos);
        while let Some(todo) = queue.pop_front() {
            if let Some(event) = self.give(&todo)? {
                let more = (self.driver.happened(now, todo.work, event)).map_err(|failed| answer(todo.work, failed))?;
                queue.extend(more);
            }
        }
        Ok(())
    }

    /// Gives one todo to what carries it out, with what it needs read from the driver now; or gives what the loop
    /// answers to it itself.
    fn give(&self, todo: &Todo) -> Result<Option<Event>, Fault> {
        let stray = |missing| Fault::Stray {
            todo: todo.clone(),
            missing,
        };
        let work = todo.work;
        let kept = self.driver.work(work).ok_or_else(|| stray(Missing::Work))?;
        let flow =
            (self.flows.get(&kept.head.flow).cloned()).ok_or_else(|| stray(Missing::Flow(kept.head.flow.clone())))?;
        let records = &kept.work.records;
        let round = |run, cell: &CellId, number, given| -> Result<(Round, Kind), Fault> {
            let spec = cell_spec(&flow, cell).ok_or_else(|| stray(Missing::Cell(cell.clone())))?;
            let round = Round {
                work,
                flow: flow.clone(),
                cell: spec.clone(),
                run,
                number,
                given,
                kept: Arc::new(kept.work.clone()),
            };
            Ok((round, spec.kind))
        };
        match &todo.effect {
            Effect::Round {
                run,
                cell,
                number,
                given,
            } => {
                let (round, kind) = round(*run, cell, *number, given.clone())?;
                let reply = RoundReply::new(work, *run, self.to.clone());
                self.rounds(kind).give(round, reply);
            }
            Effect::Stop { run } => {
                let started =
                    (records.started.iter().find(|s| s.run == *run)).ok_or_else(|| stray(Missing::Run(*run)))?;
                let number = records.rounds.iter().filter(|r| r.run == *run).count();
                let given = (kept.work.given.get(&(*run, number))).ok_or_else(|| stray(Missing::Given(*run)))?;
                let (round, kind) = round(*run, &started.cell, number, given.clone())?;
                let reply = RoundReply::new(work, *run, self.to.clone());
                self.rounds(kind).stop(round, reply);
            }
            Effect::Call {
                call,
                run,
                what: Call::Asked(Ask::Tool { name, input }),
            } => {
                let order = ToolCall {
                    work,
                    flow,
                    run: *run,
                    call: *call,
                    name: name.clone(),
                    input: input.clone(),
                    kept: Arc::new(kept.work.clone()),
                };
                self.carriers
                    .tools
                    .call(order, CallReply::new(work, *call, self.to.clone()));
            }
            // A person's call, and a run at its limit, wait for the owner's answer.
            Effect::Call { .. } => {}
            Effect::Withdraw { call } => match kept.work.calls.get(call) {
                Some(Call::Asked(Ask::Tool { name, input })) => {
                    let called =
                        (records.called.iter().find(|c| c.call == *call)).ok_or_else(|| stray(Missing::Call(*call)))?;
                    let order = ToolCall {
                        work,
                        flow,
                        run: called.run,
                        call: *call,
                        name: name.clone(),
                        input: input.clone(),
                        kept: Arc::new(kept.work.clone()),
                    };
                    self.carriers
                        .tools
                        .withdraw(order, CallReply::new(work, *call, self.to.clone()));
                }
                Some(Call::Asked(Ask::Person { .. }) | Call::AtLimit(_)) => {
                    return Ok(Some(Event::Done {
                        call: *call,
                        result: WITHDRAWN.to_string(),
                    }));
                }
                None => return Err(stray(Missing::Call(*call))),
            },
        }
        Ok(None)
    }

    /// What carries out the rounds of cells of this kind.
    fn rounds(&self, kind: Kind) -> &dyn Rounds {
        &**self.carriers.rounds(kind)
    }

    /// How many things that happened each work has taken.
    fn sizes(&self) -> BTreeMap<WorkNumber, usize> {
        (self.driver.works())
            .map(|kept| (kept.head.number, kept.work.moments.len()))
            .collect()
    }

    /// Tells whoever listens which works grew since `before`, and whether an item was added.
    fn tell(&self, before: &BTreeMap<WorkNumber, usize>, items: bool) {
        let works: Vec<WorkNumber> = (self.sizes().into_iter())
            .filter(|(work, size)| before.get(work) != Some(size))
            .map(|(work, _)| work)
            .collect();
        if !works.is_empty() || items {
            // Nobody may be listening.
            let _ = self.changed.send(Changed { works, items });
        }
    }
}

impl Handle {
    /// Reads the works as the driver holds them, between two things the loop takes.
    pub async fn read<T: Send + 'static>(
        &self,
        read: impl FnOnce(&Driver) -> T + Send + 'static,
    ) -> Result<T, Refusal> {
        let (reply, answer) = oneshot::channel();
        let read = Box::new(move |driver: &Driver| {
            let _ = reply.send(read(driver));
        });
        self.to.send(Message::Read(read)).map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)
    }

    /// Adds an item with its work, which is added paused, and gives their numbers.
    pub async fn add(&self, item: NewItem) -> Result<(ItemNumber, WorkNumber), Refusal> {
        let (reply, answer) = oneshot::channel();
        self.to.send(Message::Add { item, reply }).map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Does what the owner does to a work.
    pub async fn act(&self, work: WorkNumber, act: Act) -> Result<(), Refusal> {
        let (reply, answer) = oneshot::channel();
        self.to
            .send(Message::Act { work, act, reply })
            .map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Each change from now on.
    pub fn changes(&self) -> broadcast::Receiver<Changed> {
        self.changed.subscribe()
    }

    /// What the round of this number of the run was told and what came back of it, as the carrier of its cell's kind
    /// kept them, or, for a cell the flow no longer has, as the first carrier that kept anything of it: none where
    /// there is no such work, and inside, none where the work has no such round, or why it could not be read. Which
    /// carrier it is is read between two things the loop takes, and what it kept on a thread of its own, so that the
    /// loop goes on taking what happens while the disk is read.
    pub async fn told(
        &self,
        work: WorkNumber,
        run: RunId,
        round: usize,
    ) -> Result<Option<Result<Option<Told>, String>>, Refusal> {
        let kind = self
            .read(move |driver| {
                let kept = driver.work(work)?;
                if !kept.work.given.contains_key(&(run, round)) {
                    return Some(None);
                }
                let started = kept.work.records.started.iter().find(|s| s.run == run);
                Some(Some(
                    started
                        .and_then(|s| cell_spec(kept.flow, &s.cell))
                        .map(|spec| spec.kind),
                ))
            })
            .await?;
        let kind = match kind {
            None => return Ok(None),
            Some(None) => return Ok(Some(Ok(None))),
            Some(Some(kind)) => kind,
        };
        let carriers = self.carriers.clone();
        let told = tokio::task::spawn_blocking(move || carriers.told(kind, work, run, round)).await;
        Ok(Some(match told {
            Ok(told) => told.map(Some),
            Err(e) => Err(format!("what was kept of it could not be read: {e}")),
        }))
    }
}

/// The specification of the cell in the flow.
fn cell_spec<'a>(flow: &'a Compiled, cell: &CellId) -> Option<&'a CellSpec> {
    flow.cells.iter().find(|spec| spec.id == *cell)
}

/// The fault of an answer the driver did not take. This ends the process, and is meant to: an answer that cannot have
/// happened to the work comes from a carrier that answered twice or for nothing, which has broken its contract
/// (`Rounds`, `Tools`), and the process stops rather than go on beside it. Started again, it hands out anew what is to
/// be done.
fn answer(work: WorkNumber, failed: Failed) -> Fault {
    match failed {
        Failed::Stopped { fault, .. } => Fault::Driver(fault),
        failed => Fault::Answer { work, failed },
    }
}

/// The fault that stopped the driver, where the refusal says it stopped.
fn stopped(refusal: &Refusal) -> Option<Fault> {
    match refusal {
        Refusal::Failed(Failed::Stopped { fault, .. }) => Some(Fault::Driver(fault.clone())),
        _ => None,
    }
}
