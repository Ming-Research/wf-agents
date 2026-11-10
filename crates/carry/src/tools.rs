//! The tools an agent may ask for. There is one, `check`: it runs the project's check in the work's worktree and
//! answers whether it passed, with the end of what it said. A tool's result is all an agent learns of it, so what goes
//! wrong in running it is said in its result; withdrawn, the command is stopped where it runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::CallId;
use service::{CallReply, ToolCall, Tools as ToolsTrait, WITHDRAWN};
use store::WorkNumber;

use crate::git;
use crate::held::{Held, Stop};
use crate::process::{self, Ran};
use crate::settings::Project;

/// The name of the one tool.
pub const CHECK: &str = "check";

/// The tools, run in the works' worktrees in the data directory.
#[derive(Clone)]
pub struct Tools {
    inner: Arc<Inner>,
}

struct Inner {
    project: Project,
    data: PathBuf,
    held: Held<(WorkNumber, CallId), CallReply>,
}

/// What came of a call.
enum Answer {
    Result(String),
    Withdrawn,
}

impl Tools {
    /// The tools in this project, with this data directory, which is an absolute path.
    pub fn new(project: Project, data: PathBuf) -> Self {
        Tools {
            inner: Arc::new(Inner {
                project,
                data,
                held: Held::new(),
            }),
        }
    }
}

impl ToolsTrait for Tools {
    fn call(&self, call: ToolCall, reply: CallReply) {
        let key = (call.work, call.call);
        let Some(mut stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let answered = answer(&inner.project, &inner.data, call.work, &call.name, &mut stop).await;
            let result = match answered {
                Answer::Result(result) => result,
                Answer::Withdrawn => WITHDRAWN.to_string(),
            };
            if let Some(reply) = inner.held.take(key) {
                reply.done(result);
            }
        });
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        if let Some(reply) = self.inner.held.stop((call.work, call.call), reply) {
            reply.done(WITHDRAWN.to_string());
        }
    }
}

/// Runs the tool named in the work's worktree, and gives its result.
async fn answer(project: &Project, data: &Path, work: WorkNumber, name: &str, stop: &mut Stop) -> Answer {
    if name != CHECK {
        return Answer::Result(format!("No such tool: {name}. The tools are: {CHECK}."));
    }
    let check = &project.check;
    let ran = match git::worktree(data, work) {
        Ok(dir) => process::run(process::shell(check, &dir), None, stop).await,
        Err(why) => Err(why),
    };
    match ran {
        Ok(Ran::Ended { status, stdout, .. }) => {
            let passed = if status.success() {
                "passed".to_string()
            } else {
                format!("failed ({status})")
            };
            let tail = process::tail(&stdout);
            Answer::Result(format!(
                "The check `{check}` {passed}. The end of what it said:\n{tail}"
            ))
        }
        Ok(Ran::Stopped) => Answer::Withdrawn,
        Err(why) => Answer::Result(format!("The check `{check}` could not be run: {why}")),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::git::tests::Repo;

    const WORK: WorkNumber = WorkNumber(1);

    async fn answered(repo: &Repo, name: &str) -> String {
        match answer(&repo.project, &repo.data, WORK, name, &mut Stop::never()).await {
            Answer::Result(result) => result,
            Answer::Withdrawn => panic!("nothing withdrew it"),
        }
    }

    #[tokio::test]
    async fn the_check_says_whether_it_passed_with_the_end_of_what_it_said() {
        let repo = Repo::new("echo all is well");
        git::start(&repo.project, &repo.data, WORK).await.unwrap();
        assert_eq!(
            answered(&repo, CHECK).await,
            "The check `echo all is well` passed. The end of what it said:\nall is well\n"
        );
        let failing = Repo::new("echo it broke; exit 2");
        git::start(&failing.project, &failing.data, WORK).await.unwrap();
        let said = answered(&failing, CHECK).await;
        assert!(said.starts_with("The check `echo it broke; exit 2` failed ("), "{said}");
        assert!(said.ends_with("it broke\n"), "{said}");
    }

    #[tokio::test]
    async fn another_tool_is_none() {
        let repo = Repo::new("true");
        assert_eq!(answered(&repo, "ci").await, "No such tool: ci. The tools are: check.");
    }

    #[tokio::test]
    async fn a_check_with_no_worktree_says_so() {
        let repo = Repo::new("true");
        let said = answered(&repo, CHECK).await;
        assert!(
            said.starts_with("The check `true` could not be run: the work has no worktree"),
            "{said}"
        );
    }

    #[tokio::test]
    async fn a_withdrawn_check_is_stopped_where_it_runs() {
        let repo = Repo::new("sleep 30");
        git::start(&repo.project, &repo.data, WORK).await.unwrap();
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let began = Instant::now();
        let withdrawing = async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(held.stop(1, ()).is_none(), "what runs it is told to stop");
        };
        let (answer, ()) = tokio::join!(answer(&repo.project, &repo.data, WORK, CHECK, &mut stop), withdrawing);
        assert!(matches!(answer, Answer::Withdrawn));
        assert!(began.elapsed() < Duration::from_secs(20), "the command was stopped");
    }
}
