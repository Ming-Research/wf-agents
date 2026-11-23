//! The accounts the agents run under, as the loop opens on them: it starts whatever keeps the tokens says, and lets go
//! of the tokens of the accounts removed, until they are.

use std::sync::Arc;

use service::{Carriers, Fault, KeptTokens, Scripted, Service, Token, Tokens, TokioClock};
use store::{AccountAct, AccountRecord, Storage, Store};
use work::Time;

use crate::{START, trial_flow};

/// A store in a directory of its own, holding these records of accounts, a millisecond apart.
fn holding(records: &[(&str, AccountAct)]) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("a directory for the store");
    let mut store = Store::open(dir.path().join("store.sqlite")).expect("the store opens");
    for (n, (name, what)) in (0..).zip(records) {
        let record = AccountRecord {
            name: name.to_string(),
            what: *what,
            at: Time(START.0 + n),
            taker: store::AccountTaker::Owner,
            left_resets: None,
        };
        store.keep_account(&record).expect("the record is kept");
    }
    (dir, store)
}

/// The scripted carriers, the tokens kept by these.
fn keeping(tokens: &Arc<KeptTokens>) -> Carriers {
    let mut carriers = Scripted::new(Default::default()).carriers();
    let kept: Arc<dyn Tokens> = tokens.clone();
    carriers.tokens = kept;
    carriers
}

/// The loop opened on the store with the trial flow and these carriers, or what kept it from opening.
fn opened(store: Store, carriers: Carriers) -> Result<(), Fault> {
    let clock = Arc::new(TokioClock::new(START));
    Service::open(Box::new(store), vec![trial_flow()], carriers, clock).map(drop)
}

// What keeps the tokens being missing stops no start: an account in use whose token cannot be given fails each round
// as a command line that cannot run, which the wait every work shares holds, and the owner is shown why.
#[tokio::test(start_paused = true)]
async fn the_engine_starts_under_an_account_whose_tokens_cannot_be_given() {
    let tokens = Arc::new(KeptTokens::refusing("/usr/bin/security is not there"));
    let (_dir, store) = holding(&[("work", AccountAct::Added), ("work", AccountAct::Taken)]);
    assert_eq!(opened(store, keeping(&tokens)), Ok(()));
}

// As the engine opens, the token of every account whose latest record is its removal is let go of: one removed before
// a stop, or whose token could not be let go of then. An account removed and added again keeps its token.
#[tokio::test(start_paused = true)]
async fn the_tokens_of_the_accounts_removed_are_let_go_of_as_the_engine_opens() {
    let tokens = Arc::new(KeptTokens::new());
    for name in ["work", "old", "again"] {
        let token = Token::new(format!("sk-ant-oat01-{name}"));
        tokens.keep(name, &token).expect("kept");
    }
    let (_dir, store) = holding(&[
        ("work", AccountAct::Added),
        ("old", AccountAct::Added),
        ("again", AccountAct::Added),
        ("work", AccountAct::Taken),
        ("old", AccountAct::Removed),
        ("again", AccountAct::Removed),
        ("again", AccountAct::Added),
    ]);
    assert_eq!(opened(store, keeping(&tokens)), Ok(()));
    assert_eq!(tokens.names(), ["again", "work"]);
}
