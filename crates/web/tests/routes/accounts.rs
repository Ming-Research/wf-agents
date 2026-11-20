//! The accounts of the Settings view, asked as the page asks them, through the engine's loop: an account is added once
//! its token is checked, kept where the service takes it or it is at its usage limit and not where the service refuses
//! it or it could not be checked; taken at once; removed with its token where it is not the one in use. The page is
//! told the sign-in the agents run on where none is, and the command that makes a token. No answer holds a token.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use decide::run::Failure;
use driver::Found;
use serde_json::{Value, json};
use service::{KeptTokens, Outside, Scripted, Service, Token, Tokens, TokioClock, YEAR_MS};
use store::Store;
use web::settings::{Install, Program};

use crate::{START, get_json, post, post_text, trial_flow, trial_has, trial_script};

/// The route of the accounts.
const PATH: &str = "/api/v1/settings/accounts";

/// The outside of these tests: every check under an account finds it well, and a token is refused by the service, at
/// its usage limit, or not checked for want of the network, as its text says, else taken. The service says a token it
/// refuses back, as words the route must not pass on.
struct ByToken;

impl Outside for ByToken {
    fn check(&self, _: Option<String>) -> Pin<Box<dyn Future<Output = Found> + Send + '_>> {
        Box::pin(async { Found::Well })
    }

    fn check_token(&self, token: Token) -> Pin<Box<dyn Future<Output = Found> + Send + '_>> {
        let secret = token.secret();
        let found = if secret.contains("refused") {
            Found::Down {
                failure: Failure::CannotRun,
                why: format!("Invalid bearer token {secret}"),
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
        Box::pin(async move { found })
    }
}

/// The routes of the settings over the engine's loop on the trial flow, the tokens kept by `tokens`, the agents'
/// directory signed in as the owner; with the directory the store is in.
fn serving(tokens: Arc<KeptTokens>) -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    let kept: Arc<dyn Tokens> = tokens;
    let outside: Arc<dyn Outside> = Arc::new(ByToken);
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
        running: json!({ "agents": { "config_dir": "/data/agents/claude" } }),
        git: &[],
        program: Program {
            signed_in_as: |_| Some("owner@example.com".to_string()),
        },
    };
    (web::settings::router(install, handle), dir)
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
// was added; one at its usage limit is kept, saying its reset; a name taken, and one that is no name, are refused; and a
// body that cannot be read is refused in words that hold nothing of it. No answer holds a token.
#[tokio::test(start_paused = true)]
async fn an_account_is_added_once_its_token_is_checked_and_no_answer_holds_a_token() {
    let tokens = Arc::new(KeptTokens::new());
    let (router, _dir) = serving(tokens.clone());
    let mut answers: Vec<String> = Vec::new();
    let page = get_json(&router, PATH).await;
    assert_eq!(page["accounts"], json!([]));
    assert_eq!(page["inUse"], Value::Null);
    assert_eq!(
        page["directory"],
        json!({ "path": "/data/agents/claude", "signedInAs": "owner@example.com" })
    );
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
// one in use cannot be removed, and an account there is none of can be neither taken nor removed.
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
    assert_eq!(
        post(&router, PATH, &act("remove", "home")).await,
        (StatusCode::OK, nothing())
    );
    assert_eq!(tokens.names(), ["work"], "the removed account's token is let go of");
    assert_eq!(names(&get_json(&router, PATH).await), ["work"]);
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

// Where what keeps the tokens says no account can be added, as where the settings name a credential the agents'
// program takes above a token, the page is told why, and an adding is refused in those words, nothing kept.
#[tokio::test(start_paused = true)]
async fn no_account_is_added_while_what_keeps_the_tokens_refuses_one() {
    let why = "agents.env names ANTHROPIC_API_KEY, which the agents' program takes above an account's token";
    let tokens = Arc::new(KeptTokens::refusing(why));
    let (router, _dir) = serving(tokens.clone());
    assert_eq!(get_json(&router, PATH).await["closed"], why);
    let (status, said) = post(&router, PATH, &add("work", "sk-ant-oat01-good")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(said["error"], format!("No account was added: {why}."));
    assert_eq!(tokens.names(), Vec::<String>::new());
}
