//! A keep-warm ping runs beside nothing else of its work: what is to be done for the work while one of its pings is
//! under way waits for the ping to end.

use decide::{Decision, End};
use engine::Event;
use service::Act;

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn a_work_cancelled_while_its_session_is_pinged_lets_go_of_what_it_holds_once_the_ping_has_ended() {
    let mut trial = Trial::warm();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    // The plan asks the owner: the line's session waits, and is pinged before the cache lets it go.
    trial.step().await.unwrap();
    while trial.pinged().is_empty() {
        trial.step().await.expect("the ping is due within the hour");
    }
    assert_eq!(trial.pinged(), [work].into());
    // Cancelled while the ping runs in its worktree, the work withdraws its question and lets go of what it holds only
    // once the ping has ended.
    trial.act(work, Act::Cancel { why: None }).unwrap();
    assert_eq!(trial.decision(work), Decision::Ended(End::Cancelled));
    assert!(trial.step().await.is_none(), "nothing is let go beside the ping");
    trial.ping_ends();
    assert!(
        matches!(trial.step().await, Some(Event::Pinged { failed: None, .. })),
        "the ping's end"
    );
    assert_eq!(
        trial.step().await,
        Some(Event::Released),
        "what the work held is let go"
    );
    assert!(trial.step().await.is_none());
}
