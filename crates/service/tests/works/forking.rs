//! A fork of a round's session asked through the handle, as the pages and the commands ask it: what it took, and what
//! its program left running, are kept with the work, whether it answered or failed having run, and nothing is kept of
//! one that was refused before its program ran. The agents' carrier is the test's, which answers each fork as the test says; what the real carrier
//! gives is tested in its own crate. Each test runs the loop on the paused clock until a round ran, then asks.

use decide::RunId;
use engine::Event;
use service::{Act, ForkFailed, Forked, Message, Refusal};
use store::WorkNumber;
use work::{Left, Spent};

use crate::{ACCOUNT, Trial};

impl Trial {
    /// Asks a fork of the run's last round that ran, while the loop takes what it is sent, as it does beside a page.
    async fn fork(
        &mut self,
        work: WorkNumber,
        run: RunId,
        question: &str,
    ) -> Result<Option<Result<Forked, String>>, Refusal> {
        let handle = self.service.handle();
        let ask = handle.fork(work, run, None, question.to_string());
        tokio::pin!(ask);
        loop {
            tokio::select! {
                asked = &mut ask => return asked,
                message = self.inbox.recv() => {
                    match message.expect("the loop's channel stays open") {
                        // What a carrier says happened counts as answered, as `step` counts it, so that a work's
                        // letting go answered while the fork is asked is not left open.
                        Message::Happened { work, event } => {
                            self.happened(work, event);
                        }
                        message => self.service.take(message).expect("the loop takes it"),
                    }
                }
            }
        }
    }

    /// Takes what comes until the work's fork is, and gives it.
    async fn until_forked(&mut self) -> Event {
        loop {
            let event = self.step().await.expect("the fork's event comes");
            if matches!(event, Event::Forked { .. }) {
                return event;
            }
        }
    }

    /// The run of a round of the work that ran, once one did.
    async fn first_run(&mut self, work: WorkNumber) -> RunId {
        while self.work(work).ran.is_empty() {
            self.step().await.expect("a round of the work runs");
        }
        self.work(work).ran[0].run
    }

    fn will_answer(&self, answer: Result<Forked, ForkFailed>) {
        self.forks.lock().expect("the forks").push_back(answer);
    }
}

/// What a fork that answered returns, whatever it took.
fn answered(spent: Spent) -> Forked {
    Forked {
        answer: Some("{}".to_string()),
        text: String::new(),
        spent,
        read: Some(25_800),
        held: Some(25_800),
        expired: None,
        round: (RunId(1), 1),
        later: Vec::new(),
        kept: "kept.answer".to_string(),
        account: ACCOUNT.to_string(),
        instead: None,
        usage: None,
        left: 0,
    }
}

fn took() -> Spent {
    Spent {
        usd_micros: 81_000,
        input_tokens: 120,
        cached_tokens: 25_800,
        output_tokens: 30,
        ..Spent::default()
    }
}

// Reads: the fork's run, round and what it took are what the work keeps, as one entry of its own.
#[tokio::test(start_paused = true)]
async fn a_fork_that_answered_is_kept_with_the_work_as_the_round_it_was_asked_of_and_what_it_took() {
    let mut trial = Trial::warm();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let run = trial.first_run(work).await;
    assert!(trial.work(work).forked.is_empty());
    trial.will_answer(Ok(answered(took())));
    let asked = trial.fork(work, run, "Which words can be read two ways?").await;
    assert!(matches!(asked, Ok(Some(Ok(_)))), "{asked:?}");
    let event = trial.until_forked().await;
    assert_eq!(
        event,
        Event::Forked {
            run,
            round: 1,
            spent: took()
        }
    );
    let kept = trial.work(work).forked;
    assert_eq!(kept.len(), 1);
    assert_eq!((kept[0].run, kept[0].round), (run, 1));
    assert_eq!(
        (
            kept[0].usd_micros,
            kept[0].input_tokens,
            kept[0].cached_tokens,
            kept[0].output_tokens
        ),
        (81_000, 120, 25_800, 30)
    );
}

#[tokio::test(start_paused = true)]
async fn a_fork_that_ran_and_gave_no_answer_is_kept_with_what_it_took_and_says_why_it_failed() {
    let mut trial = Trial::warm();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let run = trial.first_run(work).await;
    trial.will_answer(Err(ForkFailed {
        why: "The fork wrote no answer.".to_string(),
        spent: Some(took()),
        left: 2,
    }));
    let asked = trial.fork(work, run, "Which words can be read two ways?").await;
    let Ok(Some(Err(said))) = asked else {
        panic!("a failed fork says why: {asked:?}");
    };
    assert!(said.starts_with("The fork wrote no answer."), "{said}");
    assert!(said.contains("It cost $0.0810."), "{said}");
    trial.until_forked().await;
    let kept = trial.work(work);
    assert_eq!(kept.forked.len(), 1);
    assert_eq!(kept.forked[0].usd_micros, 81_000);
    // What its program left running is kept with the round it was asked of, before what it took.
    let left: Vec<_> = (kept.left_behind.iter()).map(|l| (l.of.clone(), l.processes)).collect();
    assert_eq!(left, [(Left::Fork { run, round: 1 }, 2)]);
    assert!(
        kept.left_behind[0].at < kept.forked[0].at,
        "kept before what the fork took"
    );
}

#[tokio::test(start_paused = true)]
async fn a_fork_refused_before_its_program_ran_is_kept_nowhere_and_the_next_one_is_kept_alone() {
    let mut trial = Trial::warm();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let run = trial.first_run(work).await;
    trial.will_answer(Err(ForkFailed::from("No account is in use.")));
    let refused = trial.fork(work, run, "Which words can be read two ways?").await;
    assert!(matches!(refused, Ok(Some(Err(_)))), "{refused:?}");
    assert!(trial.work(work).forked.is_empty());
    assert!(trial.work(work).left_behind.is_empty());
    // The next fork's entry is the first the loop is sent of forks: the refused one sent none.
    let next = Spent {
        usd_micros: 7_000,
        ..took()
    };
    trial.will_answer(Ok(answered(next)));
    trial.fork(work, run, "And these?").await.expect("asked");
    let event = trial.until_forked().await;
    assert_eq!(
        event,
        Event::Forked {
            run,
            round: 1,
            spent: next
        }
    );
    assert_eq!(trial.work(work).forked.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_fork_of_a_work_that_has_ended_is_kept_with_it_too() {
    let mut trial = Trial::warm();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let run = trial.first_run(work).await;
    trial.drive(work).await;
    trial.will_answer(Ok(answered(took())));
    let asked = trial.fork(work, run, "Which words can be read two ways?").await;
    assert!(matches!(asked, Ok(Some(Ok(_)))), "{asked:?}");
    trial.until_forked().await;
    let kept = trial.work(work).forked;
    assert_eq!(kept.len(), 1, "an ended work keeps what its fork took");
    assert_eq!(kept[0].usd_micros, 81_000);
}
