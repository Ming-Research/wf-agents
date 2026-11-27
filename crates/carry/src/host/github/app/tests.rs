//! The engine's GitHub App: the token for it, judged by `openssl` with the public key of a key made for the test; and
//! the tokens of its installation, asked by the real `curl` of a stand-in for GitHub by the time a clock the test sets
//! reads.

use std::path::Path;

use serde_json::{Value, json};

use super::stand_in::{self, APP, INSTALLATION, Set};
use super::*;
use crate::host::github::Gh;
use crate::host::github::api::Api;
use decide::run::Failure;

/// 2026-11-26T12:00:00Z, in seconds since 1970.
const NOON: u64 = 1_795_694_400;

/// The bytes of a part of a token, read from base64url by `openssl`, which reads base64 with its padding.
fn decoded(part: &str) -> Vec<u8> {
    let mut text: String = part
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    while !text.len().is_multiple_of(4) {
        text.push('=');
    }
    let mut child = std::process::Command::new("openssl")
        .args(["base64", "-d", "-A"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("openssl runs");
    use std::io::Write;
    child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    child.wait_with_output().expect("openssl ends").stdout
}

/// Whether `openssl` finds the signature of the token's header and claims made by the key of this public key.
fn verified(token: &str, public: &Path, dir: &Path) -> bool {
    let (signed, signature) = token.rsplit_once('.').expect("a token has three parts");
    let file = dir.join("signature");
    std::fs::write(&file, decoded(signature)).unwrap();
    let mut child = std::process::Command::new("openssl")
        .args(["dgst", "-sha256", "-verify"])
        .arg(public)
        .arg("-signature")
        .arg(&file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("openssl runs");
    use std::io::Write;
    child.stdin.take().unwrap().write_all(signed.as_bytes()).unwrap();
    child.wait_with_output().expect("openssl ends").status.success()
}

/// What a part of a token holds, as JSON.
fn read(part: &str) -> Value {
    serde_json::from_slice(&decoded(part)).expect("the part is JSON")
}

// Base64url as RFC 4648 gives it in its examples (section 10), without padding, and with the two characters of its own
// alphabet in place of base64's.
#[test]
fn bytes_are_written_in_base64url_without_padding() {
    for (bytes, written) in [
        ("", ""),
        ("f", "Zg"),
        ("fo", "Zm8"),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg"),
        ("fooba", "Zm9vYmE"),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64url(bytes.as_bytes()), written, "{bytes:?}");
    }
    assert_eq!(base64url(&[0xfb, 0xff, 0xbf]), "-_-_");
}

// The token for the App is RS256, issued by the App's id a minute before the clock reads, and lasts nine minutes, under
// the ten GitHub takes; it is signed by the App's key, as openssl finds with the key's public key, and a token whose
// claims are changed after it was signed is not. A key made for the test; a second or so, most of it making the key.
#[tokio::test]
async fn the_token_for_the_app_is_rs256_by_its_id_and_its_key_and_lasts_under_ten_minutes() {
    let dir = tempfile::tempdir().unwrap();
    let (app, public) = stand_in::app(dir.path(), Set::at(NOON));
    let token = app.signed(NOON).await.expect("signed");
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3, "{token}");
    assert_eq!(read(parts[0]), json!({ "alg": "RS256", "typ": "JWT" }));
    let claims = read(parts[1]);
    assert_eq!(
        claims,
        json!({ "iat": NOON - 60, "exp": NOON - 60 + 9 * 60, "iss": APP })
    );
    let lasts = claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap();
    assert!(lasts < 10 * 60, "{lasts}");
    assert!(verified(&token, &public, dir.path()), "the App's key signed it");
    let changed = unsigned(APP + 1, NOON);
    let forged = format!("{changed}.{}", parts[2]);
    assert!(
        !verified(&forged, &public, dir.path()),
        "other claims are not what was signed"
    );
}

// GitHub takes the token for the App only in the header `Authorization: Bearer`: given to `gh`, which sends it as
// `Authorization: token`, it is refused with GitHub's words. `curl` sends it as `Bearer`, read from its standard input,
// and gets the installation's token; the token for the App is in the arguments of no program, which any process on the
// machine may read. A key made for the test, the real `curl` and a stand-in for GitHub; a second or so.
#[tokio::test]
async fn the_token_for_the_app_goes_as_bearer_and_on_no_command_line() {
    let dir = tempfile::tempdir().unwrap();
    let (app, public) = stand_in::app(dir.path(), Set::at(NOON));
    std::fs::write(dir.path().join("expires"), "2026-11-26T13:00:00Z").unwrap();
    let path = format!("app/installations/{INSTALLATION}/access_tokens");
    let signed = app.signed(NOON).await.expect("signed");
    let gh = Gh::new(
        &dir.path().join("gh").display().to_string(),
        "",
        Credential::Given(signed.clone()),
        dir.path(),
    );
    let refused = (gh.json("POST", &path, None, &[201]).await).expect_err("gh's token is refused");
    assert_eq!(refused.why, "HTTP 401: A JSON web token could not be decoded");
    assert_eq!(app.token().await.expect("a token"), "ghs_given-1");
    let calls = stand_in::calls(dir.path());
    assert_eq!(calls.len(), 2, "{calls:?}");
    let (bearer, asked) = &calls[1];
    assert!(asked.starts_with(&format!("POST /{path} ")), "{asked}");
    assert!(
        verified(bearer, &public, dir.path()),
        "the token for the App came as Bearer"
    );
    let arguments = std::fs::read_to_string(dir.path().join("arguments")).unwrap();
    assert_eq!(arguments.lines().count(), 1, "{arguments}");
    assert!(arguments.contains(&path), "{arguments}");
    for (_, args) in &calls {
        assert!(
            !args.contains(&signed),
            "gh was given the token for the App as an argument"
        );
    }
    assert!(
        !arguments.contains(bearer.as_str()),
        "curl was given the token for the App as an argument"
    );
}

// Every call is given the installation's token, which GitHub gave for a token for the App signed by its key; the same
// token is given while more than ten minutes of it are left, and a new one once less are: the hour's token is got anew
// at fifty minutes, never given to a call that could outlast it. A key made for the test and a stand-in for GitHub; a
// second or so.
#[tokio::test]
async fn every_call_is_given_the_installations_token_and_a_new_one_once_ten_minutes_of_it_are_left() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Set::at(NOON);
    let (app, public) = stand_in::app(dir.path(), clock.clone());
    std::fs::write(dir.path().join("expires"), "2026-11-26T13:00:00Z").unwrap();
    let gh = Gh::new(
        &dir.path().join("gh").display().to_string(),
        "o/r",
        Credential::App(app.clone()),
        dir.path(),
    );
    gh.pulls("work-1").await.expect("listed");
    let calls = stand_in::calls(dir.path());
    assert_eq!(calls.len(), 2, "{calls:?}");
    let (signed, asked) = &calls[0];
    assert!(
        asked.starts_with(&format!("POST /app/installations/{INSTALLATION}/access_tokens ")),
        "{asked}"
    );
    assert!(
        verified(signed, &public, dir.path()),
        "the exchange is given the token for the App"
    );
    assert_eq!(calls[1].0, "ghs_given-1");
    assert!(calls[1].1.contains("repos/o/r/pulls?head=o:work-1"), "{:?}", calls[1]);
    // At 12:49 eleven minutes are left: the same token.
    clock.pass(49 * 60);
    gh.pulls("work-1").await.expect("listed");
    let calls = stand_in::calls(dir.path());
    assert_eq!(calls.len(), 3, "no token was asked for: {calls:?}");
    assert_eq!(calls[2].0, "ghs_given-1");
    // At 12:51 nine are: a new one.
    clock.pass(2 * 60);
    std::fs::write(dir.path().join("expires"), "2026-11-26T13:51:00Z").unwrap();
    gh.pulls("work-1").await.expect("listed");
    let calls = stand_in::calls(dir.path());
    assert_eq!(calls.len(), 5, "{calls:?}");
    assert!(calls[3].1.contains("access_tokens"), "{:?}", calls[3]);
    assert_eq!(calls[4].0, "ghs_given-2");
    // Neither a token nor the key is shown.
    let shown = format!("{gh:?} {app:?} {:?}", Credential::App(app.clone()));
    assert!(!shown.contains("ghs_given") && !shown.contains(&calls[3].0), "{shown}");
}

// Where GitHub gives no token of the installation, the call that needed it is not made: it fails saying why, naming
// the App's settings, of the kind GitHub's answer tells, a refusal for the owner or GitHub's own fault, which passes;
// and a token that would expire before a call could end is refused, saying the clock may be wrong. Where the key cannot
// sign, no call of GitHub is made at all. A key made for the test and a stand-in for GitHub; a second or so.
#[tokio::test]
async fn no_call_is_made_without_the_installations_token_and_why_is_said() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _) = stand_in::app(dir.path(), Set::at(NOON));
    let gh = Gh::new(
        &dir.path().join("gh").display().to_string(),
        "o/r",
        Credential::App(app.clone()),
        dir.path(),
    );
    for (status, kind) in [
        ("401 Unauthorized", Failure::Other),
        ("503 Service Unavailable", Failure::Held),
    ] {
        std::fs::write(dir.path().join("refuse"), status).unwrap();
        let _ = std::fs::remove_file(dir.path().join("calls"));
        let failed = gh.pulls("work-1").await.expect_err("no token");
        assert!(
            failed.why.starts_with(&format!(
                "GitHub gave the engine's GitHub App {APP} (github_app.app_id) no token of its installation \
                 {INSTALLATION} (github_app.installation_id): HTTP {}",
                &status[..3]
            )),
            "{failed}"
        );
        assert_eq!(failed.kind, Some(kind), "{status}");
        let calls = stand_in::calls(dir.path());
        assert_eq!(calls.len(), 1, "only the token was asked for: {calls:?}");
        assert!(calls[0].1.contains("access_tokens"), "{calls:?}");
    }
    std::fs::remove_file(dir.path().join("refuse")).unwrap();
    std::fs::write(dir.path().join("expires"), "2026-11-26T12:05:00Z").unwrap();
    let soon = gh.pulls("work-1").await.expect_err("a token that expires too soon");
    assert!(soon.why.contains("expires 300 seconds after"), "{soon}");
    assert!(soon.why.contains("the machine's clock may be wrong"), "{soon}");
    // A key that is no key: openssl cannot sign, and GitHub is not asked.
    let other = dir.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let broken = other.join("broken.pem");
    std::fs::write(&broken, "not a key").unwrap();
    let gh_path = stand_in::gh(&other);
    let app = App::new(&stand_in::settings(&broken), Set::at(NOON)).expect("the file can be read");
    let app = app.asking(&stand_in::curl(&other), &stand_in::api(&other));
    let gh = Gh::new(
        &gh_path.display().to_string(),
        "o/r",
        Credential::App(Arc::new(app)),
        &other,
    );
    let failed = gh.pulls("work-1").await.expect_err("not signed");
    assert!(
        failed.why.starts_with(&format!(
            "openssl could not sign with the private key of the engine's GitHub App, {} (github_app.private_key)",
            broken.display()
        )),
        "{failed}"
    );
    assert!(
        !failed.why.contains("not a key"),
        "the key's text is not said: {failed}"
    );
    assert_eq!(failed.kind, Some(Failure::Other));
    assert_eq!(stand_in::calls(&other), Vec::new(), "GitHub was not asked");
}

// A key that is not there, or is no file, keeps the App from being made, saying where it was looked for and by which
// setting. Milliseconds.
#[test]
fn an_app_whose_key_cannot_be_read_is_not_made() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Set::at(NOON);
    for path in [dir.path().join("missing.pem"), dir.path().to_path_buf()] {
        let refused = App::new(&stand_in::settings(&path), clock.clone()).expect_err("refused");
        assert!(
            refused.starts_with(&format!(
                "github_app.private_key: the private key of the engine's GitHub App cannot be read at {}: ",
                path.display()
            )),
            "{refused}"
        );
    }
}
