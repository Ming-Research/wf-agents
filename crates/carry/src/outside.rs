//! The check of the outside the agents' rounds need, which the engine takes by itself while they wait for it, with no
//! round and in no work's session, under the account the engine names: the sign-in of their command line, as `auth
//! status` says it in that account's directory of settings; the network to their service, by a look at its host; and,
//! once both are well, one short request on the cheapest level's model, in a session that is not kept, with the
//! agents' command and environment and that directory. What is down is said by the first of them that is, the short
//! request's failure told apart as a round's is ([`failure`]).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use driver::Found;
use service::{Clock, Network, Outside, SystemClock};
use tokio::process::Command;
use work::Time;

use crate::settings::Settings;
use crate::{cli, failure, process};

/// How long the command line is given to say whether it is signed in.
const SIGN_IN_WAIT: Duration = Duration::from_secs(30);

/// How long the short request is given to be answered: the command line tries a service that does not answer again
/// itself, for a while, before it says so.
const REQUEST_WAIT: Duration = Duration::from_secs(120);

/// What the short request asks.
const REQUEST: &str = "Reply with the single word ok.";

/// The check of the outside of the agents' rounds, as the installation's settings say how they run.
pub struct Check {
    /// The agents' command line, as `agents.command` names it.
    command: String,
    /// The environment it is given, as a round's is ([`cli::environment`]), but for the directory of settings of the
    /// account it checks under, which each check names.
    env: Vec<(String, String)>,
    /// Which of its settings files it reads, as a round's does.
    setting_sources: String,
    /// The model of the cheapest level.
    model: String,
    /// Where the short request runs: a directory no project owns, which holds no guide for the command line to read.
    dir: PathBuf,
    /// The look at the network to the agents' service.
    network: Arc<dyn Network>,
    /// The clock a usage limit's reset is read by, where the command line gives it as a time of day.
    clock: Arc<dyn Clock>,
    /// Why the short request failed where the command line ended with no result, in the words beside the flows.
    words: flow::agents::ProgramWords,
    /// What the check finds down where it says it itself, in the words beside the flows.
    said: flow::agents::CheckWords,
}

impl Check {
    /// The check the settings say: with the agents' command, directory of settings and environment, the short request
    /// on the cheapest level's model, and the network looked at on the service's host ([`failure::Look`]); what it
    /// finds down is said in the words beside the flows of a program's end and of the check's own.
    pub fn of(settings: &Settings, cost: &flow::agents::Cost) -> Result<Check, String> {
        let levels = settings.levels()?;
        let cheapest =
            (levels.levels().first()).ok_or_else(|| "agents.levels: there is at least one level".to_string())?;
        let model = (settings.agents.levels.get(&cheapest.name))
            .map(|level| level.model.clone())
            .ok_or_else(|| format!("agents.levels: there is no level {:?}", cheapest.name))?;
        Ok(Check {
            command: settings.agents.command.clone(),
            // The check runs no project's command, so no project's variables are given it.
            env: cli::environment(&settings.agents, None, std::env::vars(), &Default::default()),
            setting_sources: settings.agents.setting_sources.clone(),
            model,
            dir: std::env::temp_dir(),
            network: Arc::new(failure::Look),
            clock: Arc::new(SystemClock),
            words: cost.program.clone(),
            said: cost.check.clone(),
        })
    }

    /// The agents' command line with these arguments, its environment under the account, and its directory.
    fn command(&self, args: &[&str], account: Option<&str>) -> Command {
        let mut command = Command::new(&self.command);
        command
            .args(args)
            .env_clear()
            .envs(self.env.iter().cloned())
            .current_dir(&self.dir);
        if let Some(dir) = account {
            command.env(crate::account::VARIABLE, dir);
        }
        command
    }

    /// What is down under the account, of the sign-in, the network and the service, in that order; well where none is.
    async fn found(&self, account: Option<&str>) -> Found {
        if let Some(down) = self.signed_out(account).await {
            return down;
        }
        let network = self.network.clone();
        let reached = tokio::task::spawn_blocking(move || network.reached()).await;
        if !reached.unwrap_or(false) {
            return Found::Down {
                failure: decide::run::Failure::Offline,
                why: self.said.offline.clone(),
                resets: None,
            };
        }
        self.request(account).await
    }

    /// What is down where the command line is not signed in, as it says in its directory of settings, or cannot be
    /// started; none where it says it is signed in, or says what cannot be read, which the short request then tells.
    async fn signed_out(&self, account: Option<&str>) -> Option<Found> {
        let cannot = |why: String| Found::Down {
            failure: decide::run::Failure::CannotRun,
            why,
            resets: None,
        };
        let output = match process::output(self.command(&["auth", "status"], account), SIGN_IN_WAIT).await {
            Ok(output) => output,
            Err(why) => return Some(cannot(why)),
        };
        // Of what it says, only that it is not signed in is kept: the rest names the account, which the page need not.
        let said = String::from_utf8_lossy(&output.stdout);
        let status: serde_json::Value = serde_json::from_str(said.trim()).ok()?;
        (status["loggedIn"] == false)
            .then(|| cannot(flow::agents::fill(&self.said.signed_out, &[("command", &self.command)])))
    }

    /// What the short request found: well where the service answered it with a message that is no error, else what
    /// is down as its failure tells it.
    async fn request(&self, account: Option<&str>) -> Found {
        let args = [
            "-p",
            "--model",
            &self.model,
            "--no-session-persistence",
            "--output-format",
            "stream-json",
            "--verbose",
            "--max-turns",
            "1",
            "--setting-sources",
            &self.setting_sources,
            REQUEST,
        ];
        let output = match process::output(self.command(&args, account), REQUEST_WAIT).await {
            Ok(output) => output,
            Err(why) => {
                // One that did not end in time had no answer from the service; one that cannot be started is the command
                // line failing to run.
                let failure = match why.contains("did not end within") {
                    true => decide::run::Failure::Unavailable,
                    false => failure::of(&why, &failure::Said::default(), None),
                };
                return Found::Down {
                    failure,
                    why,
                    resets: None,
                };
            }
        };
        let mut stream = cli::Stream::default();
        for line in output.stdout.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            stream.line(line);
        }
        if stream.answered() {
            return Found::Well;
        }
        let had_result = stream.has_result();
        let no = match stream.read(cli::Begin::New, 0) {
            Ok(_) => return Found::Well,
            Err(no) => no,
        };
        // What it said, as a round's failure says it: its result's error, or else the end of what it wrote of itself.
        let why = match (output.status.success(), had_result) {
            (true, _) => no.why,
            (false, true) => process::ended(&self.words, &self.command, output.status, &no.why),
            (false, false) => {
                let tail = process::tail(String::from_utf8_lossy(&output.stderr).trim().as_bytes());
                process::ended(&self.words, &self.command, output.status, &tail)
            }
        };
        let failure = failure::of(&why, &no.said, no.spent.as_ref());
        let now = self.clock.now().map_or(0, |now| now.0);
        let resets = (failure == decide::run::Failure::UsageLimit)
            .then(|| failure::resets(&why, &no.said, now, &failure::zone_offset))
            .flatten()
            .map(Time);
        Found::Down { failure, why, resets }
    }
}

impl Outside for Check {
    fn check(&self, account: Option<&str>) -> Pin<Box<dyn Future<Output = Found> + Send + '_>> {
        let account = account.map(str::to_string);
        Box::pin(async move { self.found(account.as_deref()).await })
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use decide::run::Failure;

    use super::*;

    /// A network that is reached, or not.
    struct Reached(bool);

    /// A clock that stands at this time: 2026-10-25 20:00 UTC, as the tests of a reset read it.
    struct At;

    impl Clock for At {
        fn now(&self) -> Result<Time, String> {
            Ok(Time(1_792_958_400_000))
        }
    }

    impl Network for Reached {
        fn reached(&self) -> bool {
            self.0
        }
    }

    /// The check of a command line that is this shell script, which keeps each call's arguments in `calls`, one line
    /// a call, with the network reached or not.
    fn check(dir: &std::path::Path, script: &str, reached: bool) -> Check {
        let command = dir.join("claude");
        let body = format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\n{script}\n",
            dir.join("calls").display()
        );
        std::fs::write(&command, body).unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755)).unwrap();
        Check {
            command: command.display().to_string(),
            env: vec![("PATH".to_string(), std::env::var("PATH").unwrap_or_default())],
            setting_sources: "user".to_string(),
            model: "claude-haiku-5-5".to_string(),
            dir: dir.to_path_buf(),
            network: Arc::new(Reached(reached)),
            clock: Arc::new(At),
            words: words().program,
            said: words().check,
        }
    }

    /// What is written beside the repository's flows of the agents' program and of the check.
    fn words() -> flow::agents::Cost {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        flow::agents::read(&root)
            .expect("the words beside the flows are read")
            .cost
    }

    /// The arguments of each call the check made of the command line, a line each.
    fn calls(dir: &std::path::Path) -> Vec<String> {
        let calls = std::fs::read_to_string(dir.join("calls")).unwrap_or_default();
        calls.lines().map(str::to_string).collect()
    }

    /// What the command line answers: signed in or not to `auth status`, and to the short request this stream.
    fn answering(signed_in: bool, stream: &str) -> String {
        format!(
            "case \"$1\" in auth) echo '{{\"loggedIn\": {signed_in}, \"authMethod\": \"claude.ai\"}}';; \
             *) cat <<'STREAM'\n{stream}\nSTREAM\n;; esac"
        )
    }

    const ANSWERED: &str = r#"{"type":"system","subtype":"init","claude_code_version":"2.1.323"}
{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m1","model":"claude-haiku-5-5","content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":12,"output_tokens":2}}}
{"type":"result","subtype":"success","is_error":false,"result":"ok","total_cost_usd":0.0001,"usage":{"input_tokens":12,"output_tokens":2}}"#;

    const OVERLOADED: &str = r#"{"type":"system","subtype":"init","claude_code_version":"2.1.323"}
{"type":"assistant","parent_tool_use_id":null,"error":"overloaded","api_error_status":529,"message":{"content":[{"type":"text","text":"API Error: 529 Overloaded"}]}}
{"type":"result","subtype":"success","is_error":true,"result":"API Error: 529 Overloaded","api_error_status":529,"total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0}}"#;

    const LIMITED: &str = r#"{"type":"system","subtype":"init","claude_code_version":"2.1.323"}
{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1793394000,"rateLimitType":"seven_day"}}
{"type":"result","subtype":"success","is_error":true,"result":"You've hit your weekly limit","total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0}}"#;

    // A few milliseconds: a shell script run twice.
    #[tokio::test]
    async fn a_check_signs_in_and_asks_under_the_account_it_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let said = format!(
            "echo \"under $CLAUDE_CONFIG_DIR\" >> '{}'\n{}",
            dir.path().join("under").display(),
            answering(true, ANSWERED)
        );
        let check = check(dir.path(), &said, true);
        assert_eq!(check.check(Some("/data/claude-second")).await, Found::Well);
        let under = std::fs::read_to_string(dir.path().join("under")).unwrap();
        assert_eq!(under, "under /data/claude-second\nunder /data/claude-second\n");
        std::fs::remove_file(dir.path().join("under")).unwrap();
        // Named none, it runs under the program's own directory.
        assert_eq!(check.check(None).await, Found::Well);
        let under = std::fs::read_to_string(dir.path().join("under")).unwrap();
        assert_eq!(under, "under \nunder \n");
    }

    // Each takes a few milliseconds: a shell script run twice or three times.
    #[tokio::test]
    async fn a_check_finds_the_outside_well_where_the_sign_in_the_network_and_a_short_request_are() {
        let dir = tempfile::tempdir().unwrap();
        let check = check(dir.path(), &answering(true, ANSWERED), true);
        assert_eq!(check.check(None).await, Found::Well);
        let calls = calls(dir.path());
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(calls[0], "auth status");
        // The short request is on the cheapest level's model, in a session not kept, and names no session of a work.
        let request = &calls[1];
        for part in [
            "-p",
            "--model claude-haiku-5-5",
            "--no-session-persistence",
            "--max-turns 1",
        ] {
            assert!(request.contains(part), "{part}: {request}");
        }
        for part in ["--resume", "--session-id"] {
            assert!(!request.contains(part), "{part}: {request}");
        }
    }

    #[tokio::test]
    async fn a_check_says_what_is_down_by_the_first_of_the_sign_in_the_network_and_the_service_that_is() {
        let dir = tempfile::tempdir().unwrap();
        // Not signed in: neither the network nor the service is tried.
        let signed_out = check(dir.path(), &answering(false, ANSWERED), true).check(None).await;
        let Found::Down { failure, why, .. } = &signed_out else {
            panic!("{signed_out:?}");
        };
        assert_eq!(*failure, Failure::CannotRun);
        // In the words beside the flows, which name the command line's own command and not the command line again.
        let command = dir.path().join("claude").display().to_string();
        let said = flow::agents::fill(&words().check.signed_out, &[("command", &command)]);
        assert_eq!(why, &said);
        assert_eq!(said, format!("It is not signed in, as `{command} auth status` says."));
        assert_eq!(calls(dir.path()), ["auth status"]);
        // Signed in, and the network down: no request is made.
        let dir = tempfile::tempdir().unwrap();
        let offline = check(dir.path(), &answering(true, ANSWERED), false).check(None).await;
        assert!(
            matches!(
                &offline,
                Found::Down {
                    failure: Failure::Offline,
                    why,
                    ..
                } if why == "The network does not reach the agents' service."
            ),
            "{offline:?}"
        );
        assert_eq!(calls(dir.path()), ["auth status"]);
        // The service overloaded, and the account at its usage limit, as the short request's failure tells them.
        let dir = tempfile::tempdir().unwrap();
        let overloaded = check(dir.path(), &answering(true, OVERLOADED), true).check(None).await;
        assert!(
            matches!(
                overloaded,
                Found::Down {
                    failure: Failure::Unavailable,
                    ..
                }
            ),
            "{overloaded:?}"
        );
        let dir = tempfile::tempdir().unwrap();
        let limited = check(dir.path(), &answering(true, LIMITED), true).check(None).await;
        assert_eq!(
            limited,
            Found::Down {
                failure: Failure::UsageLimit,
                why: "the command line's result is an error: You've hit your weekly limit".to_string(),
                resets: Some(Time(1_793_394_000_000)),
            }
        );
    }

    #[tokio::test]
    async fn a_command_line_that_cannot_be_started_cannot_run() {
        let dir = tempfile::tempdir().unwrap();
        let mut check = check(dir.path(), "", true);
        check.command = dir.path().join("missing").display().to_string();
        let found = check.check(None).await;
        assert!(
            matches!(
                found,
                Found::Down {
                    failure: Failure::CannotRun,
                    ..
                }
            ),
            "{found:?}"
        );
        // One whose sign-in says what cannot be read, and whose request prints nothing, never began.
        let dir = tempfile::tempdir().unwrap();
        let found = self::check(dir.path(), "echo 'what?' >&2; exit 1", true)
            .check(None)
            .await;
        let Found::Down { failure, why, .. } = &found else {
            panic!("{found:?}");
        };
        assert_eq!(*failure, Failure::CannotRun);
        assert!(why.ends_with("what?"), "{why}");
        assert_eq!(calls(dir.path()).len(), 2);
    }
}
