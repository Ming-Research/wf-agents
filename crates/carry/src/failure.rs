//! The kind of cause of a round of the command line that failed, and, at the account's usage limit, when the limit
//! resets. The command line names the service's errors in fields of its stream ([`Said`]), which are read first: its
//! words change between its versions, and a weekly limit was once taken for a failure in passing because its words
//! were new. Where it names none, as when it crashed or is older, the words of what it reported tell the kind, as the
//! old engine told them (`crates/sessions/src/cli.rs` on `old-main`), each met in a real report.

use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

use decide::run::Failure;
use serde_json::Value;

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

/// What the command line's stream says of the service's errors in the round, in fields of its own rather than words:
/// the error of the main loop's latest message, where that message is one, with its kind (`error`) and, where the
/// command line gives one, the service's typed kind (`api_error`), which the result repeats; and whether the latest
/// rate limit event refused the account's requests (`rate_limit_event`), with when the limit resets, as that event
/// or the refusal itself says (`rate_limit_info`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Said {
    /// The kind the command line's own kind of the error tells (`rate_limit`, `overloaded`, `authentication_failed`),
    /// where it tells one ([`ERRORS`]).
    error: Option<Failure>,
    /// The kind the service's typed kind of the error tells (`usage_limit_reached`, `connection_failed`), where it
    /// tells one ([`API_ERRORS`]).
    api_error: Option<Failure>,
    /// Whether the latest rate limit event refused the account's requests: its usage limit is reached.
    pub refused: bool,
    /// When the usage limit resets, in milliseconds since the start of 1970, where the refusal said.
    pub resets: Option<u64>,
}

/// The service's typed kinds of error (`api_error`) that tell a kind, each with it. The others say what running again
/// cannot mend and no kind says so; they are told by the words, as a failure in passing whose tries are bounded.
const API_ERRORS: &[(&str, Failure)] = &[
    // The account's own limit or a spend cap: the refusal says which, and when it resets.
    ("usage_limit_reached", Failure::UsageLimit),
    ("model_requires_usage_credits", Failure::UsageLimit),
    ("long_context_credits_required", Failure::UsageLimit),
    ("safety_monitor_blocked", Failure::Flagged),
    ("connection_failed", Failure::Offline),
    // What only the owner mends: the sign-in, the credentials, the network's own refusal, the command line's version.
    ("stored_login_refused", Failure::CannotRun),
    ("provider_credentials", Failure::CannotRun),
    ("gateway_signin_required", Failure::CannotRun),
    ("gateway_session_expired", Failure::CannotRun),
    ("api_key_auth_disabled", Failure::CannotRun),
    ("org_disabled_credential", Failure::CannotRun),
    ("invalid_credential_header", Failure::CannotRun),
    ("tls_untrusted_ca", Failure::CannotRun),
    ("network_edge_blocked", Failure::CannotRun),
    ("claude_code_version_too_old", Failure::CannotRun),
    // The service did not answer, or answered with nothing to take: tried again.
    ("no_response", Failure::Passing),
    ("stream_idle_timeout", Failure::Passing),
    ("computer_slept", Failure::Passing),
    ("empty_completion", Failure::Passing),
    ("malformed_tool_use_exhausted", Failure::Passing),
    ("max_output_tokens", Failure::Passing),
];

/// The command line's kinds of error (`error`) that tell a kind, each with it. Its `rate_limit` is shared by the
/// account's usage limit and the service's passing throttles, which the typed kind or the rate limit event tells
/// apart, and its `invalid_request` and `unknown` say nothing: the words tell those.
const ERRORS: &[(&str, Failure)] = &[
    ("authentication_failed", Failure::CannotRun),
    ("oauth_org_not_allowed", Failure::CannotRun),
    ("account_on_hold", Failure::CannotRun),
    ("verification_required", Failure::CannotRun),
    ("billing_error", Failure::CannotRun),
    ("cloud_credential_error", Failure::CannotRun),
    ("model_not_found", Failure::CannotRun),
    ("overloaded", Failure::Passing),
    ("server_error", Failure::Passing),
    ("max_output_tokens", Failure::Passing),
];

impl Said {
    /// Takes one line of the stream, read as JSON.
    pub fn line(&mut self, line: &Value) {
        let of = |table: &[(&str, Failure)], name: &Value| {
            let name = name.as_str()?;
            table.iter().find(|(n, _)| *n == name).map(|(_, kind)| *kind)
        };
        match line["type"].as_str() {
            // Each message of the main loop says whether it is an error: one that is not ends what an earlier one said,
            // the command line having gone on past it.
            Some("assistant") if line["parent_tool_use_id"].is_null() => {
                self.error = of(ERRORS, &line["error"]);
                self.api_error = of(API_ERRORS, &line["api_error"]);
                if let Some(at) = reset(&line["api_error_params"]["rate_limit_info"]) {
                    self.resets = Some(at);
                }
            }
            Some("rate_limit_event") => {
                let info = &line["rate_limit_info"];
                self.refused = info["status"] == "rejected";
                if self.refused
                    && let Some(at) = reset(info)
                {
                    self.resets = Some(at);
                }
            }
            Some("result") => {
                if let Some(kind) = of(API_ERRORS, &line["api_error"]) {
                    self.api_error = Some(kind);
                }
            }
            _ => {}
        }
    }

    /// The kind of cause the fields tell; none where they tell none, and the words decide.
    pub fn kind(&self) -> Option<Failure> {
        (self.api_error)
            .or_else(|| self.refused.then_some(Failure::UsageLimit))
            .or(self.error)
    }
}

/// When a limit resets, in milliseconds since the start of 1970, from what the command line says of it
/// (`rate_limit_info`), which gives it in seconds.
fn reset(info: &Value) -> Option<u64> {
    let at = info["resetsAt"].as_u64()?;
    // A time in seconds has at most eleven digits until the year 5138.
    Some(if at < 100_000_000_000 { at * 1000 } else { at })
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

/// The kind of cause of a failed round: by what the command line gave up on, then by the fields its stream named the
/// service's error in, then by the words of what it reported and what the round took. A round that printed no result,
/// as when the command line crashed, fails in passing unless its words say otherwise, as in the old engine: it is
/// tried again, and the bound on tries in a row pauses its work.
pub fn of(report: &str, said: &Said, spent: Option<&work::Spent>) -> Failure {
    let words = kind(report);
    // A return the command line gave up on is sent back, whatever the service said of the calls before it.
    if words == Failure::Unread {
        return words;
    }
    if let Some(kind) = said.kind() {
        return kind;
    }
    match spent {
        // A round that spent tokens reached the service: whatever words it quotes, its command line runs and its
        // network is there.
        Some(spent)
            if spent.input_tokens + spent.cached_tokens + spent.output_tokens > 0
                && matches!(words, Failure::CannotRun | Failure::Offline) =>
        {
            Failure::Passing
        }
        _ => words,
    }
}

/// The kind of cause of a failed round, by the words of what the command line reported.
pub fn kind(report: &str) -> Failure {
    let text = report.to_lowercase();
    (KINDS.iter())
        .find(|(kind, words)| {
            words.iter().any(|w| text.contains(w)) || (*kind == Failure::UsageLimit && hit_a_limit(&text))
        })
        .map_or(Failure::Passing, |(kind, _)| *kind)
}

/// Whether a report (in lower case) says the account hit a limit: "hit your", then within a few words a word that
/// begins "limit", as in "hit your weekly limit" or "hit your Opus 5-hour limit". The command line puts the limit's
/// kind or a model's name between them, and which words it puts there changes.
fn hit_a_limit(text: &str) -> bool {
    text.match_indices("hit your ").any(|(at, said)| {
        let after = &text[at + said.len()..];
        after.split_whitespace().take(4).any(|w| w.starts_with("limit"))
    })
}

/// When a usage limit resets, in milliseconds since the start of 1970: as the command line's stream said it
/// ([`Said`]), and where it did not, as the words of its report say ([`resets_at`]).
pub fn resets(report: &str, said: &Said, now: u64, offset: &dyn Fn(Option<&str>) -> Option<i64>) -> Option<u64> {
    said.resets.or_else(|| resets_at(report, now, offset))
}

/// When a usage limit the command line reported resets, in milliseconds since the start of 1970: from the time it
/// gives, as seconds since then after a bar (`limit reached|1760000000`), or as a time after "resets", with or without
/// a day before it: `resets 5pm`, `resets 11:30am (Europe/Paris)`, `resets Fri 5pm`, `resets Nov 4 at 11pm`; the next
/// time after `now` the clock shows it there. `offset` gives a time zone's distance from UTC in seconds, the local one
/// for none, as it is now: a reset beyond a change of the clocks there is read an hour off. None where the report
/// gives no time it can read.
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
    let mut words = (after.split(|c: char| c.is_whitespace() || c == ','))
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':'))
        .filter(|w| !w.is_empty());
    let mut word = words.next()?;
    if word == "on" {
        word = words.next()?;
    }
    let weekday = weekday_of(word);
    if weekday.is_some() {
        word = words.next()?;
    }
    let mut date = None;
    if let Some(month) = month_of(word) {
        let day: i64 = words.next()?.parse().ok()?;
        date = Some((month, day));
        word = words.next()?;
    }
    if word == "at" {
        word = words.next()?;
    }
    let (hour, minute) = clock_of(word, words.next())?;
    // A time zone named in brackets, as the command line names it, with its own case.
    let zone = (report.rfind('('))
        .and_then(|i| report[i + 1..].split_once(')').map(|(z, _)| z.trim()))
        .filter(|z| z.contains('/'));
    let off = offset(zone)?;
    let local = i64::try_from(now / 1000).ok()? + off;
    let today = local.div_euclid(86_400);
    let time = hour * 3600 + minute * 60;
    let at = match (date, weekday) {
        // A date: in the year that makes it the next time after now.
        (Some((month, day)), _) => {
            let (year, ..) = civil(today);
            let this = days(year, month, day)? * 86_400 + time;
            if this > local {
                this
            } else {
                days(year + 1, month, day)? * 86_400 + time
            }
        }
        // A day of the week: the next one at the time, today's where it is still to come.
        (None, Some(weekday)) => {
            let ahead = (weekday - weekday_of_day(today)).rem_euclid(7);
            let at = (today + ahead) * 86_400 + time;
            if at > local { at } else { at + 7 * 86_400 }
        }
        // A time of day alone: today's, or tomorrow's where it has passed.
        (None, None) => {
            let at = today * 86_400 + time;
            if at > local { at } else { at + 86_400 }
        }
    };
    u64::try_from((at - off) * 1000).ok()
}

/// The hour and minute a word gives as a time of day, with am or pm at its end or as the word after it: `5pm`,
/// `11:30am`, `5 pm`, `17:00`.
fn clock_of(word: &str, next: Option<&str>) -> Option<(i64, i64)> {
    let hour_text: String = word.chars().take_while(char::is_ascii_digit).collect();
    let mut hour: i64 = hour_text.parse().ok()?;
    let rest = &word[hour_text.len()..];
    let (minute, rest) = match rest.strip_prefix(':') {
        Some(r) => {
            let m: String = r.chars().take_while(char::is_ascii_digit).collect();
            (m.parse::<i64>().ok()?, &r[m.len()..])
        }
        None => (0, rest),
    };
    let meridiem = if rest.is_empty() {
        next.unwrap_or_default()
    } else {
        rest
    };
    if meridiem.starts_with("pm") && hour < 12 {
        hour += 12;
    } else if meridiem.starts_with("am") && hour == 12 {
        hour = 0;
    }
    (hour <= 23 && minute <= 59).then_some((hour, minute))
}

/// The day of the week a word names, as English names it in full or by its first three letters, counted from Monday
/// as 0.
fn weekday_of(word: &str) -> Option<i64> {
    const DAYS: [&str; 7] = [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ];
    (word.len() >= 3)
        .then(|| DAYS.iter().position(|day| day.starts_with(word)))
        .flatten()
        .map(|i| i as i64)
}

/// The month a word names, as English names it in full or by its first three letters (and `sept`), from 1.
fn month_of(word: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    (word.len() >= 3)
        .then(|| MONTHS.iter().position(|month| month.starts_with(word)))
        .flatten()
        .map(|i| i as i64 + 1)
}

/// The day of the week of a day counted from the start of 1970, from Monday as 0: that day was a Thursday.
fn weekday_of_day(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

/// The days from the start of 1970 to a date, where the date is one (Howard Hinnant's `days_from_civil`).
fn days(year: i64, month: i64, day: i64) -> Option<i64> {
    let lengths = [31, 28 + i64::from(leap(year)), 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let length = *lengths.get(usize::try_from(month - 1).ok()?)?;
    if !(1..=length).contains(&day) {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// The date of a day counted from the start of 1970: its year, month and day (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// Whether a year has a 29th of February.
fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
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
        assert_eq!(
            kind("You've hit your weekly limit · resets Fri 5pm"),
            Failure::UsageLimit
        );
        assert_eq!(kind("You've hit your Opus 5-hour limit"), Failure::UsageLimit);
        // "Hit your" alone, or with a limit far off, is no usage limit.
        assert_eq!(
            kind("API Error: 429 You hit your rate of requests, retry in a moment; the limit is per minute"),
            Failure::Passing
        );
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
            ..work::Spent::default()
        };
        let none = Said::default();
        let report = "the command line's result is an error: grep: notes.md: No such file or directory";
        assert_eq!(of(report, &none, Some(&spent)), Failure::Passing);
        assert_eq!(
            of("API Error: Connection error.", &none, Some(&spent)),
            Failure::Passing
        );
        // A result that spent nothing may say either.
        assert_eq!(of(report, &none, Some(&work::Spent::default())), Failure::CannotRun);
        assert_eq!(of("You've hit your limit", &none, Some(&spent)), Failure::UsageLimit);
    }

    // As in the old engine, a command line that crashed is tried again: the bound on failures in passing in a row
    // pauses its work where it goes on crashing.
    #[test]
    fn a_round_that_printed_no_result_fails_in_passing_unless_its_words_say_otherwise() {
        let none = Said::default();
        assert_eq!(of("claude exited with signal: 9", &none, None), Failure::Passing);
        assert_eq!(of("API Error: 529 Overloaded", &none, None), Failure::Passing);
        assert_eq!(of("claude: command not found", &none, None), Failure::CannotRun);
        assert_eq!(
            of("getaddrinfo ENOTFOUND api.anthropic.com", &none, None),
            Failure::Offline
        );
        assert_eq!(of("Prompt is too long", &none, None), Failure::PromptTooLong);
    }

    /// What the stream's lines say, read one after the other.
    fn said(lines: &[Value]) -> Said {
        let mut said = Said::default();
        for line in lines {
            said.line(line);
        }
        said
    }

    /// The message the command line gives a refusal of the service in, as version 2.1.323 writes it.
    fn refusal(error: &str, api_error: Option<&str>, info: Value) -> Value {
        let mut line = serde_json::json!({ "type": "assistant", "parent_tool_use_id": null, "error": error,
            "message": { "content": [{ "type": "text", "text": "API Error" }] } });
        if let Some(api) = api_error {
            line["api_error"] = api.into();
            line["api_error_params"] = serde_json::json!({ "rate_limit_info": info });
        }
        line
    }

    // The fields of the stream tell the kind whatever the words say: a weekly limit in words never met before is the
    // usage limit, a throttle that shares its error is in passing, and an expired sign-in cannot run.
    #[test]
    fn the_fields_the_command_line_names_an_error_in_tell_its_kind_before_its_words() {
        let none = Some(&work::Spent::default());
        let info =
            serde_json::json!({ "status": "rejected", "resetsAt": 1_793_394_000_u64, "rateLimitType": "seven_day" });
        let limit = said(&[refusal("rate_limit", Some("usage_limit_reached"), info)]);
        assert_eq!(
            of("Your quota of this week is used up", &limit, none),
            Failure::UsageLimit
        );
        assert_eq!(limit.resets, Some(1_793_394_000_000));
        let throttled = said(&[refusal("rate_limit", None, Value::Null)]);
        assert_eq!(of("API Error: 429 slow down", &throttled, none), Failure::Passing);
        // The words still tell a limit the fields leave open.
        assert_eq!(
            of("You've hit your limit · resets 5pm", &throttled, none),
            Failure::UsageLimit
        );
        let overloaded = said(&[refusal("overloaded", None, Value::Null)]);
        assert_eq!(of("Invalid API key", &overloaded, none), Failure::Passing);
        let signed_out = said(&[refusal("authentication_failed", None, Value::Null)]);
        assert_eq!(of("API Error: 401", &signed_out, none), Failure::CannotRun);
        let offline = serde_json::json!({ "type": "result", "is_error": true, "api_error": "connection_failed" });
        assert_eq!(of("API Error", &said(&[offline]), None), Failure::Offline);
        // A rate limit event that refused the account's requests says the limit and its reset.
        let event = serde_json::json!({ "type": "rate_limit_event",
            "rate_limit_info": { "status": "rejected", "resetsAt": 1_792_976_400_u64, "rateLimitType": "five_hour" } });
        let refused = said(&[event]);
        assert_eq!(of("API Error", &refused, none), Failure::UsageLimit);
        assert_eq!(refused.resets, Some(1_792_976_400_000));
        // What the command line gave up on is sent back, whatever the service said before it.
        let gave_up = "the command line's result is an error (error_max_structured_output_retries): no return";
        assert_eq!(of(gave_up, &limit, none), Failure::Unread);
    }

    // A message of the main loop that is no error ends what an earlier one said, and a subagent's error is not the
    // round's.
    #[test]
    fn an_error_the_command_line_went_on_past_tells_nothing() {
        let error = refusal("authentication_failed", None, Value::Null);
        let went_on = serde_json::json!({ "type": "assistant", "parent_tool_use_id": null, "message": {} });
        assert_eq!(said(&[error.clone(), went_on]).kind(), None);
        let mut subagents = error;
        subagents["parent_tool_use_id"] = "toolu_1".into();
        assert_eq!(said(&[subagents]).kind(), None);
        let allowed = serde_json::json!({ "type": "rate_limit_event", "rate_limit_info": { "status": "allowed" } });
        assert_eq!(said(&[allowed]).kind(), None);
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

    // A weekly limit names its day: by the day of the week, the next one at the time, or by its date. 2026-10-25 is a
    // Sunday.
    #[test]
    fn a_usage_limit_that_names_its_day_resets_on_that_day() {
        let utc = |_: Option<&str>| Some(0);
        // 2026-10-25 20:00 UTC.
        let now = 1_792_958_400_000;
        let hours = |h: u64| now + h * 3_600_000;
        assert_eq!(
            resets_at("You've hit your weekly limit · resets Fri 5pm", now, &utc),
            Some(hours(4 * 24 + 21))
        );
        // Today's day at a time still to come is today; at one passed, a week on.
        assert_eq!(resets_at("resets Sun 11pm", now, &utc), Some(hours(3)));
        assert_eq!(resets_at("resets Sunday 5 pm", now, &utc), Some(hours(7 * 24 - 3)));
        let la = |z: Option<&str>| (z == Some("America/Los_Angeles")).then_some(-7 * 3600);
        assert_eq!(
            resets_at(
                "You've hit your weekly limit · resets Nov 4 at 11pm (America/Los_Angeles)",
                now,
                &la
            ),
            Some(hours(10 * 24 + 10))
        );
        assert_eq!(
            resets_at("resets Oct 26, 7:30am", now, &utc),
            Some(hours(11) + 1_800_000)
        );
        // A date passed this year is next year's.
        assert_eq!(
            resets_at("resets Oct 25 at 12am", now, &utc),
            Some(hours(365 * 24 - 20))
        );
        assert_eq!(resets_at("resets Feb 30 at 1am", now, &utc), None);
        // When the stream says, that is when.
        let said = Said {
            resets: Some(hours(2)),
            ..Said::default()
        };
        assert_eq!(resets("resets Fri 5pm", &said, now, &utc), Some(hours(2)));
        assert_eq!(
            resets("resets Fri 5pm", &Said::default(), now, &utc),
            Some(hours(4 * 24 + 21))
        );
    }

    #[test]
    fn a_date_is_counted_in_days_from_the_start_of_1970_and_back() {
        assert_eq!(days(1970, 1, 1), Some(0));
        assert_eq!(days(2000, 2, 29), Some(11_016));
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(days(2026, 10, 25), Some(20_751));
        assert_eq!(civil(20_751), (2026, 10, 25));
        assert_eq!(weekday_of_day(20_751), 6);
        assert_eq!(days(2025, 2, 29), None);
    }
}
