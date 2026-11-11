//! The engine's own acts, as the rounds of the cells they carry out (`CellSpec.act`): `start` makes the work's branch
//! and worktree, `check` checks the work before its merge, `merge` merges the commit the check passed, and `deliver`
//! says that a deliverable item, made of the items it depends on, is delivered. An act hands in the cell's outputs with
//! what it did, and only the check writes a choice ([`faults`] says what a flow needs for them); where git or the host
//! fails, or the act cannot be done, the round fails and hands in nothing. A stopped act ends its round with nothing
//! handed in. An act takes no cost and no tokens.
//!
//! What is done in the project's local repository is done the same whatever the host; the host gives the head of
//! main, the verdict on the work's commit and the merge ([`Host`]).
//!
//! One act at a time works in the project's repository: two at once would race over its worktrees and its main branch.
//! The host's verdict is waited for beside what other acts do. `deliver` does nothing outside the engine: the items it
//! is made of were each merged by their own works, and what it writes is said from its item.

use std::path::PathBuf;
use std::sync::Arc;

use decide::{BlockId, RunId};
use engine::Came;
use flow::Compiled;
use service::{Round, RoundReply, Rounds};
use store::{ItemNumber, WorkNumber};
use tokio::sync::Mutex;
use work::{Handed, Spent, Work};

use crate::git::{self, Place, Started};
use crate::held::{Failing, Held, Stop};
use crate::host::Host;
use crate::settings::Project;

/// The values the check writes in its choice: that the work passed, and that it did not. They are written by name: a
/// cell whose choice does not offer them has what it hands in sent back by the engine.
pub const PASSED: &str = "yes";
pub const FAILED: &str = "no";

/// The first words of what the check writes in its choice, before the commit it checked; `merge` reads the commit
/// back from there.
const CHECKED: &str = "Checked commit ";

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
}

/// The acts, done in the project's repository with the works' worktrees in the data directory, on this host.
pub struct Acts<H> {
    inner: Arc<Inner<H>>,
}

struct Inner<H> {
    host: H,
    project: Project,
    data: PathBuf,
    /// The installation's part of the names of the works' branches and worktrees.
    tag: String,
    /// Held while an act works in the repository.
    repository: Mutex<()>,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
}

/// What came of an act.
enum Done {
    HandsIn(Vec<Handed>),
    Stopped,
}

impl<H: Host> Acts<H> {
    /// The acts on this host in this project, with this data directory, which is an absolute path, and the
    /// installation's tag.
    pub fn new(host: H, project: Project, data: PathBuf, tag: String) -> Self {
        Acts {
            inner: Arc::new(Inner {
                host,
                project,
                data,
                tag,
                repository: Mutex::new(()),
                held: Held::new(),
            }),
        }
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
        let Some(mut stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let done = inner.act(&round, &mut stop).await;
            match done {
                Ok(Done::HandsIn(handed)) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::HandsIn(handed), Spent::default());
                    }
                }
                Ok(Done::Stopped) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::HandsIn(Vec::new()), Spent::default());
                    }
                }
                Err(why) => match inner.held.fail(key) {
                    // An act of the engine's takes no cost and no tokens, failed or not.
                    Some(Failing::Fail(reply)) => reply.failed(why, Spent::default()),
                    Some(Failing::End(reply)) => reply.ended(Came::HandsIn(Vec::new()), Spent::default()),
                    None => {}
                },
            }
        });
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        prune(&self.inner.held, &round);
        let key = (round.work, round.run, round.number);
        if let Some(reply) = self.inner.held.stop(key, reply) {
            reply.ended(Came::HandsIn(Vec::new()), Spent::default());
        }
    }
}

impl<H: Host> Inner<H> {
    /// Does the cell's act, and gives what it hands in.
    async fn act(&self, round: &Round, stop: &mut Stop) -> Result<Done, String> {
        let act = round.cell.act.as_deref().unwrap_or_default();
        let outputs = outputs(&round.flow, round);
        let project = &self.project;
        let place = git::place(&self.data, &self.tag, round.work);
        match act {
            "start" => {
                let started = start(project, &self.host, &place, &self.repository).await?;
                let content = format!(
                    "The work is done on the branch {} in the worktree {}, which started from the commit {} of {}.",
                    started.place.reference(),
                    started.place.dir.display(),
                    started.from,
                    project.main
                );
                Ok(Done::HandsIn(prose(&outputs, &content)))
            }
            "check" => match check(project, &self.host, &place, &self.repository, stop).await? {
                Some(checked) => checked_in(&outputs, &checked, &project.main).map(Done::HandsIn),
                None => Ok(Done::Stopped),
            },
            "merge" => {
                let checked = passed(&round.flow, &round.kept)?;
                let merged = {
                    let _held = self.repository.lock().await;
                    self.host.merge(&place, checked.as_deref()).await?
                };
                let content = format!(
                    "The branch {} is merged into {}, whose head is now {merged}.",
                    place.reference(),
                    project.main
                );
                Ok(Done::HandsIn(prose(&outputs, &content)))
            }
            "deliver" => Ok(Done::HandsIn(prose(&outputs, &delivered(&round.item.depends_on)))),
            other => Err(format!("the engine has no act named {other}")),
        }
    }
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

/// Checks the work before its merge: that its worktree holds nothing uncommitted and that its branch merges into the
/// head of main the host gives with no conflict (worked out without changing anything), both found holding the
/// repository; then the host's verdict on the branch's head, waited for without it. Says why where it could not
/// check, and gives none where it was stopped; what failed is what was found, not a reason to fail.
pub(crate) async fn check(
    project: &Project,
    host: &impl Host,
    place: &Place,
    repository: &Mutex<()>,
    stop: &mut Stop,
) -> Result<Option<Checked>, String> {
    let dir = git::worktree(place)?;
    let (commit, main) = {
        let _held = repository.lock().await;
        git::configuration(project, place).await?;
        let main = host.main().await?;
        let commit = git::git(&project.repo, &["rev-parse", &place.reference()]).await?;
        let found = |failed: String| Checked {
            commit: commit.clone(),
            main: main.clone(),
            failed: Some(failed),
            by: None,
        };
        let changes = git::uncommitted(&dir).await?;
        if !changes.is_empty() {
            let failed = format!("The worktree has changes that are not committed:\n{changes}");
            return Ok(Some(found(failed)));
        }
        if let Some(files) = git::conflicts(project, &main, &commit).await? {
            let failed = format!(
                "The branch does not merge into {} at {main} without a conflict:\n{files}",
                project.main
            );
            return Ok(Some(found(failed)));
        }
        (commit, main)
    };
    let Some(verdict) = host.verdict(place, &commit, stop).await? else {
        return Ok(None);
    };
    Ok(Some(Checked {
        commit,
        main,
        failed: verdict.failed,
        by: verdict.by,
    }))
}

/// What is wrong with the flow for the acts, each in words with the cell: the cell of `check` has one choice among
/// its outputs, and it offers [`PASSED`] and [`FAILED`]; the cells of `start`, `merge` and `deliver` have no choice
/// among their outputs, since they have no outcome to choose. A flow with any of these is not run with the real
/// carriers.
pub fn faults(flow: &Compiled) -> Vec<String> {
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
                [(block, values)] if !(values.iter().any(|v| v == PASSED) && values.iter().any(|v| v == FAILED)) => {
                    faults.push(format!("{at}: its choice {block} must offer {PASSED} and {FAILED}"));
                }
                [_] => {}
                _ => faults.push(format!(
                    "{at}: it must have one choice among its outputs, offering {PASSED} and {FAILED}, and has {}",
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
fn delivered(made_of: &[ItemNumber]) -> String {
    let numbers: Vec<String> = made_of.iter().map(|item| item.0.to_string()).collect();
    match numbers.as_slice() {
        [] => "Delivered, made of no other item.".to_string(),
        [one] => format!("Delivered, made of the item {one}."),
        [first @ .., last] => format!("Delivered, made of the items {} and {last}.", first.join(", ")),
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

/// What the check hands in: its choice, [`PASSED`] or [`FAILED`], with the commit it checked on the first line and
/// against what; and, where it failed, each other output with what failed.
fn checked_in(
    outputs: &[(BlockId, Option<Vec<String>>)],
    checked: &Checked,
    main: &str,
) -> Result<Vec<Handed>, String> {
    let Some(choice) = outputs.iter().find_map(|(b, v)| v.as_ref().map(|_| b)) else {
        return Err("the check's cell has no choice to hand in whether the work passed".to_string());
    };
    let by = (checked.by.as_ref()).map_or(String::new(), |by| format!(" by {by}"));
    let what = format!("The work's branch was checked against {main} at {}{by}", checked.main);
    let (value, outcome) = match checked.failed {
        None => (PASSED, "it passed"),
        Some(_) => (FAILED, "it did not pass"),
    };
    let mut handed = vec![Handed {
        block: choice.clone(),
        content: format!("{CHECKED}{}\n\n{what}: {outcome}.", checked.commit),
        matters: true,
        value: Some(value.to_string()),
    }];
    if let Some(why) = &checked.failed {
        let others = outputs
            .iter()
            .filter(|(block, values)| block != choice && values.is_none());
        handed.extend(others.map(|(block, _)| Handed {
            block: block.clone(),
            content: format!("{CHECKED}{}\n\n{what}. {why}", checked.commit),
            matters: true,
            value: None,
        }));
    }
    Ok(handed)
}

/// The commit the work's latest check passed, read from what it wrote in its choice; none where the flow has no
/// check, so that what the branch holds is merged. Where the flow has one and no check passed, the work cannot merge.
fn passed(flow: &Compiled, work: &Work) -> Result<Option<String>, String> {
    let checks: Vec<&str> = (flow.cells.iter())
        .filter(|cell| cell.act.as_deref() == Some("check"))
        .map(|cell| cell.id.as_str())
        .collect();
    if checks.is_empty() {
        return Ok(None);
    }
    let records = &work.records;
    let latest = (records.finished.iter())
        .filter(|f| {
            let started = records.started.iter().find(|s| s.run == f.run);
            started.is_some_and(|s| checks.contains(&s.cell.as_str()))
        })
        .max_by_key(|f| f.at);
    let commit = latest.and_then(|f| {
        let write = f.writes.iter().find(|w| w.value.as_deref() == Some(PASSED))?;
        let content = work.contents.get(&(write.block.clone(), f.at))?;
        let line = content.lines().next()?;
        line.strip_prefix(CHECKED).map(|commit| commit.trim().to_string())
    });
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
        }
    }

    #[test]
    fn what_judged_the_work_is_said_with_what_it_was_checked_against() {
        let mut by = checked(None);
        by.by = Some("the CI of the pull request https://example.com/pull/7".to_string());
        let handed = checked_in(&outputs(), &by, "main").unwrap();
        assert_eq!(
            handed[0].content,
            "Checked commit c0ffee\n\nThe work's branch was checked against main at beef by the CI of the pull \
             request https://example.com/pull/7: it passed."
        );
    }

    #[test]
    fn a_work_that_passes_hands_in_yes_alone_with_the_commit_it_checked() {
        let handed = checked_in(&outputs(), &checked(None), "main").unwrap();
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
        let handed = checked_in(&outputs(), &checked(Some("The check failed.")), "main").unwrap();
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
            sources: names(&[]),
            acts: names(&["start", "check", "merge"]),
        };
        flow::compile(&source, &has).unwrap()
    }

    #[test]
    fn a_flow_whose_acts_have_what_they_need_has_no_fault() {
        assert_eq!(faults(&acts_flow()), Vec::<String>::new());
    }

    #[test]
    fn a_check_whose_choice_does_not_offer_yes_and_no_is_a_fault() {
        let mut flow = acts_flow();
        flow.blocks.get_mut(&BlockId::from("passed")).unwrap().values = Some(vec!["pass".into(), "fail".into()]);
        let faults = faults(&flow);
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
        let faults = faults(&flow);
        assert_eq!(faults.len(), 2, "{faults:?}");
        assert!(faults[0].contains("has 2"), "{faults:?}");
        assert!(
            faults[1].contains("cell merge (act merge)") && faults[1].contains("merged"),
            "{faults:?}"
        );
        let mut none = acts_flow();
        none.blocks.get_mut(&BlockId::from("passed")).unwrap().values = None;
        assert!(self::faults(&none)[0].contains("has 0"));
    }

    #[test]
    fn a_deliverable_says_the_items_it_was_made_of_in_order() {
        let items = |numbers: &[u64]| numbers.iter().copied().map(ItemNumber).collect::<Vec<_>>();
        assert_eq!(delivered(&items(&[])), "Delivered, made of no other item.");
        assert_eq!(delivered(&items(&[4])), "Delivered, made of the item 4.");
        assert_eq!(delivered(&items(&[2, 4])), "Delivered, made of the items 2 and 4.");
        assert_eq!(
            delivered(&items(&[2, 4, 7])),
            "Delivered, made of the items 2, 4 and 7."
        );
    }

    #[test]
    fn a_check_with_no_choice_fails() {
        let none = vec![("opinions".into(), None)];
        assert!(checked_in(&none, &checked(None), "main").is_err());
    }

    const WORK: WorkNumber = WorkNumber(1);

    /// The work's branch in the repository, as a host is given it.
    fn branch(place: &Place) -> Branch<'_> {
        Branch {
            work: WORK,
            title: "Greet the user",
            place,
        }
    }

    /// Checks the work in the repository with the host.
    async fn check_with(repo: &Repo, host: &impl Host, stop: &mut Stop) -> Result<Option<Checked>, String> {
        let place = repo.place(WORK);
        check(&repo.project, host, &branch(&place), &Mutex::new(()), stop).await
    }

    /// Checks the work in the repository with the local host, which must give a verdict.
    async fn checked_locally(repo: &Repo) -> Checked {
        let host = Local::new(repo.project.clone());
        check_with(repo, &host, &mut Stop::never())
            .await
            .expect("the work is checked")
            .expect("nothing stopped it")
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
