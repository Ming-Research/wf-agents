//! The engine's own acts, as the rounds of the cells they carry out (`CellSpec.act`): `start` makes the work's branch
//! and worktree, `check` checks the work before its merge, `merge` merges the commit the check passed, and `deliver`
//! says that a deliverable item, made of the items it depends on, is delivered. An act hands in the cell's outputs with
//! what it did, and only the check writes a choice ([`faults`] says what a flow needs for them, [`main_faults`] what
//! the watch of main needs, [`word_faults`] what the merge needs of the check's words); where git or the host fails,
//! or the act cannot be done, the round fails and hands in nothing. A stopped act ends its round with nothing handed
//! in. An act takes no cost and no tokens.
//!
//! What is done in the project's local repository is done the same whatever the host; the host gives the head of
//! main, the verdict on the work's commit and the merge ([`Host`]).
//!
//! One act at a time works in the project's repository: two at once would race over its worktrees and its main branch.
//! The host's verdict is waited for beside what other acts do. `deliver` does nothing outside the engine: the items it
//! is made of were each merged by their own works, and what it writes is said from its item.
//!
//! Once a work has ended and nothing of it is under way, the acts let go of what they made for it: its worktree is
//! taken away, and the host closes what it holds open for a work that did not merge, its pull request; the branch
//! stays, with the work's commits. What fails is tried again, waiting longer each time, until it is done.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use decide::{BlockId, End, Finished, RunId};
use engine::Came;
use flow::Compiled;
use service::{Release, ReleaseReply, Round, RoundReply, Rounds, Teller};
use store::{ItemNumber, WorkNumber};
use tokio::sync::{Mutex, Notify};
use work::{Handed, Question, Spent, Work};

#[cfg(test)]
mod ends;

use crate::git::{self, Place, Started};
use crate::held::{Failing, Held, Stop};
use crate::host::{Branch, Host};
use crate::settings::Project;
use crate::{protected, tree};

use flow::agents::{AskProtected, fill};

/// The acts' words, and those of the check, as `flow::agents` reads them.
pub use flow::agents::{Acts as Words, Check as CheckWords, CheckValues, Deliver};

/// What the engine writes as the flows of this repository say it, for the tests.
#[cfg(test)]
pub(crate) fn writes() -> flow::agents::Writes {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    flow::agents::writes(&root).expect("what the engine writes is read")
}

/// The acts' words as the flows of this repository write them, for the tests.
#[cfg(test)]
pub(crate) fn words() -> Words {
    writes().acts
}

/// What the check before the merge found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The commit checked: the head of the work's branch.
    pub commit: String,
    /// The head of the main branch it was checked against.
    pub main: String,
    /// What failed, with the end of what it said; none where everything passed.
    pub failed: Option<String>,
    /// What judged it, where the host says.
    pub by: Option<String>,
    /// Whether the engine made the commit, merging main into the branch for the check.
    pub took_in: bool,
    /// What the host says of the checks beside its verdict.
    pub notes: Vec<String>,
    /// What the engine asks the owner of the work's change under the paths the project protects, where it asks
    /// anything ([`crate::protected`]).
    pub asks: Option<Question>,
}

/// The paths the project protects, and the words of the question the engine asks of a change to them: none protected
/// where the acts were not told of any.
#[derive(Clone, Debug, Default)]
pub struct Protecting {
    pub paths: Vec<String>,
    pub words: Option<AskProtected>,
}

/// The acts, done in the project's repository with the works' worktrees in the data directory, on this host.
pub struct Acts<H> {
    inner: Arc<Inner<H>>,
}

struct Inner<H> {
    host: H,
    project: Project,
    /// What the acts write, as `agents/acts/` beside the flows says.
    words: Words,
    data: PathBuf,
    /// The installation's part of the names of the works' branches and worktrees.
    tag: String,
    /// Held while an act or the watch of main works in the repository.
    repository: Mutex<()>,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
    /// Told after each merge, so that the watch of main looks at once.
    merged: Notify,
    protecting: Protecting,
}

/// What came of an act: what it hands in, with what the engine asks the owner beside it.
enum Done {
    HandsIn(Vec<Handed>, Vec<Question>),
    Stopped,
}

impl<H: Host> Acts<H> {
    /// The acts on this host in this project, with this data directory, which is an absolute path, and the
    /// installation's tag.
    pub fn new(host: H, project: Project, data: PathBuf, tag: String, words: Words) -> Self {
        Acts {
            inner: Arc::new(Inner {
                host,
                project,
                words,
                data,
                tag,
                repository: Mutex::new(()),
                held: Held::new(),
                merged: Notify::new(),
                protecting: Protecting::default(),
            }),
        }
    }

    /// The same acts, the check holding a change to these paths until the owner lets it merge.
    pub fn protecting(self, protecting: Protecting) -> Self {
        let mut inner = Arc::into_inner(self.inner).expect("the acts are not shared yet");
        inner.protecting = protecting;
        Acts { inner: Arc::new(inner) }
    }
}

/// The source outside the works whose block the watch of main writes.
pub const MAIN: &str = "main";

/// How often the watch of main looks at its head: within a minute of another work's merge, the works whose check had
/// passed are checked again before they merge, and a look costs one fetch.
pub const WATCH: Duration = Duration::from_secs(60);

/// The watch of main: it looks at the head of main on the host, every so often and right after each merge of the
/// acts', and says what it found, for the loop to write into the block of main of each work ([`MAIN`]). A head that
/// could not be read is said on the standard error, once until it can be again.
pub struct Watch(Box<dyn FnOnce(Say) -> Watching + Send>);

/// What the watch says what it found to.
type Say = Box<dyn Fn(String) + Send + Sync>;

/// The watch under way.
type Watching = Pin<Box<dyn Future<Output = ()> + Send>>;

impl Watch {
    /// Watches until the process ends, saying each look's head to `say`.
    pub async fn run(self, say: impl Fn(String) + Send + Sync + 'static) {
        (self.0)(Box::new(say)).await;
    }
}

impl<H: Host> Acts<H> {
    /// The watch of main of these acts, looking this often.
    pub fn watch(&self, every: Duration) -> Watch {
        let inner = self.inner.clone();
        Watch(Box::new(move |say| {
            Box::pin(async move { inner.watch(every, say).await })
        }))
    }
}

/// Forgets the rounds of the work the loop has taken the end of, as its records show them.
pub(crate) fn prune(held: &Held<(WorkNumber, RunId, usize), RoundReply>, round: &Round) {
    let records = &round.kept.records;
    held.prune(|(work, run, number)| {
        *work == round.work && records.returned.iter().filter(|r| r.run == *run).count() >= *number
    });
}

impl<H: Host> Rounds for Acts<H> {
    fn give(&self, round: Round, reply: RoundReply) {
        prune(&self.inner.held, &round);
        let key = (round.work, round.run, round.number);
        let teller = reply.teller();
        let Some(mut stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let done = inner.act(&round, &teller, &mut stop).await;
            match done {
                Ok(Done::HandsIn(handed, engine)) => {
                    if let Some(reply) = inner.held.take(key) {
                        let came = Came {
                            engine,
                            ..Came::hands_in(handed)
                        };
                        reply.ended(came, Spent::default());
                    }
                }
                Ok(Done::Stopped) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::hands_in(Vec::new()), Spent::default());
                    }
                }
                Err(why) => match inner.held.fail(key) {
                    // An act of the engine's takes no cost and no tokens, failed or not.
                    Some(Failing::Fail(reply)) => reply.failed(why, Spent::default()),
                    Some(Failing::End(reply)) => reply.ended(Came::hands_in(Vec::new()), Spent::default()),
                    None => {}
                },
            }
        });
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        prune(&self.inner.held, &round);
        let key = (round.work, round.run, round.number);
        if let Some(reply) = self.inner.held.stop(key, reply) {
            reply.ended(Came::hands_in(Vec::new()), Spent::default());
        }
    }

    fn release(&self, release: Release, reply: ReleaseReply) {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            match inner.release(&release).await {
                Ok(()) => reply.released(),
                // Said to the owner as the engine's pause of the ended work; letting it on tries again.
                Err(why) => reply.failed(why),
            }
        });
    }
}

impl<H: Host> Inner<H> {
    /// Does the cell's act, and gives what it hands in.
    async fn act(&self, round: &Round, teller: &Teller, stop: &mut Stop) -> Result<Done, String> {
        let act = round.cell.act.as_deref().unwrap_or_default();
        let outputs = outputs(&round.flow, round);
        let project = &self.project;
        let words = &self.words;
        let place = git::place(&self.data, &self.tag, round.work);
        let about = about(&round.flow, &round.kept);
        let branch = Branch {
            work: round.work,
            title: &round.item.title,
            about: &about,
            place: &place,
        };
        match act {
            "start" => {
                let started = start(project, &self.host, &place, &self.repository).await?;
                tree::look(&round.flow, &round.kept, &project.tree, &started.place, teller).await;
                let content = fill(
                    &words.start.text,
                    &[
                        ("branch", &started.place.reference()),
                        ("worktree", &started.place.dir.display().to_string()),
                        ("from", &started.from),
                        ("main", &project.main),
                    ],
                );
                Ok(Done::HandsIn(prose(&outputs, &content), Vec::new()))
            }
            "check" => {
                let gate = Gate {
                    protecting: &self.protecting,
                    work: &round.kept,
                };
                match check(
                    project,
                    &self.host,
                    &branch,
                    &self.repository,
                    stop,
                    &words.check,
                    &gate,
                )
                .await?
                {
                    Some(checked) => {
                        // The check may have taken main in.
                        tree::look(&round.flow, &round.kept, &project.tree, &place, teller).await;
                        let asks = checked.asks.clone().into_iter().collect();
                        checked_in(&outputs, &checked, &project.main, &words.check).map(|h| Done::HandsIn(h, asks))
                    }
                    None => Ok(Done::Stopped),
                }
            }
            "merge" => {
                let checked = passed(&round.flow, &round.kept, &words.check)?;
                let merged = merge(&self.host, &branch, checked.as_deref(), &self.repository).await;
                // Merged or refused for a moved main, the watch looks at main at once.
                self.merged.notify_one();
                let merged = merged?;
                let content = fill(
                    &words.merge.text,
                    &[
                        ("branch", &place.reference()),
                        ("main", &project.main),
                        ("head", &merged),
                    ],
                );
                Ok(Done::HandsIn(prose(&outputs, &content), Vec::new()))
            }
            "deliver" => Ok(Done::HandsIn(
                prose(&outputs, &delivered(&round.item.depends_on, &words.deliver)),
                Vec::new(),
            )),
            other => Err(format!("the engine has no act named {other}")),
        }
    }
}

impl<H: Host> Inner<H> {
    /// Lets go of what the acts made for the work, which has ended, holding the repository: the host lets go of what it
    /// holds for it, and its worktrees are taken away. Tried once: what fails is said, and is not tried again unseen.
    async fn release(&self, release: &Release) -> Result<(), String> {
        let place = git::place(&self.data, &self.tag, release.work);
        let branch = Branch {
            work: release.work,
            title: &release.item.title,
            about: "",
            place: &place,
        };
        let _held = self.repository.lock().await;
        self.host.end(&branch, release.end == End::Merged).await?;
        git::remove(&self.project, &place).await
    }
}

impl<H: Host> Inner<H> {
    /// Looks at main's head every so often and after each merge, saying each head it reads.
    async fn watch(&self, every: Duration, say: Say) {
        let mut failing: Option<String> = None;
        loop {
            let head = {
                let _held = self.repository.lock().await;
                match git::repository_configuration(&self.project).await {
                    Ok(()) => self.host.main().await,
                    Err(why) => Err(why),
                }
            };
            match head {
                Ok(head) => {
                    failing = None;
                    say(head);
                }
                Err(why) if failing.as_ref() != Some(&why) => {
                    eprintln!("The head of {} could not be read: {why}", self.project.main);
                    failing = Some(why);
                }
                Err(_) => {}
            }
            tokio::select! {
                () = tokio::time::sleep(every) => {}
                () = self.merged.notified() => {}
            }
        }
    }
}

/// Merges the work's branch on the host, holding the repository: the host finds a merge made already, and refuses a
/// main the commit the check passed does not hold ([`Host::merge`]).
pub(crate) async fn merge(
    host: &impl Host,
    branch: &Branch<'_>,
    checked: Option<&str>,
    repository: &Mutex<()>,
) -> Result<String, String> {
    let _held = repository.lock().await;
    host.merge(branch, checked).await
}

/// Makes the work's branch from the head of main the host gives, and its worktree, holding the repository.
pub(crate) async fn start(
    project: &Project,
    host: &impl Host,
    place: &Place,
    repository: &Mutex<()>,
) -> Result<Started, String> {
    let _held = repository.lock().await;
    git::configuration(project, place).await?;
    let main = host.main().await?;
    git::start(project, place, &main).await
}

/// What the check holds a work's change under the protected paths to: the paths and the question's words, and the work
/// as it is kept, which says what the owner answered.
pub(crate) struct Gate<'a> {
    pub(crate) protecting: &'a Protecting,
    pub(crate) work: &'a Work,
}

/// Checks the work before its merge: that its worktree holds nothing uncommitted and that its branch merges into the
/// head of main the host gives with no conflict (worked out without changing anything), both found holding the
/// repository; then main's head is merged into the branch, where it does not hold it, as a commit of the engine's, so
/// that what is judged is what a merge would land; then that commit's own change under the paths the project protects
/// is empty or one the owner let merge, which fails the check otherwise, with what the engine asks of it, and fails
/// it too where it cannot be looked at ([`crate::protected`]); then the host's verdict on the commit, waited for
/// without the repository. Says why where it could not check, and gives none where it was stopped; what failed is what
/// was found, not a reason to fail.
pub(crate) async fn check(
    project: &Project,
    host: &impl Host,
    branch: &Branch<'_>,
    repository: &Mutex<()>,
    stop: &mut Stop,
    words: &CheckWords,
    gate: &Gate<'_>,
) -> Result<Option<Checked>, String> {
    let place = branch.place;
    let dir = git::worktree(place)?;
    let (commit, took_in, main) = {
        let _held = repository.lock().await;
        git::configuration(project, place).await?;
        git::on_branch(place).await?;
        let main = host.main().await?;
        let commit = git::git(&project.repo, &["rev-parse", &place.reference()]).await?;
        let found = |failed: String| Checked {
            commit: commit.clone(),
            main: main.clone(),
            failed: Some(failed),
            by: None,
            took_in: false,
            notes: Vec::new(),
            asks: None,
        };
        if git::merging(&dir).await? {
            let changes = git::uncommitted(&dir).await?;
            return Ok(Some(found(fill(&words.found.merging, &[("changes", &changes)]))));
        }
        let changes = git::uncommitted(&dir).await?;
        if !changes.is_empty() {
            return Ok(Some(found(fill(&words.found.uncommitted, &[("changes", &changes)]))));
        }
        if let Some(conflict) = git::conflicts(project, &main, &commit).await? {
            let failed = fill(
                &words.found.conflict,
                &[
                    ("main", &project.main),
                    ("head", &main),
                    ("files", &conflict.files),
                    ("kinds", &conflict.kinds),
                ],
            );
            return Ok(Some(found(failed)));
        }
        let (commit, took_in) = git::take_in(&dir, &main, project).await?;
        (commit, took_in, main)
    };
    let mut checked = Checked {
        commit,
        main,
        failed: None,
        by: None,
        took_in,
        notes: Vec::new(),
        asks: None,
    };
    // The commit's own change under the protected paths waits for the owner's yes: the host is not asked while it
    // does, nor where it cannot be looked at.
    if let Some(ask) = &gate.protecting.words {
        let paths = &gate.protecting.paths;
        let change = {
            let _held = repository.lock().await;
            protected::change_at(project, place, &checked.main, &checked.commit, paths).await
        };
        let change = match change {
            Ok(change) => change,
            Err(why) => {
                checked.failed = Some(fill(&words.found.protected_unread, &[("why", &why)]));
                return Ok(Some(checked));
            }
        };
        checked.asks = protected::asking(gate.work, ask, &change);
        if !change.files.is_empty() && !protected::let_merge(gate.work, ask, &change) {
            let files: Vec<String> = change.files.iter().map(|(_, path)| path.clone()).collect();
            let failed = fill(
                &words.found.protected,
                &[("question", &ask.name), ("files", &files.join("\n"))],
            );
            checked.failed = Some(failed);
            return Ok(Some(checked));
        }
    }
    let Some(verdict) = host.verdict(branch, &checked.commit, stop).await? else {
        return Ok(None);
    };
    checked.failed = verdict.failed;
    checked.by = verdict.by;
    checked.notes = verdict.notes;
    Ok(Some(checked))
}

/// What is wrong with the flow for the watch of main, each in words with the file that says what is written there: the
/// block of [`MAIN`] gives in its `writes` the `text` the watch fills, so that none is written empty. A flow with any of
/// these is not run, whatever carries its works out.
pub fn main_faults(flow: &Compiled) -> Vec<String> {
    (flow.blocks.iter())
        .filter(|(_, block)| block.writers.outside.as_deref() == Some(MAIN) && block.writes.get("text").is_none())
        .map(|(name, block)| {
            format!(
                "the flow {}, {}: the block {name} (from {MAIN}): its writes must give text",
                flow.name, block.file
            )
        })
        .collect()
}

/// What is wrong with the flow for the acts, each in words with the cell: the cell of `check` has one choice among its
/// outputs, and it offers the values the check writes; the cells of `start`, `merge` and `deliver` have no choice among
/// their outputs, since they have no outcome to choose. A flow with any of these is not run with the real carriers.
pub fn faults(flow: &Compiled, values: &CheckValues) -> Vec<String> {
    let (passed, failed) = (&values.passed, &values.failed);
    let mut faults = Vec::new();
    for cell in &flow.cells {
        let Some(act) = cell.act.as_deref() else {
            continue;
        };
        let outputs = flow
            .flow
            .place(&cell.id)
            .map_or(&[][..], |p| &flow.flow.cells()[p].outputs);
        let choices: Vec<(&BlockId, &Vec<String>)> = (outputs.iter())
            .filter_map(|block| Some((block, flow.blocks.get(block)?.values.as_ref()?)))
            .collect();
        let at = format!("the flow {}, cell {} (act {act})", flow.name, cell.id);
        match act {
            "check" => match choices.as_slice() {
                [(block, values)] if !(values.iter().any(|v| v == passed) && values.iter().any(|v| v == failed)) => {
                    faults.push(format!("{at}: its choice {block} must offer {passed} and {failed}"));
                }
                [_] => {}
                _ => faults.push(format!(
                    "{at}: it must have one choice among its outputs, offering {passed} and {failed}, and has {}",
                    choices.len()
                )),
            },
            _ if !choices.is_empty() => {
                let names: Vec<String> = choices.iter().map(|(b, _)| b.to_string()).collect();
                faults.push(format!(
                    "{at}: it writes no choice, and has {} among its outputs",
                    names.join(", ")
                ));
            }
            _ => {}
        }
    }
    faults
}

/// What `deliver` writes: that the item is delivered, and the items it was made of, by their numbers in order.
fn delivered(made_of: &[ItemNumber], words: &Deliver) -> String {
    let numbers: Vec<String> = made_of.iter().map(|item| item.0.to_string()).collect();
    match numbers.as_slice() {
        [] => words.none.clone(),
        [one] => fill(&words.one, &[("item", one)]),
        [first @ .., last] => fill(&words.many, &[("items", &first.join(&words.between)), ("last", last)]),
    }
}

/// The cell's outputs, each with its values where it is a choice.
fn outputs(flow: &Compiled, round: &Round) -> Vec<(BlockId, Option<Vec<String>>)> {
    let place = flow.flow.place(&round.cell.id);
    let outputs = place.map_or(&[][..], |place| &flow.flow.cells()[place].outputs);
    (outputs.iter())
        .map(|block| {
            let values = flow.blocks.get(block).and_then(|b| b.values.clone());
            (block.clone(), values)
        })
        .collect()
}

/// Each output that is not a choice, handed in with this content.
fn prose(outputs: &[(BlockId, Option<Vec<String>>)], content: &str) -> Vec<Handed> {
    (outputs.iter())
        .filter(|(_, values)| values.is_none())
        .map(|(block, _)| Handed {
            block: block.clone(),
            content: content.to_string(),
            matters: true,
            value: None,
        })
        .collect()
}

/// What the check hands in, in the words `agents/acts/check.toml` gives: its choice, passed or failed, with the commit
/// it checked on the first line and against what; and, where it failed, each other output with what failed.
fn checked_in(
    outputs: &[(BlockId, Option<Vec<String>>)],
    checked: &Checked,
    main: &str,
    words: &CheckWords,
) -> Result<Vec<Handed>, String> {
    let Some(choice) = outputs.iter().find_map(|(b, v)| v.as_ref().map(|_| b)) else {
        return Err("the check's cell has no choice to hand in whether the work passed".to_string());
    };
    let slots = [
        ("commit", checked.commit.as_str()),
        ("main", main),
        ("head", checked.main.as_str()),
    ];
    let by = (checked.by.as_ref()).map_or(String::new(), |by| fill(&words.by, &[("by", by)]));
    let said = fill(&words.checked, &[slots.as_slice(), &[("by", by.as_str())]].concat());
    let first = fill(&words.first, &slots);
    let mut after = Vec::new();
    if checked.took_in {
        after.push(fill(&words.took_in, &slots));
    }
    if !checked.notes.is_empty() {
        let notes: String = (checked.notes.iter())
            .map(|note| fill(&words.note, &[("note", note)]))
            .collect();
        after.push(fill(&words.notes, &[("notes", &notes)]));
    }
    let content = |line: String| [vec![first.clone(), line], after.clone()].concat().join("\n\n");
    let (value, outcome) = match checked.failed {
        None => (&words.values.passed, &words.passed),
        Some(_) => (&words.values.failed, &words.failed),
    };
    let mut handed = vec![Handed {
        block: choice.clone(),
        content: content(fill(outcome, &[("checked", &said)])),
        matters: true,
        value: Some(value.clone()),
    }];
    if let Some(why) = &checked.failed {
        let others = outputs
            .iter()
            .filter(|(block, values)| block != choice && values.is_none());
        handed.extend(others.map(|(block, _)| Handed {
            block: block.clone(),
            content: content(fill(&words.why, &[("checked", &said), ("why", why)])),
            matters: true,
            value: None,
        }));
    }
    Ok(handed)
}

/// The latest finished run of the flow's check, where it has one and it ran.
fn latest_check<'a>(flow: &Compiled, work: &'a Work) -> Option<&'a Finished> {
    let records = &work.records;
    (records.finished.iter())
        .filter(|f| {
            let started = records.started.iter().find(|s| s.run == f.run);
            started.is_some_and(|s| {
                flow.cells
                    .iter()
                    .any(|c| c.id == s.cell && c.act.as_deref() == Some("check"))
            })
        })
        .max_by_key(|f| f.at)
}

/// The commit the latest check wrote in its choice, where it wrote this value there.
fn checked_with(work: &Work, check: &Finished, words: &CheckWords) -> Option<String> {
    let value = words.values.passed.as_str();
    let write = check.writes.iter().find(|w| w.value.as_deref() == Some(value))?;
    let content = work.contents.get(&(write.block.clone(), check.at))?;
    commit_in(content, words)
}

/// The commit a content the check wrote says on its first line, read back after what `first` writes before `{commit}`.
/// The engine's words are read back so only where [`word_faults`] found nothing wrong with them.
fn commit_in(content: &str, words: &CheckWords) -> Option<String> {
    let line = content.lines().next()?;
    let (before, _) = words.first.split_once("{commit}")?;
    line.strip_prefix(before).map(|commit| commit.trim().to_string())
}

/// What is wrong with the check's words for the merge, which reads back from the first line of the check's choice the
/// commit it is to merge: `first`, filled as the check fills it, must give that commit back whole. The engine is not
/// run with words that do not.
pub fn word_faults(words: &CheckWords) -> Vec<String> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let first = fill(
        &words.first,
        &[("commit", commit), ("main", "main"), ("head", "fedcba9876543210")],
    );
    if commit_in(&first, words).as_deref() == Some(commit) {
        return Vec::new();
    }
    let file = format!("{}/{}/check.toml", flow::agents::AGENTS_DIR, flow::agents::ACTS_DIR);
    vec![format!(
        "{file}: its first must hold {{commit}} on its first line with nothing after it, so that the merge reads back \
         the commit the check passed: it gives `{first}` for the commit {commit}"
    )]
}

/// What the work says of itself where the host shows it: what it wrote in the blocks the flow's check names to
/// describe it ([`described`]). The acts and the tools work it out alike.
pub(crate) fn about(flow: &Compiled, work: &Work) -> String {
    let named: Vec<BlockId> = (flow.cells.iter())
        .filter(|cell| cell.act.as_deref() == Some("check"))
        .flat_map(|cell| cell.describes.iter().cloned())
        .collect();
    described(&named, work)
}

/// What the work wrote of itself in these blocks, which a cell names to describe it, each at its current version, in
/// the order named; the blocks not written yet are left out.
fn described(describes: &[BlockId], work: &Work) -> String {
    let versions = decide::versions(&work.records);
    let written: Vec<&str> = (describes.iter())
        .filter_map(|block| {
            let at = versions.get(block)?.last()?.at;
            work.contents.get(&(block.clone(), at))
        })
        .map(|content| content.trim())
        .filter(|content| !content.is_empty())
        .collect();
    written.join("\n\n")
}

/// The commit the work's latest check passed, read from what it wrote in its choice; none where the flow has no
/// check, so that what the branch holds is merged. Where the flow has one and no check passed, the work cannot merge.
fn passed(flow: &Compiled, work: &Work, words: &CheckWords) -> Result<Option<String>, String> {
    if !flow.cells.iter().any(|cell| cell.act.as_deref() == Some("check")) {
        return Ok(None);
    }
    let commit = latest_check(flow, work).and_then(|check| checked_with(work, check, words));
    match commit {
        Some(commit) => Ok(Some(commit)),
        None => Err(
            "the work's latest check did not pass, or said no commit: there is nothing checked to merge".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Repo, commit, marking, run};
    use crate::host::fake::{Asked, Fake};
    use crate::host::{Local, Verdict};

    fn outputs() -> Vec<(BlockId, Option<Vec<String>>)> {
        vec![
            // The values in an order other than passing first: they are written by name.
            ("passed".into(), Some(vec!["no".to_string(), "yes".to_string()])),
            ("opinions".into(), None),
        ]
    }

    fn checked(failed: Option<&str>) -> Checked {
        Checked {
            commit: "c0ffee".to_string(),
            main: "beef".to_string(),
            failed: failed.map(str::to_string),
            by: None,
            took_in: false,
            notes: Vec::new(),
            asks: None,
        }
    }

    #[test]
    fn the_merge_reads_back_the_commit_the_check_wrote_in_the_repository_s_words() {
        assert_eq!(word_faults(&words().check), Vec::<String>::new());
        let handed = checked_in(&outputs(), &checked(None), "main", &words().check).unwrap();
        assert_eq!(commit_in(&handed[0].content, &words().check).as_deref(), Some("c0ffee"));
    }

    #[test]
    fn words_the_merge_cannot_read_the_commit_back_from_are_a_fault_of_their_file() {
        for first in [
            "Checked commit {commit} on main",
            "Checked {main} at commit {commit}",
            "Checked the work",
        ] {
            let mut check = words().check;
            check.first = first.to_string();
            let faults = word_faults(&check);
            assert_eq!(faults.len(), 1, "{first}: {faults:?}");
            assert!(faults[0].starts_with("agents/acts/check.toml: "), "{}", faults[0]);
        }
    }

    #[test]
    fn a_block_of_main_that_gives_no_words_for_the_watch_is_a_fault_of_its_file() {
        let compiled = |main: &str| {
            let source = flow::Source {
                flow: format!(
                    "name = \"watched\"\norder = [\"merge\"]\n[limits]\nruns = 1\nreturns = 1\ncalls = 1\n\
                     [blocks.main]\noutside = \"main\"\n{main}[blocks.merged]\nshown = true\n"
                ),
                cells: [(
                    "merge".to_string(),
                    "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ninputs = [\"main\"]\noutputs = [\"merged\"]\n"
                        .to_string(),
                )]
                .into(),
                ..Default::default()
            };
            let has = flow::Has {
                sources: ["main".to_string()].into(),
                acts: ["merge".to_string()].into(),
                ..Default::default()
            };
            flow::compile(&source, &has).expect("the flow compiles")
        };
        let given = compiled("writes = { text = \"Main is at {head}.\" }\n");
        assert_eq!(main_faults(&given), Vec::<String>::new());
        let faults = main_faults(&compiled(""));
        assert_eq!(
            faults,
            ["the flow watched, flow.toml: the block main (from main): its writes must give text"]
        );
    }

    #[test]
    fn a_commit_the_engine_made_to_take_main_in_is_said() {
        let mut took = checked(Some("The check failed."));
        took.took_in = true;
        let handed = checked_in(&outputs(), &took, "main", &words().check).unwrap();
        let said = "\n\nThe engine merged main at beef into the work's branch for the check, as the commit c0ffee, \
                    which the worktree holds.";
        assert!(handed.iter().all(|h| h.content.ends_with(said)), "{handed:?}");
    }

    #[test]
    fn what_judged_the_work_is_said_with_what_it_was_checked_against() {
        let mut by = checked(None);
        by.by = Some("the CI of the pull request https://example.com/pull/7".to_string());
        let handed = checked_in(&outputs(), &by, "main", &words().check).unwrap();
        assert_eq!(
            handed[0].content,
            "Checked commit c0ffee\n\nThe work's branch was checked against main at beef by the CI of the pull \
             request https://example.com/pull/7: it passed."
        );
    }

    #[test]
    fn a_work_that_passes_hands_in_yes_alone_with_the_commit_it_checked() {
        let handed = checked_in(&outputs(), &checked(None), "main", &words().check).unwrap();
        assert_eq!(
            handed,
            [Handed {
                block: "passed".into(),
                content: "Checked commit c0ffee\n\nThe work's branch was checked against main at beef: it passed."
                    .to_string(),
                matters: true,
                value: Some("yes".to_string()),
            }]
        );
    }

    #[test]
    fn a_work_that_fails_hands_in_no_and_what_failed() {
        let handed = checked_in(&outputs(), &checked(Some("The check failed.")), "main", &words().check).unwrap();
        assert_eq!(handed.len(), 2);
        assert_eq!(handed[0].value.as_deref(), Some("no"));
        assert_eq!(handed[1].block, BlockId::from("opinions"));
        assert!(
            handed[1].content.ends_with("The check failed."),
            "{}",
            handed[1].content
        );
    }

    /// The flow of the end-to-end tests, which has every act.
    fn acts_flow() -> Compiled {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow");
        let source = flow::read(&dir).unwrap();
        let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        let has = flow::Has {
            sources: names(&["tree"]),
            acts: names(&["start", "check", "merge"]),
            ..Default::default()
        };
        flow::compile(&source, &has).unwrap()
    }

    #[test]
    fn a_flow_whose_acts_have_what_they_need_has_no_fault() {
        assert_eq!(faults(&acts_flow(), &words().check.values), Vec::<String>::new());
    }

    #[test]
    fn a_check_whose_choice_does_not_offer_yes_and_no_is_a_fault() {
        let mut flow = acts_flow();
        flow.blocks.get_mut(&BlockId::from("passed")).unwrap().values = Some(vec!["pass".into(), "fail".into()]);
        let faults = faults(&flow, &words().check.values);
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(
            faults[0].contains("cell check (act check)") && faults[0].contains("offer yes and no"),
            "{faults:?}"
        );
    }

    #[test]
    fn a_check_with_no_choice_or_two_and_a_merge_with_one_are_faults() {
        let mut flow = acts_flow();
        flow.blocks.get_mut(&BlockId::from("failures")).unwrap().values = Some(vec!["yes".into(), "no".into()]);
        flow.blocks.get_mut(&BlockId::from("merged")).unwrap().values = Some(vec!["done".into()]);
        let faults = faults(&flow, &words().check.values);
        assert_eq!(faults.len(), 2, "{faults:?}");
        assert!(faults[0].contains("has 2"), "{faults:?}");
        assert!(
            faults[1].contains("cell merge (act merge)") && faults[1].contains("merged"),
            "{faults:?}"
        );
        let mut none = acts_flow();
        none.blocks.get_mut(&BlockId::from("passed")).unwrap().values = None;
        assert!(self::faults(&none, &words().check.values)[0].contains("has 0"));
    }

    #[test]
    fn a_deliverable_says_the_items_it_was_made_of_in_order() {
        let items = |numbers: &[u64]| numbers.iter().copied().map(ItemNumber).collect::<Vec<_>>();
        assert_eq!(
            delivered(&items(&[]), &words().deliver),
            "Delivered, made of no other item."
        );
        assert_eq!(
            delivered(&items(&[4]), &words().deliver),
            "Delivered, made of the item 4."
        );
        assert_eq!(
            delivered(&items(&[2, 4]), &words().deliver),
            "Delivered, made of the items 2 and 4."
        );
        assert_eq!(
            delivered(&items(&[2, 4, 7]), &words().deliver),
            "Delivered, made of the items 2, 4 and 7."
        );
    }

    #[test]
    fn a_check_with_no_choice_fails() {
        let none = vec![("opinions".into(), None)];
        assert!(checked_in(&none, &checked(None), "main", &words().check).is_err());
    }

    const WORK: WorkNumber = WorkNumber(1);

    /// The work's branch in the repository, as a host is given it.
    fn branch(place: &Place) -> Branch<'_> {
        Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place,
        }
    }

    /// Checks the work in the repository with the host.
    async fn check_with(repo: &Repo, host: &impl Host, stop: &mut Stop) -> Result<Option<Checked>, String> {
        let place = repo.place(WORK);
        let (protecting, work) = (Protecting::default(), Work::default());
        let gate = Gate {
            protecting: &protecting,
            work: &work,
        };
        check(
            &repo.project,
            host,
            &branch(&place),
            &Mutex::new(()),
            stop,
            &words().check,
            &gate,
        )
        .await
    }

    /// Checks the work in the repository with the local host, which must give a verdict.
    async fn checked_locally(repo: &Repo) -> Checked {
        let host = Local::new(repo.project.clone());
        check_with(repo, &host, &mut Stop::never())
            .await
            .expect("the work is checked")
            .expect("nothing stopped it")
    }

    /// The paths the tests' project protects, and the question the engine asks of a change to them, as this
    /// repository's flows write it.
    fn protecting(paths: &[&str]) -> Protecting {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let beside = flow::agents::read(&root).expect("what is written beside the flows is read");
        Protecting {
            paths: paths.iter().map(|p| p.to_string()).collect(),
            words: Some(beside.asks.protected),
        }
    }

    /// Checks the work with the host, which must give a verdict where it is asked, holding a change to the protected
    /// paths to what the owner answered in the work.
    async fn check_held(repo: &Repo, host: &impl Host, protecting: &Protecting, work: &Work) -> Checked {
        let place = repo.place(WORK);
        let gate = Gate { protecting, work };
        let (repository, mut stop) = (Mutex::new(()), Stop::never());
        check(
            &repo.project,
            host,
            &branch(&place),
            &repository,
            &mut stop,
            &words().check,
            &gate,
        )
        .await
        .expect("the work is checked")
        .expect("nothing stopped it")
    }

    /// A verdict that passes.
    fn passes() -> Verdict {
        Verdict {
            failed: None,
            by: None,
            notes: Vec::new(),
        }
    }

    /// A work started in the repository whose branch changes `AGENTS.md`, which the tests' project protects, and
    /// another file; and main's head it started from.
    async fn changing_a_protected_file(repo: &Repo) -> (Place, String) {
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(repo, &Fake::new(&main)).await.unwrap();
        commit(&started.place.dir, "AGENTS.md", "Rules.\n");
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        (started.place, main)
    }

    #[tokio::test]
    async fn a_change_to_a_protected_path_fails_the_check_and_is_asked_before_the_host_is() {
        let repo = Repo::new("true");
        let (_, main) = changing_a_protected_file(&repo).await;
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &protecting(&["AGENTS.md", "flow/"]), &Work::default()).await;
        let failed = checked.failed.as_deref().unwrap_or_default();
        assert!(
            failed.contains("protects") && failed.contains("AGENTS.md"),
            "{checked:?}"
        );
        assert!(!failed.contains("farewell.txt"), "{failed}");
        assert!(
            matches!(&checked.asks, Some(Question::Asked { name, waits: true, .. }) if name == "protected_files"),
            "{checked:?}"
        );
        assert_eq!(host.asked(), [Asked::Main], "the host is not asked for a verdict");
    }

    #[tokio::test]
    async fn a_change_the_owner_said_to_take_back_never_passes_the_check_while_it_is_there() {
        let repo = Repo::new("true");
        let (place, main) = changing_a_protected_file(&repo).await;
        let protecting = protecting(&["AGENTS.md"]);
        let words = protecting.words.clone().unwrap();
        let first = check_held(&repo, &Fake::new(&main), &protecting, &Work::default()).await;
        let take_back = (1..=words.options.len() as u64).find(|o| *o != words.merges).unwrap();
        let work = crate::protected::tests::asked_with(first.asks.clone().unwrap(), Some(take_back));
        // The owner's answer is no yes: the check fails again and asks anew of the same change.
        let again = check_held(&repo, &Fake::new(&main), &protecting, &work).await;
        assert!(again.failed.is_some(), "{again:?}");
        assert!(matches!(&again.asks, Some(Question::Asked { .. })), "{again:?}");
        // Nor once main moves on and is taken in, nor once the work commits elsewhere.
        commit(&repo.project.repo, "greeting.txt", "Hello from main.\n");
        let moved = run(&repo.project.repo, &["rev-parse", "main"]);
        commit(&place.dir, "farewell.txt", "Goodbye.\n");
        let host = Fake::new(&moved).judges(Ok(Some(passes())));
        let later = check_held(&repo, &host, &protecting, &work).await;
        assert!(later.took_in, "main was taken in");
        assert!(later.failed.is_some(), "{later:?}");
        assert_eq!(host.asked(), [Asked::Main], "the host is not asked for a verdict");
    }

    #[tokio::test]
    async fn the_owners_yes_lets_that_very_change_pass_to_the_host_and_no_other() {
        let repo = Repo::new("true");
        let (place, main) = changing_a_protected_file(&repo).await;
        let protecting = protecting(&["AGENTS.md"]);
        let words = protecting.words.clone().unwrap();
        let first = check_held(&repo, &Fake::new(&main), &protecting, &Work::default()).await;
        let work = crate::protected::tests::asked_with(first.asks.clone().unwrap(), Some(words.merges));
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &protecting, &work).await;
        assert_eq!(
            (checked.failed.as_deref(), checked.asks.as_ref()),
            (None, None),
            "{checked:?}"
        );
        assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(checked.commit.clone())]);
        // The change changed after the yes: asked anew, and it fails.
        commit(&place.dir, "AGENTS.md", "Other rules.\n");
        let changed = check_held(&repo, &Fake::new(&main), &protecting, &work).await;
        assert!(changed.failed.is_some(), "{changed:?}");
        assert!(matches!(&changed.asks, Some(Question::Asked { .. })), "{changed:?}");
    }

    #[tokio::test]
    async fn a_change_taken_back_passes_to_the_host_and_withdraws_the_question() {
        let repo = Repo::new("true");
        let (place, main) = changing_a_protected_file(&repo).await;
        let protecting = protecting(&["AGENTS.md"]);
        let first = check_held(&repo, &Fake::new(&main), &protecting, &Work::default()).await;
        let work = crate::protected::tests::asked_with(first.asks.clone().unwrap(), None);
        run(&place.dir, &["rm", "-q", "AGENTS.md"]);
        run(&place.dir, &["commit", "-q", "-m", "Take the rules back"]);
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &protecting, &work).await;
        assert_eq!(checked.failed, None, "{checked:?}");
        assert!(matches!(&checked.asks, Some(Question::Withdrawn { .. })), "{checked:?}");
    }

    #[tokio::test]
    async fn a_look_at_the_protected_paths_that_fails_fails_the_check() {
        let repo = Repo::new("true");
        let (_, main) = changing_a_protected_file(&repo).await;
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        // A path git cannot read as one.
        let checked = check_held(&repo, &host, &protecting(&[":(nonsense)AGENTS.md"]), &Work::default()).await;
        let failed = checked.failed.as_deref().unwrap_or_default();
        assert!(failed.contains("could not be looked at"), "{checked:?}");
        assert_eq!(checked.asks, None);
        assert_eq!(host.asked(), [Asked::Main], "the host is not asked for a verdict");
    }

    /// Starts the work in the repository with the host.
    async fn start_with(repo: &Repo, host: &impl Host) -> Result<git::Started, String> {
        start(&repo.project, host, &repo.place(WORK), &Mutex::new(())).await
    }

    /// Commits a file on a branch of its own in the repository, leaving its checkout on main, and gives the commit.
    fn aside(repo: &Repo, file: &str, text: &str) -> String {
        let dir = &repo.project.repo;
        run(dir, &["switch", "-q", "-c", "aside"]);
        let made = commit(dir, file, text);
        run(dir, &["switch", "-q", "main"]);
        made
    }

    #[tokio::test]
    async fn a_work_starts_from_the_main_its_host_gives() {
        let repo = Repo::new("true");
        let main = aside(&repo, "elsewhere.txt", "Elsewhere.\n");
        let host = Fake::new(&main);
        let started = start_with(&repo, &host).await.expect("the work starts");
        assert_eq!(started.from, main);
        assert!(started.place.dir.join("elsewhere.txt").is_file());
        assert_eq!(host.asked(), [Asked::Main]);
    }

    #[tokio::test]
    async fn the_hosts_verdict_on_the_branchs_head_is_what_the_check_found() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let verdict = Verdict {
            failed: Some("The check lint failed.".to_string()),
            by: Some("the CI of the pull request 7".to_string()),
            notes: Vec::new(),
        };
        let host = Fake::new(&main).judges(Ok(Some(verdict)));
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        assert_eq!(
            checked,
            Checked {
                commit: head.clone(),
                main,
                failed: Some("The check lint failed.".to_string()),
                by: Some("the CI of the pull request 7".to_string()),
                took_in: false,
                notes: Vec::new(),
                asks: None,
            }
        );
        assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(head)]);
    }

    #[tokio::test]
    async fn a_host_that_cannot_judge_the_work_fails_the_check_with_why() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        start_with(&repo, &Fake::new(&main)).await.unwrap();
        let host = Fake::new(&main).judges(Err("GitHub cannot be reached".to_string()));
        let failed = check_with(&repo, &host, &mut Stop::never())
            .await
            .expect_err("it fails");
        assert!(failed.contains("GitHub cannot be reached"), "{failed}");
        let host = Fake::new(&main).judges(Ok(None));
        assert_eq!(
            check_with(&repo, &host, &mut Stop::never()).await,
            Ok(None),
            "a stop gives nothing"
        );
    }

    #[tokio::test]
    async fn the_check_finds_a_conflict_with_the_main_its_host_gives_without_asking_for_a_verdict() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        // The main the host gives changed the same line; the repository's own main did not.
        let main = aside(&repo, "greeting.txt", "Good day.\n");
        let host = Fake::new(&main);
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        assert_eq!(checked.main, main);
        let failed = checked.failed.expect("the check fails");
        assert!(
            failed.contains("conflict") && failed.contains("greeting.txt"),
            "{failed}"
        );
        assert_eq!(host.asked(), [Asked::Main]);
    }

    #[tokio::test]
    async fn the_check_takes_in_a_main_that_moved_and_judges_the_commit_that_merges_it() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        let built = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let main = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
        let host = Fake::new(&main).judges(Ok(Some(Verdict {
            failed: None,
            by: None,
            notes: Vec::new(),
        })));
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        // The commit judged is the branch's new head, which merges main into what was built.
        let head = run(&repo.project.repo, &["rev-parse", &repo.place(WORK).reference()]);
        assert_eq!(checked.commit, head);
        assert_eq!(checked.main, main);
        let parents = run(&repo.project.repo, &["rev-list", "--parents", "-n", "1", &head]);
        assert_eq!(parents, format!("{head} {built} {main}"));
        assert!(
            started.place.dir.join("weather.txt").is_file(),
            "the worktree holds main's change"
        );
        let author = run(&repo.project.repo, &["log", "-1", "--format=%an <%ae>", &head]);
        assert_eq!(author, "wf-agents <wf-agents@localhost>", "the engine made the commit");
        assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(head)]);
        assert!(checked.took_in, "what the check writes says so");
    }

    #[tokio::test]
    async fn an_unfinished_merge_in_the_worktree_is_reported_and_never_taken_back() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let main = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
        // A merge left unfinished, with the very words the engine's own merge has: whoever began it, what it holds
        // may be someone's work, which only they can finish.
        let message = format!("Merge main at {main} into the work's branch");
        run(
            &started.place.dir,
            &["merge", "--no-commit", "--no-ff", "-m", &message, &main],
        );
        let host = Fake::new(&main);
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        let failed = checked.failed.expect("the check fails");
        assert!(failed.contains("A merge is under way in the worktree"), "{failed}");
        assert_eq!(host.asked(), [Asked::Main]);
        run(&started.place.dir, &["rev-parse", "--verify", "MERGE_HEAD"]);
    }

    #[tokio::test]
    async fn a_worktree_off_the_works_branch_fails_the_check() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        run(&started.place.dir, &["switch", "-q", "-c", "elsewhere"]);
        let failed = check_with(&repo, &Fake::new(&main), &mut Stop::never())
            .await
            .expect_err("it fails");
        assert!(failed.contains("not on the work's branch"), "{failed}");
    }

    #[tokio::test]
    async fn a_check_given_again_takes_main_in_once() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let main = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
        let judged = || {
            Ok(Some(Verdict {
                failed: None,
                by: None,
                notes: Vec::new(),
            }))
        };
        let host = Fake::new(&main).judges(judged()).judges(judged());
        let first = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        let again = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        assert_eq!(again.commit, first.commit);
        assert!(
            again.took_in,
            "the commit an earlier check made is still said to be the engine's"
        );
    }

    #[tokio::test]
    async fn a_branch_that_holds_main_is_judged_as_it_is() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        let built = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let host = Fake::new(&main).judges(Ok(Some(Verdict {
            failed: None,
            by: None,
            notes: Vec::new(),
        })));
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        assert_eq!(checked.commit, built, "no commit is made");
    }

    #[tokio::test]
    async fn uncommitted_changes_are_found_without_asking_for_a_verdict() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        std::fs::write(started.place.dir.join("draft.txt"), "Not yet.\n").unwrap();
        let host = Fake::new(&main);
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        let failed = checked.failed.expect("the check fails");
        assert!(failed.contains("not committed"), "{failed}");
        assert_eq!(host.asked(), [Asked::Main]);
    }

    #[tokio::test]
    async fn a_work_with_no_worktree_is_not_checked() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone());
        assert!(check_with(&repo, &host, &mut Stop::never()).await.is_err());
    }

    #[tokio::test]
    async fn the_local_check_passes_a_committed_work_that_merges_and_passes_the_command() {
        let repo = Repo::new("test -f greeting.txt");
        let started = start_with(&repo, &Local::new(repo.project.clone())).await.unwrap();
        let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let checked = checked_locally(&repo).await;
        assert_eq!(checked.failed, None);
        assert_eq!(checked.by, None);
        assert_eq!(checked.commit, head);
        assert_eq!(checked.main, run(&repo.project.repo, &["rev-parse", "main"]));
    }

    #[tokio::test]
    async fn the_local_check_fails_a_failing_command_with_the_end_of_what_it_said() {
        let repo = Repo::new("echo the greeting is wrong >&2; exit 3");
        start_with(&repo, &Local::new(repo.project.clone())).await.unwrap();
        let failed = checked_locally(&repo).await.failed.expect("the check fails");
        assert!(failed.contains("the greeting is wrong"), "{failed}");
    }

    #[tokio::test]
    async fn the_local_check_fails_a_conflict_with_main_and_merges_nothing() {
        let repo = Repo::new("true");
        let started = start_with(&repo, &Local::new(repo.project.clone())).await.unwrap();
        commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        commit(&repo.project.repo, "greeting.txt", "Good day.\n");
        let failed = checked_locally(&repo).await.failed.expect("the check fails");
        assert!(failed.contains("conflict"), "{failed}");
        assert_eq!(
            std::fs::read_to_string(repo.project.repo.join("greeting.txt")).unwrap(),
            "Good day.\n"
        );
    }

    #[tokio::test]
    async fn a_stopped_local_check_gives_nothing() {
        let repo = Repo::new("sleep 30");
        let host = Local::new(repo.project.clone());
        start_with(&repo, &host).await.unwrap();
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let stopping = async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            held.stop(1, ());
        };
        let (checked, ()) = tokio::join!(check_with(&repo, &host, &mut stop), stopping);
        assert_eq!(checked, Ok(None));
    }

    #[tokio::test]
    async fn the_check_runs_no_file_system_monitor_of_the_repository() {
        let mut repo = Repo::new("true");
        // Allowed, so that the check goes on to run its commands, none of which runs it.
        repo.project.allowed_config = vec!["core.fsmonitor".to_string()];
        start_with(&repo, &Local::new(repo.project.clone())).await.unwrap();
        let marked = repo.data.join("monitor-ran");
        let monitor = repo.data.join("monitor");
        marking(&monitor, &marked);
        run(
            &repo.project.repo,
            &["config", "core.fsmonitor", &monitor.display().to_string()],
        );
        checked_locally(&repo).await;
        assert!(!marked.exists(), "the repository's file-system monitor was run");
    }

    #[tokio::test]
    async fn the_check_refuses_what_the_works_own_worktree_configuration_names() {
        let mut repo = Repo::new("true");
        start_with(&repo, &Local::new(repo.project.clone())).await.unwrap();
        run(&repo.project.repo, &["config", "extensions.worktreeConfig", "true"]);
        run(&repo.place(WORK).dir, &["config", "--worktree", "core.editor", "vim"]);
        repo.project.allowed_config = vec!["extensions.worktreeconfig".to_string()];
        let host = Local::new(repo.project.clone());
        let refused = check_with(&repo, &host, &mut Stop::never()).await.expect_err("refused");
        assert!(refused.contains("core.editor (worktree)"), "{refused}");
        assert!(!refused.contains("extensions"), "an allowed key is let be: {refused}");
    }

    #[tokio::test]
    async fn the_merge_refuses_a_main_that_moved_since_the_check() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone());
        let started = start_with(&repo, &host).await.unwrap();
        let checked = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let moved = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
        let place = repo.place(WORK);
        let refused = merge(&host, &branch(&place), Some(&checked), &Mutex::new(()))
            .await
            .expect_err("refused");
        assert!(refused.contains(&format!("main has moved to {moved}")), "{refused}");
        assert!(
            refused.contains("let the work on"),
            "the owner is told what to do: {refused}"
        );
        assert_eq!(
            run(&repo.project.repo, &["rev-parse", "main"]),
            moved,
            "nothing was merged"
        );
    }

    #[tokio::test]
    async fn a_merge_given_again_after_it_was_made_finds_it_made() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone());
        let started = start_with(&repo, &host).await.unwrap();
        let checked = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let place = repo.place(WORK);
        let lock = Mutex::new(());
        let merged = merge(&host, &branch(&place), Some(&checked), &lock)
            .await
            .expect("it merges");
        // Main moved by that very merge: the commit checked does not hold it.
        let again = merge(&host, &branch(&place), Some(&checked), &lock).await;
        assert_eq!(again, Ok(merged));
    }

    #[tokio::test]
    async fn the_watch_reads_nothing_of_a_repository_whose_configuration_is_refused() {
        let repo = Repo::new("true");
        run(&repo.project.repo, &["config", "core.pager", "less"]);
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let acts = Acts::new(
            Fake::new(&main),
            repo.project.clone(),
            repo.data.clone(),
            "0a1b2c3d".to_string(),
            words(),
        );
        let (said, mut heard) = tokio::sync::mpsc::unbounded_channel();
        let watching = tokio::spawn(acts.watch(Duration::from_secs(3600)).run(move |head| {
            let _ = said.send(head);
        }));
        let waited = tokio::time::timeout(Duration::from_millis(500), heard.recv()).await;
        assert!(waited.is_err(), "the watch said {waited:?}");
        assert_eq!(acts.inner.host.asked(), [], "the host was not asked");
        watching.abort();
    }

    #[tokio::test]
    async fn the_watch_says_mains_head_at_once_and_again_after_a_merge() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let acts = Acts::new(
            Fake::new(&main),
            repo.project.clone(),
            repo.data.clone(),
            "0a1b2c3d".to_string(),
            words(),
        );
        let (said, mut heard) = tokio::sync::mpsc::unbounded_channel();
        let watching = tokio::spawn(acts.watch(Duration::from_secs(3600)).run(move |head| {
            let _ = said.send(head);
        }));
        let within = Duration::from_secs(10);
        let first = tokio::time::timeout(within, heard.recv()).await.expect("said at once");
        assert_eq!(first, Some(main.clone()));
        acts.inner.merged.notify_one();
        let again = tokio::time::timeout(within, heard.recv())
            .await
            .expect("said after the merge");
        assert_eq!(again, first);
        watching.abort();
    }

    #[tokio::test]
    async fn the_local_host_gives_the_repositorys_main_and_merges_there() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone());
        assert_eq!(host.main().await, Ok(run(&repo.project.repo, &["rev-parse", "main"])));
        let started = start_with(&repo, &host).await.unwrap();
        let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let place = repo.place(WORK);
        let merged = host.merge(&branch(&place), Some(&head)).await.expect("it merges");
        assert_eq!(merged, run(&repo.project.repo, &["rev-parse", "main"]));
        assert!(repo.project.repo.join("farewell.txt").is_file());
    }
}
