//! The loop with the test carriers, over a store in a temporary directory, on the trial flow and its script
//! (`tests/trial`). Each test takes the loop one thing at a time, as the loop takes them, with the runtime's time paused
//! so that each round ends when the script says; after every step it checks what the loop must always keep:
//!
//! - every todo handed out is answered once, or is still to be done;
//! - nothing is handed out that the store does not hold as to be done.
//!
//! The tests are kept in modules by subject, so that a new test goes into the module of what it tests.

mod acts;
mod cancelling;
mod contract;
mod dependencies;
mod failing;
mod going;
mod pausing;
mod shared_wait;
mod stopped;
mod stopping;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use decide::{CallId, Decision, RunId, Waited, Who};
use driver::{FirstWrite, NewItem, Todo};
use engine::{Effect, Event};
use flow::Compiled;
use service::{
    Act, CallReply, Carriers, Inbox, Message, Refusal, Release, ReleaseReply, Round, RoundReply, Rounds, Script,
    Scripted, Service, TokioClock, ToolCall, Tools,
};
use store::{ItemNumber, Storage, Store, WorkNumber};
use tokio::sync::oneshot;
use work::{Ask, Call, Time, Work};

/// An item of the trial flow with a request, depending on these items, added paused: its work begins nothing until it is
/// let on.
pub fn new_item(title: &str, depends_on: &[u64]) -> NewItem {
    NewItem {
        title: title.to_string(),
        level: "1".to_string(),
        depends_on: depends_on.iter().copied().map(ItemNumber).collect(),
        flow: "trial".to_string(),
        writes: vec![FirstWrite {
            block: "request".into(),
            content: format!("{title}, please."),
            value: None,
        }],
    }
}

/// Gives the loop what the owner does, and gives what the loop answered and what came of its taking it.
pub fn act_on(service: &mut Service, work: WorkNumber, act: Act) -> (Result<(), Refusal>, Result<(), service::Fault>) {
    let (reply, mut answer) = oneshot::channel();
    let taken = service.take(Message::Act { work, act, reply });
    (answer.try_recv().expect("the loop answers at once"), taken)
}

/// Adds the item through the loop, which must take it.
pub fn add_to(service: &mut Service, item: NewItem) -> (ItemNumber, WorkNumber) {
    let (reply, mut answer) = oneshot::channel();
    service.take(Message::Add { item, reply }).expect("the loop takes it");
    answer
        .try_recv()
        .expect("the loop answers at once")
        .expect("the item is added")
}

/// When the tests' clock starts: 2026-11-10, at midnight.
pub const START: Time = Time(1_794_268_800_000);

/// The slots of the loops of the tests that do not count them: more than any of them has works.
pub const SLOTS: usize = 1000;

/// The directory of the trial flow, `flow`, and its script, `script.toml`.
pub fn trial_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trial")
}

/// The trial flow, compiled.
pub fn trial_flow() -> Compiled {
    let source = flow::read(&trial_dir().join("flow")).expect("the trial flow is read");
    let has = flow::Has {
        sources: BTreeSet::from([driver::DEPENDENCIES.to_string()]),
        acts: BTreeSet::from(["merge".to_string()]),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the trial flow compiles")
}

/// The trial flow's script.
pub fn trial_script() -> Script {
    let text = std::fs::read_to_string(trial_dir().join("script.toml")).expect("the script is read");
    Script::read(&text).expect("the script reads")
}

/// A thing to be done that a carrier answers: a round, which its end or its stop answers, or a tool's call, which
/// its result or its withdrawing answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Unit {
    Round(WorkNumber, RunId, usize),
    Call(WorkNumber, CallId),
    /// What the work held, let go once it ended.
    Release(WorkNumber),
}

/// The loop over a store in a directory of its own, with carriers that record what they are handed.
pub struct Trial {
    /// Kept so that the store's directory lives as long as the trial.
    _dir: tempfile::TempDir,
    path: PathBuf,
    script: Script,
    clock: Arc<TokioClock>,
    service: Service,
    inbox: Inbox,
    /// What the carriers were handed since the process last started.
    handed: Arc<Mutex<BTreeSet<Unit>>>,
    /// How often the loop took an answer to each unit, over every start of the process.
    answered: BTreeMap<Unit, usize>,
    /// How many things the loop took, over every start.
    pub steps: usize,
}

impl Trial {
    /// The loop with the trial's script, on an empty store.
    pub fn open() -> Trial {
        Trial::with(trial_script())
    }

    /// The loop with this script, on an empty store.
    pub fn with(script: Script) -> Trial {
        let dir = tempfile::tempdir().expect("a directory for the store");
        let path = dir.path().join("store.sqlite");
        let clock = Arc::new(TokioClock::new(START));
        let handed = Arc::new(Mutex::new(BTreeSet::new()));
        let (service, inbox) = open(&path, &script, &clock, &handed);
        let trial = Trial {
            _dir: dir,
            path,
            script,
            clock,
            service,
            inbox,
            handed,
            answered: BTreeMap::new(),
            steps: 0,
        };
        trial.check();
        trial
    }

    /// Stops the process and starts it again on the same store: what was under way is lost with it, and the loop
    /// hands out again what is to be done.
    pub fn restart(&mut self) {
        self.handed = Arc::new(Mutex::new(BTreeSet::new()));
        let (service, inbox) = open(&self.path, &self.script, &self.clock, &self.handed);
        self.service = service;
        self.inbox = inbox;
        self.check();
    }

    /// Stops the process and starts it again, and the owner, having looked, lifts at once the pause of everything the
    /// engine made as it started again on works going on, where it made one.
    pub fn restart_and_lift(&mut self) {
        self.restart();
        if self.held_by_its_start() {
            self.everything(false);
        }
    }

    /// Whether the engine holds everything since it started again with works going on, until the owner lifts it.
    pub fn held_by_its_start(&self) -> bool {
        (self.service.driver().everything_paused()).is_some_and(|paused| paused.by == decide::Pauser::Engine)
    }

    /// The owner sets the pause of everything, or lifts it, the owner's or the engine's own.
    pub fn everything(&mut self, paused: bool) {
        let (reply, mut answer) = oneshot::channel();
        self.take(Message::Everything { paused, reply });
        self.steps += 1;
        (answer.try_recv())
            .expect("the loop answers at once")
            .expect("the pause of everything is taken");
    }

    /// Takes the next thing that comes, once it comes, and checks what must always hold; says what it was, or none
    /// where nothing more comes.
    pub async fn step(&mut self) -> Option<Event> {
        // An hour of the paused clock passes at once where nothing is under way.
        let message = tokio::time::timeout(Duration::from_secs(3600), self.inbox.recv())
            .await
            .ok()??;
        let (work, event) = match message {
            Message::Happened { work, event } => (work, event),
            // The time a wait was for has come: the loop takes it as it comes, and nothing more is said of it.
            message @ (Message::Due { .. } | Message::Tick { .. }) => {
                self.service.take(message).expect("the loop takes it");
                self.steps += 1;
                self.check();
                return Some(Event::Look);
            }
            _ => panic!("only the carriers and the timers send to the loop in these tests"),
        };
        let unit = match &event {
            Event::Returned { run, .. } | Event::Failed { run, .. } => {
                Some(Unit::Round(work, *run, self.began(work, *run)))
            }
            Event::Done { call, .. } => Some(Unit::Call(work, *call)),
            Event::Released => Some(Unit::Release(work)),
            _ => None,
        };
        if let Some(unit) = unit {
            *self.answered.entry(unit).or_default() += 1;
        }
        let taken = event.clone();
        self.service
            .take(Message::Happened { work, event })
            .expect("the loop takes it");
        self.steps += 1;
        self.check();
        Some(taken)
    }

    /// Takes what comes until the work's decision is as `wanted` says, and gives it; fails where nothing more comes
    /// before.
    pub async fn until(&mut self, work: WorkNumber, wanted: impl Fn(&Decision) -> bool) -> Decision {
        loop {
            let decision = self.decision(work);
            if wanted(&decision) {
                return decision;
            }
            if self.step().await.is_none() {
                panic!("nothing more comes, and work {} is at {decision:?}", work.0);
            }
        }
    }

    /// Takes what the owner sends, and checks what must always hold.
    pub fn take(&mut self, message: Message) {
        self.service.take(message).expect("the loop takes it");
        self.check();
    }

    /// Adds an item with a request, depending on these items.
    pub fn add(&mut self, title: &str, depends_on: &[u64]) -> (ItemNumber, WorkNumber) {
        let (reply, mut answer) = oneshot::channel();
        self.take(Message::Add {
            item: new_item(title, depends_on),
            reply,
        });
        answer
            .try_recv()
            .expect("the loop answers at once")
            .expect("the item is added")
    }

    /// Does what the owner does to the work.
    pub fn act(&mut self, work: WorkNumber, act: Act) -> Result<(), Refusal> {
        let (reply, mut answer) = oneshot::channel();
        self.take(Message::Act { work, act, reply });
        self.steps += 1;
        answer.try_recv().expect("the loop answers at once")
    }

    /// Answers the owner's call the work waits for.
    pub fn answer(&mut self, work: WorkNumber, result: &str) {
        let call = self.owners_call(work).expect("the work waits for the owner's call");
        self.act(
            work,
            Act::Answer {
                call,
                result: result.to_string(),
            },
        )
        .expect("the answer is taken");
    }

    /// The owner's call the work waits for, where it waits for one.
    pub fn owners_call(&self, work: WorkNumber) -> Option<CallId> {
        match self.decision(work) {
            Decision::Wait { on, .. } => on.iter().find_map(|waited| match waited {
                Waited::Call {
                    call,
                    on: Some(Who::Owner),
                } => Some(*call),
                _ => None,
            }),
            _ => None,
        }
    }

    /// Takes the work to its end, answering the owner's questions as they come.
    pub async fn drive(&mut self, work: WorkNumber) -> Decision {
        loop {
            let decision = self.decision(work);
            if matches!(decision, Decision::Ended(_)) {
                return decision;
            }
            if self.owners_call(work).is_some() {
                self.answer(work, "Yes, by name.");
                continue;
            }
            if self.step().await.is_none() {
                panic!("nothing more comes, and work {} is at {decision:?}", work.0);
            }
        }
    }

    /// What the work does next.
    pub fn decision(&self, work: WorkNumber) -> Decision {
        self.service
            .driver()
            .view(work)
            .expect("the driver has the work")
            .decision
    }

    /// Everything kept of the work.
    pub fn work(&self, work: WorkNumber) -> Work {
        self.service
            .driver()
            .work(work)
            .expect("the driver has the work")
            .work
            .clone()
    }

    /// The cells of the work's runs that finished, in the order they finished.
    pub fn finished(&self, work: WorkNumber) -> Vec<String> {
        let kept = self.work(work);
        (kept.records.finished.iter())
            .map(|finished| {
                let started = kept.records.started.iter().find(|s| s.run == finished.run);
                started.expect("a finished run started").cell.to_string()
            })
            .collect()
    }

    /// How many rounds of the run began.
    fn began(&self, work: WorkNumber, run: RunId) -> usize {
        let kept = self.service.driver().work(work).expect("the driver has the work");
        kept.work.records.rounds.iter().filter(|r| r.run == run).count()
    }

    /// What the loop must always keep: every unit handed out is answered once or still to be done, and what is to be
    /// done and carried out by someone was handed out.
    fn check(&self) {
        if let Some((unit, times)) = self.answered.iter().find(|(_, times)| **times > 1) {
            panic!("{unit:?} was answered {times} times");
        }
        let handed = self.handed.lock().expect("the record of what was handed out").clone();
        let open: BTreeSet<Unit> = handed
            .into_iter()
            .filter(|unit| !self.answered.contains_key(unit))
            .collect();
        let driver = self.service.driver();
        let pending: BTreeSet<Unit> = (driver.pending().iter())
            .filter_map(|todo| carried(driver, todo))
            .collect();
        assert_eq!(
            open, pending,
            "what was handed out and not answered is what is to be done"
        );
    }
}

/// The unit a todo is, where a carrier carries it out.
fn carried(driver: &driver::Driver, todo: &Todo) -> Option<Unit> {
    let work = todo.work;
    match &todo.effect {
        Effect::Round { run, number, .. } => Some(Unit::Round(work, *run, *number)),
        Effect::Stop { run } => {
            let kept = driver.work(work)?;
            Some(Unit::Round(
                work,
                *run,
                kept.work.records.rounds.iter().filter(|r| r.run == *run).count(),
            ))
        }
        Effect::Call {
            call,
            what: Call::Asked(Ask::Tool { .. }),
            ..
        } => Some(Unit::Call(work, *call)),
        Effect::Call { .. } => None,
        Effect::Withdraw { call } => Some(Unit::Call(work, *call)),
        Effect::Release { .. } => Some(Unit::Release(work)),
    }
}

/// Opens the loop on the store at this path, with carriers following the script that record in `handed` what they
/// are given.
fn open(
    path: &Path,
    script: &Script,
    clock: &Arc<TokioClock>,
    handed: &Arc<Mutex<BTreeSet<Unit>>>,
) -> (Service, Inbox) {
    let store = Store::open(path).expect("the store opens");
    let recording = Arc::new(Recording {
        scripted: Scripted::new(script.clone()),
        handed: handed.clone(),
        path: path.to_path_buf(),
    });
    let carriers = Carriers {
        agents: recording.clone(),
        acts: recording.clone(),
        tools: recording.clone(),
        network: Arc::new(recording.scripted.clone()),
        slots: SLOTS,
    };
    let clock: Arc<dyn service::Clock> = clock.clone();
    Service::open(Box::new(store), vec![trial_flow()], carriers, clock).expect("the loop opens")
}

/// The scripted carriers, recording what they are handed after checking that the store holds it as to be done.
struct Recording {
    scripted: Scripted,
    handed: Arc<Mutex<BTreeSet<Unit>>>,
    path: PathBuf,
}

impl Recording {
    /// Checks that the store holds the effect as to be done, and records the unit as handed out. A round given and then
    /// stopped, or a call made and then withdrawn, is one unit, answered once; anything else is handed out once.
    fn hand(&self, work: WorkNumber, effect: Effect, unit: Unit) {
        let store = Store::open(&self.path).expect("the store opens beside the loop's");
        let kept = store.load(work).expect("the store has the work");
        assert!(
            engine::pending(&trial_flow(), &kept).contains(&effect),
            "{effect:?} of work {} is handed out, and the store does not hold it as to be done",
            work.0
        );
        let again = matches!(effect, Effect::Stop { .. } | Effect::Withdraw { .. });
        let mut handed = self.handed.lock().expect("the record of what was handed out");
        assert!(handed.insert(unit) || again, "{unit:?} is handed out twice");
    }
}

impl Rounds for Recording {
    fn give(&self, round: Round, reply: RoundReply) {
        let effect = Effect::Round {
            run: round.run,
            cell: round.cell.id.clone(),
            number: round.number,
            given: round.given.clone(),
        };
        self.hand(round.work, effect, Unit::Round(round.work, round.run, round.number));
        self.scripted.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        let unit = Unit::Round(round.work, round.run, round.number);
        self.hand(round.work, Effect::Stop { run: round.run }, unit);
        self.scripted.stop(round, reply);
    }

    fn release(&self, release: Release, reply: ReleaseReply) {
        let effect = Effect::Release { end: release.end };
        self.hand(release.work, effect, Unit::Release(release.work));
        self.scripted.release(release, reply);
    }
}

impl Tools for Recording {
    fn call(&self, call: ToolCall, reply: CallReply) {
        let effect = Effect::Call {
            call: call.call,
            run: call.run,
            what: Call::Asked(Ask::Tool {
                name: call.name.clone(),
                input: call.input.clone(),
            }),
        };
        self.hand(call.work, effect, Unit::Call(call.work, call.call));
        self.scripted.call(call, reply);
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        let unit = Unit::Call(call.work, call.call);
        self.hand(call.work, Effect::Withdraw { call: call.call }, unit);
        self.scripted.withdraw(call, reply);
    }
}
