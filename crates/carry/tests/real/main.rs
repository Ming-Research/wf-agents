//! The loop with the real carriers on the trial flow (`crates/service/tests/trial/flow`), in a git repository made for
//! each test, with a fake of the `claude` command line: a shell script that keeps what it was given and prints a
//! result that hands in the outputs of the cell its message names.
//!
//! The tests are kept in modules by subject, so that a new test goes into the module of what it tests.

mod budget;
mod failing;
mod going;

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use carry::settings::{Agents, Project};
use carry::{Settings, git};
use decide::Decision;
use driver::{FirstWrite, NewItem};
use flow::{Compiled, Has};
use service::{Act, Inbox, Message, Service, SystemClock};
use store::{Store, WorkNumber};
use tokio::sync::oneshot;
use work::Work;

/// How the fake command line answers.
pub struct Fake {
    /// The session's cost so far that every call reports, in dollars, as JSON.
    pub cost: &'static str,
    /// Whether its first call fails.
    pub fail_first: bool,
}

/// What the fake was given in one call.
pub struct Call {
    pub args: Vec<String>,
    pub stdin: String,
    /// The directory it ran in.
    pub dir: PathBuf,
}

/// The loop with the real carriers over a store in the data directory.
pub struct Real {
    _dir: tempfile::TempDir,
    pub data: PathBuf,
    pub repo: PathBuf,
    /// Where the fake keeps what it was given.
    calls: PathBuf,
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

/// The trial flow, compiled with what the real carriers have.
fn trial() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../service/tests/trial/flow");
    let source = flow::read(&dir).expect("the trial flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&[driver::DEPENDENCIES]),
        acts: names(&["start", "check", "merge"]),
    };
    flow::compile(&source, &has).expect("the trial flow compiles")
}

/// Writes the fake command line into the directory, keeping what it is given in `calls`.
fn fake(dir: &Path, calls: &Path, fake: &Fake) -> PathBuf {
    let script = format!(
        r#"#!/bin/sh
log='{calls}'
n=$(( $(ls "$log" | grep -c '\.args$') + 1 ))
printf '%s\n' "$@" > "$log/$n.args"
cat > "$log/$n.stdin"
pwd > "$log/$n.pwd"
if [ '{fail}' = yes ] && [ "$n" = 1 ]; then
    echo 'The fake fails its first call.' >&2
    exit 1
fi
first=$(head -n 1 "$log/$n.stdin")
case "$first" in
    *'cell `plan`'*) out='{{"block":"plan","content":"Greet by name."}}' ;;
    *'cell `build`'*)
        git -c user.name=Fake -c user.email=fake@example.com commit -q --allow-empty -m 'Build the greeting'
        out='{{"block":"code","content":"A greeting."}}' ;;
    *'cell `review`'*) out='{{"block":"verdict","content":"Fine.","value":"yes"}}' ;;
    *) out='' ;;
esac
printf '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":{cost},"usage":{{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":20}},"structured_output":{{"hands_in":[%s],"asks":[]}}}}\n' "$out"
"#,
        calls = calls.display(),
        fail = if fake.fail_first { "yes" } else { "no" },
        cost = fake.cost,
    );
    let path = dir.join("fake-claude");
    std::fs::write(&path, script).expect("the fake is written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("the fake can be run");
    path
}

impl Real {
    /// The loop on an empty store, with the fake answering as given and the budget given.
    pub fn open(answers: Fake, budget: Option<f64>) -> Real {
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
                check: "true".to_string(),
            },
            agents: Agents {
                command: command.display().to_string(),
                model: "claude-haiku-5-5".to_string(),
                slots: 2,
                args: Vec::new(),
                config_dir: None,
                owner_language: "Chinese".to_string(),
                work_budget_usd: budget,
            },
        };
        let carriers = carry::carriers(settings, &data).expect("the carriers are made");
        let store = Store::open(data.join("store.sqlite")).expect("the store opens");
        let flow = trial();
        let (service, inbox) = Service::open(Box::new(store), vec![flow.clone()], carriers, Arc::new(SystemClock))
            .expect("the loop opens");
        Real {
            _dir: dir,
            data,
            repo,
            calls,
            flow,
            service,
            inbox,
            failures: 0,
            holds: 0,
        }
    }

    /// Adds an item with a request, and makes its work's place where the flow has no cell that does.
    pub async fn add(&mut self) -> WorkNumber {
        let item = NewItem {
            title: "Greet the user".to_string(),
            depends_on: Vec::new(),
            flow: "trial".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user, please.".to_string(),
                value: None,
            }],
        };
        let (reply, mut answer) = oneshot::channel();
        self.service
            .take(Message::Add { item, reply })
            .expect("the loop takes it");
        let (_, work) = answer.try_recv().expect("answered").expect("the item is added");
        let starts = (self.flow.cells.iter()).any(|cell| cell.act.as_deref() == Some("start"));
        if !starts {
            let project = Project {
                repo: self.repo.clone(),
                main: "main".to_string(),
                check: "true".to_string(),
            };
            git::start(&project, &self.data, work)
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
                Message::Failed { .. } => self.failures += 1,
                Message::Hold { .. } => self.holds += 1,
                _ => {}
            }
            self.service.take(message).expect("the loop takes it");
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

    /// What the fake was given, call by call.
    pub fn calls(&self) -> Vec<Call> {
        let read = |n: usize, what: &str| std::fs::read_to_string(self.calls.join(format!("{n}.{what}")));
        (1..)
            .map_while(|n| {
                let args = read(n, "args").ok()?;
                Some(Call {
                    args: args.lines().map(str::to_string).collect(),
                    stdin: read(n, "stdin").expect("its input is kept"),
                    dir: PathBuf::from(read(n, "pwd").expect("its directory is kept").trim_end()),
                })
            })
            .collect()
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
