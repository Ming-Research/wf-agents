//! The loop with the real carriers, in a git repository made for each test, with a fake of the `claude` command line:
//! a shell script that keeps what it was given and prints a result that hands in the outputs of the cell its message
//! names. The flows are the trial flow (`crates/service/tests/trial/flow`), whose agents ask and judge, `flow` here,
//! which has the acts of an engineer's work, and the deliver flow as the repository defines it (`flow/deliver`), whose
//! one cell is the act that delivers a deliverable item.
//!
//! The tests are kept in modules by subject, so that a new test goes into the module of what it tests.

mod budget;
mod delivering;
mod failing;
mod going;
mod left;
mod permissions;
mod stopping;

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use carry::git::{self, Place};
use carry::settings::{Agents, Project};
use carry::{Settings, session};
use decide::{CallId, Decision, Pauser, RunId, Who};
use driver::{FirstWrite, NewItem};
use engine::Event;
use flow::{Compiled, Has};
use service::{Act, Inbox, Message, Round, RoundReply, Rounds, Service, SystemClock};
use store::{ItemNumber, Store, WorkNumber};
use tokio::sync::oneshot;
use work::Work;

/// How the fake command line answers.
pub struct Fake {
    /// The session's cost so far that every call reports, in dollars, as JSON.
    pub cost: &'static str,
    /// Whether its first call fails.
    pub fail_first: bool,
    /// Whether its first call prints a result that is an error, as when it ran out of retries of its structured
    /// return, the session having cost eighty cents by then, and exits with 1.
    pub error_first: bool,
    /// Whether the build asks for the check in its first round, and its second round, going on in the session, is
    /// told the first time that the session is not found.
    pub lose_session: bool,
    /// Whether the build asks the owner a question in its first round, and for the check in its second, before it
    /// hands in; with `lose_session`, its last round is the one told the session is not found.
    pub question_first: bool,
    /// How many seconds the plan's first call sleeps before it answers.
    pub plan_sleeps: u32,
}

impl Default for Fake {
    fn default() -> Self {
        Fake {
            cost: "0.01",
            fail_first: false,
            error_first: false,
            lose_session: false,
            question_first: false,
            plan_sleeps: 0,
        }
    }
}

/// Which flow a test runs on.
#[derive(Clone, Copy)]
pub enum On {
    /// The trial flow, which has no start and no check: the test makes the work's place.
    Trial,
    /// The flow here, with a start, a check and a merge.
    Acts,
    /// The deliver flow, whose one cell delivers the item once the items it is made of have merged.
    Deliver,
}

/// What the fake was given in one call.
pub struct Call {
    pub args: Vec<String>,
    pub stdin: String,
    /// The directory it ran in.
    pub dir: PathBuf,
    /// Whether a call before it was still running as it began.
    pub overlapped: bool,
}

/// The loop with the real carriers over a store in the data directory.
pub struct Real {
    _dir: tempfile::TempDir,
    pub data: PathBuf,
    pub repo: PathBuf,
    /// Where the fake keeps what it was given.
    calls: PathBuf,
    settings: Settings,
    flow: Compiled,
    service: Service,
    inbox: Inbox,
    /// How many rounds the carriers said failed, and how often they said a work is to wait for the owner.
    pub failures: usize,
    pub holds: usize,
}

/// Runs git in the directory, which must pass, and gives its output.
pub fn git_in(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().to_string()
}

/// The flow, compiled with what the real carriers have.
fn compiled(on: On) -> Compiled {
    let dir = match on {
        On::Trial => Path::new(env!("CARGO_MANIFEST_DIR")).join("../service/tests/trial/flow"),
        On::Acts => Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow"),
        On::Deliver => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/deliver"),
    };
    let source = flow::read(&dir).expect("the flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&[driver::DEPENDENCIES]),
        acts: names(&["start", "check", "merge", "deliver"]),
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// Writes the fake command line into the directory, keeping what it is given in `calls`.
fn fake(dir: &Path, calls: &Path, fake: &Fake) -> PathBuf {
    let yes = |on: bool| if on { "yes" } else { "no" };
    let script = format!(
        r#"#!/bin/sh
log='{calls}'
n=$(( $(ls "$log" | grep -c '\.args$') + 1 ))
for pid in "$log"/*.pid; do
    [ -f "$pid" ] && kill -0 "$(cat "$pid")" 2>/dev/null && echo "$pid" >> "$log/$n.overlap"
done
echo $$ > "$log/$n.pid"
printf '%s\n' "$@" > "$log/$n.args"
cat > "$log/$n.stdin"
pwd > "$log/$n.pwd"
if [ '{error}' = yes ] && [ "$n" = 1 ]; then
    printf '%s\n' '{{"type":"result","subtype":"error_max_structured_output_retries","is_error":true,"result":"Out of structured output retries.","total_cost_usd":0.8,"usage":{{"input_tokens":1000,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":300}}}}'
    exit 1
fi
if [ '{fail}' = yes ] && [ "$n" = 1 ]; then
    echo 'The fake fails its first call.' >&2
    exit 1
fi
first=$(head -n 1 "$log/$n.stdin")
asks='[]'
out=''
case "$first" in
    *'cell `plan`'*)
        if [ '{sleeps}' != 0 ] && [ ! -f "$log/slept" ]; then
            touch "$log/slept"
            sleep '{sleeps}'
        fi
        out='{{"block":"plan","content":"Greet by name.","matters":true}}' ;;
    *'cell `build`'*)
        round=${{first##*, round }}
        if [ "$round" = 1 ] && [ '{question}' = yes ]; then
            asks='[{{"question":"Which name?"}}]'
        elif {{ [ "$round" = 1 ] && [ '{lose}' = yes ]; }} || {{ [ "$round" = 2 ] && [ '{question}' = yes ]; }}; then
            asks='[{{"tool":"check","input":""}}]'
        else
            if [ '{lose}' = yes ] && [ ! -f "$log/lost" ] && grep -qx -- --resume "$log/$n.args"; then
                touch "$log/lost"
                echo 'No conversation found with session ID: 00000000-0000-5000-8000-000000000001' >&2
                exit 1
            fi
            git -c user.name=Fake -c user.email=fake@example.com commit -q --allow-empty -m 'Build the greeting'
            out='{{"block":"code","content":"A greeting.","matters":true}}'
        fi ;;
    *'cell `review`'*) out='{{"block":"verdict","content":"Fine.","value":"yes","matters":true}}' ;;
esac
printf '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":{cost},"usage":{{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":20}},"structured_output":{{"hands_in":[%s],"asks":%s}}}}\n' "$out" "$asks"
"#,
        calls = calls.display(),
        fail = yes(fake.fail_first),
        error = yes(fake.error_first),
        lose = yes(fake.lose_session),
        question = yes(fake.question_first),
        sleeps = fake.plan_sleeps,
        cost = fake.cost,
    );
    let path = dir.join("fake-claude");
    std::fs::write(&path, script).expect("the fake is written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("the fake can be run");
    path
}

/// The agents of the real carriers, but for their stops, which are dropped: a process of the engine that ends before
/// it stops a round.
struct Deaf(Arc<dyn Rounds>);

impl Rounds for Deaf {
    fn give(&self, round: Round, reply: RoundReply) {
        self.0.give(round, reply);
    }

    fn stop(&self, _: Round, _: RoundReply) {}
}

/// The loop with the real carriers of the settings, on an empty store or the one in the data directory; with agents
/// whose stops are dropped where `deaf`.
fn open(settings: &Settings, data: &Path, flow: &Compiled, deaf: bool) -> (Service, Inbox) {
    let mut carriers = carry::carriers(settings.clone(), data).expect("the carriers are made");
    if deaf {
        carriers.agents = Arc::new(Deaf(carriers.agents));
    }
    let store = Store::open(data.join("store.sqlite")).expect("the store opens");
    Service::open(Box::new(store), vec![flow.clone()], carriers, Arc::new(SystemClock)).expect("the loop opens")
}

impl Real {
    /// The loop on an empty store of the flow, with the fake answering as given and the budget given, the project's
    /// check being `check`.
    pub fn open(on: On, answers: Fake, budget: Option<f64>, check: &str) -> Real {
        let dir = tempfile::tempdir().expect("a directory");
        let root = std::fs::canonicalize(dir.path()).expect("the directory is there");
        let (repo, data, calls) = (root.join("repo"), root.join("data"), root.join("calls"));
        for made in [&repo, &data, &calls] {
            std::fs::create_dir_all(made).unwrap();
        }
        git_in(&repo, &["init", "-q", "-b", "main"]);
        git_in(&repo, &["config", "user.name", "Tester"]);
        git_in(&repo, &["config", "user.email", "tester@example.com"]);
        git_in(&repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(repo.join("greeting.txt"), "Hello.\n").unwrap();
        git_in(&repo, &["add", "greeting.txt"]);
        git_in(&repo, &["commit", "-q", "-m", "Greet"]);
        let command = fake(&root, &calls, &answers);
        let settings = Settings {
            project: Project {
                repo: repo.clone(),
                main: "main".to_string(),
                check: check.to_string(),
                allowed_config: Vec::new(),
            },
            agents: Agents {
                command: command.display().to_string(),
                model: "claude-haiku-5-5".to_string(),
                slots: 2,
                args: Vec::new(),
                env: Vec::new(),
                setting_sources: "user".to_string(),
                config_dir: None,
                owner_language: "Chinese".to_string(),
                work_budget_usd: budget,
            },
            github: None,
        };
        let flow = compiled(on);
        let (service, inbox) = open(&settings, &data, &flow, false);
        Real {
            _dir: dir,
            data,
            repo,
            calls,
            settings,
            flow,
            service,
            inbox,
            failures: 0,
            holds: 0,
        }
    }

    /// The trial flow, with the fake answering as given, no budget and a check that passes.
    pub fn trial(answers: Fake) -> Real {
        Real::open(On::Trial, answers, None, "true")
    }

    /// Stops the loop and its carriers and starts them again on the same store, as a new process of the engine
    /// would. What the old carriers run goes on running.
    pub fn restart(&mut self) {
        let (service, inbox) = open(&self.settings, &self.data, &self.flow, false);
        self.service = service;
        self.inbox = inbox;
    }

    /// Starts the loop again on the same store with agents that drop every stop they are given.
    pub fn deafen(&mut self) {
        let (service, inbox) = open(&self.settings, &self.data, &self.flow, true);
        self.service = service;
        self.inbox = inbox;
    }

    /// The owner's call the work's run waits for, where there is one.
    pub fn owners_call(&self, work: WorkNumber) -> Option<CallId> {
        let records = self.work(work).records;
        (records.called.iter())
            .find(|c| c.on == Some(Who::Owner) && !records.done.iter().any(|d| d.call == c.call))
            .map(|c| c.call)
    }

    /// The work's place, as the installation names it.
    pub fn place(&self, work: WorkNumber) -> Place {
        let installation = session::installation(&self.data).expect("the installation's id");
        git::place(&self.data, &git::tag(&installation), work)
    }

    /// Adds an item with a request, and makes its work's place where the flow has no cell that does.
    pub async fn add(&mut self) -> WorkNumber {
        self.add_made_of(&[]).await
    }

    /// Adds an item that depends on these items, with a request where its flow has one, and makes its work's place
    /// where the flow has agents to run in it and no cell that makes it.
    pub async fn add_made_of(&mut self, depends_on: &[ItemNumber]) -> WorkNumber {
        let request = (self.flow.blocks.contains_key(&"request".into())).then(|| FirstWrite {
            block: "request".into(),
            content: "Greet the user, please.".to_string(),
            value: None,
        });
        let item = NewItem {
            title: "Greet the user".to_string(),
            depends_on: depends_on.to_vec(),
            flow: self.flow.name.clone(),
            writes: request.into_iter().collect(),
        };
        let (reply, mut answer) = oneshot::channel();
        self.service
            .take(Message::Add { item, reply })
            .expect("the loop takes it");
        let (_, work) = answer.try_recv().expect("answered").expect("the item is added");
        let starts = (self.flow.cells.iter()).any(|cell| cell.act.as_deref() == Some("start"));
        let agents = (self.flow.cells.iter()).any(|cell| cell.kind == flow::Kind::Agent);
        if !starts && agents {
            let project = self.settings.project.clone();
            git::start(&project, &self.place(work), &project.main)
                .await
                .expect("the work's place is made");
        }
        work
    }

    /// Does what the owner does to the work, which the loop takes.
    pub fn act(&mut self, work: WorkNumber, act: Act) {
        let (reply, mut answer) = oneshot::channel();
        self.service
            .take(Message::Act { work, act, reply })
            .expect("the loop takes it");
        answer.try_recv().expect("answered").expect("the act is taken");
    }

    /// Takes what comes until `wanted` holds of the loop; fails where nothing comes for a minute before.
    pub async fn until(&mut self, wanted: impl Fn(&Real) -> bool) {
        while !wanted(self) {
            let message = tokio::time::timeout(Duration::from_secs(60), self.inbox.recv())
                .await
                .expect("something comes within a minute")
                .expect("the loop's channel is open");
            match &message {
                Message::Happened {
                    event: Event::Failed { .. },
                    ..
                } => self.failures += 1,
                Message::Happened {
                    event: Event::Paused { by: Pauser::Engine, .. },
                    ..
                } => self.holds += 1,
                _ => {}
            }
            self.service.take(message).expect("the loop takes it");
        }
    }

    /// Waits, taking nothing, until `wanted` holds; fails where it does not within twenty seconds.
    pub async fn wait(&self, wanted: impl Fn(&Real) -> bool) {
        let began = Instant::now();
        while !wanted(self) {
            assert!(began.elapsed() < Duration::from_secs(20), "it does not come");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// What the work does next.
    pub fn decision(&self, work: WorkNumber) -> Decision {
        self.service.driver().view(work).expect("the work").decision
    }

    /// Everything kept of the work.
    pub fn work(&self, work: WorkNumber) -> Work {
        self.service.driver().work(work).expect("the work").work.clone()
    }

    /// Whether the work is paused.
    pub fn paused(&self, work: WorkNumber) -> bool {
        decide::paused(&self.work(work).records)
    }

    /// The cells of the work's runs that finished, in the order they finished.
    pub fn finished(&self, work: WorkNumber) -> Vec<String> {
        let kept = self.work(work);
        (kept.records.finished.iter())
            .map(|f| {
                let started = kept
                    .records
                    .started
                    .iter()
                    .find(|s| s.run == f.run)
                    .expect("it started");
                started.cell.to_string()
            })
            .collect()
    }

    /// The work's latest run of the cell.
    pub fn run_of(&self, work: WorkNumber, cell: &str) -> RunId {
        let kept = self.work(work);
        let started = kept.records.started.iter().rev().find(|s| s.cell.as_str() == cell);
        started.expect("the cell ran").run
    }

    /// Whether every round of the run that began has ended.
    pub fn ended_rounds(&self, work: WorkNumber, run: RunId) -> bool {
        let records = self.work(work).records;
        let began = records.rounds.iter().filter(|r| r.run == run).count();
        let ended = records.returned.iter().filter(|r| r.run == run).count();
        began == ended
    }

    /// What the fake was given, call by call.
    pub fn calls(&self) -> Vec<Call> {
        let read = |n: usize, what: &str| std::fs::read_to_string(self.calls.join(format!("{n}.{what}")));
        (1..)
            .map_while(|n| {
                let args = read(n, "args").ok()?;
                Some(Call {
                    args: args.lines().map(str::to_string).collect(),
                    stdin: read(n, "stdin").unwrap_or_default(),
                    dir: PathBuf::from(read(n, "pwd").unwrap_or_default().trim_end()),
                    overlapped: self.calls.join(format!("{n}.overlap")).exists(),
                })
            })
            .collect()
    }
}

impl Real {
    /// The number of the process of the fake's call of this number.
    pub fn call_process(&self, n: usize) -> u32 {
        let text = std::fs::read_to_string(self.calls.join(format!("{n}.pid"))).expect("its number is kept");
        text.trim().parse().expect("a number")
    }
}

impl Call {
    /// The session the call began, or went on in: the flag, and the id after it.
    pub fn session(&self) -> (&str, &str) {
        let at = (self.args.iter())
            .position(|a| a == "--session-id" || a == "--resume")
            .expect("a session is named");
        (&self.args[at], &self.args[at + 1])
    }
}

/// Whether the decision is that the work has ended.
pub fn ended(decision: &Decision) -> bool {
    matches!(decision, Decision::Ended(_))
}
