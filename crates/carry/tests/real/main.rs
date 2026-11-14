//! The loop with the real carriers, in a git repository made for each test, with a fake of the `claude` command line:
//! a shell script that keeps what it was given and prints a result that hands in the outputs of the cell its message
//! names. The flows are the trial flow (`crates/service/tests/trial/flow`), whose agents ask and judge, `flow` here,
//! which has the acts of an engineer's work, and the deliver flow as the repository defines it (`flow/deliver`), whose
//! one cell is the act that delivers a deliverable item.
//!
//! The tests are kept in modules by subject, so that a new test goes into the module of what it tests.

mod budget;
mod catching_up;
mod delivering;
mod failing;
mod going;
mod left;
mod levels;
mod permissions;
mod sessions;
mod stopping;

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use carry::git::{self, Place};
use carry::settings::{Agents, Items, Level, Project};
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
    /// Whether its first call fails, saying the prompt is too long, which pauses the work.
    pub fail_first: bool,
    /// Whether its first call prints a result that is an error, the service's safety check having flagged the
    /// message, the session having cost and spent tokens by then, and exits with 1.
    pub flag_first: bool,
    /// Whether its first call prints a result that is an error, as when it ran out of retries of its structured
    /// return, the session having cost eighty cents by then, and exits with 1.
    pub error_first: bool,
    /// Whether the build asks for the check in its first run, and its run entered for the result, going on in the
    /// session, is told the first time that the session is not found.
    pub lose_session: bool,
    /// Whether the build asks the owner a question in its first run, and for the check in the run entered for the
    /// answer, before it hands in; with `lose_session`, its last run is the one told the session is not found.
    pub question_first: bool,
    /// How many seconds the plan's first call sleeps before it answers.
    pub plan_sleeps: u32,
    /// The context every call ends with, in tokens, as the prompt of its one call to the model; none where the result
    /// says no call.
    pub context: Option<u64>,
    /// Whether every call compacts its session once: it runs the hook the command line was given for a compaction,
    /// keeping what the hook printed in `<n>.again`.
    pub compacts: bool,
    /// Whether the plan, run again, hands in what it wrote before, saying it changes nothing of what it means.
    pub plan_again_unchanged: bool,
    /// Whether its first call prints a stream in which it refused every attempt at the return, the last writing the
    /// plan's hand-ins as a string, gave up on the return, and exits with 1.
    pub refuses_first: bool,
}

impl Default for Fake {
    fn default() -> Self {
        Fake {
            cost: "0.01",
            fail_first: false,
            flag_first: false,
            error_first: false,
            lose_session: false,
            question_first: false,
            plan_sleeps: 0,
            context: None,
            compacts: false,
            plan_again_unchanged: false,
            refuses_first: false,
        }
    }
}

/// What the command line says of a return it gave up on, as it says it, the last refusal at its end.
pub const GAVE_UP: &str = "Failed to provide valid structured output after 3 attempts — last StructuredOutput error: \
                           Output does not match required schema: /hands_in: must be array";

/// The stream of a call that refused every attempt at the return and gave up on it, the plan's hand-ins written as a
/// string in the last, in the order the command line writes it: each attempt as its message's first block, in two
/// pieces, the whole message, the block's end, the refusal as a plain string, the message's end; the result, and the
/// command line's `task_summary` after it.
fn refused_stream() -> String {
    use serde_json::json;
    let event = |event: serde_json::Value| {
        json!({ "type": "stream_event", "event": event, "parent_tool_use_id": null }).to_string()
    };
    let attempt = |id: &str, input: &str| {
        let block = json!({ "type": "tool_use", "id": id, "name": "StructuredOutput", "input": {} });
        let (one, two) = input.split_at(input.len() / 2);
        let piece = |piece: &str| {
            let delta = json!({ "type": "input_json_delta", "partial_json": piece });
            event(json!({ "type": "content_block_delta", "index": 0, "delta": delta }))
        };
        let whole = serde_json::from_str::<serde_json::Value>(input).unwrap_or(json!({}));
        let used = json!({ "type": "tool_use", "id": id, "name": "StructuredOutput", "input": whole });
        let message = json!({ "role": "assistant", "content": [used] });
        let result = json!({ "type": "tool_result", "is_error": true, "tool_use_id": id,
            "content": "Output does not match required schema: /hands_in: must be array" });
        vec![
            event(json!({ "type": "message_start", "message": {} })),
            event(json!({ "type": "content_block_start", "index": 0, "content_block": block })),
            piece(one),
            piece(two),
            json!({ "type": "assistant", "message": message, "parent_tool_use_id": null }).to_string(),
            event(json!({ "type": "content_block_stop", "index": 0 })),
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "role": "user", "content": [result] } })
                .to_string(),
            event(json!({ "type": "message_delta", "delta": {}, "usage": {} })),
            event(json!({ "type": "message_stop" })),
        ]
    };
    let plan = json!([{ "block": "plan", "content": "Greet by name.", "matters": true }]).to_string();
    let mut lines = vec![json!({ "type": "system", "subtype": "init", "session_id": "s" }).to_string()];
    lines.extend(attempt("so-1", r#"{"hands_in": "[]", "#));
    lines.extend(attempt("so-2", &json!({ "hands_in": plan, "asks": [] }).to_string()));
    let gave_up = json!({ "type": "result", "subtype": "error_max_structured_output_retries", "is_error": true,
        "result": null, "errors": [GAVE_UP], "total_cost_usd": 0.4,
        "usage": { "input_tokens": 1000, "output_tokens": 300 } });
    lines.push(gave_up.to_string());
    lines.push(json!({ "type": "system", "subtype": "task_summary", "detail": null }).to_string());
    lines.join("\n") + "\n"
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
    /// The trial flow with a plan that loads the engineer's session (`loading` here).
    Loading,
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
    /// The level the items added from now on are given.
    pub level: String,
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
        On::Loading => Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/loading"),
    };
    let source = flow::read(&dir).expect("the flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&[driver::DEPENDENCIES, driver::SESSION]),
        acts: names(&["start", "check", "merge", "deliver"]),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// Writes the fake command line into the directory, keeping what it is given in `calls`. It is given its message as
/// one line of JSON, and reads the message's first line, its heading, from it; it prints its result as a stream of one
/// line, or, where it refuses its first return, the stream of [`refused_stream`].
fn fake(dir: &Path, calls: &Path, fake: &Fake) -> PathBuf {
    let yes = |on: bool| if on { "yes" } else { "no" };
    let refused = dir.join("refused.jsonl");
    std::fs::write(&refused, refused_stream()).expect("the refused stream is written");
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
if [ '{compacts}' = yes ]; then
    hook=$(grep -A1 -x -- --settings "$log/$n.args" | tail -n 1 | sed 's/.*"SessionStart".*"command":"\([^"]*\)".*/\1/')
    sh -c "$hook" > "$log/$n.again"
fi
if [ '{refuses}' = yes ] && [ "$n" = 1 ]; then
    cat '{refused}'
    exit 1
fi
if [ '{error}' = yes ] && [ "$n" = 1 ]; then
    printf '%s\n' '{{"type":"result","subtype":"error_max_structured_output_retries","is_error":true,"result":null,"errors":["{gave_up}"],"total_cost_usd":0.8,"usage":{{"input_tokens":1000,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":300}}}}'
    exit 1
fi
if [ '{flag}' = yes ] && [ "$n" = 1 ]; then
    printf '%s\n' '{{"type":"result","subtype":"success","is_error":true,"result":"API Error: the safeguards flagged this message.","total_cost_usd":0.3,"usage":{{"input_tokens":1000,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":30}}}}'
    exit 1
fi
if [ '{fail}' = yes ] && [ "$n" = 1 ]; then
    echo 'Prompt is too long: the fake fails its first call.' >&2
    exit 1
fi
first=$(head -n 1 "$log/$n.stdin" | sed 's/^.*"text":"\([^\\]*\)\\n.*$/\1/')
asks='[]'
out=''
case "$first" in
    *'cell `plan`'*)
        if [ '{sleeps}' != 0 ] && [ ! -f "$log/slept" ]; then
            touch "$log/slept"
            sleep '{sleeps}'
        fi
        matters=true
        if [ '{unchanged}' = yes ] && [ -f "$log/planned" ]; then
            matters=false
        fi
        touch "$log/planned"
        out="{{\"block\":\"plan\",\"content\":\"Greet by name.\",\"matters\":$matters}}" ;;
    *'cell `build`'*)
        results=no
        grep -q 'The results of what you asked for' "$log/$n.stdin" && results=yes
        checked=no
        grep -q 'The tool `check`' "$log/$n.stdin" && checked=yes
        if [ "$results" = no ] && [ '{question}' = yes ]; then
            asks='[{{"question":"Which name?"}}]'
        elif {{ [ "$results" = no ] && [ '{lose}' = yes ]; }} || {{ [ "$checked" = no ] && [ '{question}' = yes ]; }}; then
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
printf '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":{cost},"usage":{{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":20{iterations}}},"structured_output":{{"hands_in":[%s],"asks":%s}}}}\n' "$out" "$asks"
"#,
        calls = calls.display(),
        fail = yes(fake.fail_first),
        flag = yes(fake.flag_first),
        error = yes(fake.error_first),
        lose = yes(fake.lose_session),
        question = yes(fake.question_first),
        sleeps = fake.plan_sleeps,
        cost = fake.cost,
        compacts = yes(fake.compacts),
        unchanged = yes(fake.plan_again_unchanged),
        refuses = yes(fake.refuses_first),
        gave_up = GAVE_UP,
        refused = refused.display(),
        iterations = fake.context.map_or_else(String::new, |context| {
            format!(r#","iterations":[{{"input_tokens":{context},"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}]"#)
        }),
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

/// The acts' words as the flows of this repository write them.
fn words() -> carry::Writes {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    flow::agents::writes(&root).expect("what the engine writes is read")
}

/// The loop with the real carriers of the settings, on an empty store or the one in the data directory; with agents
/// whose stops are dropped where `deaf`.
fn open(settings: &Settings, data: &Path, flow: &Compiled, deaf: bool) -> (Service, Inbox) {
    let mut carriers = carry::carriers(settings.clone(), data, words()).expect("the carriers are made");
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
                commits: Default::default(),
            },
            agents: Agents {
                command: command.display().to_string(),
                levels: [("1", "claude-haiku-5-5"), ("2", "claude-sonnet-5-5")]
                    .map(|(name, model)| {
                        let level = Level {
                            desc: format!("Work of level {name}"),
                            model: model.to_string(),
                        };
                        (name.to_string(), level)
                    })
                    .into(),
                slots: 2,
                args: Vec::new(),
                env: Vec::new(),
                setting_sources: "user".to_string(),
                config_dir: None,
                permissions: Default::default(),
                owner_language: "Chinese".to_string(),
                work_budget_usd: budget,
            },
            items: Items {
                default_level: "1".to_string(),
                highest_level: None,
            },
            github: None,
            backup: None,
            sessions: Default::default(),
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
            level: "1".to_string(),
        }
    }

    /// The trial flow, with the fake answering as given, no budget and a check that passes.
    pub fn trial(answers: Fake) -> Real {
        Real::open(On::Trial, answers, None, "true")
    }

    /// Stops the loop and its carriers and starts them again on the same store, as a new process of the engine
    /// would, the owner lifting at once the pause of everything it makes as it starts on works going on. What the old
    /// carriers run goes on running.
    pub fn restart(&mut self) {
        let (service, inbox) = open(&self.settings, &self.data, &self.flow, false);
        self.service = service;
        self.inbox = inbox;
        self.lift_the_starts_pause();
    }

    /// Starts the loop again on the same store with agents that drop every stop they are given, the owner lifting at
    /// once the pause of everything it makes as it starts on works going on.
    pub fn deafen(&mut self) {
        let (service, inbox) = open(&self.settings, &self.data, &self.flow, true);
        self.service = service;
        self.inbox = inbox;
        self.lift_the_starts_pause();
    }

    /// The owner lifts the pause of everything the loop made as it started on works going on, where it made one.
    fn lift_the_starts_pause(&mut self) {
        let paused = self.service.driver().everything_paused();
        if !paused.is_some_and(|paused| paused.by == Pauser::Engine) {
            return;
        }
        let (reply, mut answer) = oneshot::channel();
        self.service
            .take(Message::Everything { paused: false, reply })
            .expect("the loop takes it");
        answer.try_recv().expect("answered").expect("the pause is lifted");
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

    /// Adds an item that depends on these items, with a request where its flow has one, paused so that its work begins
    /// once the test lets it on, and makes its work's place where the flow has agents to run in it and no cell that
    /// makes it.
    pub async fn add_made_of(&mut self, depends_on: &[ItemNumber]) -> WorkNumber {
        let request = (self.flow.blocks.contains_key(&"request".into())).then(|| FirstWrite {
            block: "request".into(),
            content: "Greet the user, please.".to_string(),
            value: None,
        });
        let item = NewItem {
            title: "Greet the user".to_string(),
            level: self.level.clone(),
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
    /// Where the fake keeps what it was given.
    pub fn calls_dir(&self) -> &Path {
        &self.calls
    }

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
                // The message, from the one user message of the stream-json input the fake was given; nothing where
                // the fake was stopped before it had read it.
                let stdin = read(n, "stdin").unwrap_or_default();
                let given = serde_json::from_str::<serde_json::Value>(&stdin).ok();
                let text = given
                    .as_ref()
                    .and_then(|given| given["message"]["content"][0]["text"].as_str());
                Some(Call {
                    args: args.lines().map(str::to_string).collect(),
                    stdin: text.unwrap_or_default().to_string(),
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
