//! The tools an agent may ask for: `read`, which answers with the blocks of the work as it is kept (the module
//! `read`); `check`, which checks the work as the host checks it ([`Host::check`]): the project's check command run in
//! the work's worktree where the local repository is the host, the CI of the last commit of the work's branch where
//! GitHub is; and `check_before`, which checks in the same way the code before the work's change with the files its
//! input names put on it, so that a test is seen red or green before the change as well as after it. A tool's result
//! is all an agent learns of it, so what goes wrong in running it is said in its result, once: it is not tried again
//! here, and the agent may ask again. Withdrawn, what runs it is stopped. A `check` of a branch that does not merge
//! into main's head is answered at once with the files in conflict (the module `conflict`).

mod conflict;
mod query;

use std::path::PathBuf;
use std::sync::Arc;

use decide::CallId;
use service::{CallReply, ToolCall, Tools as ToolsTrait, WITHDRAWN};
use store::WorkNumber;

use crate::git::{self, Before};
use crate::held::{Held, Stop};
use crate::host::{Branch, Host, Named};
use crate::read::{self, READ};
use crate::settings::Project;

pub use query::{QUERY, Querying};

/// The name of the tool that checks the work.
pub const CHECK: &str = "check";

/// The name of the tool that checks the code before the work's change with some of its files put on it.
pub const CHECK_BEFORE: &str = "check_before";

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
    /// The tool `query`, closed until the settings open it.
    query: Querying,
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
                query: Querying::closed(),
            }),
        }
    }

    /// The tools with the tool `query` as this says it is.
    pub fn querying(mut self, query: Querying) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("the tools are not shared yet")
            .query = query;
        self
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
            } else if call.name == QUERY {
                // The query runs in a thread of its own, within its time limit: it blocks the one that asked.
                let asked = inner.clone();
                let input = call.input.clone();
                let said = tokio::task::spawn_blocking(move || asked.query.answer(&input)).await;
                Answer::Result(said.unwrap_or_else(|e| format!("The query could not be run: {e}")))
            } else {
                let place = git::place(&inner.data, &inner.tag, call.work);
                // What the work says of itself, as the acts work it out, so that the pull request the tool brings up
                // to date says what the check before the merge has it say.
                let about = crate::acts::about(&call.flow, &call.kept);
                let branch = Branch {
                    work: call.work,
                    title: &call.item.title,
                    about: &about,
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
    let checked = match (name, asked(input)) {
        (CHECK | CHECK_BEFORE, Err(why)) => Ok(Some(why)),
        (CHECK, Ok(asked)) if !asked.files.is_empty() => Ok(Some(FILES_BEFORE.to_string())),
        (CHECK, Ok(asked)) => check(host, project, branch, &asked.named, stop).await,
        (CHECK_BEFORE, Ok(asked)) => before(host, project, branch, &asked, stop).await,
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

/// Checks the work as the host does, unless its branch does not merge into main's head as the host last gave it, which
/// is said at once; an `Err` says why it could not.
async fn check(
    host: &impl Host,
    project: &Project,
    branch: &Branch<'_>,
    named: &Named,
    stop: &mut Stop,
) -> Result<Option<String>, String> {
    let could_not = |why: String| format!("The check could not be run: {why}");
    let found = conflict::found(project, branch.place, &host.kept_main()).await;
    if let Some(said) = found.map_err(could_not)? {
        return Ok(Some(said));
    }
    host.check(branch, named, stop).await.map_err(could_not)
}

/// What a check's input asks for: the tests and checks named, and, for the check before the change, the files.
#[derive(Debug, Default, PartialEq, Eq)]
struct Asked {
    named: Named,
    files: Vec<String>,
}

/// What a check's input asks for, a line each: `test: <name>`, `check: <name>` or `file: <path>`, each trimmed and
/// taken once, blank lines left out; or why it cannot be read, in words for the agent.
fn asked(input: &str) -> Result<Asked, String> {
    let mut asked = Asked::default();
    let mut wrong = Vec::new();
    for line in input.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let into = match line.split_once(':') {
            Some(("test", _)) => &mut asked.named.tests,
            Some(("check", _)) => &mut asked.named.checks,
            Some(("file", _)) => &mut asked.files,
            _ => {
                wrong.push(format!("{line:?}"));
                continue;
            }
        };
        let value = line.split_once(':').map_or("", |(_, v)| v.trim()).to_string();
        if !value.is_empty() && !into.contains(&value) {
            into.push(value);
        }
    }
    if wrong.is_empty() {
        Ok(asked)
    } else {
        Err(format!(
            "Nothing was checked: each line of the input is `test: <name>`, `check: <name>` or `file: <path>`, and \
             these are not: {}.",
            wrong.join(", ")
        ))
    }
}

/// What `check_before` says when its input names no file.
const NO_FILES: &str = "Nothing was checked: name in the input, a line `file: <path>` each, the files of the tests to \
                        put on the code before the change.";

/// What `check` says when its input names files, which only the check before the change takes.
const FILES_BEFORE: &str = "Nothing was checked: files are put on the code before the change by check_before; check \
                            checks your last commit as it is.";

/// Checks the code before the work's change with the files asked for put on it, as the work's last commit holds them,
/// running the tests named, and says what that is made of before what the host found; an `Err` says why it could not.
async fn before(
    host: &impl Host,
    project: &Project,
    branch: &Branch<'_>,
    asked: &Asked,
    stop: &mut Stop,
) -> Result<Option<String>, String> {
    let could_not = |why: String| format!("The check before the change could not be run: {why}");
    if asked.files.is_empty() {
        return Ok(Some(NO_FILES.to_string()));
    }
    let made = git::before(project, branch.place, &host.kept_main(), &asked.files).await;
    let before = match made.map_err(could_not)? {
        Ok(before) => before,
        Err(why) => return Ok(Some(why)),
    };
    let what = made_of(project, &before);
    let said = host.check_before(branch, &before, &asked.named, stop).await;
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
        answered_on(&host, repo, name, input).await
    }

    /// The tool's answer to this input on this host.
    async fn answered_on(host: &Local, repo: &Repo, name: &str, input: &str) -> String {
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        match answer(host, &repo.project, &branch, name, input, &mut Stop::never()).await {
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
            about: "",
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
        let said = answered_with(&repo, CHECK_BEFORE, "file: test.sh", &[]).await;
        let made = format!(
            "The code before the change, main at {main}, with test.sh as your last commit {head} holds them, is the \
             commit "
        );
        assert!(said.starts_with(&made), "{said}");
        assert!(
            said.contains("\n\nThe check `sh test.sh` failed (") && said.contains(") on it. The end of what it said:"),
            "{said}"
        );
        assert!(befores(&started.place).is_empty(), "its worktree is taken away");
        let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 2);
    }

    #[tokio::test]
    async fn a_check_before_the_change_naming_no_file_or_one_not_committed_checks_nothing() {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        assert_eq!(answered_with(&repo, CHECK_BEFORE, " ", &[]).await, NO_FILES);
        std::fs::write(started.place.dir.join("test.sh"), "true\n").unwrap();
        let said = answered_with(&repo, CHECK_BEFORE, "file: test.sh", &[]).await;
        assert!(
            said.starts_with("Nothing was checked: test.sh is not a file of your last commit"),
            "{said}"
        );
    }

    /// The worktrees of checks before the change of the work's that are there beside its own.
    fn befores(place: &git::Place) -> Vec<String> {
        let name = place.dir.file_name().unwrap().to_string_lossy().into_owned();
        let dir = place.dir.parent().unwrap();
        let mut found: Vec<String> = (std::fs::read_dir(dir).unwrap())
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(&format!("{name}-before")))
            .collect();
        found.sort();
        found
    }

    /// A started work whose last commit holds `test.sh` and `other.sh`, with the check given.
    async fn with_tests(check: &str) -> (Repo, git::Place) {
        let repo = Repo::new(check);
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        commit(&started.place.dir, "other.sh", "true\n");
        commit(&started.place.dir, "test.sh", "true\n");
        (repo, started.place)
    }

    #[tokio::test]
    async fn checks_before_the_change_asked_at_once_each_run_in_a_worktree_of_their_own() {
        // Each check is still running when the others begin: had they one worktree, a later one would take it from
        // under an earlier one, whose check would then fail on a tree no longer there.
        let (repo, _) = with_tests("sleep 1; test -f test.sh").await;
        // One host, as the tools have.
        let host = Local::new(repo.project.clone());
        let (one, same, other) = tokio::join!(
            answered_on(&host, &repo, CHECK_BEFORE, "file: test.sh"),
            answered_on(&host, &repo, CHECK_BEFORE, "file: test.sh"),
            answered_on(&host, &repo, CHECK_BEFORE, "file: test.sh\nfile: other.sh"),
        );
        for said in [&one, &same, &other] {
            assert!(
                said.contains("\n\nThe check `sleep 1; test -f test.sh` passed on it."),
                "{said}"
            );
        }
    }

    #[tokio::test]
    async fn what_an_earlier_check_before_the_change_of_this_work_left_is_taken_away() {
        let (repo, place) = with_tests("true").await;
        // As left by a process of the engine that ended while the check ran, and one of another work.
        let name = place.dir.file_name().unwrap().to_string_lossy().into_owned();
        let left = place.dir.with_file_name(format!("{name}-before-deadbeef0000"));
        let others = place.dir.with_file_name("2-0a1b2c3d-before-deadbeef0000");
        for dir in [&left, &others] {
            run(
                &repo.project.repo,
                &["worktree", "add", "-q", "--detach", &dir.display().to_string(), "main"],
            );
        }
        let said = answered_with(&repo, CHECK_BEFORE, "file: test.sh", &[]).await;
        assert!(said.contains("passed on it."), "{said}");
        assert!(befores(&place).is_empty(), "{:?}", befores(&place));
        assert!(others.is_dir(), "another work's is let be");
        let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 3);
    }

    #[tokio::test]
    async fn a_check_before_the_change_refuses_a_worktree_off_the_works_branch() {
        let (repo, place) = with_tests("true").await;
        run(&place.dir, &["checkout", "-q", "--detach"]);
        let said = answered_with(&repo, CHECK_BEFORE, "file: test.sh", &[]).await;
        assert!(
            said.starts_with("The check before the change could not be run: the worktree")
                && said.contains("not on the work's branch"),
            "{said}"
        );
    }

    #[test]
    fn a_checks_input_is_read_a_line_each_of_tests_checks_and_files() {
        let input = "test: tools::tests::a\n\n  test:  src/x.test.ts > a describe > its title \ncheck: lint\n\
                     test: tools::tests::a\nfile: crates/carry/src/tools.rs\n";
        let read = asked(input).expect("read");
        assert_eq!(
            read,
            Asked {
                named: Named {
                    tests: vec![
                        "tools::tests::a".to_string(),
                        "src/x.test.ts > a describe > its title".to_string()
                    ],
                    checks: vec!["lint".to_string()],
                },
                files: vec!["crates/carry/src/tools.rs".to_string()],
            }
        );
        assert_eq!(asked("").unwrap(), Asked::default());
        let wrong = asked("test: a\ntools::tests::b\nlint").unwrap_err();
        assert!(
            wrong.ends_with("these are not: \"tools::tests::b\", \"lint\"."),
            "{wrong}"
        );
    }

    #[tokio::test]
    async fn a_check_whose_project_runs_whole_says_the_names_are_not_run_apart() {
        let repo = Repo::new("echo all is well");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = answered_with(&repo, CHECK, "test: tools::tests::a", &[]).await;
        assert!(
            said.starts_with(
                "This check runs whole: the tests and checks you named are not run apart.\n\nThe check `echo all is \
                 well` passed."
            ),
            "{said}"
        );
        assert_eq!(answered_with(&repo, CHECK, "file: a.rs", &[]).await, FILES_BEFORE);
    }

    #[tokio::test]
    async fn a_check_that_could_not_be_run_says_why_once_and_is_not_tried_again() {
        let repo = Repo::new("true");
        let host = crate::host::fake::Fake::new("main");
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        let said = match answer(&host, &repo.project, &branch, CHECK, "", &mut Stop::never()).await {
            Answer::Result(said) => said,
            Answer::Withdrawn => panic!("nothing withdrew it"),
        };
        let why = "the fake host checks nothing for the agents' tools";
        assert_eq!(said, format!("The check could not be run: {why}"));
    }
}
