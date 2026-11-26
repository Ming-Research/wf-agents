//! The engine's GitHub App, as which the engine makes every call of GitHub: those of its REST and GraphQL APIs through
//! `gh`, and git's pushes and fetches over HTTPS. The App's private key signs a JSON web token, RS256 with the App's id
//! as its issuer, issued a minute before the engine's clock reads and lasting nine minutes, under the ten GitHub takes.
//! GitHub gives that token's bearer a token of the App's installation, which lasts an hour. Each call is given the
//! installation's token, and a new one is got once less than [`FRESH_FOR`] of the last is left, so that no call is given
//! a token that expires while it goes on. The App's calls count against a limit of the App's own, which no person's
//! use of GitHub shares.
//!
//! The key is read by `openssl` alone, which signs with it; the engine only opens it as it starts, to see that it can.
//! Neither the key nor a token is said in any words the engine gives. Where no token can be got, each call fails saying
//! why, and no other credential is tried: the engine has none.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use service::Clock;
use tokio::io::AsyncWriteExt;

use super::gh::{Gh, epoch};
use crate::host::Failed;
use crate::settings;

/// The program that signs with the App's private key.
const OPENSSL: &str = "openssl";

/// How long before the engine's clock reads a token for the App says it was issued, in seconds: GitHub refuses one
/// issued after its own clock reads, and its guide asks for a minute.
pub(crate) const ISSUED_BEFORE: u64 = 60;

/// How long a token for the App lasts from its issue, in seconds: under the ten minutes GitHub takes at most.
pub(crate) const LIFETIME: u64 = 9 * 60;

/// How much of an installation's token must be left for a call to be given it, in seconds. A call of `gh` is given up
/// after two minutes ([`super::gh::LIMIT`]), and this leaves room for a push or a fetch that goes on longer. A
/// judgment, not a measurement.
pub(crate) const FRESH_FOR: u64 = 10 * 60;

/// How long `openssl` may take to sign before it is given up: it signs in milliseconds.
const SIGN_LIMIT: Duration = Duration::from_secs(30);

/// What gives a call of GitHub its token.
#[derive(Clone)]
pub enum Credential {
    /// The token of the App's installation, as [`App::token`] gives it for each call.
    App(Arc<App>),
    /// A token given once: the token for the App that is exchanged for one of its installation, or a test's.
    Given(String),
}

impl Credential {
    /// The token for a call; or why none can be got.
    pub async fn token(&self) -> Result<String, Failed> {
        match self {
            Credential::App(app) => app.token().await,
            Credential::Given(token) => Ok(token.clone()),
        }
    }
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // A token given is not shown.
        match self {
            Credential::App(app) => f.debug_tuple("App").field(app).finish(),
            Credential::Given(_) => f.write_str("Given(..)"),
        }
    }
}

/// The App, with the installation whose tokens the engine's calls are given, and the token it holds.
pub struct App {
    /// The App's id, the issuer of each token for it.
    id: u64,
    /// The id of the App's installation, whose token the engine gets.
    installation: u64,
    /// The App's private key, by its path.
    key: PathBuf,
    /// The `gh` that asks GitHub for the installation's token.
    gh: String,
    /// The directory that `gh` keeps its configuration in.
    config_dir: PathBuf,
    clock: Arc<dyn Clock>,
    /// The installation's token got last, locked while one is got, so that calls that come together share one.
    held: tokio::sync::Mutex<Option<Held>>,
}

/// An installation's token, and when it expires, in seconds since 1970.
struct Held {
    token: String,
    expires: u64,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The token it holds is not shown.
        f.debug_struct("App")
            .field("id", &self.id)
            .field("installation", &self.installation)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl App {
    /// The App the settings name, its installation's tokens asked for with this `gh`, which keeps its configuration in
    /// this directory, by the time this clock reads; or why not, where the private key is no file that can be read.
    pub fn new(app: &settings::GitHubApp, gh: &str, config_dir: &Path, clock: Arc<dyn Clock>) -> Result<App, String> {
        let key = &app.private_key;
        let unreadable = |why: String| {
            format!(
                "github_app.private_key: the private key of the engine's GitHub App cannot be read at {}: {why}",
                key.display()
            )
        };
        let found = std::fs::metadata(key).map_err(|e| unreadable(e.to_string()))?;
        if !found.is_file() {
            return Err(unreadable("it is no file".to_string()));
        }
        std::fs::File::open(key).map_err(|e| unreadable(e.to_string()))?;
        Ok(App {
            id: app.app_id,
            installation: app.installation_id,
            key: key.clone(),
            gh: gh.to_string(),
            config_dir: config_dir.to_path_buf(),
            clock,
            held: tokio::sync::Mutex::new(None),
        })
    }

    /// The installation's token for a call: the one held, where more than [`FRESH_FOR`] of it is left; else a new one,
    /// which is held from then on. Where none can be got, why, of the kind GitHub's answer tells.
    pub async fn token(&self) -> Result<String, Failed> {
        let mut held = self.held.lock().await;
        let now = self.now()?;
        if let Some(held) = held.as_ref()
            && held.expires > now + FRESH_FOR
        {
            return Ok(held.token.clone());
        }
        let got = self.exchanged(now).await?;
        if got.expires <= now + FRESH_FOR {
            return Err(Failed::refused(format!(
                "GitHub gave the engine's GitHub App a token that expires {} seconds after the engine's clock reads, \
                 less than the {FRESH_FOR} a call needs: the machine's clock may be wrong",
                got.expires.saturating_sub(now)
            )));
        }
        let token = got.token.clone();
        *held = Some(got);
        Ok(token)
    }

    /// The time the clock reads, in seconds since 1970.
    fn now(&self) -> Result<u64, Failed> {
        let now = self.clock.now().map_err(Failed::refused)?;
        Ok(now.0 / 1000)
    }

    /// A new token of the installation, which GitHub gives for a token for the App signed at `now`, in seconds since
    /// 1970.
    async fn exchanged(&self, now: u64) -> Result<Held, Failed> {
        let signed = self.signed(now).await?;
        let gh = Gh::new(&self.gh, "", Credential::Given(signed), &self.config_dir);
        let path = format!("app/installations/{}/access_tokens", self.installation);
        let said = |why: String| {
            format!(
                "GitHub gave the engine's GitHub App {} (github_app.app_id) no token of its installation {} \
                 (github_app.installation_id): {why}",
                self.id, self.installation
            )
        };
        let answer = (gh.json("POST", &path, None, &[201]).await).map_err(|failed| failed.said(said))?;
        let token = (answer["token"].as_str())
            .filter(|token| !token.is_empty())
            .ok_or_else(|| Failed::refused(said("its answer holds no token".to_string())))?;
        let expires = (answer["expires_at"].as_str())
            .and_then(epoch)
            .and_then(|seconds| u64::try_from(seconds).ok())
            .ok_or_else(|| Failed::refused(said("its answer says no time the token expires".to_string())))?;
        Ok(Held {
            token: token.to_string(),
            expires,
        })
    }

    /// A token for the App issued [`ISSUED_BEFORE`] before `now`, in seconds since 1970, signed with its key.
    pub(crate) async fn signed(&self, now: u64) -> Result<String, Failed> {
        let unsigned = unsigned(self.id, now);
        let signature = self.sign(unsigned.as_bytes()).await?;
        Ok(format!("{unsigned}.{}", base64url(&signature)))
    }

    /// The RS256 signature of these bytes, by `openssl` with the App's private key; or why it could not sign. A key
    /// kept under a pass phrase is refused, never asked of anyone.
    async fn sign(&self, bytes: &[u8]) -> Result<Vec<u8>, Failed> {
        let cannot = |why: String| {
            Failed::refused(format!(
                "{OPENSSL} could not sign with the private key of the engine's GitHub App, {} (github_app.private_key): \
                 {why}",
                self.key.display()
            ))
        };
        let mut child = crate::programs::command(OPENSSL)
            .args(["dgst", "-sha256", "-passin", "pass:", "-sign"])
            .arg(&self.key)
            .env_clear()
            .envs(crate::programs::kept().iter().cloned())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| cannot(format!("it cannot be started: {e}")))?;
        if let Some(mut pipe) = child.stdin.take() {
            let bytes = bytes.to_vec();
            tokio::spawn(async move {
                let _ = pipe.write_all(&bytes).await;
            });
        }
        let waited = tokio::time::timeout(SIGN_LIMIT, child.wait_with_output()).await;
        let output = (waited.map_err(|_| cannot(format!("it did not end within {} seconds", SIGN_LIMIT.as_secs()))))?
            .map_err(|e| cannot(format!("it could not be waited for: {e}")))?;
        if !output.status.success() || output.stdout.is_empty() {
            let said = String::from_utf8_lossy(&output.stderr).trim().replace('\n', " ");
            return Err(cannot(format!(
                "{} {}",
                output.status,
                crate::process::tail(said.as_bytes())
            )));
        }
        Ok(output.stdout)
    }
}

/// What a token for the App signs, its header and its claims, each as base64url: issued [`ISSUED_BEFORE`] before
/// `now`, in seconds since 1970, expiring [`LIFETIME`] after its issue, by the App of this id.
pub(crate) fn unsigned(app: u64, now: u64) -> String {
    let issued = now.saturating_sub(ISSUED_BEFORE);
    let header = serde_json::json!({ "alg": "RS256", "typ": "JWT" });
    let claims = serde_json::json!({ "iat": issued, "exp": issued + LIFETIME, "iss": app });
    format!(
        "{}.{}",
        base64url(header.to_string().as_bytes()),
        base64url(claims.to_string().as_bytes())
    )
}

/// The bytes in base64url, without padding, as a JSON web token writes each of its parts (RFC 7515, section 2).
pub(crate) fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        // A chunk of n bytes gives n + 1 characters.
        for at in 0..=chunk.len() {
            out.push(char::from(ALPHABET[((group >> (18 - 6 * at)) & 0x3f) as usize]));
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod stand_in;

#[cfg(test)]
mod tests;
