//! What the tests of the engine's GitHub App stand in with: a private key made for the test by `openssl`, a clock the
//! test sets, and a `gh` that stands in for GitHub, giving tokens of the installation and keeping each call it was
//! asked with the token it was given.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use service::Clock;
use work::Time;

use super::App;
use crate::settings;

/// The App's id in the tests.
pub(crate) const APP: u64 = 4242;

/// The id of its installation in the tests.
pub(crate) const INSTALLATION: u64 = 77;

/// A private key made for the test, `app.pem` in the directory, and its public key, `app.pub.pem`.
pub(crate) fn key(dir: &Path) -> (PathBuf, PathBuf) {
    let (key, public) = (dir.join("app.pem"), dir.join("app.pub.pem"));
    openssl(&["genrsa", "-out", &key.display().to_string(), "2048"]);
    let args = [
        "rsa",
        "-in",
        &key.display().to_string(),
        "-pubout",
        "-out",
        &public.display().to_string(),
    ];
    openssl(&args);
    (key, public)
}

/// Runs `openssl` with these arguments, which must succeed.
fn openssl(args: &[&str]) {
    let output = std::process::Command::new("openssl")
        .args(args)
        .output()
        .expect("openssl runs");
    assert!(
        output.status.success(),
        "openssl {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A clock the test sets, in seconds since 1970.
pub(crate) struct Set(AtomicU64);

impl Set {
    pub(crate) fn at(seconds: u64) -> Arc<Set> {
        Arc::new(Set(AtomicU64::new(seconds)))
    }

    /// Moves the clock on by these seconds.
    pub(crate) fn pass(&self, seconds: u64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}

impl Clock for Set {
    fn now(&self) -> Result<Time, String> {
        Ok(Time(self.0.load(Ordering::SeqCst) * 1000))
    }
}

/// A `gh` in the directory that stands in for GitHub. Each call is kept as a line of the file `calls`: the token it
/// was given, then its arguments. A request for a token of the installation [`INSTALLATION`] is answered 201 with the
/// token `ghs_given-<n>`, `<n>` counting the tokens given, which expires at the time the file `expires` holds; or, while
/// the file `refuse` is there, with the status line it holds. Any other call is answered 200 with an empty list.
pub(crate) fn gh(dir: &Path) -> PathBuf {
    let script = dir.join("gh");
    let text = format!(
        r#"#!/bin/sh
d='{dir}'
printf '%s %s\n' "$GH_TOKEN" "$*" >> "$d/calls"
case "$*" in
*app/installations/{INSTALLATION}/access_tokens*)
  if [ -f "$d/refuse" ]; then printf 'HTTP/2.0 %s\r\n\r\n{{"message":"refused"}}' "$(cat "$d/refuse")"; exit 1; fi
  n=$(( $(cat "$d/given" 2>/dev/null || echo 0) + 1 ))
  echo "$n" > "$d/given"
  printf 'HTTP/2.0 201 Created\r\n\r\n{{"token":"ghs_given-%s","expires_at":"%s"}}' "$n" "$(cat "$d/expires")" ;;
*) printf 'HTTP/2.0 200 OK\r\n\r\n[]' ;;
esac
"#,
        dir = dir.display()
    );
    std::fs::write(&script, text).expect("written");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("made runnable");
    script
}

/// The calls the stand-in for GitHub was asked, each as the token it was given and its arguments.
pub(crate) fn calls(dir: &Path) -> Vec<(String, String)> {
    (std::fs::read_to_string(dir.join("calls")).unwrap_or_default().lines())
        .map(|line| {
            let (token, args) = line.split_once(' ').unwrap_or((line, ""));
            (token.to_string(), args.to_string())
        })
        .collect()
}

/// The settings of the App, its key at this path.
pub(crate) fn settings(key: &Path) -> settings::GitHubApp {
    settings::GitHubApp {
        app_id: APP,
        installation_id: INSTALLATION,
        private_key: key.to_path_buf(),
    }
}

/// The App with a key made for the test in the directory, its tokens asked of the stand-in for GitHub there, by the
/// time this clock reads; and the App's public key.
pub(crate) fn app(dir: &Path, clock: Arc<Set>) -> (Arc<App>, PathBuf) {
    let (key, public) = key(dir);
    let gh = gh(dir);
    let app = App::new(&settings(&key), &gh.display().to_string(), dir, clock).expect("the App is made");
    (Arc::new(app), public)
}
