//! The engine as a process: one loop that drives every work, the interfaces of what carries out what is to be done,
//! and the test implementations of those, which follow a script.
//!
//! One task owns the [`Driver`] and takes what happens in order, from one channel: what
//! came of a round or a call, an act of the owner, an item to add, a read for the pages. It reads the clock and gives
//! the time to the driver; nothing else reads it. After each thing it hands each [`Todo`] to what carries it out, and
//! tells whoever listens which works changed. It holds nothing of a work between two things that happen (principle
//! `state-from-records`): what a carrier is given is read from the driver as it is handed out, and what was to be
//! done is given again by the driver when the process starts again (property `a-stop-changes-nothing`).
//!
//! What carries a todo out is reached through interfaces (principle `outside-behind-interfaces`): [`Rounds`] for the
//! rounds of the cells an agent carries out and for those that are the engine's own acts, and [`Tools`] for the calls
//! a tool answers. A call for a person, and a run at a limit, are carried out by nobody: they wait for the owner's
//! answer, which comes as an act. A withdrawn call of a person is answered by the loop itself. What a work that ended
//! holds outside the engine is let go by the carrier of the engine's acts, which made it ([`Rounds::release`]).
//!
//! A carrier that cannot carry a round out says the round failed, with the kind of its cause where it tells one apart,
//! and one that finds as a round ends that its work is to wait for the owner, as when it has spent more than it may,
//! says so with the end: the engine handles the failure as its kind says, and pauses the work with the reason where
//! that is the handling, which is in its records and on the page. Nothing of it is held here: after a restart a work so
//! paused is handed out nothing but the rounds of a run that answers a note, and the round that failed is not handed
//! out again until the owner lets the work on.
//!
//! The loop is also what tells the time to what waits for it. A wait of a run before a round that failed in passing is
//! tried again is a call the engine makes: the loop gives it its result once its time has come, and answers it as
//! withdrawn itself where its run is abandoned first. A wait every work shares the driver says when to look at
//! ([`Driver::wake`]): the loop tells the driver the time then, checking the outside of the agents' rounds first where
//! the driver asks it to ([`Outside`]), and looking at the network to the acts' host ([`Network`]). Neither is held but
//! as a timer, which is set again from the records when the process starts again.
//!
//! The owner's pause of everything comes to the loop as an act of its own, which the driver keeps; so do the accounts
//! the agents' program may run under, added, removed and taken by the owner. An account is added once its token has
//! been checked by the check of the outside and kept by what keeps the tokens ([`Tokens`]), and its token let go of
//! once it is removed; a round, a ping and a check of the outside are each given the account in use as they are handed
//! out, none where none is, which no agent's call runs under (decision no-default-credential). What the agents' service
//! says of an account's usage, a carrier says under the account its call was handed out with, and the driver keeps it;
//! as the owner opens the accounts, a look is taken under each whose usage was last heard of long enough ago, with the
//! check of the outside, off the loop (crate `driver`, module `usage`). How many rounds of agents go on at once is what
//! the carriers say as the loop opens ([`Carriers::slots`]), and then what the owner last set ([`Handle::set_slots`]),
//! which the driver takes at once; it gives no more. Whether a pause holds a work back the carriers are told as the
//! loop opens and whenever it may have changed ([`Rounds::paused`]), for what a carrier holds back of a round for
//! itself.
//!
//! - `carriers`: the interfaces, what each is given, and how it says what came of it.
//! - `clock`: the clock, read by the loop alone.
//! - `levels`: the levels of the items, which choose what their agents' rounds run on.
//! - [`script`]: what the test carriers do, as a TOML file says.
//! - `scripted`: the test carriers, which follow a script.
//! - `words`: the faults and the refusals in words.

mod carriers;
mod clock;
mod levels;
mod projects;
pub mod script;
mod scripted;
mod titles;
mod words;

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use decide::{BlockId, CallId, CellId, Outsider, Pauser, RunId};
use driver::{Asked, Driver, Failed, Found, NewItem, Todo, Wake};
use engine::{Effect, Event};
use flow::{CellSpec, Compiled, Kind};
use store::{AccountAct, ItemNumber, Storage, WorkNumber};
use tokio::sync::{broadcast, mpsc, oneshot};
use work::{Ask, Call, RanOn, Time};

pub use carriers::{
    Back, CallReply, Carriers, Changes, Diff, Diffstat, Doing, Doings, FileChanged, Fork, Forked, Heard, Hosting,
    KeptTokens, Mark, Network, OfWork, Outside, Ping, PingReply, Pull, Release, ReleaseReply, Returned, Round,
    RoundReply, Rounds, Saying, Seen, Teller, Telling, Token, Tokens, Told, ToolCall, Tools, Try, WITHDRAWN, YEAR_MS,
};
pub use clock::{Clock, SystemClock, TokioClock};
pub use driver::{Along, Instead, Said, Under, Window};
pub use levels::{Level, Levels, LevelsFault};
pub use projects::Projects;
pub use script::Script;
pub use scripted::Scripted;
pub use titles::{title_fits, title_words};
pub use words::{no_one_words, record_fault_words, refused_words, store_fault_words};

/// The result of a wait the engine made before a round that failed in passing is tried again, once its time has come.
pub const WAITED: &str = "The wait is over: the round is tried again.";

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
    /// When the driver is to be told the time next, with the timer that will tell the loop so.
    wake: Option<(Wake, tokio::task::AbortHandle)>,
    /// The works a keep-warm ping of which is under way: how many are, for each line of the work, and what is to be
    /// done for the work that waits for them, so that nothing runs beside a ping that it must not: no round of the
    /// ping's line, whose session the ping's program is in, and no letting go of what the work holds, as its
    /// worktree, under the program the ping runs. Lost in a stop, when the driver gives what is to be done again.
    pinging: BTreeMap<WorkNumber, Pinging>,
    /// The lock every handle takes to add an account.
    adding: Arc<tokio::sync::Mutex<()>>,
    /// The usage limit of each account that stands, as last told to whoever listens: told again once it changed.
    limits: BTreeMap<String, driver::Limit>,
}

/// A read of the works as the driver holds them, at the time the loop's clock gives.
pub type Reader = Box<dyn FnOnce(&Driver, Time) + Send>;

/// What the loop takes, one at a time, in the order it came.
pub enum Message {
    /// Something happened to a work: what came of a round or a call, said by what carried it out.
    Happened { work: WorkNumber, event: Event },
    /// The owner acts on a work.
    Act {
        work: WorkNumber,
        act: Act,
        reply: oneshot::Sender<Result<Acted, Refusal>>,
    },
    /// The owner sets the pause of everything, or lifts it.
    Everything {
        paused: bool,
        reply: oneshot::Sender<Result<(), Refusal>>,
    },
    /// The owner has the sessions of works that wait kept warm, or has them kept no longer.
    KeepWarm {
        warm: bool,
        reply: oneshot::Sender<Result<(), Refusal>>,
    },
    /// The owner sets how many rounds of agents may go on at once.
    Slots {
        slots: usize,
        reply: oneshot::Sender<Result<(), Refusal>>,
    },
    /// The owner adds, removes or takes the account of this name; one added has its token kept already, and one
    /// removed has its token let go of once this is kept.
    Account {
        name: String,
        what: AccountAct,
        reply: oneshot::Sender<Result<(), Refusal>>,
    },
    /// An item is added, with its work.
    Add {
        item: NewItem,
        reply: oneshot::Sender<Result<(ItemNumber, WorkNumber), Refusal>>,
    },
    /// Whoever plans the work changes items, for one reason, all of the changes or none: a level is one whoever asks
    /// has checked; the reply says whether each changed its item.
    Change {
        why: String,
        changes: Vec<(ItemNumber, Asked)>,
        reply: oneshot::Sender<Result<Vec<bool>, Refusal>>,
    },
    /// A source outside the works says this now of the project of this name: the head of the project's main branch,
    /// for one.
    Outside {
        project: String,
        source: String,
        content: String,
    },
    /// The works are read, as the driver holds them now.
    Read(Reader),
    /// The time of the engine's wait, this call of this work, has come.
    Due { work: WorkNumber, call: CallId },
    /// The time the driver asked to be told has come; where it asked for a check of the outside of the agents' rounds,
    /// the check heard this, under the account of this name, the one in use as it was taken, and where it asked for a
    /// look at the network to the acts' host, the look found it reached or not.
    Tick {
        wake: Wake,
        heard: Option<Heard>,
        under: Option<String>,
        host_reached: Option<bool>,
    },
    /// The agents' service said this of the usage of the account of this name, under which a call it answered was
    /// handed out.
    Usage { account: String, said: Said },
    /// The owner opens the accounts: a look is taken under each whose usage was last heard of long enough ago, and the
    /// reply names them.
    Look {
        reply: oneshot::Sender<Result<Vec<String>, Refusal>>,
    },
    /// A look under the account of this name heard this.
    Looked { account: String, heard: Heard },
    /// The service of the agents answered the round under way of this run, with a message that is no error.
    Answered { work: WorkNumber, run: RunId },
    /// What carries out a round or a call of this work says where it stands now, or that it no longer says so.
    Along {
        work: WorkNumber,
        under: Under,
        along: Option<Along>,
    },
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
    /// Cancels the work's item, for this reason where one is given.
    Cancel {
        why: Option<String>,
    },
    /// Gives a call that waits for a person its result.
    Result {
        call: CallId,
        result: String,
    },
    /// Says these words to a cell of the work's flow, or to a line of it: to the cell the engine chooses.
    Say {
        to: engine::spoken::To,
        words: String,
    },
    /// Answers this version of a question, by its number among the versions of its cell's questions, taking this
    /// option, counted from one, and nothing else.
    Answer {
        question: String,
        version: u64,
        option: u64,
    },
}

/// What came of an act the loop took: of one that says the owner's words, the cell the engine gave them to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Acted {
    pub reached: Option<CellId>,
}

/// Something happened: these works grew, an item was added where `items`, the copies of the database changed where
/// `copies`, and the accounts, their usage or their usage limits changed where `accounts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    pub works: Vec<WorkNumber>,
    pub items: bool,
    pub copies: bool,
    pub accounts: bool,
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
    /// The token of an account to be added was not kept, for this reason, in words: no account was added.
    NotKept(String),
    /// No token can be kept or given, for this reason, as what keeps the tokens says ([`Tokens::closed`]): no account
    /// is added or taken.
    Closed(String),
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
    /// One account added at a time: a token is kept under a name only once no account of that name is found added.
    adding: Arc<tokio::sync::Mutex<()>>,
}

/// An account added: where its token met its usage limit as it was checked, when the limit resets, where that was
/// said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAdded {
    pub at_limit: bool,
    pub resets: Option<Time>,
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
        let policy = carriers.agents.policy();
        let (driver, todos) = Driver::open(store, flows, carriers.slots, policy, now).map_err(Fault::Driver)?;
        // The tokens of the accounts removed are let go of until they are, a removal before a stop among them.
        if let Some(failed) = let_go_of(&*carriers.tokens, &driver.accounts().removed()) {
            eprintln!("The tokens of accounts removed could not be let go of, and are tried again: {failed}");
        }
        let (to, inbox) = mpsc::unbounded_channel();
        let (changed, _) = broadcast::channel(CHANGES);
        let limits = driver.limits();
        let mut service = Service {
            driver,
            flows: by_name,
            carriers,
            clock,
            to,
            changed,
            wake: None,
            pinging: BTreeMap::new(),
            adding: Arc::default(),
            limits,
        };
        let works: Vec<WorkNumber> = service.driver.works().map(|kept| kept.head.number).collect();
        service.tell_paused(works);
        service.hand(now, todos)?;
        service.set_wake();
        Ok((service, Inbox(inbox)))
    }

    /// The way to the loop.
    pub fn handle(&self) -> Handle {
        Handle {
            to: self.to.clone(),
            changed: self.changed.clone(),
            carriers: self.carriers.clone(),
            adding: self.adding.clone(),
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
        let mut accounts = false;
        // Whether the rounds the service answered changed, which the usage limits that stand are worked out from.
        let mut answered = false;
        // Whether the engine's own state changed, which no record of a work holds.
        let mut engine = false;
        match message {
            Message::Read(read) => {
                let now = self.now()?;
                read(&self.driver, now);
                return Ok(());
            }
            Message::Happened { work, event } => {
                let now = self.now()?;
                let pinged = match &event {
                    Event::Pinged { line, .. } => Some(line.clone()),
                    _ => None,
                };
                let mut todos = self
                    .driver
                    .happened(now, work, event)
                    .map_err(|failed| answer(work, failed))?;
                // What was held back for the work's pings goes on once nothing holds it back.
                if let Some(line) = &pinged
                    && let Some(pinging) = self.pinging.get_mut(&work)
                {
                    todos.extend(pinging.ended(line));
                    if pinging.lines.is_empty() {
                        self.pinging.remove(&work);
                    }
                }
                self.hand(now, todos)?;
            }
            Message::Outside {
                project,
                source,
                content,
            } => {
                let now = self.now()?;
                let todos = (self.driver.outside(now, &project, &source, &content)).map_err(Fault::Driver)?;
                self.hand(now, todos)?;
            }
            Message::Due { work, call } => {
                // A wait withdrawn with its run has its result already, and the timer that went on is told nothing.
                let open = (self.driver.work(work)).is_some_and(|kept| {
                    let records = &kept.work.records;
                    records.called.iter().any(|c| c.call == call) && !records.done.iter().any(|d| d.call == call)
                });
                if open {
                    let now = self.now()?;
                    let event = Event::Done {
                        call,
                        result: WAITED.to_string(),
                    };
                    let todos = (self.driver.happened(now, work, event)).map_err(|failed| answer(work, failed))?;
                    self.hand(now, todos)?;
                }
            }
            Message::Tick {
                wake,
                heard,
                under,
                host_reached,
            } => {
                if self.wake.as_ref().is_some_and(|(set, _)| *set == wake) {
                    self.wake = None;
                }
                let now = self.now()?;
                if let (Some(account), Some(said)) = (&under, heard.as_ref().and_then(|heard| heard.usage.clone())) {
                    accounts = self.driver.heard_usage(now, account, said);
                }
                let found = heard.map(|heard| (under, heard.found));
                let todos = (self.driver.tick(now, found, host_reached)).map_err(Fault::Driver)?;
                self.hand(now, todos)?;
            }
            Message::Answered { work, run } => {
                answered = true;
                let now = self.now()?;
                let todos = self.driver.answered(now, work, run).map_err(Fault::Driver)?;
                self.hand(now, todos)?;
            }
            Message::Usage { account, said } => {
                // No record grew, and nothing that decides reads it: only the accounts are told again.
                let now = self.now()?;
                if self.driver.heard_usage(now, &account, said) {
                    self.tell_accounts();
                }
                return Ok(());
            }
            Message::Look { reply } => {
                let now = self.now()?;
                let looks = self.driver.looks(now);
                for account in &looks {
                    let (to, outside, account) = (self.to.clone(), self.carriers.outside.clone(), account.clone());
                    tokio::spawn(async move {
                        let heard = outside.check(Some(account.clone())).await;
                        let _ = to.send(Message::Looked { account, heard });
                    });
                }
                if !looks.is_empty() {
                    self.tell_accounts();
                }
                let _ = reply.send(Ok(looks));
                return Ok(());
            }
            Message::Looked { account, heard } => {
                let now = self.now()?;
                if self.driver.looked(now, &account, &heard.found, heard.usage) {
                    self.tell_accounts();
                }
                return Ok(());
            }
            Message::Along { work, under, along } => {
                // What a work does now may have changed, though no record of it grew.
                if self.driver.along(work, under, along) {
                    let _ = self.changed.send(Changed {
                        works: vec![work],
                        items: false,
                        copies: false,
                        accounts: false,
                    });
                }
                return Ok(());
            }
            Message::Act { work, act, reply } => {
                let now = self.now()?;
                match self.act(now, work, act) {
                    Ok((todos, acted)) => {
                        let _ = reply.send(Ok(acted));
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
            Message::Everything { paused, reply } => {
                let now = self.now()?;
                match self.driver.pause_everything(now, paused) {
                    Ok(todos) => {
                        let _ = reply.send(Ok(()));
                        self.hand(now, todos)?;
                        // What every work does now may have changed, though no record of one grew.
                        let works: Vec<WorkNumber> = self.driver.works().map(|kept| kept.head.number).collect();
                        self.tell_paused(works.clone());
                        let _ = self.changed.send(Changed {
                            works,
                            items: false,
                            copies: false,
                            accounts: false,
                        });
                        return Ok(());
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
            Message::KeepWarm { warm, reply } => {
                let now = self.now()?;
                match self.driver.keep_warm(now, warm) {
                    Ok(()) => {
                        let _ = reply.send(Ok(()));
                        // No record of a work grew: the notice says only that something happened, on which the pages
                        // read the engine again. A ping that has come due is given at the wake the driver asks for.
                        let _ = self.changed.send(Changed {
                            works: Vec::new(),
                            items: false,
                            copies: false,
                            accounts: false,
                        });
                        return Ok(());
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
            Message::Slots { slots, reply } => {
                let now = self.now()?;
                match self.driver.set_slots(now, slots) {
                    Ok(todos) => {
                        let _ = reply.send(Ok(()));
                        self.hand(now, todos)?;
                        engine = true;
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
            Message::Account { name, what, reply } => {
                let now = self.now()?;
                let done = match what {
                    AccountAct::Added => self.driver.add_account(now, &name).map(|()| Vec::new()),
                    AccountAct::Removed => self.driver.remove_account(now, &name).map(|()| Vec::new()),
                    AccountAct::Taken => self.driver.take_account(now, &name),
                };
                match done {
                    Ok(todos) => {
                        let _ = reply.send(Ok(()));
                        self.hand(now, todos)?;
                        // What every work does now may have changed, though no record of one grew; and the accounts
                        // did.
                        let works: Vec<WorkNumber> = self.driver.works().map(|kept| kept.head.number).collect();
                        let _ = self.changed.send(Changed {
                            works,
                            items: false,
                            copies: false,
                            accounts: true,
                        });
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
            Message::Add { item, reply } => {
                let now = self.now()?;
                match self.driver.add(now, item) {
                    Ok((item, work, todos)) => {
                        items = true;
                        let _ = reply.send(Ok((item, work)));
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
            Message::Change { why, changes, reply } => {
                let now = self.now()?;
                match self.driver.change(now, Some(&why), changes) {
                    Ok((changed, todos)) => {
                        items = changed.iter().any(|changed| *changed);
                        let _ = reply.send(Ok(changed));
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
        self.set_wake();
        self.tell(&before, items, accounts, answered, engine);
        Ok(())
    }

    /// Sets the timer that tells the loop the time the driver asks to be told next, where that changed. The one set
    /// before is stopped only where its time has not come: one whose time has come may be checking the outside, or
    /// about to tell the loop, and tells it, which costs one look at the works more. Where the driver asks for a check
    /// of the outside or a look at the network, it is taken off the loop as the time comes.
    fn set_wake(&mut self) {
        let wake = self.driver.wake();
        if self.wake.as_ref().map(|(w, _)| *w) == wake {
            return;
        }
        let now = self.clock.now().ok();
        if let Some((set, timer)) = self.wake.take()
            && now.is_some_and(|now| set.at > now)
        {
            timer.abort();
        }
        let Some(wake) = wake else {
            return;
        };
        let after = self.until(wake.at);
        let (to, outside, host) = (
            self.to.clone(),
            self.carriers.outside.clone(),
            self.carriers.host.clone(),
        );
        // The outside is checked under the account in use as the time is set: taking another sets it anew.
        let under = self.driver.accounts().in_use().map(str::to_string);
        let timer = tokio::spawn(async move {
            tokio::time::sleep(after).await;
            let heard = match wake.check {
                true => Some(outside.check(under.clone()).await),
                false => None,
            };
            // A host no network reaches is reached.
            let host_reached = match (wake.look_at_host, host) {
                (true, Some(host)) => tokio::task::spawn_blocking(move || host.reached()).await.ok(),
                (true, None) => Some(true),
                (false, _) => None,
            };
            let _ = to.send(Message::Tick {
                wake,
                heard,
                under,
                host_reached,
            });
        });
        self.wake = Some((wake, timer.abort_handle()));
    }

    /// How long from now until this time; nothing where it has come, or the clock cannot be read, when the time is
    /// taken to have come.
    fn until(&self, at: Time) -> std::time::Duration {
        let now = self.clock.now().unwrap_or(at);
        std::time::Duration::from_millis(at.0.saturating_sub(now.0))
    }

    fn now(&self) -> Result<Time, Fault> {
        self.clock.now().map_err(Fault::Clock)
    }

    /// The owner's act as what happened to the work, given to the driver, with what came of it: of words said, the cell
    /// the engine chose for them, as it chooses it on the work as it stands before it takes them.
    fn act(&mut self, now: Time, work: WorkNumber, act: Act) -> Result<(Vec<Todo>, Acted), Refusal> {
        let mut acted = Acted::default();
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
            Act::Cancel { why } => {
                // A cancel is a change of the item, judged as every change is.
                let item = (self.driver.work(work).map(|kept| kept.item.number))
                    .ok_or(Refusal::Failed(Failed::NoSuchWork(work)))?;
                let asked = vec![(item, Asked::Cancel)];
                let (_, todos) = (self.driver.change(now, why.as_deref(), asked)).map_err(Refusal::Failed)?;
                return Ok((todos, acted));
            }
            Act::Say { to, words } => {
                let kept = (self.driver.work(work)).ok_or(Refusal::Failed(Failed::NoSuchWork(work)))?;
                acted.reached = engine::spoken::receiver(kept.flow, kept.work, &to).ok();
                Event::Said { to, words }
            }
            Act::Answer {
                question,
                version,
                option,
            } => Event::Answer {
                question,
                version,
                option,
            },
            Act::Result { call, result } => {
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
        let todos = self.driver.happened(now, work, event).map_err(Refusal::Failed)?;
        Ok((todos, acted))
    }

    /// Hands out what is to be done, and gives the driver what the loop answers itself, until nothing is left.
    fn hand(&mut self, now: Time, todos: Vec<Todo>) -> Result<(), Fault> {
        let mut queue = VecDeque::from(todos);
        while let Some(todo) = queue.pop_front() {
            // A ping runs beside no round of its line and no letting go of what its work holds: these wait for the
            // pings that hold them to end, and a round stopped before it was given is not given. Everything else
            // is done beside it.
            match &todo.effect {
                Effect::Ping { line, .. } => self.pinging.entry(todo.work).or_default().begins(line),
                Effect::Stop { run } => {
                    if let Some(pinging) = self.pinging.get_mut(&todo.work) {
                        pinging
                            .held
                            .retain(|(_, t)| !matches!(t.effect, Effect::Round { run: r, .. } if r == *run));
                    }
                }
                Effect::Round { .. } | Effect::Release { .. } => {
                    let line = self.line_of(&todo);
                    if let Some(pinging) = self.pinging.get_mut(&todo.work)
                        && pinging.holds(line.as_deref(), &todo.effect)
                    {
                        pinging.held.push((line, todo));
                        continue;
                    }
                }
                _ => {}
            }
            if let Some(event) = self.give(&todo)? {
                let more = (self.driver.happened(now, todo.work, event)).map_err(|failed| answer(todo.work, failed))?;
                queue.extend(more);
            }
        }
        Ok(())
    }

    /// The line whose session the round of the todo runs in, as the version of the flow the work is on places its cell;
    /// none of any other todo, and of a round of an engine's act or of a cell that version has not.
    fn line_of(&self, todo: &Todo) -> Option<String> {
        let Effect::Round { cell, .. } = &todo.effect else {
            return None;
        };
        let kept = self.driver.work(todo.work)?;
        let flow = self.flows.get(&kept.head.flow)?;
        cell_spec(flow, cell)?.line.clone()
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
            let lines = self.driver.lines(work, spec);
            let round = Round {
                work,
                flow: flow.clone(),
                cell: spec.clone(),
                run,
                number,
                given,
                item: kept.item.clone(),
                kept: Arc::new(kept.work.clone()),
                lines,
                account: self.driver.accounts().in_use().map(str::to_string),
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
            Effect::Ping { line, session } => {
                let ping = Ping {
                    work,
                    flow,
                    line: line.clone(),
                    session: *session,
                    item: kept.item.clone(),
                    kept: Arc::new(kept.work.clone()),
                    account: self.driver.accounts().in_use().map(str::to_string),
                };
                let reply = PingReply::new(work, line.clone(), *session, self.to.clone());
                self.carriers.agents.ping(ping, reply);
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
            Effect::Tell { run, telling } => {
                let started =
                    (records.started.iter().find(|s| s.run == *run)).ok_or_else(|| stray(Missing::Run(*run)))?;
                let number = records.rounds.iter().filter(|r| r.run == *run).count();
                let given = (kept.work.given.get(&(*run, number))).ok_or_else(|| stray(Missing::Given(*run)))?;
                let (round, kind) = round(*run, &started.cell, number, given.clone())?;
                self.rounds(kind).tell(round, (**telling).clone());
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
                    item: kept.item.clone(),
                    kept: Arc::new(kept.work.clone()),
                };
                self.carriers
                    .tools
                    .call(order, CallReply::new(work, *call, self.to.clone()));
            }
            // The engine's wait has its result once its time has come.
            Effect::Call {
                call,
                what: Call::Wait { until, .. },
                ..
            } => {
                let (after, to, call) = (self.until(*until), self.to.clone(), *call);
                tokio::spawn(async move {
                    tokio::time::sleep(after).await;
                    let _ = to.send(Message::Due { work, call });
                });
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
                        item: kept.item.clone(),
                        kept: Arc::new(kept.work.clone()),
                    };
                    self.carriers
                        .tools
                        .withdraw(order, CallReply::new(work, *call, self.to.clone()));
                }
                Some(Call::Asked(Ask::Person { .. }) | Call::AtLimit(_) | Call::Wait { .. }) => {
                    return Ok(Some(Event::Done {
                        call: *call,
                        result: WITHDRAWN.to_string(),
                    }));
                }
                None => return Err(stray(Missing::Call(*call))),
            },
            // The engine's acts make the work's branch and worktree, and its pull request where the host has them.
            Effect::Release { end } => {
                let release = Release {
                    work,
                    end: *end,
                    flow,
                    item: kept.item.clone(),
                    kept: Arc::new(kept.work.clone()),
                };
                self.carriers
                    .acts
                    .release(release, ReleaseReply::new(work, self.to.clone()));
            }
        }
        Ok(None)
    }

    /// What carries out the rounds of cells of this kind.
    fn rounds(&self, kind: Kind) -> &dyn Rounds {
        &**self.carriers.rounds(kind)
    }

    /// Tells the carriers whether a pause holds each of these works back now, its own or the owner's of everything.
    fn tell_paused(&self, works: Vec<WorkNumber>) {
        let everything = self.driver.everything_paused().is_some();
        for work in works {
            let Some(kept) = self.driver.work(work) else {
                continue;
            };
            let paused = everything || decide::paused(&kept.work.records);
            self.carriers.agents.paused(work, paused);
            self.carriers.acts.paused(work, paused);
        }
    }

    /// How many things that happened each work has taken.
    fn sizes(&self) -> BTreeMap<WorkNumber, usize> {
        (self.driver.works())
            .map(|kept| (kept.head.number, kept.work.moments.len()))
            .collect()
    }

    /// Tells whoever listens which works grew since `before`, whether an item was added, and whether the accounts
    /// changed, as `accounts` says or as the usage limits that stand did; and the carriers whether a pause holds each
    /// work that grew. The limits are worked out from every work's rounds and the rounds the service answered, so they
    /// are worked out again only where a work grew or, as `answered` says, the service answered a round. Where `engine`
    /// says the engine's own state changed, whoever listens is told even where nothing else changed: the notice says
    /// that something happened, on which the pages read the engine again.
    fn tell(
        &mut self,
        before: &BTreeMap<WorkNumber, usize>,
        items: bool,
        accounts: bool,
        answered: bool,
        engine: bool,
    ) {
        let works: Vec<WorkNumber> = (self.sizes().into_iter())
            .filter(|(work, size)| before.get(work) != Some(size))
            .map(|(work, _)| work)
            .collect();
        self.tell_paused(works.clone());
        let mut accounts = accounts;
        if !works.is_empty() || answered {
            let limits = self.driver.limits();
            accounts |= limits != self.limits;
            self.limits = limits;
        }
        if !works.is_empty() || items || accounts || engine {
            // Nobody may be listening.
            let _ = self.changed.send(Changed {
                works,
                items,
                copies: false,
                accounts,
            });
        }
    }

    /// Tells whoever listens that the accounts or their usage changed, and nothing else.
    fn tell_accounts(&self) {
        // Nobody may be listening.
        let _ = self.changed.send(Changed {
            works: Vec::new(),
            items: false,
            copies: false,
            accounts: true,
        });
    }
}

impl Handle {
    /// Reads the works as the driver holds them, between two things the loop takes.
    pub async fn read<T: Send + 'static>(
        &self,
        read: impl FnOnce(&Driver) -> T + Send + 'static,
    ) -> Result<T, Refusal> {
        self.read_at(move |driver, _| read(driver)).await
    }

    /// Reads the works as the driver holds them, between two things the loop takes, at the time the loop's clock
    /// gives then: what holds now, such as the wait every work shares, is read by it.
    pub async fn read_at<T: Send + 'static>(
        &self,
        read: impl FnOnce(&Driver, Time) -> T + Send + 'static,
    ) -> Result<T, Refusal> {
        let (reply, answer) = oneshot::channel();
        let read = Box::new(move |driver: &Driver, now: Time| {
            let _ = reply.send(read(driver, now));
        });
        self.to.send(Message::Read(read)).map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)
    }

    /// The time now by the loop's clock, which nothing else reads (principle `outside-behind-interfaces`).
    pub async fn now(&self) -> Result<Time, Refusal> {
        self.read_at(|_, now| now).await
    }

    /// Tells whoever listens that the copies of the database changed: a step of keeping them ended. The notice holds no
    /// record; whoever listens reads the state again. The copies are written beside the loop, so it is not the loop
    /// that says so.
    pub fn copies_changed(&self) {
        // Nobody may be listening.
        let _ = self.changed.send(Changed {
            works: Vec::new(),
            items: false,
            copies: true,
            accounts: false,
        });
    }

    /// Says what a source outside the works says now of the project of this name, for the loop to write into the works
    /// of that project's items; nothing where the loop has ended.
    pub fn outside(&self, project: &str, source: &str, content: &str) {
        let message = Message::Outside {
            project: project.to_string(),
            source: source.to_string(),
            content: content.to_string(),
        };
        let _ = self.to.send(message);
    }

    /// Sets the owner's pause of everything, or lifts it.
    pub async fn pause_everything(&self, paused: bool) -> Result<(), Refusal> {
        let (reply, answer) = oneshot::channel();
        (self.to)
            .send(Message::Everything { paused, reply })
            .map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Has the sessions of works that wait kept warm, or has them kept no longer: the engine's own state, which holds
    /// when it starts again.
    pub async fn keep_warm(&self, warm: bool) -> Result<(), Refusal> {
        let (reply, answer) = oneshot::channel();
        (self.to)
            .send(Message::KeepWarm { warm, reply })
            .map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Sets how many rounds of agents may go on at once, which the engine takes at once: the slots added go to the works
    /// that wait for one, and with fewer no round under way is stopped. The engine keeps them only while it runs:
    /// whoever sets them keeps them where it reads them as it starts again.
    pub async fn set_slots(&self, slots: usize) -> Result<(), Refusal> {
        let (reply, answer) = oneshot::channel();
        (self.to)
            .send(Message::Slots { slots, reply })
            .map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// What keeps the tokens of the accounts, and what the agents' program says of a token.
    pub fn tokens(&self) -> &dyn Tokens {
        &*self.carriers.tokens
    }

    /// Adds an account under this name with this token, and says whether the token was at its usage limit as it was
    /// checked, and when that resets. The name is refused where it is no account's name or is one's added and not
    /// removed, the token where it is no token's shape, and the account where what keeps the tokens says none can be
    /// kept. Then the token is checked, by the check of the outside under it, off the loop, which may take as long as
    /// its short request: one the service refuses is not kept, saying the service's words; one at its usage limit is
    /// kept, saying its reset; and one that could not be checked, the network or the service not reached, is not kept,
    /// saying why. The token is kept before the account is added, and nothing of it is said in any words; what the
    /// service said of its usage is the account's once it is added.
    pub async fn add_account(&self, name: String, token: Token) -> Result<AccountAdded, Refusal> {
        if let Some(why) = driver::accounts::name_fault(&name) {
            return Err(Refusal::Failed(Failed::AccountName(why)));
        }
        if let Some(why) = driver::accounts::token_fault(token.secret()) {
            return Err(Refusal::NotKept(why));
        }
        let _one = self.adding.lock().await;
        let asked = name.clone();
        let named = self.read(move |driver| driver.accounts().is_live(&asked)).await?;
        if named {
            return Err(Refusal::Failed(Failed::NameInUse(name)));
        }
        if let Some(why) = self.carriers.tokens.closed() {
            return Err(Refusal::Closed(why));
        }
        // What the check says holds no token: the check takes it out, should the program have said it back.
        let heard = self.carriers.outside.check_token(token.clone()).await;
        let added = match heard.found {
            Found::Well => AccountAdded {
                at_limit: false,
                resets: None,
            },
            Found::Down {
                failure: decide::run::Failure::UsageLimit,
                resets,
                ..
            } => AccountAdded { at_limit: true, resets },
            Found::Down {
                failure: decide::run::Failure::CannotRun,
                why,
                ..
            } => {
                return Err(Refusal::NotKept(format!("the service refused the token: {why}")));
            }
            Found::Down { why, .. } => {
                return Err(Refusal::NotKept(format!(
                    "the token could not be checked, so it was not kept: {why}; add it again"
                )));
            }
        };
        let tokens = self.carriers.tokens.clone();
        let kept_as = name.clone();
        let kept = tokio::task::spawn_blocking(move || tokens.keep(&kept_as, &token)).await;
        match kept {
            Ok(Ok(())) => {}
            Ok(Err(why)) => return Err(Refusal::NotKept(why)),
            Err(e) => return Err(Refusal::NotKept(format!("the token could not be kept: {e}"))),
        }
        self.account(name.clone(), AccountAct::Added).await?;
        if let Some(said) = heard.usage {
            let _ = (self.to).send(Message::Usage { account: name, said });
        }
        Ok(added)
    }

    /// The owner opens the accounts: a look is taken under each whose usage was last heard of long enough ago, or
    /// never, off the loop, the accounts said changed as each ends; gives the names of the accounts looked under.
    pub async fn look(&self) -> Result<Vec<String>, Refusal> {
        let (reply, answer) = oneshot::channel();
        self.to.send(Message::Look { reply }).map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Takes the account of this name as the one the agents run under: every round handed out from now on runs under
    /// it, and the outside is checked at once under it where the agents' rounds wait for it, as they do while none is
    /// in use. It is refused where what keeps the tokens says none can be given.
    pub async fn take_account(&self, name: String) -> Result<(), Refusal> {
        if let Some(why) = self.carriers.tokens.closed() {
            return Err(Refusal::Closed(why));
        }
        self.account(name, AccountAct::Taken).await
    }

    /// Removes the account of this name, which is not the one in use, and then lets go of the token of every account
    /// removed, until it is: gives the tokens that could not be let go of, with why, the account being removed all the
    /// same and its token let go of again at the next removal and as the engine opens.
    pub async fn remove_account(&self, name: String) -> Result<Option<String>, Refusal> {
        self.account(name, AccountAct::Removed).await?;
        let removed = self.read(|driver| driver.accounts().removed()).await?;
        let tokens = self.carriers.tokens.clone();
        let let_go = tokio::task::spawn_blocking(move || let_go_of(&*tokens, &removed)).await;
        Ok(let_go.unwrap_or_else(|e| Some(e.to_string())))
    }

    /// The owner adds, removes or takes the account of this name, as the driver keeps it.
    async fn account(&self, name: String, what: AccountAct) -> Result<(), Refusal> {
        let (reply, answer) = oneshot::channel();
        (self.to)
            .send(Message::Account { name, what, reply })
            .map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Adds an item with its work, which is added paused unless it is added going on, and gives their
    /// numbers.
    pub async fn add(&self, item: NewItem) -> Result<(ItemNumber, WorkNumber), Refusal> {
        let (reply, answer) = oneshot::channel();
        self.to.send(Message::Add { item, reply }).map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Changes items, for this reason, all of the changes or none, and says whether each changed its item: a level is
    /// one the caller has checked against the installation's levels.
    pub async fn change(&self, why: String, changes: Vec<(ItemNumber, Asked)>) -> Result<Vec<bool>, Refusal> {
        let (reply, answer) = oneshot::channel();
        (self.to.send(Message::Change { why, changes, reply })).map_err(|_| Refusal::Gone)?;
        answer.await.map_err(|_| Refusal::Gone)?
    }

    /// Does what the owner does to a work, and says what came of it.
    pub async fn act(&self, work: WorkNumber, act: Act) -> Result<Acted, Refusal> {
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
                let kind = started.and_then(|s| kind_of(&kept, &s.cell));
                // What each try ran on, as the records say, by the try's number.
                let ran: BTreeMap<usize, RanOn> = (kept.work.ran.iter())
                    .filter(|r| r.run == run && r.round == round)
                    .map(|r| (r.attempt, r.on.clone()))
                    .collect();
                Some(Some((kind, ran)))
            })
            .await?;
        let (kind, ran) = match kind {
            None => return Ok(None),
            Some(None) => return Ok(Some(Ok(None))),
            Some(Some(found)) => found,
        };
        let carriers = self.carriers.clone();
        let told = tokio::task::spawn_blocking(move || carriers.told(kind, work, run, round)).await;
        Ok(Some(match told {
            Ok(told) => told.map(|mut told| {
                // A try the records say nothing of keeps what its carrier read of it, as from the file a try kept
                // before tries were recorded.
                for (n, t) in (1..).zip(told.tries.iter_mut()) {
                    if let Some(on) = ran.get(&n) {
                        t.ran = Some(on.clone());
                    }
                }
                Some(told)
            }),
            Err(e) => Err(format!("what was kept of it could not be read: {e}")),
        }))
    }

    /// The system prompt the line's session of this number, the line's first being none, began with, as the agents'
    /// carrier kept it for the work: none where there is no such work, and inside, none where nothing was kept of it,
    /// or why it could not be read. It is read off the loop, as [`Handle::told`] reads.
    pub async fn system(
        &self,
        work: WorkNumber,
        line: String,
        session: u32,
    ) -> Result<Option<Result<Option<String>, String>>, Refusal> {
        if !self.read(move |driver| driver.work(work).is_some()).await? {
            return Ok(None);
        }
        let carriers = self.carriers.clone();
        let system = tokio::task::spawn_blocking(move || carriers.system(work, &line, session)).await;
        Ok(Some(match system {
            Ok(system) => system,
            Err(e) => Err(format!("what was kept of it could not be read: {e}")),
        }))
    }

    /// Asks a fork of the session the round of this number of the run ran in, the run's last round that ran where none
    /// is named, a question, through the agents' carrier, off the loop: none where there is no such work, and inside,
    /// what the fork answered or why it could not be asked.
    pub async fn fork(
        &self,
        work: WorkNumber,
        run: RunId,
        round: Option<usize>,
        question: String,
    ) -> Result<Option<Result<Forked, String>>, Refusal> {
        let asked = self
            .read(move |driver| {
                let kept = driver.work(work)?;
                let ran = (kept.work.ran.iter()).filter(|r| r.run == run).map(|r| r.round).max();
                let Some(round) = round.or(ran) else {
                    return Some(Err(format!("Work {} has no round of run {} that ran.", work.0, run.0)));
                };
                if !kept.work.ran.iter().any(|r| r.run == run && r.round == round) {
                    return Some(Err(format!(
                        "Work {} has no round {round} of run {} that ran.",
                        work.0, run.0
                    )));
                }
                let ended = matches!(kept.decision(), decide::Decision::Ended(_));
                let outline = match fork_outline(&kept) {
                    Ok(outline) => outline,
                    Err(why) => return Some(Err(why)),
                };
                Some(Ok(Fork {
                    work,
                    outline,
                    item: kept.item.clone(),
                    kept: Arc::new(kept.work.clone()),
                    run,
                    round,
                    question,
                    ended,
                    accounts: driver.accounts().clone(),
                }))
            })
            .await?;
        let fork = match asked {
            None => return Ok(None),
            Some(Err(why)) => return Ok(Some(Err(why))),
            Some(Ok(fork)) => fork,
        };
        let forked = self.carriers.agents.fork(fork).await;
        if let Ok(forked) = &forked
            && let Some(said) = forked.usage.clone()
        {
            let _ = (self.to).send(Message::Usage {
                account: forked.account.clone(),
                said,
            });
        }
        Ok(Some(forked))
    }

    /// What the agent of the round of this number of the run did from the mark on, as the carrier of its cell's kind
    /// keeps it as it comes, and whether the round had ended as the records stood just before it was read, so that
    /// what was read of a round that had ended is all it did: none where there is no such work, and inside, none where
    /// the work has no such round, or why it could not be read. It is read off the loop, as [`Handle::told`] reads.
    pub async fn doing(
        &self,
        (work, run, round): (WorkNumber, RunId, usize),
        from: Mark,
    ) -> Result<Option<Result<Option<(Doings, bool)>, String>>, Refusal> {
        let kind = self
            .read(move |driver| {
                let kept = driver.work(work)?;
                if !kept.work.given.contains_key(&(run, round)) {
                    return Some(None);
                }
                let records = &kept.work.records;
                let started = records.started.iter().find(|s| s.run == run);
                let kind = started.and_then(|s| kind_of(&kept, &s.cell));
                // A round has ended once its run has come back from as many rounds.
                let ended = records.returned.iter().filter(|r| r.run == run).count() >= round;
                Some(Some((kind, ended)))
            })
            .await?;
        let (kind, ended) = match kind {
            None => return Ok(None),
            Some(None) => return Ok(Some(Ok(None))),
            Some(Some(found)) => found,
        };
        let carriers = self.carriers.clone();
        let doing = tokio::task::spawn_blocking(move || carriers.doing(kind, (work, run, round), from)).await;
        Ok(Some(match doing {
            Ok(doing) => doing.map(|doings| Some((doings, ended))),
            Err(e) => Err(format!("what was kept of it could not be read: {e}")),
        }))
    }

    /// What the repository of each work's project says of it beside its records, in the order asked, as the carrier
    /// of the engine's acts works it out off the loop, [`SEEN`] works at once.
    pub async fn seen(&self, of: Vec<OfWork>) -> Vec<Result<Seen, String>> {
        let at_once = Arc::new(tokio::sync::Semaphore::new(SEEN));
        let asked: Vec<_> = (of.into_iter())
            .map(|of| {
                let (carriers, at_once) = (self.carriers.clone(), at_once.clone());
                tokio::spawn(async move {
                    let _turn = at_once.acquire_owned().await;
                    carriers.seen(of).await
                })
            })
            .collect();
        let mut seen = Vec::with_capacity(asked.len());
        for asked in asked {
            seen.push(
                asked
                    .await
                    .unwrap_or_else(|e| Err(format!("it could not be worked out: {e}"))),
            );
        }
        seen
    }

    /// Each file the work changed, as the carrier of the engine's acts works it out off the loop: none where there is
    /// no such work, and inside, none where nothing tells what it changed, or why it could not be worked out.
    pub async fn changes_of(&self, work: WorkNumber) -> Result<Option<Result<Option<Changes>, String>>, Refusal> {
        let Some(of) = self.of_work(work).await? else {
            return Ok(None);
        };
        Ok(Some(self.carriers.changes(of).await))
    }

    /// The unified diff of the file of this path the work changed, as the carrier of the engine's acts works it out
    /// off the loop: none where there is no such work, and inside, none where the work changed no file of the path, or
    /// why it could not be worked out.
    pub async fn diff(&self, work: WorkNumber, path: String) -> Result<Option<Result<Option<Diff>, String>>, Refusal> {
        let Some(of) = self.of_work(work).await? else {
            return Ok(None);
        };
        Ok(Some(self.carriers.diff(of, path).await))
    }

    /// The work as the carrier of its project's repository is asked of it for its changes; none where there is no
    /// such work.
    async fn of_work(&self, work: WorkNumber) -> Result<Option<OfWork>, Refusal> {
        self.read(move |driver| {
            driver.work(work).map(|kept| OfWork {
                work,
                item: kept.item.clone(),
                merged: None,
            })
        })
        .await
    }
}

/// How many works the repository is asked of at once: each look runs a few git commands, which take tens of
/// milliseconds each, mostly the process starting, so a few at once keep a read of every work short without
/// crowding the machine.
pub const SEEN: usize = 8;

/// Lets go of the token of each account of these names, which are removed, a token not found being let go of: gives
/// each that could not be, by its name, with why; none where every one was.
fn let_go_of(tokens: &dyn Tokens, names: &[String]) -> Option<String> {
    let failed: Vec<String> = (names.iter())
        .filter_map(|name| tokens.let_go(name).err().map(|why| format!("{name}: {why}")))
        .collect();
    (!failed.is_empty()).then(|| failed.join("; "))
}

/// The specification of the cell in the flow.
fn cell_spec<'a>(flow: &'a Compiled, cell: &CellId) -> Option<&'a CellSpec> {
    flow.cells.iter().find(|spec| spec.id == *cell)
}

/// The keep-warm pings of a work that are under way, and what is held back for them.
#[derive(Default)]
struct Pinging {
    /// How many pings are under way, of each line.
    lines: BTreeMap<String, usize>,
    /// What waits for them, each with the line of the round it is, none for a letting go: in the order it came.
    held: Vec<(Option<String>, Todo)>,
}

impl Pinging {
    /// A ping of the line is under way.
    fn begins(&mut self, line: &str) {
        *self.lines.entry(line.to_string()).or_default() += 1;
    }

    /// Whether a ping holds the todo back: a round of a cell of a line whose session is pinged, and the letting go of
    /// what the work holds while any ping of it is under way.
    fn holds(&self, line: Option<&str>, effect: &Effect) -> bool {
        match effect {
            Effect::Round { .. } => line.is_some_and(|line| self.lines.contains_key(line)),
            Effect::Release { .. } => !self.lines.is_empty(),
            _ => false,
        }
    }

    /// A ping of the line has ended: what nothing holds back now, in the order it came.
    fn ended(&mut self, line: &str) -> Vec<Todo> {
        if let Some(under_way) = self.lines.get_mut(line) {
            *under_way -= 1;
            if *under_way == 0 {
                self.lines.remove(line);
            }
        }
        let (free, held): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|(line, todo)| !self.holds(line.as_deref(), &todo.effect));
        self.held = held;
        free.into_iter().map(|(_, todo)| todo).collect()
    }
}

/// Who runs the cell of the work, an agent or the engine's act, as the version the work is on has it: none of a cell
/// that version has not, or of a work on none kept, whose round both carriers are asked of.
fn kind_of(kept: &driver::Kept<'_>, cell: &CellId) -> Option<Kind> {
    kept.cell(cell)
        .map(|c| if c.agent { Kind::Agent } else { Kind::Engine })
}

/// What a fork of a round of the work reads which session the round ran in by: the outline of the version the work is
/// on. A work on none kept has none, and the fork is refused, with why.
fn fork_outline(kept: &driver::Kept<'_>) -> Result<Arc<flow::Outline>, String> {
    let outline = kept.on.outline().ok_or_else(|| {
        format!(
            "The version of the flow work {} ran on was not kept: which session its round ran in is not known.",
            kept.head.number.0
        )
    })?;
    Ok(Arc::new(outline.clone()))
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

#[cfg(test)]
mod tests {
    //! Which carrier a round of a work is read from, and what a fork of it reads its session by, as the version of the
    //! flow the work is on has them, and what a keep-warm ping holds back.

    use std::path::Path;

    use decide::End;
    use flow::Has;
    use store::{First, Item, WorkHead};
    use work::Work;

    use super::*;

    /// The flow of a deliverable item as the repository defines it (`flow/deliver`), whose one cell, `V1`, is the
    /// engine's.
    fn deliver() -> Compiled {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/deliver");
        let source = flow::read(&dir).expect("the deliver flow is read");
        let has = Has {
            sources: [driver::DEPENDENCIES.to_string()].into(),
            acts: ["deliver".to_string()].into(),
            ..Has::default()
        };
        flow::compile(&source, &has).expect("the deliver flow compiles")
    }

    /// The item of one work on the deliver flow and the work's head, as the store keeps them, with an earlier version
    /// of the flow, on which `V1` is an agent's and `W1` the engine's, where the flow installed has `V1` as the engine's
    /// and no `W1`.
    struct Stored {
        installed: Compiled,
        outline: flow::Outline,
        head: WorkHead,
        item: Item,
        work: Work,
    }

    impl Stored {
        fn new() -> Stored {
            let installed = deliver();
            let mut outline = flow::Outline::of(&installed);
            let v1 = (outline.cells.iter_mut())
                .find(|c| c.id.as_str() == "V1")
                .expect("the one cell");
            let mut w1 = v1.clone();
            v1.agent = true;
            w1.id = CellId::from("W1");
            outline.cells.push(w1);
            let first = First {
                title: "A work".to_string(),
                level: None,
                project: None,
                depends_on: Vec::new(),
                tags: Vec::new(),
            };
            Stored {
                head: WorkHead {
                    number: WorkNumber(1),
                    item: ItemNumber(1),
                    flow: installed.name.clone(),
                    added: Time(0),
                },
                item: Item::new(ItemNumber(1), Time(0), first, Vec::new()),
                work: Work::default(),
                installed,
                outline,
            }
        }

        /// The earlier version, on which the work ended cancelled.
        fn earlier(&self) -> driver::On {
            driver::On::Earlier {
                version: "earlier".to_string(),
                outline: Arc::new(self.outline.clone()),
                end: End::Cancelled,
                reading: Default::default(),
            }
        }

        /// The work on this version.
        fn kept<'a>(&'a self, on: &'a driver::On) -> driver::Kept<'a> {
            driver::Kept {
                head: &self.head,
                item: &self.item,
                flow: &self.installed,
                on,
                passed: &[],
                work: &self.work,
            }
        }
    }

    // The two tests below read a work on an earlier version as that version has it, and one on none kept as none, so
    // that a reading of the flow installed fails each. Milliseconds: what each catches is one reading of the service's,
    // which no test of a running service reaches.

    // The carrier a round of a cell is read from is the one the version has the cell run by; of a work on none kept,
    // none, so that both carriers are asked.
    #[test]
    fn a_rounds_carrier_is_the_one_the_version_the_work_is_on_has_its_cell_run_by_and_none_of_a_work_on_none_kept() {
        let stored = Stored::new();
        let earlier = stored.earlier();
        let kept = stored.kept(&earlier);
        assert_eq!(kind_of(&kept, &CellId::from("V1")), Some(Kind::Agent));
        assert_eq!(kind_of(&kept, &CellId::from("W1")), Some(Kind::Engine));
        let none = driver::On::NotKept { end: End::Cancelled };
        assert_eq!(kind_of(&stored.kept(&none), &CellId::from("V1")), None);
    }

    // A fork reads which session a round ran in by the outline of the version the work is on, and is refused for a
    // work on none kept.
    #[test]
    fn a_fork_reads_the_outline_of_the_version_the_work_is_on_and_is_refused_for_a_work_on_none_kept() {
        let stored = Stored::new();
        let earlier = stored.earlier();
        assert_eq!(fork_outline(&stored.kept(&earlier)).as_deref(), Ok(&stored.outline));
        let none = driver::On::NotKept { end: End::Cancelled };
        assert_eq!(
            fork_outline(&stored.kept(&none)),
            Err(
                "The version of the flow work 1 ran on was not kept: which session its round ran in is not known."
                    .to_string()
            )
        );
    }

    fn round(run: u64) -> Todo {
        Todo {
            work: WorkNumber(1),
            effect: Effect::Round {
                run: RunId(run),
                cell: CellId::from("V1"),
                number: 1,
                given: work::Given::Task,
            },
        }
    }

    fn release() -> Todo {
        Todo {
            work: WorkNumber(1),
            effect: Effect::Release { end: End::Merged },
        }
    }

    // A ping holds back the rounds of its own line and the letting go of what the work holds, and nothing else: a round
    // of another line, of an act, and every other todo go on beside it.
    #[test]
    fn a_ping_holds_back_the_rounds_of_its_line_and_the_letting_go_and_no_other_todo() {
        let mut pinging = Pinging::default();
        pinging.begins("engineer");
        let (round, release) = (round(1), release());
        assert!(pinging.holds(Some("engineer"), &round.effect), "its own line's round");
        assert!(!pinging.holds(Some("review"), &round.effect), "another line's round");
        assert!(!pinging.holds(None, &round.effect), "a round of an act");
        assert!(pinging.holds(None, &release.effect), "the letting go");
        let call = Effect::Withdraw { call: CallId(1) };
        assert!(!pinging.holds(None, &call), "a call is no session's program");
    }

    // What waits for pings goes on as the pings that hold it end: a round of a line once its own line's last ping has
    // ended, the letting go once none of the work's is under way, each in the order it came.
    #[test]
    fn what_waits_for_pings_goes_on_as_the_pings_that_hold_it_end() {
        let mut pinging = Pinging::default();
        pinging.begins("engineer");
        pinging.begins("engineer");
        pinging.begins("review");
        pinging.held.push((Some("engineer".to_string()), round(1)));
        pinging.held.push((None, release()));
        pinging.held.push((Some("review".to_string()), round(2)));
        // One of the engineer's two pings ends: nothing waits for less than all of a line's.
        assert!(pinging.ended("engineer").is_empty());
        // The review's ends: its round goes on, and the engineer's and the letting go still wait.
        assert_eq!(pinging.ended("review"), [round(2)]);
        // The engineer's last ends: its round goes on, then the letting go, as none is under way.
        assert_eq!(pinging.ended("engineer"), [round(1), release()]);
        assert!(pinging.lines.is_empty() && pinging.held.is_empty());
    }
}
