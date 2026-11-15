//! The player of a script (design document `testing`, 14): it plays a script whose agents are all stood in for on an
//! engine made as `wf-agents run --script` makes it, its carriers following the script, in this process, on the clock
//! of the runtime it runs on, whose time the caller pauses so that it passes as the engine waits: a script of rounds of
//! seconds plays in milliseconds, and an act given at a round under way is given at that step.
//!
//! It adds the script's items in order, lets on each that is not held, and takes what the loop is sent one thing at a
//! time ([`Stepper`]); before each, it gives each item's next act whose state the engine shows (module
//! `script::items`), stopping the engine and opening it again on its store where an act says so, and moving main once
//! every item with an act that moves it still to give stands at that act's state, so that the one write the engine
//! makes into every going work of the project is each work's own. It ends once every item stands at its path's end
//! with no act left to give, once each that does not has left its path for good, or once a bound of things taken or of
//! the clock's time has passed; then each item's work is judged against its path (module `judge`), and each act whose
//! state never came is named, as is each the engine refused.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use decide::Seq;
use driver::{Asked, FirstWrite, NewItem};
use flow::Compiled;
use flow::agents::Agents;
use store::{ItemChange, ItemNumber, WorkNumber};
use tokio::sync::oneshot;
use work::Time;

use crate::judge::{self, Added, Judged, Shown};
use crate::script::Script;
use crate::script::items::{ActScript, Does, LEVEL, Step};
use crate::{Act, Clock, Fault, Inbox, Message, Refusal, Service, TokioClock};

/// The loop run in this process one thing at a time, on the clock of the runtime it runs on: what a play takes, and
/// what the service's own tests drive.
pub struct Stepper {
    service: Service,
    inbox: Inbox,
}

impl Stepper {
    /// The loop as it was opened, with its channel.
    pub fn new((service, inbox): (Service, Inbox)) -> Stepper {
        Stepper { service, inbox }
    }

    /// The process stopped and started again: the loop opened anew, with its channel, in place of the one before,
    /// whose carriers' answers no longer reach it.
    pub fn reopen(&mut self, (service, inbox): (Service, Inbox)) {
        self.service = service;
        self.inbox = inbox;
    }

    pub fn service(&self) -> &Service {
        &self.service
    }

    /// The next thing sent to the loop, once it comes, within this much of the runtime's time; none where nothing
    /// comes by then. Where the runtime's time is paused, it passes at once while nothing is under way.
    pub async fn next(&mut self, within: Duration) -> Option<Message> {
        tokio::time::timeout(within, self.inbox.recv()).await.ok().flatten()
    }

    /// Takes it, as the loop takes what comes to it.
    pub fn take(&mut self, message: Message) -> Result<(), Fault> {
        self.service.take(message)
    }
}

/// When a play ends at the latest: once it has taken this many things, or this much of the clock's time has passed
/// since its start, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub steps: usize,
    pub ms: u64,
}

/// What came of a play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Played {
    /// Each item, in the script's order.
    pub items: Vec<ItemPlayed>,
    /// Where every item went as the script says and nothing stopped the loop, each of the script's round entries that
    /// no round of the play took, in words, as [`Script::untaken`] says it: in a play that went otherwise, a round not
    /// taken says only where it stopped.
    pub untaken: Vec<String>,
    /// How many things the loop took.
    pub steps: usize,
    /// What stopped the loop, where something did, in words.
    pub fault: Option<String>,
}

/// What came of one item of a play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemPlayed {
    pub work: WorkNumber,
    /// How its work's records stand against its path.
    pub judged: Judged,
    /// Its acts whose state never came, by their places among its acts, counted from one.
    pub unreached: Vec<usize>,
    /// Its acts the engine refused, by their places, counted from one, with why.
    pub refused: Vec<(usize, String)>,
}

impl Played {
    /// Whether the play went as its script says: every item went as it says, and nothing stopped the loop.
    pub fn took(&self) -> bool {
        self.fault.is_none() && self.items.iter().all(ItemPlayed::took)
    }

    /// What stopped the loop, in a line, where something did.
    pub fn stopped(&self) -> Option<String> {
        (self.fault.as_ref()).map(|fault| format!("The loop stopped: {fault}"))
    }
}

impl ItemPlayed {
    /// Whether the item went as the script says: its work took its path, and every act of it was given and taken.
    pub fn took(&self) -> bool {
        self.judged == Judged::Took && self.unreached.is_empty() && self.refused.is_empty()
    }

    /// What came of the item, at this place among the script's items, in one line: how its work's records stand
    /// against its path, and each act of it not given or refused.
    pub fn line(&self, place: usize) -> String {
        let mut line = format!("items[{place}], W{}: {}", self.work.0, self.judged);
        for act in &self.unreached {
            line.push_str(&format!("; its act {act} was not given: its state never came"));
        }
        for (act, why) in &self.refused {
            line.push_str(&format!("; its act {act} was refused: {why}"));
        }
        line
    }
}

/// What came of the play: a line for each item, one for each round entry no round took, and one for what stopped the
/// loop.
impl fmt::Display for Played {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, item) in self.items.iter().enumerate() {
            writeln!(f, "{}", item.line(k))?;
        }
        for untaken in &self.untaken {
            writeln!(f, "{untaken}")?;
        }
        if let Some(stopped) = self.stopped() {
            writeln!(f, "{stopped}")?;
        }
        Ok(())
    }
}

/// The engine a play is played on, opened on the clock it is given over a store that holds nothing of another play, its
/// carriers following the script: as `wf-agents run --script` opens it (crate `wf-agents`), and opened again where an
/// act stops it; or why it cannot be opened, in words.
pub type Open<'a> = dyn Fn(Arc<dyn Clock>) -> Result<(Service, Inbox), String> + 'a;

/// Plays the script on these flows, with what is written beside them, on the engine `open` opens, its items added to
/// the project of this name, its clock starting at the time the script names as its start, or at `start` where it names
/// none, within the bounds; or says what is wrong with the script for them, each in words with where it is, as its
/// check says, and each item with no path, since a play judges what it plays. It runs on the runtime it is called on,
/// whose time the caller pauses.
pub async fn play(
    script: &Script,
    flows: &[Compiled],
    words: &Agents,
    open: &Open<'_>,
    project: &str,
    start: Time,
    bounds: Bounds,
) -> Result<Played, Vec<String>> {
    let mut faults = script.check(flows, words);
    if script.items.is_empty() {
        faults.push("items: a play adds an item at least, and judges each".to_string());
    }
    for (k, item) in script.items.iter().enumerate() {
        if item.path.is_empty() {
            faults.push(format!(
                "items[{k}].path: a played item says the path its work must take"
            ));
        }
    }
    if !faults.is_empty() {
        return Err(faults);
    }
    // Every time the script names is an offset from its start, which places the clock where it names one.
    let start = script.start.map_or(start, Time);
    let clock = Arc::new(TokioClock::new(start));
    let open = || -> Result<(Service, Inbox), String> {
        let clock: Arc<dyn Clock> = clock.clone();
        open(clock)
    };
    let mut stepper = Stepper::new(open().map_err(|fault| vec![fault])?);
    let mut play = Play {
        script,
        project,
        stepper: &mut stepper,
        items: Vec::new(),
        works: Vec::new(),
        next: vec![0; script.items.len()],
        refused: vec![Vec::new(); script.items.len()],
        judged: vec![None; script.items.len()],
    };
    for item in &script.items {
        let depends_on = (item.depends_on.iter()).map(|place| play.items[place - 1]).collect();
        let new = NewItem {
            title: item.title.clone(),
            level: LEVEL.to_string(),
            project: project.to_string(),
            depends_on,
            tags: Vec::new(),
            flow: item.flow.clone(),
            writes: (item.writes.iter())
                .map(|write| FirstWrite {
                    block: write.block.as_str().into(),
                    content: write.content.clone(),
                    value: write.value.clone(),
                })
                .collect(),
        };
        let (reply, answer) = oneshot::channel();
        let added = play
            .send(Message::Add { item: new, reply }, answer)
            .map_err(|why| vec![why])?;
        let (item, work) = added.map_err(|refusal| vec![refusal.to_string()])?;
        play.items.push(item);
        play.works.push(work);
    }
    for (k, item) in script.items.iter().enumerate() {
        if !item.held {
            let work = play.works[k];
            play.act(work, Act::Resume)
                .map_err(|why| vec![format!("items[{k}]: it could not be let on: {why}")])?;
        }
    }
    let until = Time(start.0.saturating_add(bounds.ms));
    let mut steps = 0;
    let mut fault = None;
    loop {
        if let Err(stopped) = play.give(&open) {
            fault = Some(stopped);
            break;
        }
        if play.done() || steps >= bounds.steps {
            break;
        }
        let now = clock.now().unwrap_or(until);
        let within = Duration::from_millis(until.0.saturating_sub(now.0));
        let Some(message) = play.stepper.next(within).await else {
            break;
        };
        steps += 1;
        if let Err(stopped) = play.stepper.take(message) {
            fault = Some(stopped.to_string());
            break;
        }
    }
    let items = (script.items.iter().enumerate())
        .map(|(k, item)| ItemPlayed {
            work: play.works[k],
            judged: play.judge(k),
            unreached: (play.next[k]..item.acts.len()).map(|j| j + 1).collect(),
            refused: play.refused[k].clone(),
        })
        .collect::<Vec<_>>();
    let mut untaken = Vec::new();
    if fault.is_none() && items.iter().all(ItemPlayed::took) {
        let driver = play.stepper.service().driver();
        let kept: Vec<_> = (play.works.iter())
            .filter_map(|work| driver.work(*work).map(|kept| (work.0, kept)))
            .collect();
        let works: Vec<(&str, u64, &decide::Records)> = (kept.iter())
            .map(|(work, kept)| (kept.flow.name.as_str(), *work, &kept.work.records))
            .collect();
        untaken = script.untaken(&works);
    }
    Ok(Played {
        items,
        untaken,
        steps,
        fault,
    })
}

/// A play under way: the loop, and how far each item's acts have gone.
struct Play<'a> {
    script: &'a Script,
    project: &'a str,
    stepper: &'a mut Stepper,
    /// The number of each item added, in the script's order.
    items: Vec<ItemNumber>,
    /// The work of each.
    works: Vec<WorkNumber>,
    /// The place of each item's next act to give.
    next: Vec<usize>,
    /// Each item's acts the engine refused, by their places counted from one, with why.
    refused: Vec<Vec<(usize, String)>>,
    /// How each item's work was last judged, with the next place in its records and what the engine showed of it then:
    /// a work is judged again only once either changed.
    judged: Vec<Option<(Seq, Shown, Judged)>>,
}

impl Play<'_> {
    /// Gives the loop a message from someone who is not the loop, and gives its answer, which comes at once.
    fn send<T>(&mut self, message: Message, mut answer: oneshot::Receiver<T>) -> Result<T, String> {
        self.stepper.take(message).map_err(|fault| fault.to_string())?;
        answer.try_recv().map_err(|_| "the loop did not answer".to_string())
    }

    /// Does what the owner does to the work; the loop's refusal said, where it refused.
    fn act(&mut self, work: WorkNumber, act: Act) -> Result<(), String> {
        let (reply, answer) = oneshot::channel();
        let acted = self.send(Message::Act { work, act, reply }, answer)?;
        acted.map(|_| ()).map_err(|refusal| refusal.to_string())
    }

    /// Whether the play can end before its bounds: every item stands at its path's end with no act left to give, or
    /// has left its path for good.
    fn done(&mut self) -> bool {
        let script = self.script;
        (0..script.items.len()).all(|k| match self.judge(k) {
            Judged::Took => self.next[k] == script.items[k].acts.len(),
            Judged::Left { .. } => true,
            Judged::Short { .. } => false,
        })
    }

    /// How the item's work stands against its path now: judged again only where its records grew or what the engine
    /// shows of it changed since it was last judged.
    fn judge(&mut self, k: usize) -> Judged {
        let item = &self.script.items[k];
        let driver = self.stepper.service().driver();
        let work = self.works[k];
        let (Some(kept), Some(shown)) = (driver.work(work), Shown::of(driver, work)) else {
            return Judged::Short {
                step: 1,
                said: (item.path.first().and_then(|step| step.step().ok()))
                    .map(|step| step.to_string())
                    .unwrap_or_default(),
                shown: Some("no work: the engine holds none of this number".to_string()),
            };
        };
        let tick = kept.work.tick();
        if let Some((at, then, judged)) = &self.judged[k]
            && *at == tick
            && *then == shown
        {
            return judged.clone();
        }
        let path: Vec<Step> = item.path.iter().filter_map(|step| step.step().ok()).collect();
        let added = Added {
            writes: item.writes.len(),
            held: item.held,
        };
        let judged = judge::judge(kept.flow, kept.work, added, &path, &shown);
        self.judged[k] = Some((tick, shown, judged.clone()));
        judged
    }

    /// The states the engine shows the item's work at now.
    fn states(&self, k: usize) -> Vec<Step> {
        let driver = self.stepper.service().driver();
        let work = self.works[k];
        match (driver.work(work), Shown::of(driver, work)) {
            (Some(kept), Some(shown)) => judge::states(kept.work, &shown),
            _ => Vec::new(),
        }
    }

    /// Whether the item's next act, of this place, waits for a state the work stands at now.
    fn due(&self, k: usize, j: usize) -> bool {
        let act = &self.script.items[k].acts[j];
        let Ok(at) = act.at.step() else {
            return false;
        };
        self.states(k).iter().any(|state| at.matches(state))
    }

    /// Gives each item's next act whose state the work stands at, again and again until none is due; says what stopped
    /// the loop, where something did, or the engine could not be opened again.
    fn give(&mut self, open: &dyn Fn() -> Result<(Service, Inbox), String>) -> Result<(), String> {
        let script = self.script;
        loop {
            let mut given = false;
            for k in 0..script.items.len() {
                let j = self.next[k];
                let Some(act) = script.items[k].acts.get(j) else {
                    continue;
                };
                if !self.due(k, j) {
                    continue;
                }
                if act.does == Does::Main {
                    // Main moves once, for every item with an act that moves it still to give, once each stands at it.
                    let moving: Vec<usize> = (0..script.items.len())
                        .filter(|&m| {
                            script.items[m].acts[self.next[m]..]
                                .iter()
                                .any(|a| a.does == Does::Main)
                        })
                        .collect();
                    let all = moving.iter().all(|&m| {
                        let next = &script.items[m].acts[self.next[m]];
                        next.does == Does::Main && self.due(m, self.next[m])
                    });
                    if !all {
                        continue;
                    }
                    let message = Message::Outside {
                        project: self.project.to_string(),
                        source: driver::MAIN.to_string(),
                        content: act.content.clone().unwrap_or_default(),
                    };
                    self.stepper.take(message).map_err(|fault| fault.to_string())?;
                    for m in moving {
                        self.next[m] += 1;
                    }
                    given = true;
                    continue;
                }
                self.next[k] += 1;
                given = true;
                if let Err(why) = self.carry_out(k, act, open)? {
                    self.refused[k].push((j + 1, why));
                }
            }
            if !given {
                return Ok(());
            }
        }
    }

    /// Carries out the act of the item at place `k`: the loop's refusal, where it refused, inside what stopped the loop
    /// or kept the engine from opening again.
    fn carry_out(
        &mut self,
        k: usize,
        act: &ActScript,
        open: &dyn Fn() -> Result<(Service, Inbox), String>,
    ) -> Result<Result<(), String>, String> {
        let work = self.works[k];
        let text = |text: &Option<String>| text.clone().unwrap_or_default();
        let owners = match act.does {
            Does::Write => Some(Act::Write {
                block: text(&act.block).as_str().into(),
                content: text(&act.content),
                value: act.value.clone(),
            }),
            Does::LetGo => Some(Act::LetGo {
                cell: text(&act.cell).as_str().into(),
            }),
            Does::Pause => Some(Act::Pause),
            Does::Resume => Some(Act::Resume),
            Does::Cancel => Some(Act::Cancel { why: act.why.clone() }),
            Does::Say => Some(Act::Say {
                to: match &act.line {
                    Some(line) => engine::spoken::To::Line(line.clone()),
                    None => engine::spoken::To::Question(text(&act.question)),
                },
                words: text(&act.words),
            }),
            Does::Answer => {
                let question = text(&act.question);
                let driver = self.stepper.service().driver();
                let version = (driver.work(work).into_iter())
                    .flat_map(|kept| &kept.work.records.questions)
                    .filter(|q| q.name == question)
                    .map(|q| q.at)
                    .max();
                match version {
                    Some(version) => Some(Act::Answer {
                        question,
                        version,
                        option: act.option.unwrap_or(1),
                        why: act.why.clone(),
                    }),
                    None => return Ok(Err(format!("the work has no question named {question}"))),
                }
            }
            Does::NewSession => Some(Act::NewSession { line: text(&act.line) }),
            _ => None,
        };
        if let Some(owners) = owners {
            return Ok(self.act(work, owners));
        }
        let item = self.items[k];
        let other = |place: Option<usize>| self.items[place.unwrap_or(1) - 1];
        let changed = match act.does {
            Does::Title => Some(Asked::Item(ItemChange::Title(text(&act.title)))),
            Does::DependsOn => Some(Asked::Item(ItemChange::DependsOn(other(act.item)))),
            Does::NoLongerDependsOn => Some(Asked::Item(ItemChange::NoLongerDependsOn(other(act.item)))),
            Does::Tag => Some(Asked::Item(ItemChange::Tagged(text(&act.tag)))),
            Does::Untag => Some(Asked::Item(ItemChange::Untagged(text(&act.tag)))),
            Does::Reopen => Some(Asked::Reopen),
            _ => None,
        };
        if let Some(changed) = changed {
            let (reply, answer) = oneshot::channel();
            let message = Message::Change {
                why: text(&act.why),
                changes: vec![(item, changed)],
                reply,
            };
            let made = self.send(message, answer)?;
            return Ok(made.map(|_| ()).map_err(|refusal: Refusal| refusal.to_string()));
        }
        match act.does {
            Does::PauseEverything | Does::ResumeEverything => {
                let (reply, answer) = oneshot::channel();
                let paused = act.does == Does::PauseEverything;
                let set = self.send(Message::Everything { paused, reply }, answer)?;
                Ok(set.map_err(|refusal| refusal.to_string()))
            }
            Does::Restart => {
                self.stepper.reopen(open()?);
                Ok(Ok(()))
            }
            _ => Ok(Ok(())),
        }
    }
}
