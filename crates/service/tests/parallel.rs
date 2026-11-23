//! The list of a project's works under way, as the loop has the carrier of the project's repository make it and writes
//! it into the block of the source `parallel`: the files each work's branch has changed, as the repository's carrier
//! says them, and the files its plan names.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
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
    /// The first read of a work's changes waits for this, where it is given.
    gate: Mutex<Option<oneshot::Receiver<()>>>,
}

impl Rounds for Repository {
    fn give(&self, round: Round, reply: RoundReply) {
        self.acts.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.acts.stop(round, reply);
    }

    fn changes(&self, _: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        let gate = self.gate.lock().unwrap().take();
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
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

/// What the latest version of the work's block says now.
fn latest(service: &Service, work: WorkNumber, block: &str) -> Option<String> {
    let kept = service.driver().work(work)?;
    let versions = decide::versions(&kept.work.records);
    let last = versions.get(&BlockId::from(block))?.last()?;
    kept.work.contents.get(&(BlockId::from(block), last.at)).cloned()
}

/// What the work's block of the list says now.
fn said(service: &Service, work: WorkNumber) -> Option<String> {
    latest(service, work, "parallel")
}

/// A service on the flow `planned` whose repository gives the files of `Repository`, the first read of which waits for
/// the gate where one is given.
fn opened(gate: Option<oneshot::Receiver<()>>) -> (Service, Inbox, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(Script::read(SCRIPT).unwrap()).carriers();
    carriers.acts = Arc::new(Repository {
        acts: carriers.acts.clone(),
        gate: Mutex::new(gate),
    });
    let clock = Arc::new(TokioClock::new(Time(1_794_268_800_000)));
    let (service, inbox) = Service::open(Box::new(store), vec![planned()], carriers, clock).unwrap();
    (service, inbox, dir)
}

/// Adds the item of the flow `planned`, with its work.
fn added(service: &mut Service) -> WorkNumber {
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
    work
}

/// Lets the work go on, so that it starts its first run.
fn resumed(service: &mut Service, work: WorkNumber) {
    let (reply, mut answer) = oneshot::channel();
    service
        .take(Message::Act {
            work,
            act: Act::Resume,
            reply,
        })
        .unwrap();
    answer.try_recv().unwrap().unwrap();
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
    let (mut service, mut inbox, _dir) = opened(None);
    let work = added(&mut service);
    assert_eq!(said(&service, work), None, "the work has started no run");
    resumed(&mut service, work);

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

// The lists of one project are written in the order they were asked for: the read of the first is held back until the
// plan's files are handed in and the second is asked for, and the list the loop ends up with is the second's, which
// holds the plan's files, though the second could be read at once. Fails where a later list is written before an
// earlier one, which then overwrites it with the older. Seconds of the paused clock; milliseconds.
#[tokio::test(start_paused = true)]
async fn the_lists_of_a_project_are_written_in_the_order_they_were_asked_for() {
    let (gate, held) = oneshot::channel();
    let (mut service, mut inbox, _dir) = opened(Some(held));
    let work = added(&mut service);
    resumed(&mut service, work);
    until(&mut service, &mut inbox, |s| latest(s, work, "plan_files").is_some()).await;
    assert_eq!(said(&service, work), None, "the first list is still being read");
    gate.send(()).unwrap();
    // What comes until nothing more does: a hundred rounds of the paused clock are more than the loop has to say.
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_secs(3600), inbox.recv()).await {
            Ok(Some(message)) => service.take(message).expect("the loop takes it"),
            _ => break,
        }
    }
    assert_eq!(
        said(&service, work).as_deref(),
        Some("W1 (#1)\nchanged:\nsrc/a.rs\nplanned:\nsrc/a.rs\nsrc/b.rs")
    );
}

/// What the repository says of the branch of one work.
#[derive(Clone)]
enum Branch {
    /// Its changed files, each with whether its head holds the change, as the carrier says them.
    Files(Vec<(&'static str, bool)>),
    /// The work has no branch yet.
    Nothing,
    /// The repository cannot be read, for this reason.
    Unreadable(&'static str),
}

/// The acts' repository says of each work, by its number, what its `Branch` says; of a work it names none, no branch.
struct Branches {
    acts: Arc<dyn Rounds>,
    of: BTreeMap<u64, Branch>,
}

impl Rounds for Branches {
    fn give(&self, round: Round, reply: RoundReply) {
        self.acts.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.acts.stop(round, reply);
    }

    fn changes(&self, of: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        let branch = self.of.get(&of.work.0).cloned().unwrap_or(Branch::Nothing);
        Box::pin(async move {
            match branch {
                Branch::Nothing => Ok(None),
                Branch::Unreadable(why) => Err(why.to_string()),
                Branch::Files(files) => Ok(Some(Changes {
                    base: "b0".to_string(),
                    head: "h1".to_string(),
                    worktree: true,
                    files: (files.into_iter())
                        .map(|(path, committed)| FileChanged {
                            path: path.to_string(),
                            added: 1,
                            removed: 0,
                            binary: false,
                            committed,
                        })
                        .collect(),
                })),
            }
        })
    }
}

/// The time of the first service of a test; a service opened on the same store again starts a day or more later.
const BEGIN: u64 = 1_794_268_800_000;
const DAY: u64 = 86_400_000;

/// A service on the flow `planned`, over the store in this directory, whose repository says this of each work's
/// branch, its clock starting at this time.
fn opened_on(dir: &Path, of: BTreeMap<u64, Branch>, start: u64) -> (Service, Inbox) {
    let store = Store::open(dir.join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(Script::read(SCRIPT).unwrap()).carriers();
    carriers.acts = Arc::new(Branches {
        acts: carriers.acts.clone(),
        of,
    });
    let clock = Arc::new(TokioClock::new(Time(start)));
    Service::open(Box::new(store), vec![planned()], carriers, clock).unwrap()
}

/// Adds the item of the flow `planned` of this project, with its work.
fn added_in(service: &mut Service, project: &str) -> WorkNumber {
    let item = NewItem {
        title: "a".to_string(),
        level: "1".to_string(),
        project: project.to_string(),
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
    work
}

/// Takes what comes until nothing more does: a hundred rounds of the paused clock are more than the loop has to say.
async fn settled(service: &mut Service, inbox: &mut Inbox) {
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_secs(3600), inbox.recv()).await {
            Ok(Some(message)) => service.take(message).expect("the loop takes it"),
            _ => break,
        }
    }
}

/// How many versions the work's block has.
fn versions_of(service: &Service, work: WorkNumber, block: &str) -> usize {
    let kept = service.driver().work(work).expect("the work is there");
    (decide::versions(&kept.work.records).get(&BlockId::from(block))).map_or(0, |versions| versions.len())
}

/// The plan's files as the script hands them in, which the list names under `planned:`.
const PLANNED: &str = "planned:\nsrc/a.rs\nsrc/b.rs";

/// Three works that have started, the first two of the project `one` and the third of `two`, each on a branch that
/// changed a file of its own, taken until the loop has nothing more to say.
async fn three_started(dir: &Path) -> (Service, Inbox, [WorkNumber; 3]) {
    let of = BTreeMap::from([
        (1, Branch::Files(vec![("a.rs", true)])),
        (2, Branch::Files(vec![("b.rs", true)])),
        (3, Branch::Files(vec![("c.rs", true)])),
    ]);
    let (mut service, mut inbox) = opened_on(dir, of, BEGIN);
    let works = [
        added_in(&mut service, "one"),
        added_in(&mut service, "one"),
        added_in(&mut service, "two"),
    ];
    for work in works {
        resumed(&mut service, work);
    }
    settled(&mut service, &mut inbox).await;
    (service, inbox, works)
}

// Each work of a project is written the list of that project's works alone, the same text for each, in the order of
// the works' numbers, and a work of another project the list of its own: with two works of one project and a third of
// another, each of the first two is told of both and the third of itself only. Fails where a project's works are
// written under another's, where a work is left out of the list of its project or the lists differ between works of a
// project, or where the works are not in the order of their numbers (reference: the module documentation of
// `driver::parallel` and `flow/data/parallel.toml`, which say the same list is given to every work of the project).
// Seconds of the paused clock; milliseconds.
#[tokio::test(start_paused = true)]
async fn each_works_list_holds_the_works_of_its_own_project_alone_in_the_order_of_their_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _inbox, [a, b, c]) = three_started(dir.path()).await;
    let both = format!("W1 (#1)\nchanged:\na.rs\n{PLANNED}\nW2 (#2)\nchanged:\nb.rs\n{PLANNED}");
    assert_eq!(said(&service, a).as_deref(), Some(both.as_str()));
    assert_eq!(said(&service, b).as_deref(), Some(both.as_str()));
    let alone = format!("W3 (#3)\nchanged:\nc.rs\n{PLANNED}");
    assert_eq!(said(&service, c).as_deref(), Some(alone.as_str()));
}

// A work that is cancelled leaves the list its project's other works have, which is written to them as it changes; the
// works of another project are written nothing, so that their list has no new version. Fails where the loop does not ask
// again after a cancel, or asks and writes a project that did not change. Milliseconds.
#[tokio::test(start_paused = true)]
async fn a_work_cancelled_leaves_the_list_of_its_project_and_no_other_project_s_list_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let (mut service, mut inbox, [a, b, c]) = three_started(dir.path()).await;
    let others = versions_of(&service, c, "parallel");
    let (reply, mut answer) = oneshot::channel();
    service
        .take(Message::Act {
            work: a,
            act: Act::Cancel { why: None },
            reply,
        })
        .unwrap();
    answer.try_recv().unwrap().unwrap();
    settled(&mut service, &mut inbox).await;
    let left = format!("W2 (#2)\nchanged:\nb.rs\n{PLANNED}");
    assert_eq!(said(&service, b).as_deref(), Some(left.as_str()));
    assert_eq!(versions_of(&service, c, "parallel"), others);
}

// What the repository can say of a branch is said in the list's words, none of it left empty or dropped: a branch the
// repository cannot read gives its reason, a work with no branch yet and a work whose changes are in its worktree only
// both read as nothing yet, and one work's failure leaves the other works on the list. Fails where a mapping of
// the carrier's answer is missing (an error taken as no change, no branch taken as an error, an uncommitted file
// taken as a change) or where one failed read stops the list (reference: `flow/data/parallel.toml` for the words).
// Milliseconds.
#[tokio::test(start_paused = true)]
async fn what_the_repository_cannot_say_of_a_branch_is_said_in_the_flow_s_words_and_the_other_works_stay_listed() {
    let dir = tempfile::tempdir().unwrap();
    let of = BTreeMap::from([
        (1, Branch::Unreadable("boom")),
        (2, Branch::Nothing),
        (3, Branch::Files(vec![("wip.rs", false)])),
    ]);
    let (mut service, mut inbox) = opened_on(dir.path(), of, BEGIN);
    let works: Vec<WorkNumber> = (0..3).map(|_| added_in(&mut service, "one")).collect();
    for work in &works {
        resumed(&mut service, *work);
    }
    settled(&mut service, &mut inbox).await;
    let text = format!(
        "W1 (#1)\nchanged:\ncould not be read: boom\n{PLANNED}\n\
         W2 (#2)\nchanged:\nnothing yet\n{PLANNED}\n\
         W3 (#3)\nchanged:\nnothing yet\n{PLANNED}"
    );
    for work in works {
        assert_eq!(said(&service, work).as_deref(), Some(text.as_str()), "work {}", work.0);
    }
}

// After a restart the loop reads the repository again, and the list is written into the work only where it differs from
// what the work holds: a repository that says the same writes no new version, and one that now says another file
// writes a new version with it, though no record of the work changed. Fails where a restarted loop asks for no list
// (the stale one stays), or writes the same list again (reference: `driver::parallel`'s module documentation, which says
// the lists are given once more after a stop and written where they differ). Milliseconds.
#[tokio::test(start_paused = true)]
async fn after_a_restart_the_list_is_made_again_and_written_only_where_it_differs() {
    let dir = tempfile::tempdir().unwrap();
    let branch = |file| BTreeMap::from([(1, Branch::Files(vec![(file, true)]))]);
    let (mut service, mut inbox) = opened_on(dir.path(), branch("a.rs"), BEGIN);
    let work = added_in(&mut service, "one");
    resumed(&mut service, work);
    settled(&mut service, &mut inbox).await;
    let first = format!("W1 (#1)\nchanged:\na.rs\n{PLANNED}");
    assert_eq!(said(&service, work).as_deref(), Some(first.as_str()));
    let versions = versions_of(&service, work, "parallel");
    drop((service, inbox));

    let (mut service, mut inbox) = opened_on(dir.path(), branch("a.rs"), BEGIN + DAY);
    settled(&mut service, &mut inbox).await;
    assert_eq!(said(&service, work).as_deref(), Some(first.as_str()));
    assert_eq!(
        versions_of(&service, work, "parallel"),
        versions,
        "the same list was written again"
    );
    drop((service, inbox));

    let (mut service, mut inbox) = opened_on(dir.path(), branch("z.rs"), BEGIN + 2 * DAY);
    settled(&mut service, &mut inbox).await;
    let later = format!("W1 (#1)\nchanged:\nz.rs\n{PLANNED}");
    assert_eq!(said(&service, work).as_deref(), Some(later.as_str()));
    assert_eq!(versions_of(&service, work, "parallel"), versions + 1);
}
