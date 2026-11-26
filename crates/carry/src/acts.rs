//! The engine's own acts, as the rounds of the cells they carry out (`CellSpec.act`): `start` makes the work's branch
//! and worktree, `check` checks the work before its merge, `merge` merges the commit the check passed, `deliver` says
//! that a deliverable item, made of the items it depends on, is delivered, and `conclude` says that a study is
//! concluded. An act hands in the cell's outputs with what it did, and only the check writes a choice ([`faults`] says
//! what a flow needs for them, [`main_faults`] what the watch of main needs, [`word_faults`] what the merge needs of
//! the check's words); where git or the host fails, or the act cannot be done, the round fails and hands in nothing. A
//! stopped act ends its round with nothing handed in. An act takes no cost and no tokens. The first of an act's requests
//! that the host answers without an error tells its round that the host answered it ([`Answering`]).
//!
//! What is done in the project's local repository is done the same whatever the host; the host gives the head of
//! main, the verdict on the work's commit and the merge ([`Host`]).
//!
//! One act at a time works in the project's repository: two at once would race over its worktrees and its main branch.
//! The host's verdict is waited for beside what other acts do. `deliver` does nothing outside the engine: the items it
//! is made of were each merged by their own works, and what it writes is said from its item; `conclude` does nothing
//! outside the engine either.
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
use service::{Changes, Diff, OfWork, OpenPull, Pull, Release, ReleaseReply, Round, RoundReply, Rounds, Seen, Teller};
use store::{ItemNumber, WorkNumber};
use tokio::sync::{Mutex, Notify};
use work::{Handed, Question, Said, Spent, Work};

mod answering;
#[cfg(test)]
mod ends;
mod failing;

use answering::Answering;

use crate::branch::{self, Looking};
use crate::changed;
use crate::git::{self, Started};
use crate::held::{Failing, Held, Stop};
use crate::host::{Asking, Branch, Failed, Host, NotMerged};
use crate::protected;
use crate::settings::Project;

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
    /// What failed, with the end of what it said; none where everything passed, or where what failed holds it.
    pub failed: Option<String>,
    /// What failed and fails on the head of main as well, which holds the work without failing it: the work's change
    /// is not what fails it, and it waits for main to pass it ([`crate::host::Verdict::held`]).
    pub held: Option<String>,
    /// What judged it, where the host says.
    pub by: Option<String>,
    /// Whether the engine made the commit, merging main into the branch for the check.
    pub took_in: bool,
    /// What the host says of the checks beside its verdict.
    pub notes: Vec<String>,
    /// What the host found beyond the work, each worth an item of its own, which the check hands in as observations.
    pub observations: Vec<String>,
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
    host: Arc<H>,
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
    /// The look at the network to the host; none where no network reaches it.
    look: Option<Arc<dyn service::Network>>,
    /// What was worked out of what the works changed, by their heads.
    known: changed::Known,
}

/// What came of an act: what it hands in, with what the engine asks the owner beside it, what it observed beyond the
/// work and, of a check that holds the work, the block of main what it wrote waits for.
enum Done {
    HandsIn(Box<Came>),
    Stopped,
}

impl<H: Host> Acts<H> {
    /// The acts on this host, which they may share, in this project, with this data directory, which is an absolute
    /// path, and the installation's tag.
    pub fn new(host: Arc<H>, project: Project, data: PathBuf, tag: String, words: Words) -> Self {
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
                look: None,
                known: changed::Known::default(),
            }),
        }
    }

    /// The same acts, the check holding a change to these paths until the owner lets it merge.
    pub fn protecting(self, protecting: Protecting) -> Self {
        let mut inner = Arc::into_inner(self.inner).expect("the acts are not shared yet");
        inner.protecting = protecting;
        Acts { inner: Arc::new(inner) }
    }

    /// The same acts, looking at the network to the host with `look` where an act fails for it; acts with no look
    /// reach their host with no network.
    pub fn looking(self, look: Arc<dyn service::Network>) -> Self {
        let mut inner = Arc::into_inner(self.inner).expect("the acts are not shared yet");
        inner.look = Some(look);
        Acts { inner: Arc::new(inner) }
    }
}

/// The source outside the works whose block the watch of main writes.
pub const MAIN: &str = driver::MAIN;

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
        let saying = reply.saying(round.number);
        let hosting = reply.hosting();
        let Some(stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let mut stop = stop.saying(saying).hosting(hosting).test_time(round.lines.test_time);
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let done = inner.act(&round, &teller, &mut stop).await;
            match done {
                Ok(Done::HandsIn(came)) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(*came, Spent::default());
                    }
                }
                Ok(Done::Stopped) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::hands_in(Vec::new()), Spent::default());
                    }
                }
                Err(failed) => {
                    // The network to the act's own host is looked at, off the loop.
                    let host = inner.look.clone();
                    let look = async || {
                        let host = host?;
                        Some(
                            tokio::task::spawn_blocking(move || host.reached())
                                .await
                                .unwrap_or(false),
                        )
                    };
                    let kind = failing::kind(&failed, look).await;
                    match inner.held.fail(key) {
                        // An act of the engine's takes no cost and no tokens, failed or not.
                        Some(Failing::Fail(reply)) => reply.failed_as(kind, None, failed.why, Spent::default()),
                        Some(Failing::End(reply)) => reply.ended(Came::hands_in(Vec::new()), Spent::default()),
                        None => {}
                    }
                }
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

    fn seen(&self, of: OfWork) -> Pin<Box<dyn Future<Output = Result<Seen, String>> + Send + '_>> {
        Box::pin(self.inner.seen(of))
    }

    // The acts keep one project's repository, whatever name the loop asks by.
    fn open_pulls(&self, _: &str) -> Pin<Box<dyn Future<Output = Result<Option<Vec<OpenPull>>, String>> + Send + '_>> {
        Box::pin(self.inner.host.open_pulls())
    }

    fn changes(&self, of: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        Box::pin(async move {
            let inner = &self.inner;
            let place = git::place(&inner.data, &inner.tag, of.work);
            let changed = (inner.known)
                .changed(&inner.project.repo, &inner.host.kept_main(), of.work, &place)
                .await?;
            Ok(changed.map(|changed| changed.changes))
        })
    }

    fn diff(
        &self,
        of: OfWork,
        path: String,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Diff>, String>> + Send + '_>> {
        Box::pin(async move {
            let inner = &self.inner;
            let place = git::place(&inner.data, &inner.tag, of.work);
            let main = inner.host.kept_main();
            (inner.known)
                .diff(&inner.project.repo, &main, (of.work, &place), &path)
                .await
        })
    }
}

impl<H: Host> Inner<H> {
    /// Does the cell's act, and gives what it hands in; or why it could not, with the kind of its cause where the act
    /// knows it.
    async fn act(&self, round: &Round, teller: &Teller, stop: &mut Stop) -> Result<Done, Failed> {
        let act = round.cell.act.as_deref().unwrap_or_default();
        let outputs = outputs(&round.flow, round);
        let project = &self.project;
        let words = &self.words;
        let place = git::place(&self.data, &self.tag, round.work);
        let about = about(&round.flow, &round.kept);
        let name = name(&round.flow, &round.kept, round.work, &project.commits);
        let branch = Branch {
            work: round.work,
            item: round.item.number,
            title: &name,
            about: &about,
            place: &place,
            continues: round.item.continues.as_ref(),
        };
        // What an act reads of the work's branch is no round's commit: it is written from outside.
        let read_with = branch::Reading {
            tree: project.tree.clone(),
            main: self.host.kept_main(),
        };
        let looking = || Looking::of(&round.flow, &round.kept, read_with.clone(), Some(round.run));
        // The round is told once the host has answered a request of the act.
        let host = Answering::new(self.host.clone(), teller.clone());
        match act {
            "start" => {
                let started = start(project, &host, &branch, &self.repository).await?;
                looking().tell(&started.place, teller, false).await;
                let (reference, worktree) = (started.place.reference(), started.place.dir.display().to_string());
                let mut parts = vec![
                    ("branch", reference.as_str()),
                    ("worktree", worktree.as_str()),
                    ("from", started.from.as_str()),
                    ("main", project.main.as_str()),
                ];
                let continues = branch.continues.map(store::Continues::said);
                let text = match &continues {
                    Some(continues) => {
                        parts.push(("continues", continues.as_str()));
                        &words.start.continued
                    }
                    None => &words.start.text,
                };
                let content = fill(text, &parts);
                Ok(Done::HandsIn(Box::new(Came::hands_in(prose(&outputs, &content)))))
            }
            "check" => {
                // The check reads the branch for its round, and holds the work's head to what the judges that take it
                // dealt with, as the flow's graph finds them.
                let mut looking = looking();
                let reviewed = branch::reviewed(&round.flow, &round.kept, &round.cell.id);
                let wrote = |block: BlockId, content: String| teller.outside_wrote_for_round(block, content);
                let mut gate = Gate {
                    branch: &mut looking,
                    wrote: &wrote,
                    reviewed: reviewed.as_deref(),
                    protecting: &self.protecting,
                    work: &round.kept,
                    tree: &words.tree,
                };
                let asking = Asking::Round(round.run, round.number);
                // The round reads main at the head the check takes in, which it is told of, so that main's block at
                // that head runs the check no more.
                let main_at = main_at(&round.flow, &project.main);
                let reads = |head: &str| {
                    if let Some((block, said)) = main_at(head) {
                        teller.outside_wrote_for_round(block, said);
                    }
                };
                let checked = check(
                    project,
                    &host,
                    &branch,
                    &self.repository,
                    (asking, stop, &reads),
                    &words.check,
                    &mut gate,
                );
                match checked.await {
                    Ok(Some(checked)) => {
                        // The check may have taken main in.
                        looking.tell(&place, teller, false).await;
                        let handed = checked_in(&outputs, &checked, &project.main, &words.check)?;
                        // A check that holds the work waits for main to move: the work waits for main, not the owner.
                        let waits_for = checked.held.as_ref().and_then(|_| main_block(&round.flow));
                        Ok(Done::HandsIn(Box::new(Came {
                            engine: checked.asks.into_iter().collect(),
                            said: Said {
                                observations: checked.observations,
                                ..Said::default()
                            },
                            waits_for,
                            ..Came::hands_in(handed)
                        })))
                    }
                    Ok(None) => Ok(Done::Stopped),
                    Err(why) => Err(why),
                }
            }
            "merge" => {
                // A commit made since the check passed moves the work's head: no round made it, and it is written from
                // outside, so that the review and the check run on it before the merge is tried again. A read that
                // fails fails the round, as the check's does: nothing merges past a head that could not be read.
                let mut looking = looking();
                let new = looking.read(&place).await?;
                let moved = looking.head_in(&new).map(str::to_string);
                for (block, content) in new {
                    teller.outside_wrote(block, content);
                }
                if let Some(head) = moved {
                    return Err(Failed::passing(fill(&words.merge.unreviewed, &[("head", &head)])));
                }
                let checked = passed(&round.flow, &round.kept, &words.check)?;
                let merged = merge(&host, &branch, checked.as_deref(), &self.repository).await;
                // Merged or refused for a moved main, the watch looks at main at once.
                self.merged.notify_one();
                let merged = merged_or_failed(merged, &project.main, &words.merge);
                let merged = merged?;
                let content = fill(
                    &words.merge.text,
                    &[
                        ("branch", &place.reference()),
                        ("main", &project.main),
                        ("head", &merged),
                    ],
                );
                Ok(Done::HandsIn(Box::new(Came::hands_in(prose(&outputs, &content)))))
            }
            "deliver" => {
                let content = delivered(&round.item.depends_on, &words.deliver);
                Ok(Done::HandsIn(Box::new(Came::hands_in(prose(&outputs, &content)))))
            }
            "conclude" => {
                let content = words.conclude.text.clone();
                Ok(Done::HandsIn(Box::new(Came::hands_in(prose(&outputs, &content)))))
            }
            other => Err(format!("the engine has no act named {other}").into()),
        }
    }
}

impl<H: Host> Inner<H> {
    /// What the project's repository says of the work beside its records: what it changed, and, of a work that merged
    /// with no pull request kept, the pull request the commit its merge made names at the end of its title, as GitHub
    /// titles the commit of a merge, at the address the host gives it.
    async fn seen(&self, of: OfWork) -> Result<Seen, String> {
        let (repo, main) = (&self.project.repo, self.host.kept_main());
        let place = git::place(&self.data, &self.tag, of.work);
        let changed = self.known.stat(repo, &main, (of.work, &place)).await?;
        let merged = (of.merged.as_deref()).and_then(|said| changed::merged_commit(said, &self.words.merge.text));
        let named = match merged {
            Some(commit) => self.known.pull_named(repo, &commit).await?,
            None => None,
        };
        let pull = named.and_then(|number| {
            Some(Pull {
                number,
                url: self.host.pull_at(number)?,
            })
        });
        Ok(Seen { changed, pull })
    }

    /// Lets go of what the acts made for the work, which has ended, holding the repository: the host lets go of what it
    /// holds for it, and its worktrees are taken away. Tried once: what fails is said, and is not tried again unseen.
    async fn release(&self, release: &Release) -> Result<(), String> {
        let place = git::place(&self.data, &self.tag, release.work);
        let name = name(&release.flow, &release.kept, release.work, &self.project.commits);
        let branch = Branch {
            work: release.work,
            item: release.item.number,
            title: &name,
            about: "",
            place: &place,
            continues: None,
        };
        let _held = self.repository.lock().await;
        self.host.end(&branch, release.end == End::Merged).await?;
        git::remove(&self.project, &place).await
    }
}

/// What came of a merge, as the act gives it: main's new head; or, where main moved since the check, a failure in
/// passing, in the words `agents/acts/merge.toml` gives, for the check to run again on main's new head before the merge
/// is tried again; or the failure the host gave.
fn merged_or_failed(
    merged: Result<String, NotMerged>,
    main: &str,
    words: &flow::agents::Merge,
) -> Result<String, Failed> {
    match merged {
        Ok(merged) => Ok(merged),
        Err(NotMerged::Moved { main: head, checked }) => {
            let slots = [("main", main), ("head", head.as_str()), ("checked", checked.as_str())];
            Err(Failed::passing(fill(&words.moved, &slots)))
        }
        Err(NotMerged::Failed(why)) => Err(why),
    }
}

impl<H: Host> Inner<H> {
    /// Looks at main's head every so often and after each merge, saying each head it reads as it reads it, the
    /// repository held: a check tells its round of the head it reads the same way, so that the two are said in the
    /// order they were read, and main's block never goes back to an older head.
    async fn watch(&self, every: Duration, say: Say) {
        let mut failing: Option<String> = None;
        loop {
            {
                let _held = self.repository.lock().await;
                let head = match git::repository_configuration(&self.project).await {
                    Ok(()) => self.host.main().await,
                    Err(why) => Err(why),
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
            }
            tokio::select! {
                () = tokio::time::sleep(every) => {}
                () = self.merged.notified() => {}
            }
        }
    }
}

/// Merges the work's branch on the host, holding the repository: the host finds a merge made already, and refuses a
/// main the commit the check passed does not hold ([`Host::merge`]). Before it gives the merge, it reads the head of
/// main, which the merge moved, so that the work that merged ends only once the reference that holds main's head has
/// the merge: a work that waited for it and goes on at once takes main in from that reference. A read that fails fails
/// the merge, which is found made already when it is tried again.
pub(crate) async fn merge(
    host: &impl Host,
    branch: &Branch<'_>,
    checked: Option<&str>,
    repository: &Mutex<()>,
) -> Result<String, NotMerged> {
    let _held = repository.lock().await;
    let commit = host.merge(branch, checked).await?;
    host.main().await.map_err(|why| {
        NotMerged::Failed(Failed::passing(format!(
            "the head of main could not be read after the merge: {why}"
        )))
    })?;
    Ok(commit)
}

/// Makes the work's branch, and its worktree, holding the repository: from the head of main the host gives, or, where
/// the work's item continues a branch or a pull request and the branch is not made yet, from the head of that
/// ([`Host::continued`]), which is asked of the host only then, so that a start given again finds the branch it made
/// whatever became of what it continued. Where the branch was there already, as a work's that was cancelled and
/// reopened, what the host held for it that the cancel let go is taken up again ([`Host::reopen`]), on every start,
/// worktree or not: a start that failed after the branch was made, or a cancel whose letting go failed after the pull
/// request was closed, leaves it to the next.
pub(crate) async fn start(
    project: &Project,
    host: &impl Host,
    branch: &Branch<'_>,
    repository: &Mutex<()>,
) -> Result<Started, Failed> {
    let _held = repository.lock().await;
    let place = branch.place;
    git::configuration(project, place).await?;
    let main = host.main().await?;
    let at = match branch.continues {
        Some(continues) if !git::branch_made(project, place).await? => host.continued(continues).await?,
        _ => main.clone(),
    };
    let started = git::start(project, place, &at, &main).await?;
    if started.kept {
        host.reopen(branch).await?;
    }
    Ok(started)
}

/// What the check tells the head of main it is checked against, as it reads it.
pub(crate) type ReadsMain<'a> = &'a (dyn Fn(&str) + Sync);

/// What the check tells what it reads of the work's branch, as the outside's word for its round.
pub(crate) type Wrote<'a> = &'a (dyn Fn(BlockId, String) + Sync);

/// What the check holds a work to: the work's head, read of its branch for the check's round, each datum of the branch
/// told to `wrote` as it now stands, against what each cell that judges the work by its head dealt with last
/// ([`branch::reviewed`]), none where the flow has no head; for its change under the protected paths, the paths and
/// the question's words; and the work as it is kept, which says what the owner answered, of that change and of the
/// questions whose answers go into the design tree; and the tree's rules the check holds the design tree to.
pub(crate) struct Gate<'a> {
    pub(crate) branch: &'a mut Looking,
    pub(crate) wrote: Wrote<'a>,
    pub(crate) reviewed: Option<&'a [Option<String>]>,
    pub(crate) protecting: &'a Protecting,
    pub(crate) work: &'a Work,
    /// The tree's rules, which the files of the design tree are held to.
    pub(crate) tree: &'a flow::agents::Tree,
}

/// The work's head, where the check holds it to what the judges dealt with and one of them dealt last with another
/// head, or with none: the head of a work whose flow has a head, as the gate's last read of the branch left it.
fn unreviewed<'g>(gate: &'g Gate<'_>) -> Option<&'g str> {
    let head = gate.branch.head()?;
    let reviewed = gate.reviewed?;
    reviewed
        .iter()
        .any(|seen| seen.as_deref() != Some(head))
        .then_some(head)
}

/// Checks the work before its merge: that no merge begun in its worktree is left uncommitted, which the work waits for
/// the owner for. What the worktree holds besides the commits is no part of what is judged. Then that its branch merges
/// into the head of main the host gives with no conflict (worked out without changing anything), and that the work's
/// own change against that main changes the design tree where a question of the work marked `tree` has the owner's answer
/// ([`crate::tree::unwritten`]), both said where both fail; then that the work's head, read of the branch for the
/// round, is the one each judge that takes it dealt with last ([`Gate`]), which fails the round in passing otherwise,
/// so that the judge runs first on the new head and the check is taken again after it; all found holding the
/// repository. Then main's head is merged into the branch, where it does not hold it, as a commit of the engine's, so
/// that what is judged is what a merge would land, which fails the check where it changes nothing against main, there
/// being nothing to merge; then that, where the work's change changes the design tree, that commit holds the tree's
/// files in their forms ([`crate::tree::unformed`]), so that a fault only main and the work together make is found;
/// then that commit's own change under the paths the project protects is empty or one the owner
/// let merge, which fails the check otherwise, with what the engine asks of it, and fails it too where it cannot be
/// looked at ([`crate::protected`]); then the host's verdict on the commit, waited for without the repository, as the
/// round of the check that asks finds it, which may hold the work where what failed fails on main as well, and may
/// observe what it found beyond the work. The head of main it is checked against is told to `reads` as it is read, the
/// repository held, so that what the round read is said before any look at main after it. Says why where it could not
/// check, and gives none where it was stopped; what failed is what was found, not a reason to fail.
pub(crate) async fn check(
    project: &Project,
    host: &impl Host,
    branch: &Branch<'_>,
    repository: &Mutex<()>,
    (asking, stop, reads): (Asking, &mut Stop, ReadsMain<'_>),
    words: &CheckWords,
    gate: &mut Gate<'_>,
) -> Result<Option<Checked>, Failed> {
    let place = branch.place;
    let dir = git::worktree(place)?;
    let (commit, took_in, main) = {
        let _held = repository.lock().await;
        git::configuration(project, place).await?;
        git::on_branch(place).await?;
        let main = host.main().await?;
        reads(&main);
        let commit = git::git(&project.repo, &["rev-parse", &place.reference()]).await?;
        let found = |failed: String| Checked {
            commit: commit.clone(),
            main: main.clone(),
            failed: Some(failed),
            held: None,
            by: None,
            took_in: false,
            notes: Vec::new(),
            observations: Vec::new(),
            asks: None,
        };
        // What is judged is the commit, whatever the worktree holds besides it. A merge begun and not committed stands in
        // the way of the check's own merge of main, and the engine takes back no merge it did not begin: the work waits
        // for the owner, and nothing is written into the check's blocks, which no cell could answer by a commit.
        if git::merging(&dir).await? {
            let changes = git::uncommitted(&dir).await?;
            let refused = fill(
                &words.refused.merging,
                &[("main", &project.main), ("changes", &changes)],
            );
            return Err(Failed::refused(refused));
        }
        // A conflict with main and an answer missing from the design tree are both said where both are found: neither
        // look depends on the other, and one commit mends both.
        let mut failed = Vec::new();
        if let Some(conflict) = git::conflicts(project, &main, &commit).await? {
            failed.push(fill(
                &words.found.conflict,
                &[
                    ("main", &project.main),
                    ("head", &main),
                    ("files", &conflict.files),
                    ("kinds", &conflict.kinds),
                ],
            ));
        }
        // An answer that takes the recommended option to a question that does not wait runs no cell, so one given after
        // the change was handed in is found only here. The check has no return that says anything beside its work.
        let unwritten = crate::tree::unwritten(project, place, &main, &project.tree, gate.work, &[]).await?;
        if !unwritten.is_empty() {
            let questions = unwritten.join("\n");
            let slots = [("main", project.main.as_str()), ("questions", questions.as_str())];
            failed.push(fill(&words.found.tree_unchanged, &slots));
        }
        if !failed.is_empty() {
            return Ok(Some(found(failed.join("\n\n"))));
        }
        // Each datum of the branch is told to the round as it now stands, a new version where none says it: the round
        // consumes the head it judged, so that a version written since its run began runs the check no more.
        gate.branch.read(place).await?;
        for (block, content) in gate.branch.latest() {
            (gate.wrote)(block, content);
        }
        if let Some(head) = unreviewed(gate) {
            return Err(Failed::passing(fill(&words.found.unreviewed, &[("head", head)])));
        }
        let (commit, took_in) = git::take_in(&dir, &main, project, branch.whose()).await?;
        let tree = |of: &str| format!("{of}^{{tree}}");
        let ours = git::git(&project.repo, &["rev-parse", &tree(&commit)]).await?;
        let theirs = git::git(&project.repo, &["rev-parse", &tree(&main)]).await?;
        if ours == theirs {
            let empty = fill(&words.found.empty, &[("main", &project.main), ("head", &main)]);
            return Ok(Some(Checked {
                commit,
                took_in,
                ..found(empty)
            }));
        }
        // The commit that would merge, with main taken in, is what holds the files of the design tree in their forms
        // where the work's change changes the tree: a name that main and the work each added, or an entry main took
        // out that the work names, is a fault of neither alone.
        let unformed = crate::tree::unformed(project, place, &main, &project.tree, gate.tree, &commit).await?;
        if !unformed.is_empty() {
            let faults = unformed.join("\n");
            let text = fill(
                &words.found.tree_form,
                &[("main", project.main.as_str()), ("faults", &faults)],
            );
            return Ok(Some(Checked {
                commit,
                took_in,
                ..found(text)
            }));
        }
        (commit, took_in, main)
    };
    let mut checked = Checked {
        commit,
        main,
        failed: None,
        held: None,
        by: None,
        took_in,
        notes: Vec::new(),
        observations: Vec::new(),
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
        // The check has no return to say why: it asks with what the question that stands says of each file as it is.
        checked.asks = protected::asking(gate.work, ask, &change, &Default::default());
        if !change.files.is_empty() && !protected::let_merge(gate.work, ask, &change) {
            let files: Vec<String> = change.files.iter().map(|file| file.path.clone()).collect();
            let failed = fill(
                &words.found.protected,
                &[("question", &ask.name), ("files", &files.join("\n"))],
            );
            checked.failed = Some(failed);
            return Ok(Some(checked));
        }
    }
    let Some(verdict) = host
        .verdict(branch, &checked.commit, &checked.main, asking, repository, stop)
        .await?
    else {
        return Ok(None);
    };
    checked.failed = verdict.failed;
    checked.held = verdict.held;
    checked.by = verdict.by;
    checked.notes = verdict.notes;
    checked.observations = verdict.observations;
    Ok(Some(checked))
}

/// The block of the flow the watch of main writes ([`MAIN`]); none where the flow has none.
fn main_block(flow: &Compiled) -> Option<BlockId> {
    let block = (flow.blocks.iter()).find(|(_, block)| block.writers.outside.as_deref() == Some(MAIN));
    block.map(|(id, _)| id.clone())
}

/// What the block of main of the flow says of main's head, as the watch writes it ([`MAIN`]): the block, and its
/// content at each head; none where the flow has no block of main, or one that gives no words for it.
fn main_at<'a>(flow: &'a Compiled, main: &'a str) -> impl Fn(&str) -> Option<(BlockId, String)> + 'a {
    let block = (flow.blocks.iter()).find(|(_, block)| block.writers.outside.as_deref() == Some(MAIN));
    let says = block.and_then(|(id, block)| Some((id, block.writes.get("text")?)));
    move |head| says.map(|(id, text)| (id.clone(), fill(text, &[("main", main), ("head", head)])))
}

/// What is wrong with the flow for the watch of main, each in words with the file that says what is written there: the
/// block of [`MAIN`] gives in its `writes` the `text` the watch fills, so that none is written empty, and that text
/// holds the head (`{head}`), which is how the driver tells whether a run of the tests was on the head main has now
/// ([`driver::red_main`]). A flow with any of these is not run, whatever carries its works out.
pub fn main_faults(flow: &Compiled) -> Vec<String> {
    let mut faults = Vec::new();
    for (name, block) in (flow.blocks.iter()).filter(|(_, block)| block.writers.outside.as_deref() == Some(MAIN)) {
        let fault = |what: &str| {
            format!(
                "the flow {}, {}: the block {name} (from {MAIN}): {what}",
                flow.name, block.file
            )
        };
        match block.writes.get("text") {
            None => faults.push(fault("its writes must give text")),
            Some(text) if !text.contains("{head}") => {
                faults.push(fault("its text must say the head of main, as {head}"));
            }
            Some(_) => {}
        }
    }
    faults
}

/// What is wrong with the flow for the acts, each in words with the cell: the cell of `check` has one choice among its
/// outputs, and it offers the values the check writes; the cells of `start`, `merge`, `deliver` and `conclude` have no
/// choice among their outputs, since they have no outcome to choose. A flow with any of these is not run with the real
/// carriers.
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
    let numbers: Vec<String> = made_of.iter().map(ToString::to_string).collect();
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
/// it checked on the first line and against what; and, where it failed, each other output with what failed. Where what
/// failed holds the work, failing on main as well, the choice says so, and nothing else is written: no cell is told of
/// a failure that is not the work's.
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
    let (value, line) = match (&checked.failed, &checked.held) {
        (None, None) => (&words.values.passed, fill(&words.passed, &[("checked", &said)])),
        (None, Some(held)) => {
            let slots = [slots.as_slice(), &[("checked", said.as_str()), ("why", held.as_str())]].concat();
            (&words.values.failed, fill(&words.held, &slots))
        }
        (Some(_), _) => (&words.values.failed, fill(&words.failed, &[("checked", &said)])),
    };
    let mut handed = vec![Handed {
        block: choice.clone(),
        content: content(line),
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

/// The name of the work's merge, which the host gives its pull request and the commit its merge makes: the first line
/// of the latest version of the block the flow's merge is named by, which the work writes in the repository's
/// language; where the work wrote nothing there, the words of the engine's commits for a work with no name, by its
/// number. The acts and the tools work it out alike.
pub(crate) fn name(flow: &Compiled, work: &Work, number: WorkNumber, commits: &flow::agents::Commits) -> String {
    let named: Vec<BlockId> = (flow.cells.iter())
        .filter(|cell| cell.act.as_deref() == Some("merge"))
        .filter_map(|cell| cell.named_by.clone())
        .collect();
    let written = described(&named, work);
    let line = written.lines().map(str::trim).find(|line| !line.is_empty());
    line.map_or_else(
        || fill(&commits.unnamed, &[("work", &number.to_string())]),
        str::to_string,
    )
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
    use crate::git::Place;
    use crate::git::tests::{Repo, commit, marking, run};
    use crate::host::fake::{Asked, Fake};
    use crate::host::{Local, Named, Verdict};
    use crate::tree::tests::{asked, asked_in, withdrawn};

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
            held: None,
            by: None,
            took_in: false,
            notes: Vec::new(),
            observations: Vec::new(),
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

    /// A flow that watches main, its block of main given as the TOML after `[blocks.main]`'s own line.
    fn watched(main: &str) -> Compiled {
        let source = flow::Source {
            flow: format!(
                "name = \"watched\"\norder = [\"merge\"]\n[limits]\nruns = 1\nreturns = 1\ncalls = 1\n\
                 [blocks.main]\noutside = \"main\"\n{main}[blocks.merged]\nshown = true\n"
            ),
            cells: [(
                "merge".to_string(),
                "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"main\"]\noutputs = [\"merged\"]\n"
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
    }

    #[test]
    fn a_block_of_main_that_gives_no_words_for_the_watch_is_a_fault_of_its_file() {
        let given = watched("writes = { text = \"Main is at {head}.\" }\n");
        assert_eq!(main_faults(&given), Vec::<String>::new());
        let faults = main_faults(&watched(""));
        assert_eq!(
            faults,
            ["the flow watched, flow.toml: the block main (from main): its writes must give text"]
        );
        // Words that do not say the head leave the driver unable to tell which head a run of the tests was on.
        let faults = main_faults(&watched("writes = { text = \"Main moved.\" }\n"));
        assert_eq!(
            faults,
            ["the flow watched, flow.toml: the block main (from main): its text must say the head of main, as {head}"]
        );
    }

    // The head the host reads, whole, is the one the block of main says in the words the flow's data give: the engine
    // tells a run of the tests on main's head from another by whether the block holds the run's commit, so a head
    // shortened or left out of the words would leave every red main unseen.
    #[tokio::test]
    async fn the_block_of_main_says_the_head_the_host_reads_whole_in_the_words_the_flow_gives() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone(), repo.check.clone());
        let head = host.main().await.expect("main is read");
        assert!(
            head.len() == 40 && head.bytes().all(|b| b.is_ascii_hexdigit()),
            "{head}"
        );
        let data: toml::Value = toml::from_str(include_str!("../../../flow/data/main.toml")).expect("the data read");
        let words = data["writes"]["text"].as_str().expect("the words are text");
        let flow = watched(&format!(
            "writes = {{ text = {} }}\n",
            toml::Value::String(words.to_string())
        ));
        assert_eq!(main_faults(&flow), Vec::<String>::new());
        let (_, said) = main_at(&flow, &repo.project.main)(&head).expect("the flow says it");
        assert!(said.contains(&head), "{said}");
    }

    #[test]
    fn a_commit_the_engine_made_to_take_main_in_is_said() {
        let mut took = checked(Some("The check failed."));
        took.took_in = true;
        let handed = checked_in(&outputs(), &took, "main", &words().check).unwrap();
        let said = "\n\nFor the check, the engine merged main at beef into the work's branch in the commit c0ffee, \
                    which is now in the worktree.";
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
            sources: names(&["tree", "main"]),
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
            "Delivered, made of the item #4."
        );
        assert_eq!(
            delivered(&items(&[2, 4]), &words().deliver),
            "Delivered, made of the items #2 and #4."
        );
        assert_eq!(
            delivered(&items(&[2, 4, 7]), &words().deliver),
            "Delivered, made of the items #2, #4 and #7."
        );
    }

    #[test]
    fn a_check_with_no_choice_fails() {
        let none = vec![("opinions".into(), None)];
        assert!(checked_in(&none, &checked(None), "main", &words().check).is_err());
    }

    const WORK: WorkNumber = WorkNumber(1);

    /// The round of the check that asks for its verdict.
    const ROUND: Asking = Asking::Round(RunId(1), 1);

    /// The work's branch in the repository, as a host is given it.
    fn branch(place: &git::Place) -> Branch<'_> {
        Branch {
            work: WORK,
            item: store::ItemNumber(36),
            title: "Greet the user",
            about: "",
            place,
            continues: None,
        }
    }

    /// Checks the work in the repository with the host, its flow holding the work's head to nothing.
    async fn check_with(repo: &Repo, host: &impl Host, stop: &mut Stop) -> Result<Option<Checked>, String> {
        check_failing(repo, host, stop).await.map_err(String::from)
    }

    /// As [`check_with`], with the kind of the failure the check fails with.
    async fn check_failing(repo: &Repo, host: &impl Host, stop: &mut Stop) -> Result<Option<Checked>, Failed> {
        let place = repo.place(WORK);
        let (protecting, work) = (Protecting::default(), Work::default());
        let tree = words().tree;
        let mut looking = Looking::nothing();
        let mut gate = Gate {
            branch: &mut looking,
            wrote: &|_, _| {},
            reviewed: None,
            protecting: &protecting,
            work: &work,
            tree: &tree,
        };
        check(
            &repo.project,
            host,
            &branch(&place),
            &Mutex::new(()),
            (ROUND, stop, &|_| {}),
            &words().check,
            &mut gate,
        )
        .await
    }

    /// Checks the work in the repository with the host, holding its head, whose latest version says `last`, to what the
    /// judges that take it dealt with last, `reviewed`: what came of it, and what it told of the head as the outside's
    /// word for its round.
    async fn check_reviewed(
        repo: &Repo,
        host: &impl Host,
        last: Option<&str>,
        reviewed: &[Option<String>],
    ) -> (Result<Option<Checked>, Failed>, Vec<(BlockId, String)>) {
        let place = repo.place(WORK);
        let (lock, mut stop, words, branch) = (Mutex::new(()), Stop::never(), words(), branch(&place));
        let (protecting, work) = (Protecting::default(), Work::default());
        let reading = branch::Reading {
            tree: repo.project.tree.clone(),
            main: crate::host::kept_main(&repo.project),
        };
        let mut looking = Looking::head_only(reading, last);
        let told = std::sync::Mutex::new(Vec::new());
        let wrote = |block: BlockId, content: String| told.lock().unwrap().push((block, content));
        let mut gate = Gate {
            branch: &mut looking,
            wrote: &wrote,
            reviewed: Some(reviewed),
            protecting: &protecting,
            work: &work,
            tree: &words.tree,
        };
        let checked = check(
            &repo.project,
            host,
            &branch,
            &lock,
            (ROUND, &mut stop, &|_| {}),
            &words.check,
            &mut gate,
        )
        .await;
        (checked, told.into_inner().unwrap())
    }

    // A merge refused for a main that moved since the check fails in passing, in the words of the merge's file, so
    // that the check runs again on main's new head before the merge is tried again; any other refusal is the host's.
    #[test]
    fn a_merge_refused_for_a_moved_main_fails_in_passing_in_the_merges_words() {
        let words = words().merge;
        let moved = NotMerged::Moved {
            main: "fedcba".to_string(),
            checked: "abcdef".to_string(),
        };
        let failed = merged_or_failed(Err(moved), "main", &words).expect_err("not merged");
        let said = fill(
            &words.moved,
            &[("main", "main"), ("head", "fedcba"), ("checked", "abcdef")],
        );
        assert_eq!(failed, Failed::passing(said));
        assert!(failed.why.contains("fedcba"), "{failed:?}");
        let refused = merged_or_failed(Err(NotMerged::Failed("no".into())), "main", &words);
        assert_eq!(refused, Err(Failed::from("no".to_string())));
        assert_eq!(
            merged_or_failed(Ok("head".to_string()), "main", &words),
            Ok("head".to_string())
        );
    }

    // What is checked and merged is what the review saw: the work's head, read of its branch for the check's round, is
    // the one each judge that takes it dealt with last. A commit no round handed in, as a person's, is read by the check
    // and told to its round as the outside's word, and fails the check in passing with nothing judged, so that the
    // review runs on it first; so does a head a judge dealt with no version of, or with another one. In W9 the check
    // passed, by the commit an answer to the check named, a change of a decision no review had seen.
    #[tokio::test]
    async fn the_check_passes_only_the_head_the_review_saw_and_reads_a_commit_made_outside_any_round_from_outside() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        let built = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let seen = [Some(built.clone())];
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let (checked, told) = check_reviewed(&repo, &host, Some(&built), &seen).await;
        let checked = checked.expect("the work is checked").expect("nothing stopped it");
        assert_eq!((checked.commit.as_str(), checked.failed), (built.as_str(), None));
        let head = |at: &str| vec![(BlockId::from("head"), at.to_string())];
        assert_eq!(told, head(&built), "the round is told the head it judged");
        // A commit made outside any round, which no version says yet.
        let after = commit(&started.place.dir, "weather.txt", "Sunny.\n");
        let host = Fake::new(&main);
        let (checked, told) = check_reviewed(&repo, &host, Some(&built), &seen).await;
        let failed = checked.expect_err("the check fails in passing");
        assert_eq!(failed.kind, Some(decide::run::Failure::Passing), "{failed:?}");
        let said = format!("The work's head is {after}, which the review has not seen");
        assert!(failed.why.starts_with(&said), "{failed:?}");
        assert_eq!(told, head(&after), "the commit is told to the round, as the outside's");
        assert_eq!(host.asked(), [Asked::Main], "no verdict is asked for");
        // Once a version says it, a judge that dealt with another, or with none, fails the check all the same.
        for seen in [
            vec![Some(built.clone())],
            vec![None],
            vec![Some(after.clone()), Some(built.clone())],
        ] {
            let (checked, told) = check_reviewed(&repo, &Fake::new(&main), Some(&after), &seen).await;
            assert!(checked.is_err(), "{seen:?}");
            assert_eq!(told, head(&after), "{seen:?}");
        }
        // Every judge saw it, or no judge takes the head: the check goes on.
        for seen in [vec![Some(after.clone())], Vec::new()] {
            let host = Fake::new(&main).judges(Ok(Some(passes())));
            let (checked, _) = check_reviewed(&repo, &host, Some(&after), &seen).await;
            let checked = checked.expect("the work is checked").expect("nothing stopped it");
            assert_eq!((checked.commit, checked.failed), (after.clone(), None), "{seen:?}");
        }
    }

    // The engine's merges of main on top of the head the review saw bring only main's code: the work's head is still
    // the one the review saw, and the branch is checked with main taken in.
    #[tokio::test]
    async fn the_engines_merge_of_main_on_the_head_the_review_saw_is_checked_as_it() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        let built = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let seen = [Some(built.clone())];
        let main = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let head = vec![(BlockId::from("head"), built.clone())];
        let (first, told) = check_reviewed(&repo, &host, Some(&built), &seen).await;
        let first = first.expect("the work is checked").expect("nothing stopped it");
        assert!(first.took_in && first.failed.is_none(), "{first:?}");
        assert_eq!(told, head);
        // Main moves again: the branch's head is now the engine's merge, and the work's the one the review saw.
        let main = commit(&repo.project.repo, "rain.txt", "Rainy.\n");
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let (again, told) = check_reviewed(&repo, &host, Some(&built), &seen).await;
        let again = again.expect("the work is checked").expect("nothing stopped it");
        assert!(again.took_in && again.failed.is_none(), "{again:?}");
        assert_ne!(again.commit, first.commit);
        assert_eq!(told, head);
    }

    /// Checks the work in the repository with the local host, which must give a verdict.
    async fn checked_locally(repo: &Repo) -> Checked {
        let host = Local::new(repo.project.clone(), repo.check.clone());
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
    /// paths to what the owner answered in the work, and its head to nothing.
    async fn check_held(repo: &Repo, host: &impl Host, protecting: &Protecting, work: &Work) -> Checked {
        let place = repo.place(WORK);
        let tree = words().tree;
        let mut looking = Looking::nothing();
        let mut gate = Gate {
            branch: &mut looking,
            wrote: &|_, _| {},
            reviewed: None,
            protecting,
            work,
            tree: &tree,
        };
        let (repository, mut stop) = (Mutex::new(()), Stop::never());
        check(
            &repo.project,
            host,
            &branch(&place),
            &repository,
            (ROUND, &mut stop, &|_| {}),
            &words().check,
            &mut gate,
        )
        .await
        .expect("the work is checked")
        .expect("nothing stopped it")
    }

    /// A verdict that passes.
    fn passes() -> Verdict {
        Verdict {
            failed: None,
            held: None,
            by: None,
            notes: Vec::new(),
            observations: Vec::new(),
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
        let protecting = protecting(&["AGENTS.md", "flow/"]);
        let checked = check_held(&repo, &host, &protecting, &Work::default()).await;
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
        // No round said why: the question says so of the file, above its change.
        let no_reason = protecting
            .words
            .as_ref()
            .map(|w| w.no_reason.clone())
            .unwrap_or_default();
        assert!(
            matches!(&checked.asks, Some(Question::Asked { ask, .. })
                if ask.contains(&format!("`AGENTS.md`: {no_reason}\n\n```diff\n"))),
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
        assert!(failed.contains("could not look at the change"), "{checked:?}");
        assert_eq!(checked.asks, None);
        assert_eq!(host.asked(), [Asked::Main], "the host is not asked for a verdict");
    }

    /// Starts the work in the repository with the host.
    async fn start_with(repo: &Repo, host: &impl Host) -> Result<git::Started, Failed> {
        let place = repo.place(WORK);
        start(&repo.project, host, &branch(&place), &Mutex::new(())).await
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
    async fn a_start_on_a_branch_there_already_takes_up_what_the_host_held_for_it_every_time() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let host = Fake::new(&main);
        let started = start_with(&repo, &host).await.expect("the work starts");
        assert!(!started.kept);
        // Given again with its worktree there, and again with it taken away as a cancel takes it: each time.
        assert!(start_with(&repo, &host).await.expect("it starts again").kept);
        git::remove(&repo.project, &started.place)
            .await
            .expect("the worktree is taken away");
        let again = start_with(&repo, &host).await.expect("it starts again");
        assert!(again.kept && again.place.dir.is_dir());
        let name = started.place.branch.clone();
        let reopened: Vec<Asked> = (host.asked().into_iter())
            .filter(|a| matches!(a, Asked::Reopen(_)))
            .collect();
        assert_eq!(reopened, [Asked::Reopen(name.clone()), Asked::Reopen(name)]);
    }

    /// Starts the work in the repository with the host, its item continuing what is given.
    async fn start_continuing(
        repo: &Repo,
        host: &impl Host,
        continues: &store::Continues,
    ) -> Result<git::Started, Failed> {
        let place = repo.place(WORK);
        let branch = Branch {
            continues: Some(continues),
            ..branch(&place)
        };
        start(&repo.project, host, &branch, &Mutex::new(())).await
    }

    #[tokio::test]
    async fn a_work_whose_item_continues_a_branch_starts_from_its_head_and_leaves_that_branch_as_it_was() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let old = aside(&repo, "old.txt", "Old.\n");
        let continues = store::Continues::Branch("aside".to_string());
        let host = Fake::new(&main).continued_at(Ok(old.clone()));
        let started = start_continuing(&repo, &host, &continues)
            .await
            .expect("the work starts");
        assert_eq!(run(&repo.project.repo, &["rev-parse", &started.place.reference()]), old);
        assert_eq!(started.from, main);
        assert!(started.place.dir.join("old.txt").is_file());
        assert_eq!(run(&repo.project.repo, &["rev-parse", "refs/heads/aside"]), old);
        assert_eq!(host.asked(), [Asked::Main, Asked::Continued(continues)]);
    }

    #[tokio::test]
    async fn a_start_given_again_asks_the_host_for_what_the_item_continues_no_more() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let old = aside(&repo, "old.txt", "Old.\n");
        let continues = store::Continues::Pull(226);
        let host = Fake::new(&main).continued_at(Ok(old.clone()));
        let started = start_continuing(&repo, &host, &continues)
            .await
            .expect("the work starts");
        // Whatever became of what it continued, the branch is the one made.
        let host = Fake::new(&main);
        let again = start_continuing(&repo, &host, &continues)
            .await
            .expect("it starts again");
        assert!(again.kept);
        assert_eq!(run(&repo.project.repo, &["rev-parse", &again.place.reference()]), old);
        assert_eq!(host.asked(), [Asked::Main, Asked::Reopen(started.place.branch)]);
    }

    #[tokio::test]
    async fn a_start_whose_continued_branch_is_refused_fails_with_the_refusal_and_makes_no_branch_nor_starts_from_main()
    {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let continues = store::Continues::Branch("gone".to_string());
        let host = Fake::new(&main).continued_at(Err(Failed::refused("neither side has the branch gone")));
        let refused = start_continuing(&repo, &host, &continues)
            .await
            .expect_err("it is refused");
        assert_eq!(
            (refused.why.as_str(), refused.kind),
            ("neither side has the branch gone", Some(decide::run::Failure::Other))
        );
        let place = repo.place(WORK);
        assert!(
            !git::branch_made(&repo.project, &place).await.unwrap(),
            "no branch is made"
        );
        assert!(!place.dir.exists(), "no worktree is made");
        // Corrected, the item continues what is there, and the same work starts from it.
        let old = aside(&repo, "old.txt", "Old.\n");
        let host = Fake::new(&main).continued_at(Ok(old.clone()));
        let started = start_continuing(&repo, &host, &store::Continues::Branch("aside".to_string()))
            .await
            .expect("it starts");
        assert_eq!(run(&repo.project.repo, &["rev-parse", &started.place.reference()]), old);
    }

    #[tokio::test]
    async fn a_failure_to_reach_the_host_for_what_the_item_continues_keeps_the_kind_it_was_given() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let continues = store::Continues::Branch("aside".to_string());
        let host = Fake::new(&main).continued_at(Err(Failed::passing("GitHub did not answer")));
        let failed = start_continuing(&repo, &host, &continues).await.expect_err("it fails");
        assert_eq!(failed.kind, Some(decide::run::Failure::Passing));
        assert!(!git::branch_made(&repo.project, &repo.place(WORK)).await.unwrap());
    }

    #[tokio::test]
    async fn the_local_host_continues_a_branch_of_the_repository_and_no_pull_request() {
        let repo = Repo::new("true");
        let old = aside(&repo, "old.txt", "Old.\n");
        let host = Local::new(repo.project.clone(), repo.check.clone());
        assert_eq!(
            host.continued(&store::Continues::Branch("aside".to_string())).await,
            Ok(old)
        );
        let missing = (host.continued(&store::Continues::Branch("gone".to_string())).await).expect_err("refused");
        assert_eq!(missing.kind, Some(decide::run::Failure::Other));
        assert!(missing.why.contains("no branch gone"), "{}", missing.why);
        let pull = host.continued(&store::Continues::Pull(3)).await.expect_err("refused");
        assert_eq!(pull.kind, Some(decide::run::Failure::Other));
        assert!(pull.why.contains("no pull requests"), "{}", pull.why);
    }

    #[tokio::test]
    async fn the_hosts_verdict_on_the_branchs_head_is_what_the_check_found() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let verdict = Verdict {
            failed: Some("The check lint failed.".to_string()),
            held: None,
            by: Some("the CI of the pull request 7".to_string()),
            notes: Vec::new(),
            observations: Vec::new(),
        };
        let host = Fake::new(&main).judges(Ok(Some(verdict)));
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        assert_eq!(
            checked,
            Checked {
                commit: head.clone(),
                main,
                failed: Some("The check lint failed.".to_string()),
                held: None,
                by: Some("the CI of the pull request 7".to_string()),
                took_in: false,
                notes: Vec::new(),
                observations: Vec::new(),
                asks: None,
            }
        );
        assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(head)]);
    }

    #[tokio::test]
    async fn a_host_that_cannot_judge_the_work_fails_the_check_with_why() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let host = Fake::new(&main).judges(Err("GitHub cannot be reached".into()));
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

    /// A work started in the repository whose branch changes a file outside the design tree; and main's head it
    /// started from.
    async fn building(repo: &Repo) -> (Place, String) {
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(repo, &Fake::new(&main)).await.unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        (started.place, main)
    }

    /// What the check says where these questions, marked `tree`, have the owner's answer, and the work's change against
    /// main changes nothing in the design tree.
    fn not_in_the_tree(questions: &[&str]) -> String {
        let questions = questions.join("\n");
        fill(
            &words().check.found.tree_unchanged,
            &[("main", "main"), ("questions", &questions)],
        )
    }

    // The owner answers a question marked `tree` after the change was handed in, taking the option it recommended,
    // which runs no cell: the check finds the answer missing from the tree, names the question and asks the host for
    // no verdict; once the work commits a file of the tree, it goes on to the host. Well under a second alone: a
    // repository on disk and the fake host; no other test holds the check to the owner's answers.
    #[tokio::test]
    async fn an_answer_marked_tree_that_the_works_change_leaves_out_of_the_tree_fails_the_check() {
        let repo = Repo::new("true");
        let (place, main) = building(&repo).await;
        let work = asked_in(vec![(asked("placement", Some(true)), true)]);
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &Protecting::default(), &work).await;
        assert_eq!(checked.failed, Some(not_in_the_tree(&["placement"])), "{checked:?}");
        assert_eq!(host.asked(), [Asked::Main], "the host is not asked for a verdict");
        std::fs::create_dir_all(place.dir.join("design")).unwrap();
        commit(&place.dir, "design/greeting.md", "The option taken.\n");
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &Protecting::default(), &work).await;
        assert_eq!(checked.failed, None, "{checked:?}");
        assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(checked.commit.clone())]);
    }

    // The work's change leaves the tree as main has it, and the work holds an answered question marked no, one kept
    // before questions said `tree`, and one marked `tree` and withdrawn since its answer: none holds the check, which
    // goes on to the host. Well under a second alone: three checks in one repository on disk with the fake host; it
    // catches a check that names a question the look at a return would not name.
    #[tokio::test]
    async fn an_answer_marked_no_or_not_marked_or_one_withdrawn_does_not_hold_the_check() {
        let repo = Repo::new("true");
        let (_, main) = building(&repo).await;
        for work in [
            asked_in(vec![(asked("placement", Some(false)), true)]),
            asked_in(vec![(asked("placement", None), true)]),
            withdrawn(asked_in(vec![(asked("placement", Some(true)), true)]), "placement"),
        ] {
            let host = Fake::new(&main).judges(Ok(Some(passes())));
            let checked = check_held(&repo, &host, &Protecting::default(), &work).await;
            assert_eq!(checked.failed, None, "{checked:?}");
            assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(checked.commit.clone())]);
        }
    }

    /// What the check says where the commit that would merge holds files of the design tree that are not in their
    /// forms: these faults, a line each.
    fn not_in_form(faults: &[&str]) -> String {
        let faults = faults.join("\n");
        fill(&words().check.found.tree_form, &[("main", "main"), ("faults", &faults)])
    }

    /// A file of the design tree of one decision, in the forms of the tree's rules, named `name`.
    fn formed(title: &str, name: &str) -> String {
        format!(
            "# {title}\n\nCovers {title}.\n\n### decision {name}\nIt holds.\n- rejected: an agent proposed dropping it; \
             the owner rejected it, because it is needed.\n"
        )
    }

    // The work's change changes the design tree and a file of it is not in the forms of the tree's rules: the check
    // names each fault and asks the host for no verdict; once the work puts it right, it goes on to the host. Well
    // under a second alone: a repository on disk and the fake host; no other test holds the check to the forms.
    #[tokio::test]
    async fn a_design_tree_the_works_change_leaves_out_of_its_forms_fails_the_check_until_it_is_in_them() {
        let repo = Repo::new("true");
        let (place, main) = building(&repo).await;
        std::fs::create_dir_all(place.dir.join("design/decisions")).unwrap();
        commit(
            &place.dir,
            "design/decisions/x.md",
            "# X\n\nCovers X.\n\n### decision keep-it\nIt holds.\n",
        );
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &Protecting::default(), &Work::default()).await;
        let fault = "design/decisions/x.md:6: decision keep-it: a decision has a `rejected` or a `replaced` line, or \
                     more, under its first line";
        assert_eq!(checked.failed, Some(not_in_form(&[fault])), "{checked:?}");
        assert_eq!(host.asked(), [Asked::Main], "the host is not asked for a verdict");
        commit(&place.dir, "design/decisions/x.md", &formed("X", "keep-it"));
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &Protecting::default(), &Work::default()).await;
        assert_eq!(checked.failed, None, "{checked:?}");
        assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(checked.commit.clone())]);
    }

    // Main and the work each add an entry of one name, each file in its forms and each branch clean: it is the commit
    // that would merge, with main taken in, that holds two of the name, which only the check on it finds. Well under a
    // second alone: a repository on disk and the fake host; it catches a check made on the work's head alone.
    #[tokio::test]
    async fn a_fault_only_main_and_the_work_together_make_in_the_design_tree_fails_the_check() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        std::fs::create_dir_all(started.place.dir.join("design/decisions")).unwrap();
        commit(&started.place.dir, "design/decisions/b.md", &formed("B", "keep-it"));
        std::fs::create_dir_all(repo.project.repo.join("design/decisions")).unwrap();
        let main = aside(&repo, "design/decisions/a.md", &formed("A", "keep-it"));
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &Protecting::default(), &Work::default()).await;
        let fault = "design/decisions/b.md:5: decision keep-it: the name is taken by design/decisions/a.md:5";
        assert_eq!(checked.failed, Some(not_in_form(&[fault])), "{checked:?}");
        assert!(
            checked.took_in,
            "it is the commit with main taken in that was looked at"
        );
        assert_eq!(host.asked(), [Asked::Main]);
    }

    // The work's change leaves the design tree alone, though main's holds a fault: nothing is said of what main holds,
    // and the check goes on to the host. Well under a second alone: a repository on disk and the fake host.
    #[tokio::test]
    async fn a_fault_in_the_design_tree_the_work_does_not_change_holds_nothing_back() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        std::fs::create_dir_all(repo.project.repo.join("design/decisions")).unwrap();
        let main = aside(
            &repo,
            "design/decisions/a.md",
            "# A\n\nCovers A.\n\n### decision keep-it\nIt holds.\n",
        );
        let host = Fake::new(&main).judges(Ok(Some(passes())));
        let checked = check_held(&repo, &host, &Protecting::default(), &Work::default()).await;
        assert_eq!(checked.failed, None, "{checked:?}");
        assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(checked.commit.clone())]);
    }

    // A conflict with main and answers missing from the tree are both said, a blank line between, so that one commit
    // of the work mends both. Well under a second alone: a repository on disk and the fake host; no other test holds
    // the check to say both.
    #[tokio::test]
    async fn a_conflict_and_answers_missing_from_the_tree_are_said_together() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        let main = aside(&repo, "greeting.txt", "Good day.\n");
        let work = asked_in(vec![
            (asked("wording", Some(true)), true),
            (asked("placement", Some(true)), true),
        ]);
        let host = Fake::new(&main);
        let checked = check_held(&repo, &host, &Protecting::default(), &work).await;
        let failed = checked.failed.expect("the check fails");
        let tree = format!("\n\n{}", not_in_the_tree(&["placement", "wording"]));
        let conflict = failed
            .strip_suffix(&tree)
            .expect("the answers are said after the conflict");
        assert!(
            conflict.starts_with("The branch does not merge") && conflict.contains("greeting.txt"),
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
            held: None,
            by: None,
            notes: Vec::new(),
            observations: Vec::new(),
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

    // A merge begun and not committed is whoever's, and only they can finish it, so the check, whose own merge of main it
    // would stand in the way of, takes nothing back and does not write it into the check's blocks, which no cell could
    // answer by a commit: the work waits for the owner, with what git says of the merge.
    #[tokio::test]
    async fn an_unfinished_merge_in_the_worktree_is_refused_and_never_taken_back() {
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
        let failed = check_failing(&repo, &host, &mut Stop::never())
            .await
            .expect_err("the check is refused");
        let changes = run(&started.place.dir, &["status", "--porcelain", "--untracked-files=no"]);
        let words = fill(
            &words().check.refused.merging,
            &[("main", "main"), ("changes", &changes)],
        );
        assert_eq!(failed, Failed::refused(words));
        assert!(failed.why.contains("weather.txt"), "{failed}");
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
                held: None,
                by: None,
                notes: Vec::new(),
                observations: Vec::new(),
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
    async fn a_branch_that_changes_nothing_against_main_fails_the_check_before_the_host_is_asked() {
        let repo = Repo::new("true");
        let local = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
        // Nothing built, main moved on: the branch, main taken in, holds only what main does.
        let main = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
        let host = Fake::new(&main);
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        let empty = fill(&words().check.found.empty, &[("main", "main"), ("head", &main)]);
        assert_eq!(checked.failed.as_deref(), Some(empty.as_str()));
        assert_eq!(host.asked(), [Asked::Main], "the host is not asked for a verdict");
        assert!(checked.took_in);
        // A file added and taken back again changes nothing either.
        commit(&started.place.dir, "draft.txt", "Soon.\n");
        run(&started.place.dir, &["rm", "-q", "draft.txt"]);
        run(&started.place.dir, &["commit", "-q", "-m", "Take the draft back"]);
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        assert_eq!(checked.failed.as_deref(), Some(empty.as_str()));
    }

    #[tokio::test]
    async fn a_branch_that_holds_main_is_judged_as_it_is() {
        let repo = Repo::new("true");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = start_with(&repo, &Fake::new(&main)).await.unwrap();
        let built = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let host = Fake::new(&main).judges(Ok(Some(Verdict {
            failed: None,
            held: None,
            by: None,
            notes: Vec::new(),
            observations: Vec::new(),
        })));
        let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
        assert_eq!(checked.commit, built, "no commit is made");
    }

    /// A host whose head of main is `main` and that passes the `n` commits it is asked a verdict on.
    fn passing(main: &str, n: usize) -> Fake {
        (0..n).fold(Fake::new(main), |host, _| host.judges(Ok(Some(passes()))))
    }

    // What the check judges is the commit: a file git does not track is in no commit, whoever left it, and neither
    // holds the check back nor is touched by it, where main has not moved and where its merge is made.
    #[tokio::test]
    async fn a_file_git_does_not_track_stops_nothing_and_is_left_where_it_is() {
        for moved in [false, true] {
            let repo = Repo::new("true");
            let local = run(&repo.project.repo, &["rev-parse", "main"]);
            let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
            let built = commit(&started.place.dir, "farewell.txt", "Bye.\n");
            let main = match moved {
                true => commit(&repo.project.repo, "weather.txt", "Sunny.\n"),
                false => local,
            };
            std::fs::write(started.place.dir.join("draft.txt"), "Not yet.\n").unwrap();
            let host = passing(&main, 1);
            let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
            assert_eq!(checked.failed, None, "main moved: {moved}");
            assert_eq!(checked.took_in, moved);
            let head = run(&repo.project.repo, &["rev-parse", &repo.place(WORK).reference()]);
            assert_eq!(checked.commit, head);
            assert_eq!((head == built), !moved, "the merge of main is made where it moved");
            assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(head)], "main moved: {moved}");
            let draft = std::fs::read_to_string(started.place.dir.join("draft.txt"));
            assert_eq!(draft.as_deref().ok(), Some("Not yet.\n"), "main moved: {moved}");
        }
    }

    // The same of a change to a file git tracks that is not committed, where main changed no file it touches: it is no
    // part of the commit judged, and the check's own merge, where main moved, leaves it in the worktree.
    #[tokio::test]
    async fn a_change_to_a_tracked_file_that_is_not_committed_is_no_part_of_what_is_judged() {
        for (moved, staged) in [(false, false), (true, false), (false, true)] {
            let repo = Repo::new("true");
            let local = run(&repo.project.repo, &["rev-parse", "main"]);
            let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
            let dir = &started.place.dir;
            commit(dir, "farewell.txt", "Bye.\n");
            let main = match moved {
                true => commit(&repo.project.repo, "weather.txt", "Sunny.\n"),
                false => local,
            };
            std::fs::write(dir.join("farewell.txt"), "Goodbye.\n").unwrap();
            if staged {
                run(dir, &["add", "farewell.txt"]);
            }
            let host = passing(&main, 1);
            let checked = check_with(&repo, &host, &mut Stop::never()).await.unwrap().unwrap();
            let case = format!("main moved: {moved}, staged: {staged}");
            assert_eq!(checked.failed, None, "{case}");
            let head = run(&repo.project.repo, &["rev-parse", &repo.place(WORK).reference()]);
            assert_eq!(checked.commit, head, "{case}");
            assert_eq!(host.asked(), [Asked::Main, Asked::Verdict(head)], "{case}");
            let kept = if staged { "M  farewell.txt" } else { " M farewell.txt" };
            assert_eq!(
                git::uncommitted(dir).await.unwrap(),
                kept,
                "the change is left as it was: {case}"
            );
            let said = std::fs::read_to_string(dir.join("farewell.txt"));
            assert_eq!(said.as_deref().ok(), Some("Goodbye.\n"), "{case}");
        }
    }

    // Where main moved, the check's own merge is the git's to refuse: a staged change, a change to a file main changed
    // and an untracked file at a path main adds would each be lost or mixed into it. The check takes nothing back and
    // asks the host nothing: it fails with git's own words, which the work waits on, and the worktree is as it was.
    #[tokio::test]
    async fn a_merge_of_main_that_git_refuses_for_what_the_worktree_holds_fails_the_check_with_git_s_words() {
        type Case = (&'static str, &'static str, fn(&std::path::Path, &std::path::Path));
        let cases: [Case; 3] = [
            ("a staged change", "would be overwritten by merge", |dir, _| {
                std::fs::write(dir.join("farewell.txt"), "Goodbye.\n").unwrap();
                run(dir, &["add", "farewell.txt"]);
            }),
            (
                "a change to a file main changed",
                "would be overwritten by merge",
                |dir, repo| {
                    commit(repo, "greeting.txt", "Good day.\n");
                    std::fs::write(dir.join("greeting.txt"), "Hello, you.\n").unwrap();
                },
            ),
            (
                "an untracked file at a path main adds",
                "untracked working tree files",
                |dir, repo| {
                    commit(repo, "weather.txt", "Sunny.\n");
                    std::fs::write(dir.join("weather.txt"), "Mine.\n").unwrap();
                },
            ),
        ];
        for (name, git_says, hold) in cases {
            let repo = Repo::new("true");
            let local = run(&repo.project.repo, &["rev-parse", "main"]);
            let started = start_with(&repo, &Fake::new(&local)).await.unwrap();
            let dir = &started.place.dir;
            let built = commit(dir, "farewell.txt", "Bye.\n");
            // Main moves on, by a file the case's change does not touch where it adds none of its own.
            if name == "a staged change" {
                commit(&repo.project.repo, "weather.txt", "Sunny.\n");
            }
            hold(dir, &repo.project.repo);
            let main = run(&repo.project.repo, &["rev-parse", "main"]);
            let (before, status) = (run(dir, &["rev-parse", "HEAD"]), run(dir, &["status", "--porcelain"]));
            let host = Fake::new(&main);
            let failed = check_failing(&repo, &host, &mut Stop::never()).await.expect_err(name);
            assert!(failed.why.contains(git_says), "{name}: {failed}");
            assert_eq!(host.asked(), [Asked::Main], "{name}: the host is asked no verdict");
            assert!(!git::merging(dir).await.unwrap(), "{name}: no merge is left under way");
            assert_eq!(
                run(dir, &["rev-parse", "HEAD"]),
                before,
                "{name}: the branch is as it was"
            );
            assert_eq!(before, built, "{name}");
            assert_eq!(
                run(dir, &["status", "--porcelain"]),
                status,
                "{name}: the worktree is as it was"
            );
        }
    }

    #[tokio::test]
    async fn a_work_with_no_worktree_is_not_checked() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone(), repo.check.clone());
        assert!(check_with(&repo, &host, &mut Stop::never()).await.is_err());
    }

    #[tokio::test]
    async fn the_local_check_passes_a_committed_work_that_merges_and_passes_the_command() {
        let repo = Repo::new("test -f greeting.txt");
        let started = start_with(&repo, &Local::new(repo.project.clone(), repo.check.clone()))
            .await
            .unwrap();
        let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let checked = checked_locally(&repo).await;
        assert_eq!(checked.failed, None);
        assert_eq!(checked.by, None);
        assert_eq!(checked.commit, head);
        assert_eq!(checked.main, run(&repo.project.repo, &["rev-parse", "main"]));
    }

    // The command judges the commit, which is what a merge would land: a file left in the worktree and committed in no
    // commit is not in the checkout it runs on, and the checkout is taken away as the command ends.
    #[tokio::test]
    async fn the_local_check_runs_on_a_checkout_of_the_commit_alone_and_takes_it_away() {
        let repo = Repo::new("test -f new.txt");
        let started = start_with(&repo, &Local::new(repo.project.clone(), repo.check.clone()))
            .await
            .unwrap();
        let dir = &started.place.dir;
        commit(dir, "farewell.txt", "Bye.\n");
        std::fs::write(dir.join("new.txt"), "Forgotten.\n").unwrap();
        let failed = checked_locally(&repo)
            .await
            .failed
            .expect("a file in no commit is not there");
        assert!(failed.contains("test -f new.txt"), "{failed}");
        assert!(!git::checking(&started.place).exists(), "the checkout is taken away");
        let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(
            worktrees.lines().filter(|l| l.starts_with("worktree ")).count(),
            2,
            "{worktrees}"
        );
        // Committed, the file is in what is judged.
        run(dir, &["add", "new.txt"]);
        run(dir, &["commit", "-q", "-m", "Add new.txt"]);
        assert_eq!(checked_locally(&repo).await.failed, None);
        assert!(!git::checking(&started.place).exists(), "the checkout is taken away");
    }

    /// A work with one commit in a repository whose check command writes `started` in its data directory, waits for
    /// `release` there and writes `ended`, so that a test can act while the command runs; with the host that runs it,
    /// the commit, and the three files.
    async fn waiting_check() -> (Repo, Local, Place, String, [PathBuf; 3]) {
        let mut repo = Repo::new("true");
        let files = ["started", "release", "ended"].map(|name| repo.data.join(name));
        let [started, release, ended] = &files;
        repo.check = format!(
            "touch '{}'; while [ ! -f '{}' ]; do sleep 0.02; done; touch '{}'",
            started.display(),
            release.display(),
            ended.display()
        );
        let host = Local::new(repo.project.clone(), repo.check.clone());
        let work = start_with(&repo, &host).await.unwrap();
        let commit = commit(&work.place.dir, "farewell.txt", "Bye.\n");
        (repo, host, work.place, commit, files)
    }

    /// Waits until the file is there, for ten seconds at the most.
    async fn appears(file: &std::path::Path) {
        for _ in 0..200 {
            if file.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("{} did not appear", file.display());
    }

    // What the project's check leaves running as it ends is stopped, and said through the way of saying what the host
    // found of what ran it, before its verdict or its result is given: for the act's round that ran it as the check
    // before the merge, and for the call of `run_tests` that ran it on the worktree. Nobody reads what it does once the
    // check has ended. Under a second alone, most of it the two leftovers ending on TERM; no other test holds a check of
    // the local host that leaves something running.
    #[tokio::test]
    async fn what_the_local_check_leaves_running_is_stopped_and_said_for_its_round_or_call() {
        let mut repo = Repo::new("true");
        let left = repo.data.join("left");
        repo.check = format!("sleep 30 >/dev/null 2>&1 & echo $! > '{}'", left.display());
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../flow/agents/tools/run_tests.toml"
        ));
        let tool: flow::agents::Tool = toml::from_str(text).expect("the file of run_tests reads");
        let host = Local::new(repo.project.clone(), repo.check.clone()).saying(Some(&tool));
        let work = start_with(&repo, &host).await.unwrap();
        let commit = commit(&work.place.dir, "farewell.txt", "Bye.\n");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let (to, mut said) = tokio::sync::mpsc::unbounded_channel();
        let reply = service::CallReply::new(WorkNumber(1), decide::CallId(1), to);
        let stopped = || {
            let process = std::fs::read_to_string(&left).expect("the check wrote what it left");
            let running = std::process::Command::new("kill")
                .args(["-0", process.trim()])
                .stderr(std::process::Stdio::null())
                .status()
                .expect("kill runs")
                .success();
            let _ = std::process::Command::new("kill")
                .args(["-s", "KILL", process.trim()])
                .status();
            !running
        };
        let mut heard = || {
            let mut events = Vec::new();
            while let Ok(message) = said.try_recv() {
                if let service::Message::Happened { event, .. } = message {
                    events.push(event);
                }
            }
            events
        };

        let mut stop = Stop::never().hosting(reply.hosting());
        let lock = Mutex::new(());
        let verdict = host
            .verdict(&branch(&work.place), &commit, &main, ROUND, &lock, &mut stop)
            .await;
        assert_eq!(verdict.unwrap().expect("judged").failed, None);
        assert!(stopped(), "what the check left runs on after its verdict was given");
        let of = work::Left::Act {
            run: RunId(1),
            round: 1,
        };
        assert_eq!(heard(), [engine::Event::LeftBehind { of, processes: 1 }]);

        let call = decide::CallId(4);
        let mut stop = Stop::never().hosting(reply.hosting());
        let ran = host
            .run_tests(&branch(&work.place), &Named::default(), Asking::Call(call), &mut stop)
            .await;
        assert!(ran.expect("it ran").is_some(), "nothing stopped it");
        assert!(stopped(), "what the check left runs on after its result was given");
        let of = work::Left::Call { call };
        // The local host reads no JUnit, so it says the run of its command with no job and no test before it ends.
        let reading = work::Reading {
            asking: Asking::Call(call).name(),
            source: work::Source::Command,
            commit: String::new(),
            workflow: String::new(),
            jobs: Vec::new(),
            tests: Vec::new(),
        };
        assert_eq!(
            heard(),
            [
                engine::Event::Timed { reading },
                engine::Event::LeftBehind { of, processes: 1 }
            ]
        );
    }

    // The checkout registers a worktree in the repository, as the acts' own worktrees are, so it is made holding the
    // repository (decision the-repository-is-worked-in-one-act-at-a-time): nothing of it begins while another act
    // holds the lock.
    #[tokio::test]
    async fn the_local_check_makes_its_checkout_holding_the_repository() {
        let (repo, host, place, commit, [started, release, _]) = waiting_check().await;
        let lock = Mutex::new(());
        let held = lock.lock().await;
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let (branched, mut stop) = (branch(&place), Stop::never());
        let verdict = host.verdict(&branched, &commit, &main, ROUND, &lock, &mut stop);
        let driver = async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(
                !git::checking(&place).exists(),
                "the checkout was made while the repository was held"
            );
            assert!(!started.exists(), "the command ran while the repository was held");
            drop(held);
            std::fs::write(release, "").unwrap();
        };
        let (verdict, ()) = tokio::join!(verdict, driver);
        assert_eq!(verdict.unwrap().expect("judged").failed, None);
    }

    // The command takes as long as the project's check takes: it runs without the repository, which the acts of other
    // works need meanwhile.
    #[tokio::test]
    async fn the_local_check_runs_its_command_without_the_repository() {
        let (repo, host, place, commit, [started, release, _]) = waiting_check().await;
        let lock = Mutex::new(());
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let (branched, mut stop) = (branch(&place), Stop::never());
        let verdict = host.verdict(&branched, &commit, &main, ROUND, &lock, &mut stop);
        let driver = async {
            appears(&started).await;
            assert!(lock.try_lock().is_ok(), "the repository was held while the command ran");
            std::fs::write(release, "").unwrap();
        };
        let (verdict, ()) = tokio::join!(verdict, driver);
        assert_eq!(verdict.unwrap().expect("judged").failed, None);
    }

    // The checkout is dropped from the repository's worktrees holding it too: it is not taken away while another act
    // holds the repository.
    #[tokio::test]
    async fn the_local_check_takes_its_checkout_away_holding_the_repository() {
        let (repo, host, place, commit, [started, release, ended]) = waiting_check().await;
        let lock = Mutex::new(());
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let (branched, mut stop) = (branch(&place), Stop::never());
        let verdict = host.verdict(&branched, &commit, &main, ROUND, &lock, &mut stop);
        let driver = async {
            appears(&started).await;
            let held = lock.lock().await;
            std::fs::write(release, "").unwrap();
            appears(&ended).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(
                git::checking(&place).exists(),
                "the checkout was taken away while the repository was held"
            );
            drop(held);
        };
        let (verdict, ()) = tokio::join!(verdict, driver);
        assert_eq!(verdict.unwrap().expect("judged").failed, None);
        assert!(
            !git::checking(&place).exists(),
            "the checkout is taken away once the repository is free"
        );
    }

    // A run of the check that was stopped with the engine may leave its checkout, which the next run of the work's
    // check takes away first, and the work's end takes away with its worktree.
    #[tokio::test]
    async fn a_checkout_a_stopped_engine_left_is_taken_away_before_the_next_is_made_and_with_the_work() {
        let repo = Repo::new("test -f farewell.txt");
        let started = start_with(&repo, &Local::new(repo.project.clone(), repo.check.clone()))
            .await
            .unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let left = git::checking(&started.place);
        run(
            &repo.project.repo,
            &["worktree", "add", "--detach", &left.display().to_string(), "main"],
        );
        assert!(!left.join("farewell.txt").exists(), "what was left is main's");
        assert_eq!(checked_locally(&repo).await.failed, None, "the check ran on the commit");
        assert!(!left.exists(), "the checkout is taken away");
        // One left again is taken away with the work's own worktree.
        run(
            &repo.project.repo,
            &["worktree", "add", "--detach", &left.display().to_string(), "main"],
        );
        git::remove(&repo.project, &started.place).await.unwrap();
        assert!(!left.exists() && !started.place.dir.exists());
        let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(
            worktrees.lines().filter(|l| l.starts_with("worktree ")).count(),
            1,
            "{worktrees}"
        );
    }

    #[tokio::test]
    async fn the_local_check_fails_a_failing_command_with_the_end_of_what_it_said() {
        let repo = Repo::new("echo the greeting is wrong >&2; exit 3");
        let started = start_with(&repo, &Local::new(repo.project.clone(), repo.check.clone()))
            .await
            .unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let failed = checked_locally(&repo).await.failed.expect("the check fails");
        assert!(failed.contains("the greeting is wrong"), "{failed}");
    }

    #[tokio::test]
    async fn the_local_check_fails_a_conflict_with_main_and_merges_nothing() {
        let repo = Repo::new("true");
        let started = start_with(&repo, &Local::new(repo.project.clone(), repo.check.clone()))
            .await
            .unwrap();
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
        let host = Local::new(repo.project.clone(), repo.check.clone());
        let started = start_with(&repo, &host).await.unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let stopping = async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            held.stop(1, ());
        };
        let (checked, ()) = tokio::join!(check_with(&repo, &host, &mut stop), stopping);
        assert_eq!(checked, Ok(None));
        assert!(!git::checking(&started.place).exists(), "the checkout is taken away");
    }

    #[tokio::test]
    async fn the_check_runs_no_file_system_monitor_of_the_repository() {
        let mut repo = Repo::new("true");
        // Allowed, so that the check goes on to run its commands, none of which runs it.
        repo.project.allowed_config = vec!["core.fsmonitor".to_string()];
        let started = start_with(&repo, &Local::new(repo.project.clone(), repo.check.clone()))
            .await
            .unwrap();
        commit(&started.place.dir, "farewell.txt", "Bye.\n");
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
        start_with(&repo, &Local::new(repo.project.clone(), repo.check.clone()))
            .await
            .unwrap();
        run(&repo.project.repo, &["config", "extensions.worktreeConfig", "true"]);
        run(&repo.place(WORK).dir, &["config", "--worktree", "core.editor", "vim"]);
        repo.project.allowed_config = vec!["extensions.worktreeconfig".to_string()];
        let host = Local::new(repo.project.clone(), repo.check.clone());
        let refused = check_with(&repo, &host, &mut Stop::never()).await.expect_err("refused");
        assert!(refused.contains("core.editor (worktree)"), "{refused}");
        assert!(!refused.contains("extensions"), "an allowed key is let be: {refused}");
    }

    #[tokio::test]
    async fn the_merge_refuses_a_main_that_moved_since_the_check() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone(), repo.check.clone());
        let started = start_with(&repo, &host).await.unwrap();
        let checked = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let moved = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
        let place = repo.place(WORK);
        let refused = merge(&host, &branch(&place), Some(&checked), &Mutex::new(()))
            .await
            .expect_err("refused");
        assert_eq!(
            refused,
            NotMerged::Moved {
                main: moved.clone(),
                checked
            }
        );
        assert_eq!(
            run(&repo.project.repo, &["rev-parse", "main"]),
            moved,
            "nothing was merged"
        );
    }

    #[tokio::test]
    async fn the_merge_reads_main_again_before_it_gives_the_merge() {
        let repo = Repo::new("true");
        let host = Fake::new("0a1b2c3d");
        let place = repo.place(WORK);
        let merged = merge(&host, &branch(&place), Some("abc"), &Mutex::new(())).await;
        assert_eq!(merged, Ok("0a1b2c3d".to_string()));
        assert_eq!(host.asked(), [Asked::Merge(Some("abc".to_string())), Asked::Main]);
    }

    #[tokio::test]
    async fn a_merge_whose_main_could_not_be_read_fails_and_gives_the_merge_when_given_again() {
        let repo = Repo::new("true");
        let host = Fake::new("0a1b2c3d");
        host.unreadable(Some("the network is down"));
        let place = repo.place(WORK);
        let failed = merge(&host, &branch(&place), Some("abc"), &Mutex::new(()))
            .await
            .expect_err("a merge main of which is not read is not given");
        let NotMerged::Failed(failed) = failed else {
            panic!("{failed:?}");
        };
        assert!(failed.why.contains("the network is down"), "{}", failed.why);
        assert_eq!(failed.kind, Some(decide::run::Failure::Passing), "it is tried again");
        host.unreadable(None);
        let merged = merge(&host, &branch(&place), Some("abc"), &Mutex::new(())).await;
        assert_eq!(merged, Ok("0a1b2c3d".to_string()));
    }

    #[tokio::test]
    async fn a_merge_given_again_after_it_was_made_finds_it_made() {
        let repo = Repo::new("true");
        let host = Local::new(repo.project.clone(), repo.check.clone());
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
            Arc::new(Fake::new(&main)),
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
            Arc::new(Fake::new(&main)),
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
        let host = Local::new(repo.project.clone(), repo.check.clone());
        assert_eq!(host.main().await, Ok(run(&repo.project.repo, &["rev-parse", "main"])));
        let started = start_with(&repo, &host).await.unwrap();
        let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        let place = repo.place(WORK);
        let merged = host.merge(&branch(&place), Some(&head)).await.expect("it merges");
        assert_eq!(merged, run(&repo.project.repo, &["rev-parse", "main"]));
        assert!(repo.project.repo.join("farewell.txt").is_file());
        let subject = run(&repo.project.repo, &["log", "-1", "--format=%s", "main"]);
        assert_eq!(subject, "Greet the user", "the merge is named as the branch says");
    }
}
