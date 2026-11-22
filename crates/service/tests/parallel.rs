//! The list of a project's works under way, as the loop has the carrier of the project's repository make it and writes
//! it into the block of the source `parallel`: the files each work's branch has changed, as the repository's carrier
//! says them, and the files its plan names.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use decide::BlockId;
use driver::{FirstWrite, NewItem};
use flow::{Compiled, Has, Source, compile};
use service::{
    Act, Changes, FileChanged, Inbox, Message, OfWork, Round, RoundReply, Rounds, Script, Scripted, Service, TokioClock,
};
use store::{Store, WorkNumber};
use tokio::sync::oneshot;
use work::Time;

/// A flow whose plan names, in `plan_files`, the files it will touch; its merge waits for the owner, so that the work
/// goes on while the tests look at it.
fn planned() -> Compiled {
    let flow = r#"
        name = "planned"
        order = ["plan", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.head]
        outside = "head"
        [blocks.plan_files]
        shown = true
        [blocks.parallel]
        outside = "parallel"
        planned_in = "plan_files"
        [blocks.parallel.writes]
        work = "W{work} (#{item})"
        changed = "changed:"
        planned = "planned:"
        none_yet = "nothing yet"
        not_stated = "not stated"
        unread = "could not be read: {why}"
        [blocks.seen]
        [blocks.ready]
        values = ["yes", "no"]
        people = true
        [blocks.merged]
        shown = true
    "#;
    let plan = r#"
        title = "Plan"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Plan it."
        watches = ["request"]
        reads = ["head", "parallel"]
        outputs = ["plan_files", "seen"]

        [kinds]
        request = "last_and_latest"
        head = "latest"
        parallel = "latest"
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { ready = "yes" }
        watches = ["seen"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("plan", plan), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: ["head", "parallel"].map(str::to_string).into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the flow compiles")
}

/// The plan names two files as its round ends.
const SCRIPT: &str = r#"
    [[flows.planned.rounds]]
    cell = "plan"
    hands_in = [
        { block = "plan_files", content = "src/a.rs\nsrc/b.rs\n" },
        { block = "seen", content = "planned" },
    ]
"#;

/// The acts' repository says every work changed two files, one committed and one only its worktree holds.
struct Repository {
    acts: Arc<dyn Rounds>,
}

impl Rounds for Repository {
    fn give(&self, round: Round, reply: RoundReply) {
        self.acts.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.acts.stop(round, reply);
    }

    fn changes(&self, _: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        Box::pin(async {
            let file = |path: &str, committed| FileChanged {
                path: path.to_string(),
                added: 1,
                removed: 0,
                binary: false,
                committed,
            };
            Ok(Some(Changes {
                base: "b0".to_string(),
                head: "h1".to_string(),
                worktree: true,
                files: vec![file("src/a.rs", true), file("wip.rs", false)],
            }))
        })
    }
}

/// What the work's block of the list says now.
fn said(service: &Service, work: WorkNumber) -> Option<String> {
    let kept = service.driver().work(work)?;
    let versions = decide::versions(&kept.work.records);
    let last = versions.get(&BlockId::from("parallel"))?.last()?;
    kept.work.contents.get(&(BlockId::from("parallel"), last.at)).cloned()
}

/// Takes what comes, once it comes, until this holds of the loop.
async fn until(service: &mut Service, inbox: &mut Inbox, held: impl Fn(&Service) -> bool) {
    for _ in 0..500 {
        if held(service) {
            return;
        }
        // An hour of the paused clock passes at once where nothing is under way.
        let message = tokio::time::timeout(Duration::from_secs(3600), inbox.recv())
            .await
            .expect("something comes")
            .expect("the loop is open");
        service.take(message).expect("the loop takes it");
    }
    panic!("what was waited for did not come");
}

// A work that has started a run is on the list of its project, with the files its branch has changed as the carrier of
// the repository says them, the files only its worktree holds left out, and the files its plan names once it names
// them; the list is written into the work, and runs no cell. Fails where the loop never asks for the list, where the
// carrier's files are not in it, where uncommitted ones are, or where the plan's files do not come into it when the
// plan is handed in. Seconds of the paused clock; milliseconds.
#[tokio::test(start_paused = true)]
async fn the_loop_writes_the_list_of_the_works_under_way_as_the_repository_and_the_plans_say() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(Script::read(SCRIPT).unwrap()).carriers();
    carriers.acts = Arc::new(Repository {
        acts: carriers.acts.clone(),
    });
    let clock = Arc::new(TokioClock::new(Time(1_794_268_800_000)));
    let (mut service, mut inbox) = Service::open(Box::new(store), vec![planned()], carriers, clock).unwrap();

    let item = NewItem {
        title: "a".to_string(),
        level: "1".to_string(),
        project: "one".to_string(),
        depends_on: Vec::new(),
        tags: Vec::new(),
        flow: "planned".to_string(),
        writes: vec![FirstWrite {
            block: "request".into(),
            content: "Plan it, please.".to_string(),
            value: None,
        }],
    };
    let (reply, mut answer) = oneshot::channel();
    service.take(Message::Add { item, reply }).unwrap();
    let (_, work) = answer.try_recv().unwrap().unwrap();
    assert_eq!(said(&service, work), None, "the work has started no run");
    let (reply, mut answer) = oneshot::channel();
    service
        .take(Message::Act {
            work,
            act: Act::Resume,
            reply,
        })
        .unwrap();
    answer.try_recv().unwrap().unwrap();

    until(&mut service, &mut inbox, |s| said(s, work).is_some()).await;
    assert_eq!(
        said(&service, work).as_deref(),
        Some("W1 (#1)\nchanged:\nsrc/a.rs\nplanned:\nnot stated")
    );
    until(&mut service, &mut inbox, |s| {
        said(s, work).is_some_and(|text| text.contains("src/b.rs"))
    })
    .await;
    assert_eq!(
        said(&service, work).as_deref(),
        Some("W1 (#1)\nchanged:\nsrc/a.rs\nplanned:\nsrc/a.rs\nsrc/b.rs")
    );
}
