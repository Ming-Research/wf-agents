//! A try of a case, built through the engine (design document `testing`, 12, 13): on an installation of its own, the
//! case's item is added and let on, and the engine's own loop is driven one thing at a time, so that each act of the
//! history goes in where the case's order puts it ([`cases::order`]). The history's rounds are handed in as the case
//! says; the case's cell's line is begun anew as the history ends; and the cell's line's reading round and the cell's
//! first round are carried by the agents' carrier, with the agents' program the installation names: the real one, or
//! a stand-in that returns what it is told to, which builds the texts the cell is sent with no model.
//!
//! The case is refused where the engine does not take the history as it is written: a round of it sent back, the walk
//! entering a cell the history does not give next and that nothing then stops, or the case's cell entered otherwise
//! than the case says.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cases::{Act, Case, Order};
use decide::{BlockId, CellId, RunId};
use driver::{FirstWrite, NewItem};
use engine::Event;
use engine::spoken::To;
use flow::Compiled;
use service::{Carriers, Message, Rounds, Service, SystemClock};
use store::{Store, WorkNumber};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot;

use super::carriers::{Acts, Agents, Calls, Making, Note, Shared};
use super::installation::{self, Installation, MAIN_REF};
use super::made;

/// How long the loop may wait for the next thing to happen while no round runs on the agents' program: a history's
/// round and an act end in moments, so a wait this long is a round held that nothing will stop.
const QUIET: Duration = Duration::from_secs(5);

/// How long the loop may wait for the next thing while a round runs on the agents' program, which its own guard and
/// the work's budget hold besides.
const RUNNING: Duration = Duration::from_secs(3 * 60 * 60);

/// What a try of a case is built with.
pub struct With<'a> {
    pub flow: &'a Compiled,
    pub case: &'a Case,
    pub item: &'a cases::written::Item,
    /// The real installation's settings and the project whose settings the cases run as, by its name there.
    pub settings: &'a carry::Settings,
    pub project: &'a carry::settings::Project,
    /// The made project's name, imported repository, and the commit of each of its names the case uses.
    pub made: &'a str,
    pub imported: &'a Path,
    pub commits: &'a BTreeMap<String, String>,
    pub beside: &'a flow::agents::Agents,
    /// The rules the cases' calls get their results by, by name, and how the made project's tests are found.
    pub rules: &'a BTreeMap<String, cases::written::Rules>,
    pub tests: Option<&'a cases::written::Tests>,
    /// The directory the try's installation is made in.
    pub dir: &'a Path,
    /// The agents' program, where another stands in for the installation's.
    pub program: Option<&'a Path>,
    /// Whether the build ends with the cell's first round, as it does to give the texts the cell is sent.
    pub first_only: bool,
}

/// What the case's cell was sent, as the engine sent it, and the situation it was entered in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Built {
    /// The system prompt the session of the cell's line began with.
    pub system: String,
    /// The message of the round of the cell's line that read the tree for it in that session, where one did.
    pub reading: Option<String>,
    /// The message of the cell's first round.
    pub message: String,
    /// What ran the cell, as it watches it.
    pub new: Vec<String>,
    /// Whether a run of the cell finished before.
    pub ran_before: bool,
}

/// A case built, with the loop that goes on driving its work: what the cell was sent, and how far it went.
pub struct Building {
    pub built: Built,
    pub installation: Installation,
    pub work: WorkNumber,
    /// The run of the cell's first round.
    pub run: RunId,
    service: Service,
    inbox: service::Inbox,
    notes: UnboundedReceiver<Note>,
    agents: Arc<Agents>,
    shared: Arc<Shared>,
    /// Whether the build ends with the cell's first round, as it does where a stand-in carries it.
    first_only: bool,
    /// A tool the cell called that the case holds no result for, where it called one.
    pub unanswered: Option<String>,
}

/// A round of the case's cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rounded {
    pub run: RunId,
    pub number: usize,
    /// What it was told.
    pub message: String,
    /// What came back of it, as the work keeps it.
    pub back: String,
    pub usd_micros: u64,
    pub tokens: u64,
    /// The id of the session it ran in.
    pub session: String,
}

/// The block people write that is no choice: the item's background, written as it is added and anew by the owner.
fn background(flow: &Compiled) -> Result<BlockId, String> {
    let prose: Vec<&BlockId> = (flow.blocks.iter())
        .filter(|(_, b)| b.writers.people && b.values.is_none())
        .map(|(id, _)| id)
        .collect();
    match prose.as_slice() {
        [one] => Ok((*one).clone()),
        _ => Err(format!(
            "{} has not one block people write as an item's background",
            flow.name
        )),
    }
}

/// Builds the try: the installation, the work through its history, and the cell's first round.
pub async fn build(with: &With<'_>) -> Result<Building, String> {
    let (flow, case) = (with.flow, with.case);
    let installation = installation::make(with)?;
    let id = carry::session::installation(&installation.data)?;
    let tag = carry::git::tag(&id);
    let main =
        (with.commits.get(&with.item.main)).ok_or_else(|| format!("the made project has no {}", with.item.main))?;
    let order = cases::order(case, |cell| {
        (flow.cells.iter()).find(|c| &c.id == cell).and_then(|c| c.line.clone())
    });
    let (to_runner, notes) = tokio::sync::mpsc::unbounded_channel();
    let shared = Shared::new(Making {
        rounds: order.rounds.iter().map(|due| due.round.clone()).collect(),
        target: case.cell.clone(),
        commits: with.commits.clone(),
        beside: installation.beside.clone(),
        project: installation.project.clone(),
        data: installation.data.clone(),
        tag,
        main: main.clone(),
        stand_in: with.program.map(|_| installation.data.join(STAND_IN)),
        notes: to_runner,
    });
    let of = carry::Of::new(
        &installation.settings,
        installation.project.clone(),
        installation.beside.clone(),
        MAIN_REF.to_string(),
    );
    let real = carry::Agents::new(
        installation.settings.clone(),
        installation.data.clone(),
        id,
        BTreeMap::from([(installation.name.clone(), of)]),
    );
    let agents = Arc::new(Agents {
        shared: shared.clone(),
        real,
    });
    let outside = carry::outside::Check::of(&installation.settings, &installation.beside.cost)?;
    let carriers = Carriers {
        agents: agents.clone(),
        acts: Arc::new(Acts { shared: shared.clone() }),
        tools: Arc::new(Calls {
            shared: shared.clone(),
            rules: (case.results.as_ref())
                .and_then(|name| with.rules.get(name))
                .cloned()
                .zip(with.tests.cloned()),
            query: carry::Querying::new(
                &installation.settings.query,
                installation.beside.tools.get(carry::QUERY),
                &installation.data,
                store::SCHEMA,
            )
            .ok()
            .map(Arc::new),
        }),
        slots: 1,
        outside: Arc::new(outside),
        host: None,
    };
    let store = Store::open(installation.data.join(store::FILE)).map_err(|e| format!("the store: {e:?}"))?;
    let opened = Service::open(Box::new(store), vec![flow.clone()], carriers, Arc::new(SystemClock));
    let (service, inbox) = opened.map_err(|fault| format!("the engine cannot start: {fault}"))?;
    let mut building = Building {
        built: Built::default(),
        installation,
        work: WorkNumber(0),
        run: RunId(0),
        service,
        inbox,
        notes,
        agents,
        shared,
        first_only: with.first_only,
        unanswered: None,
    };
    building.work = building.add(flow, case, with.item)?;
    building.history(flow, case, &order, with.commits).await?;
    Ok(building)
}

/// The file in the installation's data directory the stand-in for the agents' program reads what it returns from.
pub const STAND_IN: &str = "stand-in.json";

/// How the history and the cell's round stand, as the carriers have said it.
#[derive(Default)]
struct Heard {
    /// The run of each history round handed in, by its place in the history.
    handed: BTreeMap<RunId, usize>,
    /// The rounds held, which nothing of the history gives, not stopped yet.
    held: Vec<(CellId, RunId)>,
    /// The rounds given to the agents' program, and those of them that have ended.
    real: Vec<(CellId, RunId, usize)>,
    ended: Vec<RunId>,
    /// A call made that the case holds no result for.
    called: Option<String>,
    /// Why a round of the history could not be handed in.
    not_handed: Option<String>,
}

impl Heard {
    fn take(&mut self, note: Note) {
        match note {
            Note::Handed { index, run } => {
                self.handed.insert(run, index);
            }
            Note::NotHanded { index, why } => self.not_handed = Some(format!("history {}: {why}", index + 1)),
            Note::Held { cell, run } => self.held.push((cell, run)),
            Note::Stopped { run } => self.held.retain(|(_, r)| *r != run),
            Note::Real { cell, run, number } => self.real.push((cell, run, number)),
            Note::Called { tool } => self.called = Some(tool),
        }
    }

    /// Whether a round runs on the agents' program now.
    fn running(&self) -> bool {
        (self.real.iter()).any(|(_, run, _)| !self.ended.contains(run))
    }
}

impl Building {
    /// Adds the case's item, its first background written, and gives its work's number.
    fn add(&mut self, flow: &Compiled, case: &Case, item: &cases::written::Item) -> Result<WorkNumber, String> {
        let (reply, mut answer) = oneshot::channel();
        let item = NewItem {
            title: item.title.clone(),
            level: self.installation.settings.items.default_level.clone(),
            project: self.installation.name.clone(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            flow: flow.name.clone(),
            writes: vec![FirstWrite {
                block: background(flow)?,
                content: item.background.clone(),
                value: None,
            }],
        };
        self.service
            .take(Message::Add { item, reply })
            .map_err(|f| f.to_string())?;
        let added = answer
            .try_recv()
            .map_err(|_| format!("{}: the item was not added", case.id))?;
        added.map(|(_, work)| work).map_err(|refusal| refusal.to_string())
    }

    /// Puts an act of the history in through the loop.
    fn put(&mut self, flow: &Compiled, act: &Act, commits: &BTreeMap<String, String>) -> Result<(), String> {
        let act = match act {
            Act::Background(text) => service::Act::Write {
                block: background(flow)?,
                content: text.clone(),
                value: None,
            },
            Act::Words { to, text } => service::Act::Say {
                to: To::Cell(to.clone()),
                words: text.clone(),
            },
            Act::Answer { question, option } => {
                let kept = self.service.driver().work(self.work).ok_or("the work is not kept")?;
                let asked = (kept.work.records.questions.iter()).rev().find(|q| &q.name == question);
                let version = asked.ok_or_else(|| format!("no question {question} was asked"))?.at;
                service::Act::Answer {
                    question: question.clone(),
                    version,
                    option: *option,
                }
            }
            Act::NewSession(line) => service::Act::NewSession { line: line.clone() },
            Act::Main(name) => {
                let id = commits
                    .get(name)
                    .ok_or_else(|| format!("the made project has no commit {name}"))?;
                made::git(&self.installation.repo, &["update-ref", "refs/heads/main", id.as_str()])?;
                let content = main_says(flow, id);
                let source = carry::MAIN.to_string();
                let project = self.installation.name.clone();
                return (self.service.take(Message::Outside {
                    project,
                    source,
                    content,
                }))
                .map_err(|f| f.to_string());
            }
        };
        let (reply, mut answer) = oneshot::channel();
        let work = self.work;
        self.service
            .take(Message::Act { work, act, reply })
            .map_err(|f| f.to_string())?;
        match answer.try_recv() {
            Ok(Err(refusal)) => Err(refusal.to_string()),
            _ => Ok(()),
        }
    }

    /// Lets the work on, puts the history in as its order says, and takes what happens until the cell's first round
    /// ends; then reads what the cell was sent.
    async fn history(
        &mut self,
        flow: &Compiled,
        case: &Case,
        order: &Order,
        commits: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        for act in &order.start {
            self.put(flow, act, commits)?;
        }
        self.put_act(service::Act::Resume)?;
        let mut heard = Heard::default();
        let first = loop {
            let wait = if heard.running() { RUNNING } else { QUIET };
            let message = match tokio::time::timeout(wait, self.inbox.recv()).await {
                Ok(Some(message)) => message,
                Ok(None) => return Err("the engine's loop ended".into()),
                Err(_) => return Err(stalled(case, &heard)),
            };
            while let Ok(note) = self.notes.try_recv() {
                heard.take(note);
            }
            let ended = match &message {
                Message::Happened {
                    event: Event::Returned { run, .. } | Event::Failed { run, .. },
                    ..
                } => Some(*run),
                _ => None,
            };
            let index = ended.and_then(|run| heard.handed.get(&run).copied());
            let of_cell = |run: RunId| (heard.real.iter()).any(|(cell, r, _)| *cell == case.cell && *r == run);
            if self.first_only && ended.is_some_and(of_cell) {
                // Nothing the engine gives once the cell's first round ends is carried out.
                self.shared.over();
            }
            if let Some(i) = index {
                for act in &order.rounds[i].before {
                    self.put(flow, act, commits)?;
                }
            }
            self.service.take(message).map_err(|f| f.to_string())?;
            if let Some(i) = index {
                for act in &order.rounds[i].after {
                    self.put(flow, act, commits)?;
                }
            }
            while let Ok(note) = self.notes.try_recv() {
                heard.take(note);
            }
            if let Some(why) = heard.not_handed.take() {
                return Err(format!("{}: {why}", case.id));
            }
            if let (Some(index), Some(run)) = (index, ended) {
                self.sent_back(case, index, run)?;
            }
            if let Some(run) = ended {
                heard.ended.push(run);
            }
            let first = (heard.real.iter())
                .find(|(cell, _, _)| *cell == case.cell)
                .map(|(_, run, _)| *run);
            let Some(first) = first else {
                if let Some(tool) = &heard.called {
                    return Err(format!("{}: a call of {tool} before its cell was entered", case.id));
                }
                continue;
            };
            if self.first_only && heard.ended.contains(&first) {
                break first;
            }
            // The case ends where the cell's run asked for a call the case holds no result for, where the engine
            // went on to another cell, or where nothing of the work is under way or waits for a result.
            if heard.called.is_some() || !heard.held.is_empty() || (!heard.running() && self.at_rest()) {
                self.shared.over();
                break first;
            }
        };
        self.run = first;
        self.unanswered = heard.called.take();
        self.built = self.read(flow, case, &heard)?;
        Ok(())
    }

    /// Whether nothing of the work is under way or waits for a result: every round begun has ended, and every call
    /// the engine made has its result.
    fn at_rest(&self) -> bool {
        let Some(kept) = self.service.driver().work(self.work) else {
            return true;
        };
        let records = &kept.work.records;
        // A call a person gives the result of, as the owner's word at a limit, waits for nobody here.
        let engine_calls = records.called.iter().filter(|c| c.on.is_none());
        let done = |c: &&decide::Called| records.done.iter().any(|d| d.call == c.call);
        records.rounds.len() == records.returned.len() && engine_calls.clone().all(|c| done(&c))
    }

    /// What the case's cell did, from its first round on, as the work's records keep it.
    pub fn did(&self, flow: &Compiled, case: &Case) -> Result<cases::judged::Did, String> {
        let kept = self.service.driver().work(self.work).ok_or("the work is not kept")?;
        Ok(cases::judged::did(flow, kept.work, &case.cell, self.run))
    }

    /// Each round of the case's cell, from its first on: its run and number, what it was told, what it took, and the
    /// session it ran in.
    pub fn rounds(&self, flow: &Compiled, case: &Case) -> Result<Vec<Rounded>, String> {
        let kept = self.service.driver().work(self.work).ok_or("the work is not kept")?;
        let spec = (flow.cells.iter())
            .find(|c| c.id == case.cell)
            .ok_or("the cell is not in the flow")?;
        let line = spec.line.clone().unwrap_or_default();
        let id = carry::session::installation(&self.installation.data)?;
        let mut out = Vec::new();
        let runs = (kept.work.records.started.iter()).filter(|s| s.cell == case.cell && s.run >= self.run);
        for started in runs {
            for ((run, number), spent) in kept.work.spent.iter().filter(|((r, _), _)| *r == started.run) {
                let told = self.agents.told(self.work, *run, *number)?;
                let message = told.tries.first().map(|t| t.message.clone()).unwrap_or_default();
                let at = work::sessions::start(kept.work, *run, *number);
                let session = work::sessions::number_at(kept.work, &line, at);
                out.push(Rounded {
                    run: *run,
                    number: *number,
                    message,
                    back: format!("{:?}", kept.work.back.get(&(*run, *number))),
                    usd_micros: spent.usd_micros,
                    tokens: spent.input_tokens + spent.cached_tokens + spent.output_tokens,
                    session: carry::session::session(id, self.work, &line, session).to_string(),
                });
            }
        }
        Ok(out)
    }

    fn put_act(&mut self, act: service::Act) -> Result<(), String> {
        let (reply, mut answer) = oneshot::channel();
        let work = self.work;
        self.service
            .take(Message::Act { work, act, reply })
            .map_err(|f| f.to_string())?;
        match answer.try_recv() {
            Ok(Err(refusal)) => Err(refusal.to_string()),
            _ => Ok(()),
        }
    }

    /// Refuses the case where the engine sent back the history's round of this place, the run's: the history does not
    /// hold for the flow.
    fn sent_back(&self, case: &Case, index: usize, run: RunId) -> Result<(), String> {
        let kept = self.service.driver().work(self.work).ok_or("the work is not kept")?;
        let back = (kept.work.given.iter()).find_map(|((r, _), given)| match given {
            work::Given::SentBack(unmet) if *r == run => Some(unmet.clone()),
            _ => None,
        });
        match back {
            Some(unmet) => Err(format!(
                "{}: history {}: the engine sent its round back: {unmet:?}",
                case.id,
                index + 1
            )),
            None => Ok(()),
        }
    }

    /// What the cell was sent in its first round, and the situation it was entered in, as the work's records and the
    /// agents' carrier keep them.
    fn read(&self, flow: &Compiled, case: &Case, heard: &Heard) -> Result<Built, String> {
        let kept = self.service.driver().work(self.work).ok_or("the work is not kept")?;
        let spec = (flow.cells.iter())
            .find(|c| c.id == case.cell)
            .ok_or("the cell is not in the flow")?;
        let line = spec.line.clone().unwrap_or_default();
        let told = |run: RunId, number: usize| -> Result<String, String> {
            let told = self.agents.told(self.work, run, number)?;
            let first = told.tries.into_iter().next().ok_or("the round kept no message")?;
            match first.cut {
                None => Ok(first.message),
                Some(_) => Err("the round's message was kept cut".into()),
            }
        };
        let message = told(self.run, 1)?;
        let reading = (heard.real.iter())
            .find(|(cell, _, _)| *cell != case.cell)
            .map(|(_, run, number)| told(*run, *number))
            .transpose()?;
        let at = work::sessions::start(kept.work, self.run, 1);
        let session = work::sessions::number_at(kept.work, &line, at);
        let system = (self.agents.system(self.work, &line, session)?).ok_or("the session kept no system prompt")?;
        let ran_by = decide::told(&flow.flow, &kept.work.records, self.run)
            .map(|t| t.ran_by)
            .unwrap_or_default();
        let new = spec.ran(&ran_by).map(|w| w.name.clone()).collect();
        let records = &kept.work.records;
        let of_cell = |run: &RunId| (records.started.iter()).any(|s| s.run == *run && s.cell == case.cell);
        let ran_before = (records.finished.iter()).any(|f| f.run != self.run && of_cell(&f.run));
        Ok(Built {
            system,
            reading,
            message,
            new,
            ran_before,
        })
    }

    /// Where the case is refused because the cell was entered otherwise than it says: what ran it, and whether it ran
    /// before.
    pub fn entered_as_said(&self, case: &Case) -> Result<(), String> {
        let mut said = case.new.clone();
        said.sort();
        let mut was = self.built.new.clone();
        was.sort();
        let mut wrong = Vec::new();
        if said != was {
            wrong.push(format!("the case says {said:?} ran it, and {was:?} did"));
        }
        if case.ran_before != self.built.ran_before {
            wrong.push(match case.ran_before {
                true => "the case says it ran before, and it did not".to_string(),
                false => "the case says it never ran, and it did".to_string(),
            });
        }
        match wrong.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "{}: {} was entered otherwise: {}",
                case.id,
                case.cell,
                wrong.join("; ")
            )),
        }
    }

    /// The installation's directory, removed once what the try gave is kept.
    pub fn remove(self) -> Result<(), String> {
        let Building { installation, .. } = self;
        let dir = installation
            .data
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(installation.data);
        made::remove(&dir)
    }
}

/// Why the work stopped with nothing more happening: a round the history does not give, held, or nothing at all.
fn stalled(case: &Case, heard: &Heard) -> String {
    match heard.held.first() {
        Some((cell, _)) => format!(
            "{}: the walk entered {cell}, which the history does not give there, and nothing stopped it",
            case.id
        ),
        None => format!("{}: the work came to rest before {} was entered", case.id, case.cell),
    }
}

/// What the block of main says as main's head is this commit, in the words its definition gives, as the watch of
/// main writes it.
fn main_says(flow: &Compiled, head: &str) -> String {
    let words = (flow.blocks.values())
        .find(|block| block.writers.outside.as_deref() == Some(carry::MAIN))
        .and_then(|block| block.writes.get("text").map(str::to_string))
        .unwrap_or_default();
    flow::agents::fill(&words, &[("main", made::MAIN), ("head", head)])
}

/// The directory a try of a case is built in, under the run's.
pub fn try_dir(run: &Path, case: &str, number: usize) -> PathBuf {
    run.join(format!("{case}-{number}"))
}
