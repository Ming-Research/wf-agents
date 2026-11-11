//! The kind of cause of a round of the command line that failed, told apart by what the command line reported, and,
//! at the account's usage limit, when the limit resets. Brought from the old engine's reading of the same command line
//! (`crates/sessions/src/cli.rs` on `main`), whose words were each met in a real report.

use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

use decide::run::Failure;

/// How long a look at the network waits for the service's host to take a connection.
const LOOK_WAIT: Duration = Duration::from_secs(5);

/// The look at the network while the agents' rounds wait for it: whether the agents' service's host, the one
/// `ANTHROPIC_BASE_URL` names in the engine's environment or else Anthropic's API, resolves and takes a connection
/// within a few seconds. Looking costs no round and writes nothing in a session.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look;

impl service::Network for Look {
    fn reached(&self) -> bool {
        let at = host(std::env::var("ANTHROPIC_BASE_URL").ok().as_deref());
        (at.to_socket_addrs().ok().and_then(|mut a| a.next()))
            .is_some_and(|a| TcpStream::connect_timeout(&a, LOOK_WAIT).is_ok())
    }
}

/// The host and port a look at the network connects to, from the service's address where one is given.
fn host(base: Option<&str>) -> String {
    let named = base.and_then(|url| {
        let rest = url.split_once("://").map_or(url, |(_, r)| r);
        rest.split('/').next().filter(|h| !h.is_empty()).map(str::to_string)
    });
    let host = named.unwrap_or_else(|| "api.anthropic.com".to_string());
    if host.contains(':') {
        host
    } else {
        format!("{host}:443")
    }
}

/// The words that tell each kind apart in what the command line reports, in the order they are looked for: the first
/// kind whose words the report holds, in any case, is the kind. A report that holds none of them is a failure in
/// passing: the service's passing errors are many and change their wording, and an unknown one taken so costs at most
/// the few tries before the bound pauses the work.
const KINDS: [(Failure, &[&str]); 6] = [
    // First, since what the command line reports of a return it gave up on may quote the return, which may hold any
    // words.
    (
        Failure::Unread,
        &["max_structured_output_retries", "structured output retries"],
    ),
    // The service's own words when its safety check stops a round or refuses its message.
    (
        Failure::Flagged,
        &["safeguards flagged", "stopped by a safety classifier"],
    ),
    (
        Failure::UsageLimit,
        &[
            "usage limit",
            "hit your limit",
            "limit reached",
            "limit will reset",
            "out of extra usage",
        ],
    ),
    (
        Failure::CannotRun,
        &[
            "not logged in",
            "/login",
            "invalid api key",
            "oauth token",
            "token has expired",
            "authentication_error",
            "authentication failed",
            "no such file or directory",
            "command not found",
        ],
    ),
    (
        Failure::PromptTooLong,
        &["prompt is too long", "prompt too long", "context length"],
    ),
    (
        Failure::Offline,
        &[
            "enotfound",
            "eai_again",
            "enetunreach",
            "econnrefused",
            "getaddrinfo",
            "network is unreachable",
            "connection error",
            "unable to connect",
            "could not resolve host",
        ],
    ),
];

/// The kind of cause of a failed round, by what the command line reported of it and what the round took: none where
/// the program printed no result it could be read by.
pub fn of(report: &str, spent: Option<&work::Spent>) -> Failure {
    let kind = kind(report);
    match spent {
        // A crash with no result says nothing of what it took, and is not tried again unseen by the budget: it
        // pauses its work, but where its words say a cause every work shares, or another its handling needs.
        None if kind == Failure::Passing => Failure::Other,
        // A round that spent tokens reached the service: whatever words it quotes, its command line runs and its
        // network is there.
        Some(spent)
            if spent.input_tokens + spent.cached_tokens + spent.output_tokens > 0
                && matches!(kind, Failure::CannotRun | Failure::Offline) =>
        {
            Failure::Passing
        }
        _ => kind,
    }
}

/// The kind of cause of a failed round, by what the command line reported of it.
pub fn kind(report: &str) -> Failure {
    let text = report.to_lowercase();
    (KINDS.iter())
        .find(|(_, words)| words.iter().any(|w| text.contains(w)))
        .map_or(Failure::Passing, |(kind, _)| *kind)
}

/// When a usage limit the command line reported resets, in milliseconds since the start of 1970: from the time it
/// gives, as seconds since then after a bar (`limit reached|1760000000`), or as a time of day after "resets" (`resets
/// 5pm`, `resets 11:30am (Europe/Paris)`), the next time the clock shows it there. `offset` gives a time zone's
/// distance from UTC in seconds, the local one for none. None where the report gives no time it can read.
pub fn resets_at(report: &str, now: u64, offset: &dyn Fn(Option<&str>) -> Option<i64>) -> Option<u64> {
    if let Some((_, rest)) = report.split_once('|') {
        let digits: String = rest.trim().chars().take_while(char::is_ascii_digit).collect();
        if let Ok(n) = digits.parse::<u64>()
            && digits.len() >= 9
        {
            return Some(if digits.len() >= 13 { n } else { n * 1000 });
        }
    }
    let lower = report.to_lowercase();
    let after = &lower[lower.find("resets")? + "resets".len()..];
    let after = after.trim_start().trim_start_matches("at ").trim_start();
    // The time of day: the first run of digits, with minutes after a colon, and am or pm after it.
    let start = after.find(|c: char| c.is_ascii_digit())?;
    let rest = &after[start..];
    let hour_text: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let mut hour: i64 = hour_text.parse().ok()?;
    let rest = &rest[hour_text.len()..];
    let (minute, rest) = match rest.strip_prefix(':') {
        Some(r) => {
            let m: String = r.chars().take_while(char::is_ascii_digit).collect();
            (m.parse::<i64>().ok()?, &r[m.len()..])
        }
        None => (0, rest),
    };
    let rest = rest.trim_start();
    if rest.starts_with("pm") && hour < 12 {
        hour += 12;
    } else if rest.starts_with("am") && hour == 12 {
        hour = 0;
    }
    if hour > 23 || minute > 59 {
        return None;
    }
    // A time zone named in brackets, as the command line names it, with its own case.
    let zone = (report.rfind('('))
        .and_then(|i| report[i + 1..].split_once(')').map(|(z, _)| z.trim()))
        .filter(|z| z.contains('/'));
    let off = offset(zone)?;
    let local = i64::try_from(now / 1000).ok()? + off;
    let day = local - local.rem_euclid(86_400);
    let mut at = day + hour * 3600 + minute * 60;
    if at <= local {
        at += 86_400;
    }
    u64::try_from((at - off) * 1000).ok()
}

/// A time zone's distance from UTC in seconds, as `date` gives it there; the local one for none.
pub fn zone_offset(zone: Option<&str>) -> Option<i64> {
    let mut c = Command::new("date");
    c.arg("+%z");
    if let Some(z) = zone {
        c.env("TZ", z);
    }
    let out = c.output().ok()?;
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (sign, digits) = match t.split_at_checked(1)? {
        ("-", d) => (-1, d),
        ("+", d) => (1, d),
        _ => return None,
    };
    let h: i64 = digits.get(..2)?.parse().ok()?;
    let m: i64 = digits.get(2..4)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_round_is_told_apart_by_what_the_command_line_reports() {
        // What the command line reports of a return it could not parse may quote any words of the return.
        assert_eq!(
            kind(
                "the command line's result is an error (error_max_structured_output_retries): Failed to provide \
                 valid structured output after 3 attempts: please run /login"
            ),
            Failure::Unread
        );
        assert_eq!(kind("Claude AI usage limit reached|1760000000"), Failure::UsageLimit);
        assert_eq!(kind("You've hit your limit · resets 5pm"), Failure::UsageLimit);
        assert_eq!(kind("Invalid API key · Please run /login"), Failure::CannotRun);
        assert_eq!(kind("Prompt is too long"), Failure::PromptTooLong);
        assert_eq!(
            kind("claude cannot be started: No such file or directory (os error 2)"),
            Failure::CannotRun
        );
        assert_eq!(
            kind("API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)"),
            Failure::Offline
        );
        assert_eq!(kind("API Error: 529 Overloaded"), Failure::Passing);
        assert_eq!(
            kind("API Error: Opus 5.5's safeguards flagged this message (https://www.anthropic.com/legal/aup)."),
            Failure::Flagged
        );
        assert_eq!(kind("claude exited with signal: 9 and no result"), Failure::Passing);
    }

    #[test]
    fn a_round_that_reached_the_service_neither_cannot_run_nor_is_offline() {
        let spent = work::Spent {
            usd_micros: 1000,
            input_tokens: 10,
            cached_tokens: 0,
            output_tokens: 5,
        };
        let report = "the command line's result is an error: grep: notes.md: No such file or directory";
        assert_eq!(of(report, Some(&spent)), Failure::Passing);
        assert_eq!(of("API Error: Connection error.", Some(&spent)), Failure::Passing);
        // A result that spent nothing may say either.
        assert_eq!(of(report, Some(&work::Spent::default())), Failure::CannotRun);
        assert_eq!(of("You've hit your limit", Some(&spent)), Failure::UsageLimit);
    }

    #[test]
    fn a_round_that_printed_no_result_pauses_its_work_unless_its_words_say_a_cause_every_work_shares() {
        assert_eq!(of("claude exited with signal: 9", None), Failure::Other);
        assert_eq!(of("API Error: 529 Overloaded", None), Failure::Other);
        assert_eq!(of("claude: command not found", None), Failure::CannotRun);
        assert_eq!(of("getaddrinfo ENOTFOUND api.anthropic.com", None), Failure::Offline);
        assert_eq!(of("Prompt is too long", None), Failure::PromptTooLong);
    }

    #[test]
    fn the_network_is_looked_at_on_the_services_host() {
        assert_eq!(host(None), "api.anthropic.com:443");
        assert_eq!(host(Some("https://proxy.example:8443/v1")), "proxy.example:8443");
        assert_eq!(host(Some("http://gateway.example")), "gateway.example:443");
    }

    #[test]
    fn a_usage_limit_resets_when_the_command_line_says() {
        let utc = |_: Option<&str>| Some(0);
        // 2026-10-25 20:00 UTC.
        let now = 1_792_958_400_000;
        assert_eq!(
            resets_at("Claude AI usage limit reached|1792962000", now, &utc),
            Some(1_792_962_000_000)
        );
        assert_eq!(
            resets_at("You've hit your limit · resets 9pm", now, &utc),
            Some(now + 3_600_000)
        );
        // A time already past today is tomorrow's.
        assert_eq!(
            resets_at("5-hour limit reached ∙ resets 7:30pm", now, &utc),
            Some(now + 23 * 3_600_000 + 1_800_000)
        );
        let paris = |z: Option<&str>| (z == Some("Europe/Paris")).then_some(3600);
        assert_eq!(
            resets_at("You've hit your limit · resets 11pm (Europe/Paris)", now, &paris),
            Some(now + 2 * 3_600_000)
        );
        assert_eq!(resets_at("usage limit reached", now, &utc), None);
    }
}
