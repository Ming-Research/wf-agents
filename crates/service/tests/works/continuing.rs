//! What an item continues is read by the act that starts a work from the item as it stands when the act's round is
//! handed out, so that an item corrected after a refused start is started from what it was corrected to.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use driver::{Asked, FirstWrite, NewItem};
use flow::{Compiled, Has, Source};
use service::{Act, Carriers, Inbox, Message, Round, RoundReply, Rounds, Scripted, Service, TokioClock};
use store::{Continues, ItemChange, Store};
use tokio::sync::oneshot;
use work::Spent;

use crate::{START, act_on, add_to, trial_script};

/// The flow `placing`: the act that starts a work, then the act that merges it.
fn placing() -> Compiled {
    let flow = r#"
        name = "placing"
        order = ["place", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [blocks.place]
        shown = true
        [blocks.merged]
        shown = true
        [blocks.start]
        outside = "start"
        writes = { text = "The owner started the work." }
    "#;
    let place = r#"
        title = "Place"
        kind = "engine"
        act = "start"
        watches = ["start"]
        outputs = ["place"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        watches = ["place"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: BTreeMap::from([
            ("place".to_string(), place.to_string()),
            ("merge".to_string(), merge.to_string()),
        ]),
        ..Source::default()
    };
    let has = Has {
        sources: [driver::START.to_string()].into(),
        acts: ["merge".to_string(), "start".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// The carrier of the engine's acts: it keeps what the item of each round it is given continues, refuses the first
/// round, as a start does for a branch that is not there, and leaves the others under way.
#[derive(Default)]
struct Starting {
    seen: Mutex<Vec<Option<Continues>>>,
}

impl Starting {
    fn seen(&self) -> Vec<Option<Continues>> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl Rounds for Starting {
    fn give(&self, round: Round, reply: RoundReply) {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        seen.push(round.item.continues.clone());
        if seen.len() == 1 {
            reply.failed("neither side has the branch gone".to_string(), Spent::default());
        }
    }

    fn stop(&self, _: Round, reply: RoundReply) {
        reply.failed("the round was stopped".to_string(), Spent::default());
    }
}

/// Takes what comes until nothing more does.
async fn take_what_comes(service: &mut Service, inbox: &mut Inbox) {
    while let Ok(Some(message)) = tokio::time::timeout(Duration::from_secs(1), inbox.recv()).await {
        service.take(message).expect("the loop takes it");
    }
}

fn branch(name: &str) -> Option<Continues> {
    Some(Continues::Branch(name.to_string()))
}

#[tokio::test(start_paused = true)]
async fn a_start_refused_and_given_again_after_the_item_is_corrected_is_given_the_corrected_item() {
    let dir = tempfile::tempdir().expect("a directory for the store");
    let store = Store::open(dir.path().join("store.sqlite")).expect("the store opens");
    let starting = Arc::new(Starting::default());
    let mut carriers: Carriers = Scripted::new(trial_script()).carriers();
    carriers.acts = starting.clone();
    let clock: Arc<dyn service::Clock> = Arc::new(TokioClock::new(START));
    let (mut service, mut inbox) =
        Service::open(Box::new(store), vec![placing()], carriers, clock).expect("the loop opens");
    let item = NewItem {
        title: "Continue the old work".to_string(),
        level: "1".to_string(),
        project: "trial".to_string(),
        depends_on: Vec::new(),
        tags: Vec::new(),
        continues: branch("gone"),
        flow: "placing".to_string(),
        writes: Vec::<FirstWrite>::new(),
        why: None,
        waits_for: Vec::new(),
    };
    let (number, work) = add_to(&mut service, item);

    let (resumed, taken) = act_on(&mut service, work, Act::Resume);
    resumed.expect("the work is let on");
    taken.expect("the loop takes it");
    take_what_comes(&mut service, &mut inbox).await;
    assert_eq!(
        starting.seen(),
        [branch("gone")],
        "the first start is given the item as added"
    );

    // The owner corrects the item, and lets the work on.
    let (reply, mut answer) = oneshot::channel();
    service
        .take(Message::Change {
            why: "The branch is called right".to_string(),
            changes: vec![(number, Asked::Item(ItemChange::Continues(branch("right"))))],
            reply,
        })
        .expect("the loop takes it");
    assert_eq!(answer.try_recv().expect("the loop answers at once"), Ok(vec![true]));
    let (resumed, taken) = act_on(&mut service, work, Act::Resume);
    resumed.expect("the work is let on");
    taken.expect("the loop takes it");
    take_what_comes(&mut service, &mut inbox).await;
    assert_eq!(
        starting.seen(),
        [branch("gone"), branch("right")],
        "the start given again is given the item as it was corrected"
    );
}
