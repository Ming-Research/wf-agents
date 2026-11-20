//! A round runs under the account it was handed out with: the stand-in for the program is given that account's token
//! and the engine's directory, and the credentials scrubbed from what it runs, while each try records the account by
//! its name and the directory; the token is in no argument, no message, no file the engine keeps and no record of the
//! store, and what the service says of the account's usage is kept under it. With accounts added and none in use, a
//! round runs nothing and fails as a command line that cannot run, the wait every work shares holding every work until
//! the owner takes one. A token what keeps the tokens cannot give fails the round so too, in its words, and the program
//! is not run.

use std::path::{Path, PathBuf};

use decide::run::Failure;
use service::{Act, Message, Token, Tokens};
use store::AccountAct;
use tokio::sync::oneshot;
use work::Back;

use crate::{Fake, On, Real, USED, ended};

/// The token the tests keep for the account `work`.
const SECRET: &str = "sk-ant-oat01-a-token-of-the-tests";

/// The owner adds the account of this name and takes it, which the loop takes.
fn added_and_taken(real: &mut Real, name: &str) {
    for what in [AccountAct::Added, AccountAct::Taken] {
        done(real, name, what);
    }
}

/// The owner does this to the account of this name, which the loop takes.
fn done(real: &mut Real, name: &str, what: AccountAct) {
    let (reply, mut answer) = oneshot::channel();
    let account = Message::Account {
        name: name.to_string(),
        what,
        reply,
    };
    real.service.take(account).expect("the loop takes it");
    answer.try_recv().expect("answered").expect("the account is changed");
}

/// Why the work's first round that failed failed, and of what kind; none where none has.
fn failed(real: &Real, work: store::WorkNumber) -> Option<(String, Failure)> {
    (real.work(work).back.values()).find_map(|back| match back {
        Back::Failed { why, failure, .. } => Some((why.clone(), *failure)),
        _ => None,
    })
}

/// What the stand-in kept of the call of this number: what it was given in this variable, by the file's extension.
fn given(real: &Real, n: usize, what: &str) -> String {
    std::fs::read_to_string(real.calls_dir().join(format!("{n}.{what}"))).unwrap_or_default()
}

/// Every file under the directory whose bytes hold the text.
fn holding(dir: &Path, text: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => found.extend(holding(&path, text)),
            Ok(kind) if kind.is_file() => {
                let bytes = std::fs::read(&path).unwrap_or_default();
                if bytes.windows(text.len()).any(|w| w == text.as_bytes()) {
                    found.push(path);
                }
            }
            _ => {}
        }
    }
    found
}

// Three rounds of the stand-in, with git: about a second.
#[tokio::test]
async fn a_round_runs_under_the_account_in_use_given_its_token_and_recording_its_name_and_the_token_is_kept_nowhere() {
    let mut real = Real::trial(Fake {
        says_usage: true,
        ..Fake::default()
    });
    real.tokens.keep("work", &Token::new(SECRET.to_string())).unwrap();
    added_and_taken(&mut real, "work");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 3);
    let profile = real.data.join("profile").display().to_string();
    for (n, call) in (1..).zip(&calls) {
        assert_eq!(
            given(&real, n, "token"),
            SECRET,
            "call {n} runs under the account's token"
        );
        assert_eq!(
            given(&real, n, "profile"),
            profile,
            "call {n} runs in the engine's directory"
        );
        assert_eq!(
            given(&real, n, "scrub"),
            "1",
            "call {n} has its commands' credentials scrubbed"
        );
        assert!(!call.args.iter().any(|a| a.contains(SECRET)), "{:?}", call.args);
        assert!(!call.stdin.contains(SECRET));
    }
    let kept = real.work(work);
    assert!(!kept.ran.is_empty());
    for ran in &kept.ran {
        assert_eq!(
            (ran.on.account.as_deref(), ran.on.directory.as_deref()),
            (Some("work"), Some(profile.as_str())),
            "the try records the account by its name, and the directory"
        );
    }
    assert!(real.data.join("profile").is_dir(), "the engine made its directory");
    // What the service said of the account's week, under the account the rounds were handed out with.
    let usage = real
        .service
        .driver()
        .usage()
        .of("work")
        .cloned()
        .expect("the account's usage");
    let windows = usage.seen.expect("its windows").1;
    assert_eq!(windows.len(), 1);
    assert_eq!(
        (windows[0].name.as_str(), windows[0].utilization),
        ("seven_day", Some(USED))
    );
    assert_eq!(
        real.service.driver().usage().of("tests").and_then(|u| u.seen.clone()),
        None
    );
    assert_eq!(
        holding(&real.data, SECRET),
        Vec::<PathBuf>::new(),
        "kept in a file of the engine's"
    );
}

// With accounts added and none in use, the round runs nothing and fails as a command line that cannot run, saying no
// account is in use, and the wait every work shares holds it, the work not paused; taking an account has the outside
// checked at once under it, which lets every work on once it finds the outside well (the loop's tests of the shared
// wait take that through). Git and the loop alone, no call of the stand-in: well under a second.
#[tokio::test]
async fn with_accounts_added_and_none_in_use_nothing_runs_until_the_owner_takes_one() {
    let mut real = Real::with_no_account(On::Trial, Fake::default(), None, "true");
    real.tokens.keep("work", &Token::new(SECRET.to_string())).unwrap();
    done(&mut real, "work", AccountAct::Added);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| failed(real, work).is_some()).await;
    assert_eq!(
        failed(&real, work),
        Some(("no account is in use".to_string(), Failure::CannotRun))
    );
    assert!(real.calls().is_empty(), "nothing is run");
    let outage = real
        .service
        .driver()
        .outage()
        .expect("the wait every work shares stands");
    assert_eq!(outage.failure, Failure::CannotRun);
    assert!(!real.paused(work), "the work is not paused: the wait holds it");
    done(&mut real, "work", AccountAct::Taken);
    let wake = real
        .service
        .driver()
        .wake()
        .expect("the driver asks to be told the time");
    assert!(wake.check, "the outside is checked at once under the account taken");
    assert_eq!(real.service.driver().accounts().in_use(), Some("work"));
}

#[tokio::test]
async fn a_token_that_cannot_be_given_fails_the_round_as_a_command_line_that_cannot_run_in_the_words_of_what_keeps_it()
{
    let mut real = Real::trial(Fake::default());
    // The account is added and taken, and no token is kept under its name, as where the keychain lost it.
    added_and_taken(&mut real, "work");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| failed(real, work).is_some()).await;
    let (why, failure) = failed(&real, work).expect("the round failed");
    assert_eq!(failure, Failure::CannotRun);
    assert_eq!(
        why,
        "the token of the account work could not be read: no token is kept under the name work"
    );
    assert_eq!(real.calls().len(), 0, "the program is not run");
}
