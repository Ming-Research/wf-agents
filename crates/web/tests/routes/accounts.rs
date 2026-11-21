//! The accounts of the Settings view, asked as the page asks them, through the engine's loop: an account is added once
//! its token is checked, kept where the service takes it or it is at its usage limit and not where the service refuses
//! it or it could not be checked; taken at once; removed with its token where it is not the one in use. The page is
//! told what the service last said of each account's usage, and looks under each not heard of lately as it opens; and
//! the command that makes a token. No answer holds a token.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::http::StatusCode;
use decide::run::Failure;
use driver::{Found, LOOK_AFTER_MS, Said, Window};
use serde_json::{Value, json};
use service::{Heard, KeptTokens, Outside, Scripted, Service, Token, Tokens, TokioClock, YEAR_MS};
use store::Store;
use web::settings::Install;

use crate::{START, get_json, post, post_text, trial_flow, trial_has, trial_script};

/// The route of the accounts.
const PATH: &str = "/api/v1/settings/accounts";

/// The week's window of usage, so much of it used, the service letting calls through in it or not.
fn week(utilization: f64, allowed: bool) -> Said {
    Said::Windows(vec![Window {
        name: "seven_day".to_string(),
        utilization: Some(utilization),
        resets: Some(work::Time(START + 7 * 24 * 3_600_000)),
        allowed,
    }])
}

/// The outside of these tests: every check under an account finds it well, and says a third of its week used; and a
/// token is refused by the service, at its usage limit, saying its week used up, or not checked for want of the
/// network, as its text says, else taken, saying a tenth of its week used; at the limit, one whose text says `silent`
/// names no window. A refusal's words hold no token, as the
/// check gives them. It counts the checks taken under an account, each of which would spend a short request.
#[derive(Default)]
struct ByToken {
    checks: AtomicUsize,
}

impl Outside for ByToken {
    fn check(&self, account: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            match account {
                Some(_) => Heard {
                    found: Found::Well,
                    usage: Some(week(0.3, true)),
                },
                None => Found::Down {
                    failure: Failure::CannotRun,
                    why: "No account is in use: choose one on the Settings view.".to_string(),
                    resets: None,
                }
                .into(),
            }
        })
    }

    fn check_token(&self, token: Token) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        let secret = token.secret();
        let found = if secret.contains("refused") {
            Found::Down {
                failure: Failure::CannotRun,
                why: "Invalid bearer token …".to_string(),
                resets: None,
            }
        } else if secret.contains("limited") {
            Found::Down {
                failure: Failure::UsageLimit,
                why: "You've hit your limit".to_string(),
                resets: Some(work::Time(START + 3_600_000)),
            }
        } else if secret.contains("offline") {
            Found::Down {
                failure: Failure::Offline,
                why: "The network does not reach the agents' service.".to_string(),
                resets: None,
            }
        } else {
            Found::Well
        };
        // At the limit, a token whose text says `silent` names no window.
        let silent = secret.contains("silent");
        let usage = match &found {
            Found::Well => Some(week(0.1, true)),
            Found::Down {
                failure: Failure::UsageLimit,
                resets,
                ..
            } if silent => Some(Said::Limit { resets: *resets }),
            Found::Down {
                failure: Failure::UsageLimit,
                ..
            } => Some(week(1.0, false)),
            Found::Down { .. } => None,
        };
        Box::pin(async move { Heard { found, usage } })
    }
}

/// The routes of the settings over the engine's loop on the trial flow, the tokens kept by `tokens`; with the
/// directory the store is in.
fn serving(tokens: Arc<KeptTokens>) -> (Router, tempfile::TempDir) {
    let (router, dir, _) = serving_counted(tokens);
    (router, dir)
}

/// The routes [`serving`] gives, with the outside, which counts its checks.
fn serving_counted(tokens: Arc<KeptTokens>) -> (Router, tempfile::TempDir, Arc<ByToken>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    let kept: Arc<dyn Tokens> = tokens;
    let by_token = Arc::new(ByToken::default());
    let outside: Arc<dyn Outside> = by_token.clone();
    carriers.tokens = kept;
    carriers.outside = outside;
    let flow = flow::compile(&trial_flow(), &trial_has(&flow::Source::default())).unwrap();
    let clock = Arc::new(TokioClock::new(work::Time(START)));
    let (service, inbox) = Service::open(Box::new(store), vec![flow], carriers, clock).unwrap();
    let handle = service.handle();
    tokio::spawn(service.run(inbox));
    let file = dir.path().join("install.toml");
    std::fs::write(&file, "[agents]\ncommand = \"claude\"\n").unwrap();
    let install = Install {
        file,
        read: Box::new(|_| Ok(json!({}))),
        running: json!({ "agents": { "command": "claude" } }),
        git: &[],
    };
    (web::settings::router(install, handle), dir, by_token)
}

fn add(name: &str, token: &str) -> Value {
    json!({ "kind": "add", "name": name, "token": token })
}

fn act(kind: &str, name: &str) -> Value {
    json!({ "kind": kind, "name": name })
}

/// The names of the accounts the page lists, in order.
fn names(page: &Value) -> Vec<String> {
    (page["accounts"].as_array().expect("the accounts"))
        .iter()
        .map(|a| a["name"].as_str().expect("a name").to_string())
        .collect()
}

/// What a route answered where nothing is done.
fn nothing() -> Value {
    json!({ "atLimit": false, "resets": null, "notLetGo": null })
}

// Every way an adding goes: a token the service refuses is not kept, the service's words said without it; one that
// could not be checked is not kept, saying why; a working one is kept and listed, its token lasting a year from when it
// was added; one at its usage limit is kept, saying its reset; a name taken, and one that is no name, are refused; and
// a body that cannot be read is refused in words that hold nothing of it. No answer holds a token.
#[tokio::test(start_paused = true)]
async fn an_account_is_added_once_its_token_is_checked_and_no_answer_holds_a_token() {
    let tokens = Arc::new(KeptTokens::new());
    let (router, _dir) = serving(tokens.clone());
    let mut answers: Vec<String> = Vec::new();
    let page = get_json(&router, PATH).await;
    assert_eq!(page["accounts"], json!([]));
    assert_eq!(page["inUse"], Value::Null);
    assert_eq!(page.get("directory"), None, "no directory's sign-in is shown");
    assert_eq!(page["command"], "claude setup-token");
    assert_eq!(page["closed"], Value::Null);
    let (status, said) = post(&router, PATH, &add("home", "sk-ant-oat01-refused")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let error = said["error"].as_str().expect("why").to_string();
    assert!(
        error.starts_with("No account was added: the service refused the token: Invalid bearer token"),
        "{error}"
    );
    answers.push(said.to_string());
    let (status, said) = post(&router, PATH, &add("away", "sk-ant-oat01-offline")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let error = said["error"].as_str().expect("why").to_string();
    assert!(error.contains("could not be checked, so it was not kept"), "{error}");
    answers.push(said.to_string());
    assert_eq!(tokens.names(), Vec::<String>::new(), "neither was kept");
    let (status, said) = post(&router, PATH, &add("work", "sk-ant-oat01-good")).await;
    assert_eq!((status, said.clone()), (StatusCode::OK, nothing()));
    answers.push(said.to_string());
    let (status, said) = post(&router, PATH, &add("max", "sk-ant-oat01-limited")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        said,
        json!({ "atLimit": true, "resets": START + 3_600_000, "notLetGo": null })
    );
    answers.push(said.to_string());
    assert_eq!(tokens.names(), ["max", "work"]);
    // A name taken is refused, and the token kept under it is left as it was.
    let (status, said) = post(&router, PATH, &add("work", "sk-ant-oat01-another")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        said["error"]
            .as_str()
            .is_some_and(|e| e.contains("account named work is added already"))
    );
    answers.push(said.to_string());
    assert_eq!(
        tokens.token("work").map(|t| t.secret().to_string()),
        Ok("sk-ant-oat01-good".to_string())
    );
    let (status, said) = post(&router, PATH, &add("Work!", "sk-ant-oat01-good")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    answers.push(said.to_string());
    // A paste that is no token's shape is refused at once, before it is checked, in words that hold nothing of it.
    for token in ["", "sk-ant-oat01 good"] {
        let (status, said) = post(&router, PATH, &add("pasted", token)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            said["error"],
            "No account was added: the token is empty, or holds a space or a control character: copy it again whole."
        );
        answers.push(said.to_string());
    }
    let (status, said) = post_text(&router, PATH, r#"{"kind": "add", "token": "sk-ant-oat01-good"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        said["error"],
        "The act cannot be read: it is no adding, taking or removing of an account."
    );
    answers.push(said.to_string());
    let page = get_json(&router, PATH).await;
    assert_eq!(names(&page), ["work", "max"]);
    let work = &page["accounts"][0];
    assert_eq!(work["lasts"].as_u64(), work["added"].as_u64().map(|at| at + YEAR_MS));
    assert_eq!(work["limit"], Value::Null);
    answers.push(page.to_string());
    for answer in &answers {
        assert!(!answer.contains("sk-ant"), "an answer holds a token: {answer}");
    }
}

// The owner takes an account, which the page then says is in use, and removes another, whose token is let go of; the
// one in use cannot be removed, and an account there is none of can be neither taken nor removed. A token that could
// not be let go of is said, and let go of at the next removal.
#[tokio::test(start_paused = true)]
async fn an_account_is_taken_at_once_and_one_not_in_use_is_removed_with_its_token() {
    let tokens = Arc::new(KeptTokens::new());
    let (router, _dir) = serving(tokens.clone());
    for name in ["work", "home"] {
        assert_eq!(
            post(&router, PATH, &add(name, "sk-ant-oat01-good")).await.0,
            StatusCode::OK
        );
    }
    assert_eq!(
        post(&router, PATH, &act("take", "work")).await,
        (StatusCode::OK, nothing())
    );
    assert_eq!(get_json(&router, PATH).await["inUse"], "work");
    let (status, said) = post(&router, PATH, &act("remove", "work")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        said["error"]
            .as_str()
            .is_some_and(|e| e.contains("agents run under work: take another account first")),
        "{said}"
    );
    tokens.hold(Some("User interaction is not allowed."));
    assert_eq!(
        post(&router, PATH, &act("remove", "home")).await,
        (
            StatusCode::OK,
            json!({ "atLimit": false, "resets": null, "notLetGo": "home: User interaction is not allowed." })
        )
    );
    assert_eq!(tokens.names(), ["home", "work"], "its token is held");
    assert_eq!(names(&get_json(&router, PATH).await), ["work"]);
    tokens.hold(None);
    assert_eq!(
        post(&router, PATH, &add("other", "sk-ant-oat01-good")).await.0,
        StatusCode::OK
    );
    assert_eq!(
        post(&router, PATH, &act("remove", "other")).await,
        (StatusCode::OK, nothing())
    );
    assert_eq!(tokens.names(), ["work"], "the removed accounts' tokens are let go of");
    for kind in ["take", "remove"] {
        let (status, said) = post(&router, PATH, &act(kind, "home")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            said["error"]
                .as_str()
                .is_some_and(|e| e.contains("no account named home")),
            "{said}"
        );
    }
}

// Where what keeps the tokens can keep and give none, as where the keychain's program is missing, the page is told why,
// and an adding and a taking are refused in those words, nothing kept and nothing taken.
#[tokio::test(start_paused = true)]
async fn no_account_is_added_or_taken_while_what_keeps_the_tokens_refuses_one() {
    let why = "/usr/bin/security is not on this system, and no token can be kept in the login keychain";
    let tokens = Arc::new(KeptTokens::new());
    let (router, _dir) = serving(tokens.clone());
    assert_eq!(
        post(&router, PATH, &add("work", "sk-ant-oat01-good")).await.0,
        StatusCode::OK
    );
    tokens.refuse(Some(why));
    assert_eq!(get_json(&router, PATH).await["closed"], why);
    let refused = (
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": format!("No account can be added or taken: {why}.") }),
    );
    assert_eq!(post(&router, PATH, &add("home", "sk-ant-oat01-good")).await, refused);
    assert_eq!(post(&router, PATH, &act("take", "work")).await, refused);
    assert_eq!(tokens.names(), ["work"]);
    assert_eq!(get_json(&router, PATH).await["inUse"], Value::Null);
}

/// The usage of the account of this name as the page lists it.
fn usage_of(page: &Value, name: &str) -> Value {
    let accounts = page["accounts"].as_array().expect("the accounts");
    let account = accounts.iter().find(|a| a["name"] == name).expect("the account");
    account["usage"].clone()
}

/// The week's window as the page is given it.
fn week_shown(utilization: f64, allowed: bool) -> Value {
    json!([{ "name": "seven_day", "utilization": utilization, "resets": START + 7 * 24 * 3_600_000, "allowed": allowed }])
}

// Each account shows what the service last said of its usage: the check of its token says it as it is added, at its
// limit as well, where it names no window too; and reading the page spends nothing, no check being taken. Opening the
// accounts looks under each not heard of lately, none within five minutes of the last word on it; the page, read again
// once the looks are back, shows what they heard. Milliseconds of the paused clock, nothing run: the one test of what
// the route spends and shows.
#[tokio::test(start_paused = true)]
async fn each_account_shows_the_services_latest_word_on_its_usage_and_an_opening_looks_under_the_stale() {
    const LOOK: &str = "/api/v1/settings/accounts/look";
    let (router, _dir, outside) = serving_counted(Arc::new(KeptTokens::new()));
    let added = [
        ("work", "sk-ant-oat01-good"),
        ("max", "sk-ant-oat01-limited"),
        ("spent", "sk-ant-oat01-limited-silent"),
    ];
    for (name, token) in added {
        assert_eq!(post(&router, PATH, &add(name, token)).await.0, StatusCode::OK);
    }
    for _ in 0..3 {
        get_json(&router, PATH).await;
    }
    let page = get_json(&router, PATH).await;
    assert_eq!(outside.checks.load(Ordering::SeqCst), 0, "reading spends nothing");
    let work = usage_of(&page, "work");
    assert_eq!(work["windows"], week_shown(0.1, true));
    assert_eq!(
        (work["atLimit"].clone(), work["looking"].clone(), work["failed"].clone()),
        (Value::Null, json!(false), Value::Null)
    );
    assert!(work["seen"].is_u64(), "{work}");
    assert_eq!(
        usage_of(&page, "max")["windows"],
        week_shown(1.0, false),
        "at its limit"
    );
    let spent = usage_of(&page, "spent");
    assert_eq!(
        (spent["windows"].clone(), spent["atLimit"].clone()),
        (json!([]), json!({ "resets": START + 3_600_000 })),
        "at its limit, naming no window"
    );
    assert_eq!(
        page["accounts"][1]["limit"],
        Value::Null,
        "no round of the engine met it"
    );
    // All were heard of just now: no look.
    assert_eq!(
        post(&router, LOOK, &json!({})).await,
        (StatusCode::OK, json!({ "looking": [] }))
    );
    tokio::time::advance(std::time::Duration::from_millis(LOOK_AFTER_MS + 1)).await;
    assert_eq!(
        post(&router, LOOK, &json!({})).await,
        (StatusCode::OK, json!({ "looking": ["work", "max", "spent"] }))
    );
    let mut page = get_json(&router, PATH).await;
    for _ in 0..100 {
        if !page["accounts"]
            .as_array()
            .expect("the accounts")
            .iter()
            .any(|a| a["usage"]["looking"] == true)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        page = get_json(&router, PATH).await;
    }
    assert_eq!(outside.checks.load(Ordering::SeqCst), 3, "a look under each");
    for name in ["work", "max", "spent"] {
        assert_eq!(
            usage_of(&page, name)["windows"],
            week_shown(0.3, true),
            "{name} looked under"
        );
    }
}
