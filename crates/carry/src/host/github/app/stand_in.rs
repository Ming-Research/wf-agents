//! What the tests of the engine's GitHub App stand in with: a private key made for the test by `openssl`, a clock the
//! test sets, a stand-in for GitHub's REST API on a port of this machine, which the real `curl` asks for the
//! installation's tokens, and a `gh` that stands in for GitHub. Both stand-ins keep each request with the token it was
//! given.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
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

/// What GitHub answers, with the status 401, to a token for the App that does not come in the header
/// `Authorization: Bearer`, as GitHub answered it to one sent as `Authorization: token`, the way `gh` sends the token
/// it is given.
pub(crate) const UNDECODED: &str = r#"{"message":"A JSON web token could not be decoded","documentation_url":"https://docs.github.com/rest","status":"401"}"#;

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

/// GitHub's REST API as it answers requests for a token of the installation, at an address of this machine,
/// `http://127.0.0.1:<port>`, for the files of the directory. Each request is kept as a line of the file `calls`: the
/// token of its header `Authorization`, then its request line. One for a token of the installation [`INSTALLATION`]
/// whose header is not `Bearer` is answered 401 with [`UNDECODED`], as GitHub answers it; while the file `refuse` is
/// there, with the status line it holds; else 201 with the token `ghs_given-<n>`, `<n>` counting the tokens given,
/// which expires at the time the file `expires` holds. Any other request is answered 404. It answers until the test's
/// process ends.
pub(crate) fn api(dir: &Path) -> String {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a port");
    let address = format!("http://{}", listener.local_addr().expect("its address"));
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        let mut given = 0;
        for stream in listener.incoming().flatten() {
            answer(&dir, stream, &mut given);
        }
    });
    address
}

/// Answers one request of the stand-in for GitHub's API, as [`api`] says, `given` counting the tokens given.
fn answer(dir: &Path, mut stream: TcpStream, given: &mut u64) {
    let mut reader = BufReader::new(stream.try_clone().expect("the connection is read"));
    let mut request = String::new();
    let _ = reader.read_line(&mut request);
    let mut authorization = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("authorization")
        {
            authorization = value.trim().to_string();
        }
    }
    let token = authorization.rsplit(' ').next().unwrap_or_default();
    let mut calls = (std::fs::OpenOptions::new().create(true).append(true))
        .open(dir.join("calls"))
        .expect("the calls are kept");
    writeln!(calls, "{token} {}", request.trim_end()).expect("the call is kept");
    let path = request.split_whitespace().nth(1).unwrap_or_default();
    let (status, body) = if path != format!("/app/installations/{INSTALLATION}/access_tokens") {
        ("404 Not Found".to_string(), r#"{"message":"Not Found"}"#.to_string())
    } else if !authorization.starts_with("Bearer ") {
        ("401 Unauthorized".to_string(), UNDECODED.to_string())
    } else if let Ok(status) = std::fs::read_to_string(dir.join("refuse")) {
        (status.trim().to_string(), r#"{"message":"refused"}"#.to_string())
    } else {
        *given += 1;
        let expires = std::fs::read_to_string(dir.join("expires")).expect("the test says when the token expires");
        let body = format!(r#"{{"token":"ghs_given-{given}","expires_at":"{}"}}"#, expires.trim());
        ("201 Created".to_string(), body)
    };
    let answered = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: \
         close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(answered.as_bytes());
}

/// A `curl` in the directory that keeps the arguments of each call as a line of the file `arguments`, and is then the
/// real `curl`.
pub(crate) fn curl(dir: &Path) -> PathBuf {
    let script = dir.join("curl");
    let text = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}/arguments'\nexec curl \"$@\"\n",
        dir.display()
    );
    std::fs::write(&script, text).expect("written");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("made runnable");
    script
}

/// A `gh` in the directory that stands in for GitHub. Each call is kept as a line of the file `calls`: the token it
/// was given, then its arguments. `gh` sends the token it is given in the header `Authorization: token`, so a request
/// for a token of the installation [`INSTALLATION`] is answered 401 with [`UNDECODED`], as GitHub answers it. Any
/// other call is answered 200 with an empty list.
pub(crate) fn gh(dir: &Path) -> PathBuf {
    let script = dir.join("gh");
    let text = format!(
        r#"#!/bin/sh
d='{dir}'
printf '%s %s\n' "$GH_TOKEN" "$*" >> "$d/calls"
case "$*" in
*app/installations/{INSTALLATION}/access_tokens*)
  printf 'HTTP/2.0 401 Unauthorized\r\n\r\n%s' '{UNDECODED}'; exit 1 ;;
*) printf 'HTTP/2.0 200 OK\r\n\r\n[]' ;;
esac
"#,
        dir = dir.display()
    );
    std::fs::write(&script, text).expect("written");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("made runnable");
    script
}

/// The calls the stand-ins for GitHub were asked, each as the token it was given and its arguments or its request
/// line.
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

/// The App with a key made for the test in the directory, its tokens asked by `curl` ([`curl`]) of the stand-in for
/// GitHub's API there ([`api`]), by the time this clock reads, beside a `gh` that stands in for GitHub there ([`gh`]);
/// and the App's public key.
pub(crate) fn app(dir: &Path, clock: Arc<Set>) -> (Arc<App>, PathBuf) {
    let (key, public) = key(dir);
    gh(dir);
    let app = App::new(&settings(&key), clock).expect("the App is made");
    (Arc::new(app.asking(&curl(dir), &api(dir))), public)
}
