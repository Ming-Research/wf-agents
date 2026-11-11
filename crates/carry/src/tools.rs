//! The tools an agent may ask for: `read`, which answers with the blocks of the work as it is kept (the module
//! `read`); `check`, which checks the work as the host checks it ([`Host::check`]): the project's check command run in
//! the work's worktree where the local repository is the host, the CI of the last commit of the work's branch where
//! GitHub is; and `check_before`, which checks in the same way the code before the work's change with the files its
//! input names put on it, so that a test is seen red or green before the change as well as after it. A tool's result
//! is all an agent learns of it, so what goes wrong in running it is said in its result, once: it is not tried again
//! here, and the agent may ask again. Withdrawn, what runs it is stopped.

use std::path::PathBuf;
use std::sync::Arc;

use decide::CallId;
use service::{CallReply, ToolCall, Tools as ToolsTrait, WITHDRAWN};
use store::WorkNumber;

use crate::git::{self, Before};
use crate::held::{Held, Stop};
use crate::host::{Branch, Host};
use crate::read::{self, READ};
use crate::settings::{Project, Settings};

/// The name of the tool that checks the work.
pub const CHECK: &str = "check";

/// The name of the tool that checks the code before the work's change with some of its files put on it.
pub const CHECK_BEFORE: &str = "check_before";

/// What the agents are told `check` does where the local repository is the host.
const CHECK_HERE: &str = "which runs the project's check in the worktree and says whether it passed, with the end of \
                          what it said.";

/// What the agents are told `check` does where GitHub is the host.
const CHECK_ON_GITHUB: &str = "which pushes your last commit to the work's pull request and gives the CI's verdict on \
                               it: each check's result, and what the log of a red one says of the failure. Only what \
                               is committed is checked: with changes not committed, nothing is pushed.";

/// What the agents are told `check` does, on the host the settings say: put into their system prompt.
pub(crate) fn told(settings: &Settings) -> &'static str {
    if settings.github.is_some() {
        CHECK_ON_GITHUB
    } else {
        CHECK_HERE
    }
}

/// The tools, run on this host, with the works' worktrees in the data directory.
pub struct Tools<H> {
    inner: Arc<Inner<H>>,
}

struct Inner<H> {
    host: H,
    project: Project,
    data: PathBuf,
    /// The installation's part of the names of the works' worktrees.
    tag: String,
    held: Held<(WorkNumber, CallId), CallReply>,
}

/// What came of a call.
enum Answer {
    Result(String),
    Withdrawn,
}

impl<H: Host> Tools<H> {
    /// The tools on this host in this project, with this data directory, which is an absolute path, and the
    /// installation's tag.
    pub fn new(host: H, project: Project, data: PathBuf, tag: String) -> Self {
        Tools {
            inner: Arc::new(Inner {
                host,
                project,
                data,
                tag,
                held: Held::new(),
            }),
        }
    }
}

impl<H: Host> ToolsTrait for Tools<H> {
    fn call(&self, call: ToolCall, reply: CallReply) {
        self.prune(&call);
        let key = (call.work, call.call);
        let Some(mut stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let answered = if call.name == READ {
                Answer::Result(read::answer(&call.flow, &call.kept, &call.input))
            } else {
                let place = git::place(&inner.data, &inner.tag, call.work);
                let branch = Branch {
                    work: call.work,
                    title: &call.item.title,
                    place: &place,
                };
                answer(&inner.host, &inner.project, &branch, &call.name, &call.input, &mut stop).await
            };
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
        self.prune(&call);
        if let Some(reply) = self.inner.held.stop((call.work, call.call), reply) {
            reply.done(WITHDRAWN.to_string());
        }
    }
}

impl<H> Tools<H> {
    /// Forgets the calls of the work whose result the loop has taken, as its records show them.
    fn prune(&self, call: &ToolCall) {
        let done = &call.kept.records.done;
        (self.inner.held).prune(|(work, id)| *work == call.work && done.iter().any(|d| d.call == *id));
    }
}

/// Runs the tool named, other than `read`, on the work's branch, with this input, and gives its result.
async fn answer(
    host: &impl Host,
    project: &Project,
    branch: &Branch<'_>,
    name: &str,
    input: &str,
    stop: &mut Stop,
) -> Answer {
    let checked = match name {
        CHECK => (host.check(branch, stop).await).map_err(|why| format!("The check could not be run: {why}")),
        CHECK_BEFORE => before(host, project, branch, input, stop).await,
        _ => {
            return Answer::Result(format!(
                "No such tool: {name}. The tools are: {READ}, {CHECK}, {CHECK_BEFORE}."
            ));
        }
    };
    match checked {
        Ok(Some(said)) | Err(said) => Answer::Result(said),
        Ok(None) => Answer::Withdrawn,
    }
}

/// What `check_before` says when its input names no file.
const NO_FILES: &str = "Nothing was checked: name in the input the files of the tests to put on the code before the \
                        change, each apart from the next by a space.";

/// Checks the code before the work's change with the files the input names put on it, as the work's last commit holds
/// them, and says what that is made of before what the host found; an `Err` says why it could not.
async fn before(
    host: &impl Host,
    project: &Project,
    branch: &Branch<'_>,
    input: &str,
    stop: &mut Stop,
) -> Result<Option<String>, String> {
    let could_not = |why: String| format!("The check before the change could not be run: {why}");
    let files: Vec<String> = input.split_whitespace().map(str::to_string).collect();
    if files.is_empty() {
        return Ok(Some(NO_FILES.to_string()));
    }
    let made = git::before(project, branch.place, &host.kept_main(), &files).await;
    let before = match made.map_err(could_not)? {
        Ok(before) => before,
        Err(why) => return Ok(Some(why)),
    };
    let what = made_of(project, &before);
    let said = host.check_before(branch, &before, stop).await;
    let said = said.map_err(|why| format!("{what}\n\n{}", could_not(why)))?;
    Ok(said.map(|said| format!("{what}\n\n{said}")))
}

/// What the commit the check before the change checks is made of.
fn made_of(project: &Project, before: &Before) -> String {
    format!(
        "The code before the change, {} at {}, with {} as your last commit {} holds them, is the commit {}.",
        project.main,
        before.base,
        before.files.join(", "),
        before.head,
        before.commit
    )
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::git::tests::{Repo, commit, run};
    use crate::host::Local;

    const WORK: WorkNumber = WorkNumber(1);

    async fn answered(repo: &Repo, name: &str) -> String {
        answered_with(repo, name, "", &[]).await
    }

    /// The tool's answer to this input, the check being given these variables of the engine's environment.
    async fn answered_with(repo: &Repo, name: &str, input: &str, env: &[String]) -> String {
        let host = Local::new(repo.project.clone()).giving(env.to_vec());
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            place: &place,
        };
        match answer(&host, &repo.project, &branch, name, input, &mut Stop::never()).await {
            Answer::Result(result) => result,
            Answer::Withdrawn => panic!("nothing withdrew it"),
        }
    }

    #[tokio::test]
    async fn the_check_says_whether_it_passed_with_the_end_of_what_it_said() {
        let repo = Repo::new("echo all is well");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        assert_eq!(
            answered(&repo, CHECK).await,
            "The check `echo all is well` passed. The end of what it said:\nall is well\n"
        );
        let failing = Repo::new("echo it broke; exit 2");
        git::start(&failing.project, &failing.place(WORK), "main")
            .await
            .unwrap();
        let said = answered(&failing, CHECK).await;
        assert!(said.starts_with("The check `echo it broke; exit 2` failed ("), "{said}");
        assert!(said.ends_with("it broke\n"), "{said}");
    }

    #[tokio::test]
    async fn the_check_is_given_of_the_engines_environment_only_what_the_agents_are() {
        // Cargo gives the tests' process this variable, which no agent is given unless the settings name it: the
        // token for GitHub is one such.
        assert!(std::env::var("CARGO_PKG_NAME").is_ok(), "cargo runs the tests");
        let repo = Repo::new("env");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = answered(&repo, CHECK).await;
        assert!(said.contains("\nHOME=") || said.contains(":\nHOME="), "{said}");
        assert!(!said.contains("CARGO_PKG_NAME="), "{said}");
        let named = answered_with(&repo, CHECK, "", &["CARGO_PKG_NAME".to_string()]).await;
        assert!(named.contains("CARGO_PKG_NAME=carry"), "{named}");
    }

    #[tokio::test]
    async fn another_tool_is_none() {
        let repo = Repo::new("true");
        assert_eq!(
            answered(&repo, "ci").await,
            "No such tool: ci. The tools are: read, check, check_before."
        );
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
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let began = Instant::now();
        let withdrawing = async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(held.stop(1, ()).is_none(), "what runs it is told to stop");
        };
        let host = Local::new(repo.project.clone());
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            place: &place,
        };
        let asked = answer(&host, &repo.project, &branch, CHECK, "", &mut stop);
        let (answer, ()) = tokio::join!(asked, withdrawing);
        assert!(matches!(answer, Answer::Withdrawn));
        assert!(began.elapsed() < Duration::from_secs(20), "the command was stopped");
    }

    #[tokio::test]
    async fn the_check_before_the_change_runs_the_check_on_main_with_the_tests_put_on_it() {
        let repo = Repo::new("sh test.sh");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        std::fs::write(started.place.dir.join("greeting.txt"), "Bye.\n").unwrap();
        run(&started.place.dir, &["add", "greeting.txt"]);
        let head = commit(&started.place.dir, "test.sh", "grep Bye greeting.txt\n");
        let after = answered(&repo, CHECK).await;
        assert!(after.starts_with("The check `sh test.sh` passed."), "{after}");
        let said = answered_with(&repo, CHECK_BEFORE, "test.sh", &[]).await;
        let made = format!(
            "The code before the change, main at {main}, with test.sh as your last commit {head} holds them, is the \
             commit "
        );
        assert!(said.starts_with(&made), "{said}");
        assert!(
            said.contains("\n\nThe check `sh test.sh` failed (") && said.contains(") on it. The end of what it said:"),
            "{said}"
        );
        assert!(!git::before_dir(&started.place).exists(), "its worktree is taken away");
        let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 2);
    }

    #[tokio::test]
    async fn a_check_before_the_change_naming_no_file_or_one_not_committed_checks_nothing() {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        assert_eq!(answered_with(&repo, CHECK_BEFORE, " ", &[]).await, NO_FILES);
        std::fs::write(started.place.dir.join("test.sh"), "true\n").unwrap();
        let said = answered_with(&repo, CHECK_BEFORE, "test.sh", &[]).await;
        assert!(
            said.starts_with("Nothing was checked: test.sh is not a file of your last commit"),
            "{said}"
        );
    }

    #[test]
    fn the_agents_are_told_how_the_check_runs_on_their_host() {
        let mut settings = Settings::read(include_str!("../install.sample.toml")).expect("the sample reads");
        assert_eq!(told(&settings), CHECK_HERE);
        settings.github = Some(toml::from_str("repository = \"owner/name\"\nchecks = [\"check\"]\n").unwrap());
        assert_eq!(told(&settings), CHECK_ON_GITHUB);
    }
}
