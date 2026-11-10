//! The engine's own acts, as the rounds of the cells they carry out (`CellSpec.act`): `start` makes the work's branch
//! and worktree, `check` checks the work before its merge, and `merge` merges it. An act hands in the cell's outputs
//! with what it did; where git fails, or the act cannot be done, the round fails and hands in nothing. An act takes no
//! cost and no tokens.

use std::path::PathBuf;
use std::sync::Arc;

use decide::{BlockId, RunId};
use engine::Came;
use flow::Compiled;
use service::{Round, RoundReply, Rounds};
use store::WorkNumber;
use work::{Handed, Spent};

use crate::git::{self, Checked};
use crate::held::{Held, Stop};
use crate::settings::Project;

/// The acts, done in the project's repository with the works' worktrees in the data directory.
#[derive(Clone)]
pub struct Acts {
    inner: Arc<Inner>,
}

struct Inner {
    project: Project,
    data: PathBuf,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
}

impl Acts {
    /// The acts in this project, with this data directory, which is an absolute path.
    pub fn new(project: Project, data: PathBuf) -> Self {
        Acts {
            inner: Arc::new(Inner {
                project,
                data,
                held: Held::new(),
            }),
        }
    }
}

impl Rounds for Acts {
    fn give(&self, round: Round, reply: RoundReply) {
        let key = (round.work, round.run, round.number);
        let Some(mut stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let done = inner.act(&round, &mut stop).await;
            match done {
                Ok(handed) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::HandsIn(handed), Spent::default());
                    }
                }
                Err(why) => {
                    if let Some(reply) = inner.held.forget(key) {
                        reply.failed(why);
                    }
                }
            }
        });
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        let key = (round.work, round.run, round.number);
        if let Some(reply) = self.inner.held.stop(key, reply) {
            reply.ended(Came::HandsIn(Vec::new()), Spent::default());
        }
    }
}

impl Inner {
    /// Does the cell's act, and gives what it hands in.
    async fn act(&self, round: &Round, stop: &mut Stop) -> Result<Vec<Handed>, String> {
        let act = round.cell.act.as_deref().unwrap_or_default();
        let outputs = outputs(&round.flow, round);
        let (project, data, work) = (&self.project, &self.data, round.work);
        match act {
            "start" => {
                let started = git::start(project, data, work).await?;
                let content = format!(
                    "The work is done on the branch {} in the worktree {}, which started from the commit {} of {}.",
                    started.place.branch,
                    started.place.dir.display(),
                    started.from,
                    project.main
                );
                Ok(prose(&outputs, &content))
            }
            "check" => {
                let checked = git::check(project, data, work, stop).await?;
                checked_in(&outputs, &checked, &project.main)
            }
            "merge" => {
                let merged = git::merge(project, data, work).await?;
                let branch = git::place(data, work).branch;
                let content = format!(
                    "The branch {branch} is merged into {}, whose head is now {merged}.",
                    project.main
                );
                let mut handed = prose(&outputs, &content);
                // A choice the merge writes is given its first value: the merge has one outcome.
                for (block, values) in outputs.iter().filter_map(|(b, v)| Some((b, v.as_ref()?))) {
                    handed.push(Handed {
                        block: block.clone(),
                        content: content.clone(),
                        matters: true,
                        value: values.first().cloned(),
                    });
                }
                Ok(handed)
            }
            other => Err(format!("the engine has no act named {other}")),
        }
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

/// What the check hands in: its choice, whose first value is that the work passed and whose second that it did not,
/// with what was checked on which commit; and, where it failed, each other output with what failed.
fn checked_in(
    outputs: &[(BlockId, Option<Vec<String>>)],
    checked: &Checked,
    main: &str,
) -> Result<Vec<Handed>, String> {
    let Some((choice, values)) = outputs.iter().find_map(|(b, v)| Some((b, v.as_ref()?))) else {
        return Err("the check's cell has no choice to hand in whether the work passed".to_string());
    };
    let [passed, failed, ..] = values.as_slice() else {
        return Err(format!(
            "the check's choice {choice} has fewer than two values: one for passing and one for failing"
        ));
    };
    let what = format!(
        "The commit {} of the work's branch was checked against {main} at {}",
        checked.commit, checked.main
    );
    let mut handed = vec![Handed {
        block: choice.clone(),
        content: match checked.failed {
            None => format!("{what}: it passed."),
            Some(_) => format!("{what}: it did not pass."),
        },
        matters: true,
        value: Some(if checked.failed.is_none() { passed } else { failed }.clone()),
    }];
    if let Some(why) = &checked.failed {
        let others = outputs
            .iter()
            .filter(|(block, values)| block != choice && values.is_none());
        handed.extend(others.map(|(block, _)| Handed {
            block: block.clone(),
            content: format!("{what}. {why}"),
            matters: true,
            value: None,
        }));
    }
    Ok(handed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outputs() -> Vec<(BlockId, Option<Vec<String>>)> {
        vec![
            ("passed".into(), Some(vec!["yes".to_string(), "no".to_string()])),
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
    fn a_work_that_passes_hands_in_the_choices_first_value_alone() {
        let handed = checked_in(&outputs(), &checked(None), "main").unwrap();
        assert_eq!(
            handed,
            [Handed {
                block: "passed".into(),
                content: "The commit c0ffee of the work's branch was checked against main at beef: it passed."
                    .to_string(),
                matters: true,
                value: Some("yes".to_string()),
            }]
        );
    }

    #[test]
    fn a_work_that_fails_hands_in_the_second_value_and_what_failed() {
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

    #[test]
    fn a_check_with_no_choice_of_two_values_fails() {
        let none = vec![("opinions".into(), None)];
        assert!(checked_in(&none, &checked(None), "main").is_err());
        let one = vec![("passed".into(), Some(vec!["yes".to_string()]))];
        assert!(checked_in(&one, &checked(None), "main").is_err());
    }
}
