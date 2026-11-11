//! A carrier that breaks its contract: the loop stops rather than go on beside it.

use std::sync::{Arc, Mutex};

use decide::RunId;
use driver::Failed;
use engine::{Came, Refused};
use service::{Act, Carriers, Fault, Round, RoundReply, Rounds, Scripted, Service, TokioClock};
use store::Store;
use work::Spent;

use crate::{START, act_on, add_to, new_item, trial_flow};

/// Rounds that are ended twice: by the reply each was given with and by the one its stop is given with.
#[derive(Default)]
struct Twice {
    given: Mutex<Vec<RoundReply>>,
}

impl Rounds for Twice {
    fn give(&self, _: Round, reply: RoundReply) {
        self.given.lock().unwrap().push(reply);
    }

    fn stop(&self, _: Round, reply: RoundReply) {
        for given in self.given.lock().unwrap().drain(..) {
            given.ended(Came::HandsIn(Vec::new()), Spent::default());
        }
        reply.ended(Came::HandsIn(Vec::new()), Spent::default());
    }
}

#[tokio::test(start_paused = true)]
async fn a_carrier_that_answers_a_round_twice_stops_the_loop() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let twice = Arc::new(Twice::default());
    let carriers = Carriers {
        agents: twice.clone(),
        acts: twice,
        tools: Arc::new(Scripted::new(Default::default())),
        slots: crate::SLOTS,
    };
    let clock = Arc::new(TokioClock::new(START));
    let (mut service, mut inbox) = Service::open(Box::new(store), vec![trial_flow()], carriers, clock).unwrap();
    let (_, work) = add_to(&mut service, new_item("Greet the user", &[]));
    assert_eq!(act_on(&mut service, work, Act::Resume), (Ok(()), Ok(())));
    assert_eq!(act_on(&mut service, work, Act::Cancel), (Ok(()), Ok(())));
    // The first end of the round is taken; the second cannot have happened.
    service.take(inbox.recv().await.unwrap()).unwrap();
    assert_eq!(
        service.take(inbox.recv().await.unwrap()),
        Err(Fault::Answer {
            work,
            failed: Failed::Refused(Refused::NoRoundUnderWay(RunId(1)))
        })
    );
}
