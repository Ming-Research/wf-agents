//! What the calls of an agent's round under way have come to, as the loop tells of it: it is said changed apart from the
//! works, so that nothing that reads a work reads it again for each call of a round; where a carrier says where a round
//! waits or runs, the work is said changed as before.

use decide::RunId;
use driver::{Along, Going, Under};
use engine::Effect;
use service::{Act, Changed, Message};
use store::WorkNumber;
use tokio::sync::broadcast;

use crate::Trial;

/// The notices that have come, none left behind.
fn told(changes: &mut broadcast::Receiver<Changed>) -> Vec<Changed> {
    std::iter::from_fn(|| changes.try_recv().ok()).collect()
}

/// The run and the round number of the work's first round, which has begun and has not ended.
fn round_under_way(trial: &Trial, work: WorkNumber) -> (RunId, usize) {
    (trial.service.driver().pending().into_iter())
        .find_map(|todo| match todo.effect {
            Effect::Round { run, number, .. } if todo.work == work => Some((run, number)),
            _ => None,
        })
        .expect("a round of the work is under way")
}

// What a carrier says of the calls of a round is said changed alone: no work named, no other part of what the owner
// sees said changed, and `going` set; the same said again is no news; where it says the round runs as a CI run, it is
// the work that is said changed, and `going` is not set. Fails where the figures name their work, which makes every
// listener of the work read it for each call. Milliseconds of the paused clock, nothing run.
#[tokio::test(start_paused = true)]
async fn the_figures_of_a_round_are_said_changed_alone_and_where_it_runs_the_work_is() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let (run, round) = round_under_way(&trial, work);
    let mut changes = trial.service.handle().changes();
    told(&mut changes);

    let under = Under::Round { run, round };
    let says = |along: Along| Message::Along {
        work,
        under,
        along: Some(along),
    };
    let figures = Along::Going(Going {
        context: Some(12_000),
        ..Going::default()
    });
    trial.take(says(figures.clone()));
    assert_eq!(
        told(&mut changes),
        [Changed {
            works: Vec::new(),
            items: false,
            copies: false,
            accounts: false,
            observations: false,
            going: true,
        }]
    );
    trial.take(says(figures));
    let again = told(&mut changes);
    assert!(again.is_empty(), "the same figures said again are no news: {again:?}");

    trial.take(says(Along::Runs {
        at: "https://example.com/run/1".to_string(),
        queued_for: None,
    }));
    assert_eq!(
        told(&mut changes),
        [Changed {
            works: vec![work],
            items: false,
            copies: false,
            accounts: false,
            observations: false,
            going: false,
        }]
    );
}
