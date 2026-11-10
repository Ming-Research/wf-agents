//! The engine's own acts, as the rounds of the cells they carry out (`CellSpec.act`): `start` makes the work's branch
//! and worktree, `check` checks the work before its merge, and `merge` merges the commit the check passed. An act
//! hands in the cell's outputs with what it did, and only the check writes a choice ([`faults`] says what a flow needs
//! for them); where git fails, or the act cannot be done, the round fails and hands
//! in nothing. A stopped act ends its round with nothing handed in. An act takes no cost and no tokens.
//!
//! One act at a time works in the project's repository: two at once would race over its worktrees and its main branch.
//! The check's own command runs in the work's worktree, beside what other acts do.

use std::path::PathBuf;
use std::sync::Arc;

use decide::{BlockId, RunId};
use engine::Came;
use flow::Compiled;
use service::{Round, RoundReply, Rounds};
use store::WorkNumber;
use tokio::sync::Mutex;
use work::{Handed, Spent, Work};

use crate::git::{self, Checked};
use crate::held::{Failing, Held, Stop};
use crate::settings::Project;

/// The values the check writes in its choice: that the work passed, and that it did not. They are written by name: a
/// cell whose choice does not offer them has what it hands in sent back by the engine.
pub const PASSED: &str = "yes";
pub const FAILED: &str = "no";

/// The first words of what the check writes in its choice, before the commit it checked; `merge` reads the commit
/// back from there.
const CHECKED: &str = "Checked commit ";

/// The acts, done in the project's repository with the works' worktrees in the data directory.
#[derive(Clone)]
pub struct Acts {
    inner: Arc<Inner>,
}

struct Inner {
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

impl Acts {
    /// The acts in this project, with this data directory, which is an absolute path, and the installation's tag.
    pub fn new(project: Project, data: PathBuf, tag: String) -> Self {
        Acts {
            inner: Arc::new(Inner {
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

impl Rounds for Acts {
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

impl Inner {
    /// Does the cell's act, and gives what it hands in.
    async fn act(&self, round: &Round, stop: &mut Stop) -> Result<Done, String> {
        let act = round.cell.act.as_deref().unwrap_or_default();
        let outputs = outputs(&round.flow, round);
        let project = &self.project;
        let place = git::place(&self.data, &self.tag, round.work);
        match act {
            "start" => {
                let started = {
                    let _held = self.repository.lock().await;
                    git::start(project, &place).await?
                };
                let content = format!(
                    "The work is done on the branch {} in the worktree {}, which started from the commit {} of {}.",
                    started.place.reference(),
                    started.place.dir.display(),
                    started.from,
                    project.main
                );
                Ok(Done::HandsIn(prose(&outputs, &content)))
            }
            "check" => match git::check(project, &place, &self.repository, stop).await? {
                Some(checked) => checked_in(&outputs, &checked, &project.main).map(Done::HandsIn),
                None => Ok(Done::Stopped),
            },
            "merge" => {
                let checked = passed(&round.flow, &round.kept)?;
                let merged = {
                    let _held = self.repository.lock().await;
                    git::merge(project, &place, checked.as_deref()).await?
                };
                let content = format!(
                    "The branch {} is merged into {}, whose head is now {merged}.",
                    place.reference(),
                    project.main
                );
                Ok(Done::HandsIn(prose(&outputs, &content)))
            }
            other => Err(format!("the engine has no act named {other}")),
        }
    }
}

/// What is wrong with the flow for the acts, each in words with the cell: the cell of `check` has one choice among
/// its outputs, and it offers [`PASSED`] and [`FAILED`]; the cells of `start` and `merge` have no choice among their
/// outputs, since they have no outcome to choose. A flow with any of these is not run with the real carriers.
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
    let what = format!("The work's branch was checked against {main} at {}", checked.main);
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
        }
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
    fn a_check_with_no_choice_fails() {
        let none = vec![("opinions".into(), None)];
        assert!(checked_in(&none, &checked(None), "main").is_err());
    }
}
