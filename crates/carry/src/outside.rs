//! The check of the outside the agents' rounds need, which the engine takes by itself while they wait for it, with no
//! round and in no work's session, under the account in use: an account to run under; the network to their service,
//! by a look at its host; and, once both are well, one short request on the cheapest level's model, in a session that
//! is not kept, with the agents' command, the program's directory and environment, built as a round's is, with the
//! account's token, at each check. What is down is said by the first of them that is, the short request's failure told
//! apart as a round's is ([`failure`]). With no account in use, the check is down with nothing run, in the words beside
//! the flows (decision no-default-credential).
//!
//! Only the short request finds a token the service refuses, as a command line that cannot run. A token the keychain
//! cannot give is that too, in the keychain's words. A token not yet kept is checked the same way before it is kept
//! ([`Check::check_token`]). What the short request's stream says of the account's usage is heard with what was found:
//! the owner's look at the accounts is this check, under each account looked under.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use driver::{Found, Said};
use service::{Clock, Heard, Network, Outside, SystemClock, Token, Tokens};
use tokio::process::Command;
use work::Time;

use crate::settings::{Agents, Settings};
use crate::{account, cli, failure, process};

/// How long the short request is given to be answered: the command line tries a service that does not answer again
/// itself, for a while, before it says so.
const REQUEST_WAIT: Duration = Duration::from_secs(120);

/// The check of the outside of the agents' rounds, as the installation's settings say how they run.
pub struct Check {
    /// The agents' command line, as `agents.command` names it.
    command: String,
    /// How the agents are run, of which the environment is built at each check, as a round's is
    /// ([`cli::environment`]).
    agents: Agents,
    /// The program's directory of settings and sessions, the engine's.
    profile: PathBuf,
    /// The engine's own environment, of which the command line is given what a round is.
    engine: Vec<(String, String)>,
    /// What keeps the tokens of the accounts.
    tokens: Arc<dyn Tokens>,
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
    /// The check the settings say: with the agents' command, the program's directory, `profile`, and environment, the
    /// token of the account it is taken under as what keeps the tokens gives it, the short request on the cheapest
    /// level's model, and the network looked at on the service's host ([`failure::Look`]); what it finds down is said
    /// in the words beside the flows of a program's end and of the check's own.
    pub fn of(
        settings: &Settings,
        cost: &flow::agents::Cost,
        tokens: Arc<dyn Tokens>,
        profile: &Path,
    ) -> Result<Check, String> {
        let levels = settings.levels()?;
        let cheapest =
            (levels.levels().first()).ok_or_else(|| "agents.levels: there is at least one level".to_string())?;
        let model = (settings.agents.levels.get(&cheapest.name))
            .map(|level| level.model.clone())
            .ok_or_else(|| format!("agents.levels: there is no level {:?}", cheapest.name))?;
        Ok(Check {
            command: settings.agents.command.clone(),
            agents: settings.agents.clone(),
            profile: profile.to_path_buf(),
            engine: crate::programs::engine().to_vec(),
            tokens,
            model,
            dir: std::env::temp_dir(),
            network: Arc::new(failure::Look),
            clock: Arc::new(SystemClock),
            words: cost.program.clone(),
            said: cost.check.clone(),
        })
    }

    /// The agents' command line with these arguments, its environment, with the token, and its directory.
    fn command(&self, args: &[&str], token: &Token) -> Command {
        let mut command = crate::programs::command(&self.command);
        command
            .args(args)
            .env_clear()
            .envs(cli::environment(
                &self.agents,
                &self.profile,
                self.engine.iter().cloned(),
                // The program is run for no project's command, so no project's variables are given it.
                &BTreeMap::new(),
                token,
            ))
            .current_dir(&self.dir);
        command
    }

    /// What is heard under the account of this name: with none, that none is in use, nothing run; its token not given,
    /// a command line that cannot run; else as [`Check::found`] finds it.
    async fn under(&self, account: Option<String>) -> Heard {
        let cannot = |why: String| {
            Heard::from(Found::Down {
                failure: decide::run::Failure::CannotRun,
                why,
                resets: None,
            })
        };
        match account::token_of(&self.tokens, account.as_deref(), &self.said.no_account).await {
            Ok(token) => self.found(&token).await,
            Err(why) => cannot(why),
        }
    }

    /// What is down, of the network and the service, in that order, under this token, with what the service said of
    /// its usage; well where neither is.
    async fn found(&self, token: &Token) -> Heard {
        let network = self.network.clone();
        let reached = tokio::task::spawn_blocking(move || network.reached()).await;
        if !reached.unwrap_or(false) {
            return Found::Down {
                failure: decide::run::Failure::Offline,
                why: self.said.offline.clone(),
                resets: None,
            }
            .into();
        }
        self.request(token).await
    }

    /// What the short request heard, as [`Check::asked`] finds it, said for every caller alike: words of the program's
    /// that hold the token hold it no more, and a request that met the usage limit, where the service named no window,
    /// says the account is at its limit, with its reset where it was said.
    async fn request(&self, token: &Token) -> Heard {
        let (found, said) = self.asked(token).await;
        let found = match found {
            Found::Down { failure, why, resets } if !token.secret().is_empty() => Found::Down {
                failure,
                why: why.replace(token.secret(), "…"),
                resets,
            },
            found => found,
        };
        let usage = match (&found, said) {
            (_, Some(said)) => Some(said),
            (
                Found::Down {
                    failure: decide::run::Failure::UsageLimit,
                    resets,
                    ..
                },
                None,
            ) => Some(Said::Limit { resets: *resets }),
            (_, None) => None,
        };
        Heard { found, usage }
    }

    /// What the short request found: well where the service answered it with a message that is no error, else what
    /// is down as its failure tells it; with what its stream said of the usage, where it said anything.
    async fn asked(&self, token: &Token) -> (Found, Option<Said>) {
        let settings = cli::keeping().to_string();
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
            cli::SETTING_SOURCES,
            "--settings",
            &settings,
            &self.said.request,
        ];
        let output = match process::output(self.command(&args, token), REQUEST_WAIT).await {
            Ok(output) => output,
            Err(unrun) => {
                // One that did not end in time had no answer from the service; one that cannot be started is the
                // command line failing to run.
                let failure = match &unrun {
                    process::Unrun::Timeout(_) => decide::run::Failure::Unavailable,
                    process::Unrun::Cannot(why) => failure::of(why, &failure::Said::default(), None),
                };
                let down = Found::Down {
                    failure,
                    why: unrun.into(),
                    resets: None,
                };
                return (down, None);
            }
        };
        let mut stream = cli::Stream::default();
        for line in output.stdout.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            stream.line(line);
        }
        let said = stream.usage().cloned();
        if stream.answered() {
            return (Found::Well, said);
        }
        let had_result = stream.has_result();
        let no = match stream.read(&self.words) {
            Ok(_) => return (Found::Well, said),
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
        let report = match (output.status.success(), had_result) {
            (true, _) | (false, true) => no.report,
            (false, false) => String::from_utf8_lossy(&output.stderr).trim().to_string(),
        };
        let failure = failure::of(&report, &no.said, no.spent.as_ref());
        let now = self.clock.now().map_or(0, |now| now.0);
        let resets = (failure == decide::run::Failure::UsageLimit)
            .then(|| failure::resets(&report, &no.said, now, &failure::zone_offset))
            .flatten()
            .map(Time);
        (Found::Down { failure, why, resets }, said)
    }
}

impl Outside for Check {
    fn check(&self, account: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        Box::pin(self.under(account))
    }

    fn check_token(&self, token: Token) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        Box::pin(async move { self.found(&token).await })
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use decide::run::Failure;
    use driver::Window;

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
    /// a call, and the token and the directory it was given in `given`, with the network reached or not; the token of
    /// the account `work` kept.
    fn check(dir: &std::path::Path, script: &str, reached: bool) -> Check {
        let command = dir.join("claude");
        let body = format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\necho \"$CLAUDE_CODE_OAUTH_TOKEN $CLAUDE_CONFIG_DIR\" >> '{}'\n{script}\n",
            dir.join("calls").display(),
            dir.join("given").display()
        );
        std::fs::write(&command, body).unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755)).unwrap();
        let settings = Settings::read(include_str!("../install.sample.toml")).expect("the sample reads");
        let kept = service::KeptTokens::new();
        kept.keep("work", &Token::new("sk-ant-oat01-secret".to_string()))
            .unwrap();
        Check {
            command: command.display().to_string(),
            agents: settings.agents,
            profile: dir.join("profile"),
            engine: vec![("PATH".to_string(), std::env::var("PATH").unwrap_or_default())],
            tokens: Arc::new(kept),
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

    /// The lines a file the script wrote holds, a line each call.
    fn lines(dir: &std::path::Path, file: &str) -> Vec<String> {
        let calls = std::fs::read_to_string(dir.join(file)).unwrap_or_default();
        calls.lines().map(str::to_string).collect()
    }

    /// The arguments of each call the check made of the command line, a line each.
    fn calls(dir: &std::path::Path) -> Vec<String> {
        lines(dir, "calls")
    }

    /// What the command line answers to the short request: this stream.
    fn answering(stream: &str) -> String {
        format!("cat <<'STREAM'\n{stream}\nSTREAM")
    }

    const ANSWERED: &str = r#"{"type":"system","subtype":"init","claude_code_version":"2.1.323"}
{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"seven_day":{"utilization":0.04,"resetsAt":1795399200,"status":"allowed"}}}}
{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m1","model":"claude-haiku-5-5","content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":12,"output_tokens":2}}}
{"type":"result","subtype":"success","is_error":false,"result":"ok","total_cost_usd":0.0001,"usage":{"input_tokens":12,"output_tokens":2}}"#;

    const OVERLOADED: &str = r#"{"type":"system","subtype":"init","claude_code_version":"2.1.323"}
{"type":"assistant","parent_tool_use_id":null,"error":"overloaded","api_error_status":529,"message":{"content":[{"type":"text","text":"API Error: 529 Overloaded"}]}}
{"type":"result","subtype":"success","is_error":true,"result":"API Error: 529 Overloaded","api_error_status":529,"total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0}}"#;

    const LIMITED: &str = r#"{"type":"system","subtype":"init","claude_code_version":"2.1.323"}
{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1793394000,"rateLimitType":"seven_day"}}
{"type":"result","subtype":"success","is_error":true,"result":"You've hit your weekly limit","total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0}}"#;

    /// The week's window as [`ANSWERED`] names it.
    fn week() -> Window {
        Window {
            name: "seven_day".to_string(),
            utilization: Some(0.04),
            resets: Some(Time(1_795_399_200_000)),
            allowed: true,
        }
    }

    // Each takes a few milliseconds: a shell script run once or twice.
    #[tokio::test]
    async fn a_check_finds_the_outside_well_where_the_network_and_a_short_request_are_and_hears_the_usage() {
        let dir = tempfile::tempdir().unwrap();
        let mut check = check(dir.path(), &answering(ANSWERED), true);
        // What it asks is the words' `request`, whatever they are.
        check.said.request = "Say pong.".to_string();
        let heard = check.check(Some("work".to_string())).await;
        assert_eq!(heard.found, Found::Well);
        assert_eq!(
            heard.usage,
            Some(Said::Windows(vec![week()])),
            "the account's usage, as the request's stream names it"
        );
        let calls = calls(dir.path());
        assert_eq!(calls.len(), 1, "no sign-in is asked of: {calls:?}");
        // The short request is on the cheapest level's model, in a session not kept, and names no session of a work;
        // it is given the settings that keep the program's sessions.
        let request = &calls[0];
        assert!(request.ends_with("Say pong."), "{request}");
        for part in [
            "-p",
            "--model claude-haiku-5-5",
            "--no-session-persistence",
            "--max-turns 1",
            r#"--settings {"cleanupPeriodDays":365000}"#,
        ] {
            assert!(request.contains(part), "{part}: {request}");
        }
        for part in ["--resume", "--session-id", "auth"] {
            assert!(!request.contains(part), "{part}: {request}");
        }
        // Under the account's token, in the engine's directory.
        let profile = dir.path().join("profile").display().to_string();
        assert_eq!(lines(dir.path(), "given"), [format!("sk-ant-oat01-secret {profile}")]);
    }

    // A few milliseconds: a shell script run three times. The order the check asks in, which keeps a request from
    // being spent where no account is in use or the network is down.
    #[tokio::test]
    async fn a_check_says_what_is_down_by_the_first_of_the_account_the_network_and_the_service_that_is() {
        let dir = tempfile::tempdir().unwrap();
        // No account in use: nothing is run, and the words beside the flows say so.
        let heard = check(dir.path(), &answering(ANSWERED), true).check(None).await;
        let Found::Down { failure, why, .. } = &heard.found else {
            panic!("{heard:?}");
        };
        assert_eq!(*failure, Failure::CannotRun);
        assert_eq!(why, &words().check.no_account);
        assert_eq!(why, "No account is in use: choose one on the Settings view.");
        assert!(calls(dir.path()).is_empty(), "nothing is run");
        // The network down: no request is made.
        let offline = check(dir.path(), &answering(ANSWERED), false)
            .check(Some("work".to_string()))
            .await;
        assert!(
            matches!(
                &offline.found,
                Found::Down {
                    failure: Failure::Offline,
                    why,
                    ..
                } if why == "The network does not reach the agents' service."
            ),
            "{offline:?}"
        );
        assert!(calls(dir.path()).is_empty());
        // The service overloaded, and the account at its usage limit, as the short request's failure tells them; the
        // limit is a window at its limit, as the service names it.
        let overloaded = check(dir.path(), &answering(OVERLOADED), true)
            .check(Some("work".to_string()))
            .await;
        assert!(
            matches!(
                overloaded.found,
                Found::Down {
                    failure: Failure::Unavailable,
                    ..
                }
            ),
            "{overloaded:?}"
        );
        let limited = check(dir.path(), &answering(LIMITED), true)
            .check(Some("work".to_string()))
            .await;
        assert_eq!(
            limited.found,
            Found::Down {
                failure: Failure::UsageLimit,
                why: "The command line's result is an error: You've hit your weekly limit".to_string(),
                resets: Some(Time(1_793_394_000_000)),
            }
        );
        assert_eq!(
            limited.usage,
            Some(Said::Windows(vec![Window {
                name: "seven_day".to_string(),
                utilization: None,
                resets: Some(Time(1_793_394_000_000)),
                allowed: false,
            }]))
        );
    }

    // A few milliseconds: a shell script run twice. Every caller of the check, the owner's look and the check of a
    // token among them, is given words that hold no token and a limit that names no window; tested once, here.
    #[tokio::test]
    async fn the_check_says_the_program_s_words_without_the_token_and_a_limit_it_named_no_window_of() {
        let dir = tempfile::tempdir().unwrap();
        let said_back = "echo 'Invalid bearer token sk-ant-oat01-secret' >&2; exit 1";
        let heard = check(dir.path(), said_back, true).check(Some("work".to_string())).await;
        let Found::Down { failure, why, .. } = &heard.found else {
            panic!("{heard:?}");
        };
        assert_eq!(*failure, Failure::CannotRun);
        assert!(why.ends_with("Invalid bearer token …"), "{why}");
        assert!(!why.contains("sk-ant"), "{why}");
        // A limit met with no event, the result's words saying its reset: the account is at its limit.
        let words = r#"{"type":"system","subtype":"init","claude_code_version":"2.1.323"}
{"type":"result","subtype":"success","is_error":true,"result":"You've hit your limit · resets 9pm (UTC)","total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0}}"#;
        let dir = tempfile::tempdir().unwrap();
        let heard = check(dir.path(), &answering(words), true)
            .check(Some("work".to_string()))
            .await;
        let Found::Down {
            failure: Failure::UsageLimit,
            resets,
            ..
        } = heard.found
        else {
            panic!("{heard:?}");
        };
        assert!(resets.is_some(), "the reset its words say");
        assert_eq!(heard.usage, Some(Said::Limit { resets }));
    }

    // A token not yet kept is checked under itself, and an account whose token cannot be given is a command line that
    // cannot run, in the words of what keeps the tokens, and nothing is run for it. A few milliseconds: a shell script
    // run once.
    #[tokio::test]
    async fn a_token_is_checked_under_itself_and_an_account_whose_token_is_not_given_cannot_run() {
        let dir = tempfile::tempdir().unwrap();
        let check = check(dir.path(), &answering(ANSWERED), true);
        let fresh = Token::new("sk-ant-oat01-fresh".to_string());
        assert_eq!(check.check_token(fresh).await.found, Found::Well);
        let given = lines(dir.path(), "given");
        assert_eq!(given.len(), 1);
        assert!(given[0].starts_with("sk-ant-oat01-fresh "), "{given:?}");
        let heard = check.check(Some("home".to_string())).await;
        let Found::Down { failure, why, .. } = heard.found else {
            panic!("a token not given is no outside well");
        };
        assert_eq!(failure, Failure::CannotRun);
        assert!(
            why.starts_with("the token of the account home could not be read: "),
            "{why}"
        );
        assert_eq!(calls(dir.path()).len(), 1, "nothing is run for it");
    }

    // The kind of what is down where the short request did not end in time is the way it ended, a timeout, and no
    // word of the words beside the flows says it: with them changed to words of a usage limit, a command line that
    // does not answer is the same. The clock stands still, so that the request's time passes at once; the one call of
    // the script is `sleep`, which is killed with its check.
    #[tokio::test(start_paused = true)]
    async fn a_short_request_that_does_not_end_in_time_is_unavailable_whatever_the_words_beside_the_flows_say() {
        let limit = "You've hit your weekly limit. It resets Fri 5pm.".to_string();
        for changed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut check = check(dir.path(), "exec sleep 1000", true);
            if changed {
                check.said = flow::agents::CheckWords {
                    no_account: limit.clone(),
                    offline: limit.clone(),
                    request: limit.clone(),
                };
                for text in [
                    &mut check.words.exited,
                    &mut check.words.signal,
                    &mut check.words.cut_off,
                    &mut check.words.no_stream_result,
                    &mut check.words.no_return,
                    &mut check.words.error,
                    &mut check.words.no_reason,
                ] {
                    *text = limit.clone();
                }
            }
            let heard = check.check(Some("work".to_string())).await;
            let Found::Down { failure, resets, .. } = &heard.found else {
                panic!("{changed}: {heard:?}");
            };
            assert_eq!(*failure, Failure::Unavailable, "{changed}");
            assert_eq!(*resets, None, "{changed}");
        }
    }

    #[tokio::test]
    async fn a_command_line_that_cannot_be_started_cannot_run() {
        let dir = tempfile::tempdir().unwrap();
        let mut check = check(dir.path(), "", true);
        check.command = dir.path().join("missing").display().to_string();
        let heard = check.check(Some("work".to_string())).await;
        assert!(
            matches!(
                heard.found,
                Found::Down {
                    failure: Failure::CannotRun,
                    ..
                }
            ),
            "{heard:?}"
        );
        // One whose request prints nothing but what it says of itself never began.
        let dir = tempfile::tempdir().unwrap();
        let heard = self::check(dir.path(), "echo 'what?' >&2; exit 1", true)
            .check(Some("work".to_string()))
            .await;
        let Found::Down { failure, why, .. } = &heard.found else {
            panic!("{heard:?}");
        };
        assert_eq!(*failure, Failure::CannotRun);
        assert!(why.ends_with("what?"), "{why}");
        assert_eq!(calls(dir.path()).len(), 1);
    }
}
