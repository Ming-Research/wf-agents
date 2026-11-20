//! A round runs under the account it was handed out with: the stand-in for the program is given that account's token,
//! and the credentials scrubbed from what it runs, while each try records the account by its name; the token is in no
//! argument, no message, no file the engine keeps and no record of the store. With no account in use the program runs
//! on its directory's sign-in, given no token, each try recording the directory. A token what keeps the tokens cannot
//! give fails the round as a command line that cannot run, in its words, and the program is not run.

use std::path::{Path, PathBuf};

use decide::run::Failure;
use service::{Act, Message, Token, Tokens};
use store::AccountAct;
use tokio::sync::oneshot;
use work::Back;

use crate::{Fake, Real, ended};

/// The token the tests keep for the account `work`.
const SECRET: &str = "sk-ant-oat01-a-token-of-the-tests";

/// The owner adds the account of this name and takes it, which the loop takes.
fn added_and_taken(real: &mut Real, name: &str) {
    for what in [AccountAct::Added, AccountAct::Taken] {
        let (reply, mut answer) = oneshot::channel();
        let account = Message::Account {
            name: name.to_string(),
            what,
            reply,
        };
        real.service.take(account).expect("the loop takes it");
        answer.try_recv().expect("answered").expect("the account is changed");
    }
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
    let mut real = Real::trial(Fake::default());
    real.tokens.keep("work", &Token::new(SECRET.to_string())).unwrap();
    added_and_taken(&mut real, "work");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 3);
    for (n, call) in (1..).zip(&calls) {
        assert_eq!(
            given(&real, n, "token"),
            SECRET,
            "call {n} runs under the account's token"
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
            ran.on.account.as_deref(),
            Some("work"),
            "the try records the account by its name"
        );
    }
    assert_eq!(
        holding(&real.data, SECRET),
        Vec::<PathBuf>::new(),
        "kept in a file of the engine's"
    );
}

#[tokio::test]
async fn with_no_account_in_use_the_program_runs_on_its_directory_given_no_token_and_the_try_records_the_directory() {
    let mut real = Real::trial(Fake::default());
    let directory = real.data.join("agents");
    real.settings.agents.config_dir = Some(directory.clone());
    real.restart();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 3);
    for n in 1..=calls.len() {
        assert_eq!(given(&real, n, "token"), "", "call {n} is given no token");
        assert_eq!(given(&real, n, "scrub"), "1");
    }
    let recorded = directory.display().to_string();
    for ran in &real.work(work).ran {
        assert_eq!(ran.on.account.as_deref(), Some(recorded.as_str()));
    }
}

#[tokio::test]
async fn a_token_that_cannot_be_given_fails_the_round_as_a_command_line_that_cannot_run_in_the_words_of_what_keeps_it()
{
    let mut real = Real::trial(Fake::default());
    // The account is added and taken, and no token is kept under its name, as where the keychain lost it.
    added_and_taken(&mut real, "work");
    let work = real.add().await;
    real.act(work, Act::Resume);
    let failed = |real: &Real| {
        (real.work(work).back.values()).find_map(|back| match back {
            Back::Failed { why, failure, .. } => Some((why.clone(), *failure)),
            _ => None,
        })
    };
    real.until(|real| failed(real).is_some()).await;
    let (why, failure) = failed(&real).expect("the round failed");
    assert_eq!(failure, Failure::CannotRun);
    assert_eq!(
        why,
        "the token of the account work could not be read: no token is kept under the name work"
    );
    assert_eq!(real.calls().len(), 0, "the program is not run");
}
