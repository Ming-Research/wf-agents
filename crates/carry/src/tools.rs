//! The tools an agent may ask for: `read`, which answers with the blocks of the work as it is kept (the module
//! `read`); `query`, one read-only SQL query on the store, run in a thread of its own and interrupted where it is
//! withdrawn (the module `query`); and `run_tests`, which runs the tests and checks the request names, or the whole CI
//! where it names none, as the host runs them ([`Host::run_tests`]): the project's check command in the work's worktree
//! where the local repository is the host, the CI of the last commit of the work's branch where GitHub is. A tool's
//! result is all an agent learns of it, so what goes wrong in running it is said in its result, once: it is not tried
//! again here, and the agent may ask again. Withdrawn, what runs it is stopped. A `run_tests` of a branch that does not
//! merge into main's head is answered at once with the files in conflict (the module `conflict`). The result is kept
//! whole in the records; what is given to the agent of it is cut where the message is made (module `message`).

mod conflict;
mod query;

use std::path::PathBuf;
use std::sync::Arc;

use decide::CallId;
use flow::agents::fill;
use serde::Deserialize;
use service::{CallReply, ToolCall, Tools as ToolsTrait, WITHDRAWN};
use store::WorkNumber;

use crate::git;
use crate::held::{Held, Stop};
use crate::host::{Branch, Host, Named};
use crate::read::{self, READ};
use crate::settings::Project;

pub use query::{QUERY, Querying};

/// The name of the tool that runs the work's tests and checks.
pub const RUN_TESTS: &str = "run_tests";

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
    /// What the agent is told of a request it cannot be read, from `agents/tools/run_tests.toml`.
    unread: Option<String>,
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
    /// The tools on this host in this project, with this data directory, which is an absolute path, the
    /// installation's tag, and what an agent is told of a request of `run_tests` that cannot be read, `{error}` what
    /// the reading said.
    pub fn new(host: H, project: Project, data: PathBuf, tag: String, unread: Option<String>) -> Self {
        Tools {
            inner: Arc::new(Inner {
                host,
                project,
                data,
                tag,
                unread,
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
                // The query runs in a thread of its own, within its time limit; withdrawn, it is interrupted.
                let asked = inner.clone();
                let input = call.input.clone();
                let cancel = Arc::new(query::Cancel::default());
                let told = cancel.clone();
                let mut running = tokio::task::spawn_blocking(move || asked.query.answer(&input, &told));
                tokio::select! {
                    said = &mut running => Answer::Result(said.unwrap_or_else(|_| inner.query.unavailable())),
                    () = stop.told() => {
                        cancel.tell();
                        Answer::Withdrawn
                    }
                }
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
                let tools = flow::form::tools(&call.flow);
                let told = Told {
                    unread: inner.unread.as_deref(),
                    tools: &tools,
                };
                answer(
                    &inner.host,
                    &inner.project,
                    &branch,
                    &told,
                    &call.name,
                    &call.input,
                    &mut stop,
                )
                .await
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

/// What a result of `answer` says that is the flow's: the words an unreadable request is told in, and the tools the
/// flow gives its agents.
struct Told<'a> {
    unread: Option<&'a str>,
    tools: &'a [&'a str],
}

/// Runs the tool named, other than `read` and `query`, on the work's branch, with this input, and gives its result.
async fn answer(
    host: &impl Host,
    project: &Project,
    branch: &Branch<'_>,
    told: &Told<'_>,
    name: &str,
    input: &str,
    stop: &mut Stop,
) -> Answer {
    if name != RUN_TESTS {
        let tools = told.tools.join(", ");
        return Answer::Result(format!("No such tool: {name}. The tools are: {tools}."));
    }
    let checked = match asked(input) {
        Err(why) => Ok(Some(
            told.unread.map_or(why.clone(), |words| fill(words, &[("error", &why)])),
        )),
        Ok(named) => run_tests(host, project, branch, &named, stop).await,
    };
    match checked {
        Ok(Some(said)) | Err(said) => Answer::Result(said),
        Ok(None) => Answer::Withdrawn,
    }
}

/// Runs the tests and checks as the host does, unless the work's branch does not merge into main's head as the host
/// last gave it, which is said at once; an `Err` says why it could not.
async fn run_tests(
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
    host.run_tests(branch, named, stop).await.map_err(could_not)
}

/// A request of `run_tests`: the tests and the checks it names, each list left out where it names none.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    checks: Vec<String>,
}

/// What a request names, each name trimmed and taken once, a blank one left out; an input with nothing in it names
/// nothing. Or why the input is not a request, as the reading says it.
fn asked(input: &str) -> Result<Named, String> {
    if input.trim().is_empty() {
        return Ok(Named::default());
    }
    let request: Request = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let once = |names: Vec<String>| {
        let mut taken: Vec<String> = Vec::new();
        for name in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
            if !taken.iter().any(|t| t == name) {
                taken.push(name.to_string());
            }
        }
        taken
    };
    Ok(Named {
        tests: once(request.tests),
        checks: once(request.checks),
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::git::tests::Repo;
    use crate::host::Local;
    use crate::host::fake::{Asked, Fake};

    const WORK: WorkNumber = WorkNumber(1);

    /// The local host, saying what `run_tests` says as this repository's file beside the flows has it.
    fn local(repo: &Repo) -> Local {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../flow/agents/tools/run_tests.toml"
        ));
        let tool: flow::agents::Tool = toml::from_str(text).expect("the file of run_tests reads");
        Local::new(repo.project.clone()).saying(Some(&tool))
    }

    async fn answered(repo: &Repo, name: &str) -> String {
        answered_with(repo, name, "", &[]).await
    }

    /// The tool's answer to this input, the check being given these variables of the engine's environment.
    async fn answered_with(repo: &Repo, name: &str, input: &str, env: &[String]) -> String {
        let host = local(repo).giving(env.to_vec());
        answered_on(&host, repo, name, input).await
    }

    /// The tool's answer to this input on this host.
    async fn answered_on(host: &impl Host, repo: &Repo, name: &str, input: &str) -> String {
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        match answer(host, &repo.project, &branch, &TOLD, name, input, &mut Stop::never()).await {
            Answer::Result(result) => result,
            Answer::Withdrawn => panic!("nothing withdrew it"),
        }
    }

    /// The words an unreadable request is told in, as a file of the flow gives them.
    const TOLD: Told<'static> = Told {
        unread: Some("The request cannot be read: {error}"),
        tools: &["query", "run_tests"],
    };

    #[tokio::test]
    async fn the_tests_say_whether_they_passed_with_what_they_said() {
        let repo = Repo::new("echo all is well");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        assert_eq!(
            answered(&repo, RUN_TESTS).await,
            "The check `echo all is well` passed. What it said:\nall is well\n"
        );
        let failing = Repo::new("echo it broke; exit 2");
        git::start(&failing.project, &failing.place(WORK), "main")
            .await
            .unwrap();
        let said = answered(&failing, RUN_TESTS).await;
        assert!(said.starts_with("The check `echo it broke; exit 2` failed ("), "{said}");
        assert!(said.ends_with("it broke\n"), "{said}");
    }

    #[tokio::test]
    async fn what_the_tests_said_is_given_whole_to_be_kept_whole() {
        let repo = Repo::new("seq 1 3000");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = answered(&repo, RUN_TESTS).await;
        assert!(
            said.contains("\n1\n2\n3\n") && said.ends_with("\n2999\n3000\n"),
            "{said}"
        );
    }

    #[tokio::test]
    async fn the_tests_are_given_of_the_engines_environment_only_what_the_agents_are() {
        // Cargo gives the tests' process this variable, which no agent is given unless the settings name it: the
        // token for GitHub is one such.
        assert!(std::env::var("CARGO_PKG_NAME").is_ok(), "cargo runs the tests");
        let repo = Repo::new("env");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = answered(&repo, RUN_TESTS).await;
        assert!(said.contains("\nHOME=") || said.contains(":\nHOME="), "{said}");
        assert!(!said.contains("CARGO_PKG_NAME="), "{said}");
        let named = answered_with(&repo, RUN_TESTS, "", &["CARGO_PKG_NAME".to_string()]).await;
        assert!(named.contains("CARGO_PKG_NAME=carry"), "{named}");
    }

    #[tokio::test]
    async fn a_tool_the_engine_does_not_have_is_none_and_the_old_checks_are_gone() {
        let repo = Repo::new("true");
        for name in ["ci", "check", "check_before"] {
            assert_eq!(
                answered(&repo, name).await,
                format!("No such tool: {name}. The tools are: query, run_tests.")
            );
        }
    }

    #[tokio::test]
    async fn tests_with_no_worktree_say_so() {
        let repo = Repo::new("true");
        let said = answered(&repo, RUN_TESTS).await;
        assert!(
            said.starts_with("The check `true` could not be run: the work has no worktree"),
            "{said}"
        );
    }

    #[tokio::test]
    async fn withdrawn_tests_are_stopped_where_they_run() {
        let repo = Repo::new("sleep 30");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let began = Instant::now();
        let withdrawing = async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(held.stop(1, ()).is_none(), "what runs it is told to stop");
        };
        let host = local(&repo);
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        let asked = answer(&host, &repo.project, &branch, &TOLD, RUN_TESTS, "", &mut stop);
        let (answer, ()) = tokio::join!(asked, withdrawing);
        assert!(matches!(answer, Answer::Withdrawn));
        assert!(began.elapsed() < Duration::from_secs(20), "the command was stopped");
    }

    #[test]
    fn a_request_names_tests_and_checks_each_once_or_nothing_for_the_whole() {
        let input = r#"{"tests": ["tools::tests::a", "  src/x.test.ts > a describe > its title ", "tools::tests::a", " "],
                        "checks": ["lint"]}"#;
        assert_eq!(
            asked(input).expect("read"),
            Named {
                tests: vec![
                    "tools::tests::a".to_string(),
                    "src/x.test.ts > a describe > its title".to_string()
                ],
                checks: vec!["lint".to_string()],
            }
        );
        for whole in ["", "  ", "{}", r#"{"tests": [], "checks": []}"#] {
            assert_eq!(asked(whole).unwrap(), Named::default(), "{whole:?}");
        }
        for wrong in [r#"{"test": ["a"]}"#, r#"{"tests": "a"}"#, "tools::tests::a"] {
            assert!(asked(wrong).is_err(), "{wrong:?}");
        }
    }

    #[tokio::test]
    async fn a_request_that_cannot_be_read_is_sent_back_in_the_words_of_the_flow_and_runs_nothing() {
        let repo = Repo::new("true");
        let host = Fake::new("main").runs("ran");
        let said = answered_on(&host, &repo, RUN_TESTS, r#"{"test": ["a"]}"#).await;
        assert!(
            said.starts_with("The request cannot be read: unknown field `test`"),
            "{said}"
        );
        assert!(host.asked().is_empty(), "{:?}", host.asked());
    }

    #[tokio::test]
    async fn the_host_is_asked_for_what_the_request_names_and_what_it_says_is_the_result() {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        crate::git::tests::commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        let host = Fake::new("main").runs("ran what was named");
        let input = r#"{"tests": ["a"], "checks": ["lint"]}"#;
        assert_eq!(answered_on(&host, &repo, RUN_TESTS, input).await, "ran what was named");
        assert_eq!(answered_on(&host, &repo, RUN_TESTS, "{}").await, "ran what was named");
        let named = Named {
            tests: vec!["a".to_string()],
            checks: vec!["lint".to_string()],
        };
        assert_eq!(
            host.asked(),
            [Asked::RunTests(named), Asked::RunTests(Named::default())]
        );
    }

    #[tokio::test]
    async fn tests_whose_project_runs_whole_say_the_names_are_not_run_apart() {
        let repo = Repo::new("echo all is well");
        git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let said = answered_with(&repo, RUN_TESTS, r#"{"tests": ["tools::tests::a"]}"#, &[]).await;
        assert!(
            said.starts_with(
                "The tests and checks you named are not run apart: the whole command runs.\n\nThe check `echo all is \
                 well` passed."
            ),
            "{said}"
        );
    }

    #[tokio::test]
    async fn tests_that_could_not_be_run_say_why_once_and_are_not_tried_again() {
        let repo = Repo::new("true");
        let host = Fake::new("main");
        let said = answered_on(&host, &repo, RUN_TESTS, "").await;
        let why = crate::host::fake::NO_TOOLS;
        assert_eq!(said, format!("The check could not be run: {why}"));
        assert_eq!(host.asked(), [Asked::RunTests(Named::default())]);
    }
}
