//! The kind of cause of a round of the command line that failed, and, at the account's usage limit, when the limit
//! resets. The command line names the round's failure in fields of its stream ([`Said`]), which are read first: the
//! service's typed kind of the error, its own kind of it, a rate limit event that refused, why the round ended, and the
//! status the service answered with. Its words change between its versions, and what is told by them misses each new
//! wording: the words of what it reported, each met in a real report, tell the kind only where the fields say nothing,
//! as when it crashed or is of a version that has none, and within the class a status tells. An error nothing tells
//! the kind of is not taken as passing.
//!
//! A cause outside every work and the same for all of them (the sign-in, the account's usage limit, the network, the
//! service down or overloaded, the command line itself failing to run) holds every work's rounds, and the engine then
//! checks the outside by itself ([`crate::outside`]); what is one round's own, as a crash after it reached the service
//! or a cut answer, is tried again in passing.

use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

use decide::run::Failure;
use serde_json::Value;

/// How long a look at the network waits for the service's host to take a connection.
const LOOK_WAIT: Duration = Duration::from_secs(5);

/// The look at the network to the agents' service, which the check of the outside takes while the agents' rounds wait
/// for it ([`crate::outside`]): whether the service's host, the one `ANTHROPIC_BASE_URL` names in the engine's
/// environment or else Anthropic's API, resolves and takes a connection within a few seconds. Looking costs no round
/// and writes nothing in a session.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look;

impl service::Network for Look {
    fn reached(&self) -> bool {
        reaches(&host(std::env::var("ANTHROPIC_BASE_URL").ok().as_deref()))
    }
}

/// The look at the network to a host of the engine's acts, by its `host:port`: whether it resolves and takes a
/// connection within a few seconds.
#[derive(Clone, Debug)]
pub struct LookAt(pub String);

impl service::Network for LookAt {
    fn reached(&self) -> bool {
        reaches(&self.0)
    }
}

/// Whether the `host:port` resolves and takes a connection within a few seconds.
fn reaches(at: &str) -> bool {
    (at.to_socket_addrs().ok().and_then(|mut a| a.next()))
        .is_some_and(|a| TcpStream::connect_timeout(&a, LOOK_WAIT).is_ok())
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

/// What the command line's stream says of the round's failure in fields of its own rather than words: the error of the
/// main loop's latest message, where that message is one, with its kind (`error`), the service's typed kind
/// (`api_error`), which the result repeats, and the status the service answered with (`api_error_status`); whether the
/// latest rate limit event refused the account's requests (`rate_limit_event`), with when the limit resets, as that
/// event or the refusal itself says (`rate_limit_info`); whether it said it began, as its first line does once it has
/// read its input (`system`, `init`); and of the result, whether it was printed at all, and why the round ended
/// (`terminal_reason`, and the result's `subtype`). An event that refuses while the command line carries on, on its
/// overage or in a grace, refuses nothing, as the command line itself reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Said {
    /// The kind the command line's own kind of the error tells (`overloaded`, `authentication_failed`), where it tells
    /// one ([`ERRORS`]).
    error: Option<Failure>,
    /// Whether the command line's own kind of the error is `rate_limit`, which a rate limit (429) is of: the account's
    /// usage limit or a throttle, as a rate limit's status is.
    limited: bool,
    /// The kind the service's typed kind of the error tells (`usage_limit_reached`, `connection_failed`), where it
    /// tells one ([`API_ERRORS`]).
    api_error: Option<Failure>,
    /// Whether the latest rate limit event refused the account's requests: its usage limit is reached.
    pub refused: bool,
    /// When the usage limit resets, in milliseconds since the start of 1970, where the refusal said.
    pub resets: Option<u64>,
    /// The kind why the round ended tells, where it tells one ([`ENDINGS`]).
    ending: Option<Failure>,
    /// The status the service answered the call that failed with, where it answered: the main loop's latest message's
    /// while that message is an error, and the result's.
    status: Option<u16>,
    /// Whether the command line printed its result. One that printed none crashed, or was stopped.
    pub printed: bool,
    /// Whether the command line said it began: as its first line does once it has read its input, and as any message of
    /// its main loop does.
    pub began: bool,
}

/// The service's typed kinds of error (`api_error`) that tell a kind, each with it. The others say what running again
/// cannot mend and no kind says so; they are told by the status and the words. A cause that is one session's or one
/// model's is never one every work shares.
const API_ERRORS: &[(&str, Failure)] = &[
    // The account's own limit or a spend cap: the refusal says which, and when it resets.
    ("usage_limit_reached", Failure::UsageLimit),
    // The model this level runs on needs usage credits the account has not: its works pause, with why.
    ("model_requires_usage_credits", Failure::Other),
    // This session's context is past what the account may send without credits: its work pauses, with why.
    ("long_context_credits_required", Failure::PromptTooLong),
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
    // The service did not answer: it holds every work's rounds until it serves again.
    ("no_response", Failure::Unavailable),
    ("stream_idle_timeout", Failure::Unavailable),
    // The round's own, or an answer with nothing to take: tried again.
    ("computer_slept", Failure::Passing),
    ("empty_completion", Failure::Passing),
    ("malformed_tool_use_exhausted", Failure::Passing),
    ("max_output_tokens", Failure::Passing),
];

/// The command line's kinds of error (`error`) that tell a kind, each with it. Its `rate_limit` is shared by the
/// account's usage limit and the service's throttles, which the typed kind, the rate limit event or else the words tell
/// apart, as within a rate limit's status ([`Class::Limit`]), and its `invalid_request` and `unknown` say nothing: the
/// status and the words tell those.
const ERRORS: &[(&str, Failure)] = &[
    ("authentication_failed", Failure::CannotRun),
    ("oauth_org_not_allowed", Failure::CannotRun),
    ("account_on_hold", Failure::CannotRun),
    ("verification_required", Failure::CannotRun),
    ("billing_error", Failure::CannotRun),
    ("cloud_credential_error", Failure::CannotRun),
    ("model_not_found", Failure::NoModel),
    ("overloaded", Failure::Unavailable),
    ("server_error", Failure::Unavailable),
    ("max_output_tokens", Failure::Passing),
];

/// The reasons a round ended in error (the result's `terminal_reason`, or its `subtype`) that tell a kind, each with
/// it. The others (`api_error`, `model_error`, `turn_setup_failed`) say only that it failed: the status and the words
/// tell those.
const ENDINGS: &[(&str, Failure)] = &[
    // The command line gave up on the return, having refused every attempt at it: what it last attempted is sent back.
    ("structured_output_retry_exhausted", Failure::Unread),
    ("error_max_structured_output_retries", Failure::Unread),
    // The session's context is past what the model takes, or fills again as fast as it is compacted: run again in the
    // same session, the round meets it again, and its work pauses, with why.
    ("prompt_too_long", Failure::PromptTooLong),
    ("blocking_limit", Failure::PromptTooLong),
    ("rapid_refill_breaker", Failure::PromptTooLong),
    // The model wrote a use of a tool that could not be read as many times as the command line tries: tried again.
    ("malformed_tool_use_exhausted", Failure::Passing),
    // The command line failed as it carried the round out, as a crash does: tried again.
    ("error_during_execution", Failure::Passing),
    // A bound the round was run with was reached: run again, it reaches it again.
    ("error_max_turns", Failure::Other),
    ("error_max_budget_usd", Failure::Other),
    ("budget_exhausted", Failure::Other),
];

/// What a status the service answered with says of the failure: the class of cause it is of, which the words of the
/// report tell apart within it where it holds more than one kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// The service failed, was overloaded or took too long (408, 500 to 599, 529): it is unavailable, whatever the
    /// words.
    Unavailable,
    /// The service refused the sign-in (401, 403), as the command line itself reads it: the command line cannot run.
    CannotRun,
    /// A rate limit (429): the account's usage limit where the words say so, else a throttle of the account's
    /// requests, which makes the service unavailable to every work alike.
    Limit,
    /// The service refused the request (any other 4xx): run again, it is refused again. The words may tell the kind of
    /// the refusal, as a context too long, a flagged message or the usage limit; else its work pauses, with why.
    Refused,
}

impl Class {
    /// The class of the status, where it has one.
    fn of(status: u16) -> Option<Class> {
        match status {
            408 | 500..=599 => Some(Class::Unavailable),
            401 | 403 => Some(Class::CannotRun),
            429 => Some(Class::Limit),
            400..=499 => Some(Class::Refused),
            _ => None,
        }
    }

    /// The kind of a failure of this class, the words of its report telling this kind where they tell one.
    fn kind(self, words: Option<Failure>) -> Failure {
        match self {
            Class::Unavailable => Failure::Unavailable,
            Class::CannotRun => Failure::CannotRun,
            Class::Limit if words == Some(Failure::UsageLimit) => Failure::UsageLimit,
            Class::Limit => Failure::Unavailable,
            Class::Refused => (words)
                .filter(|kind| !matches!(kind, Failure::Passing | Failure::Offline))
                .unwrap_or(Failure::Other),
        }
    }
}

impl Said {
    /// Takes one line of the stream, read as JSON.
    pub fn line(&mut self, line: &Value) {
        let of = |table: &[(&str, Failure)], name: &Value| {
            let name = name.as_str()?;
            table.iter().find(|(n, _)| *n == name).map(|(_, kind)| *kind)
        };
        let status = |line: &Value| (line["api_error_status"].as_u64()).and_then(|s| u16::try_from(s).ok());
        match line["type"].as_str() {
            // Each message of the main loop says whether it is an error: one that is not ends what an earlier one said,
            // the command line having gone on past it.
            Some("assistant") if line["parent_tool_use_id"].is_null() => {
                self.began = true;
                self.error = of(ERRORS, &line["error"]);
                self.limited = line["error"] == "rate_limit";
                self.api_error = of(API_ERRORS, &line["api_error"]);
                self.status = status(line);
                if let Some(at) = reset(&line["api_error_params"]["rate_limit_info"]) {
                    self.resets = Some(at);
                }
            }
            Some("rate_limit_event") => {
                let info = &line["rate_limit_info"];
                self.refused = refuses(info);
                if self.refused
                    && let Some(at) = reset(info)
                {
                    self.resets = Some(at);
                }
            }
            Some("system") if line["subtype"] == "init" => self.began = true,
            Some("result") => {
                self.printed = true;
                if let Some(kind) = of(API_ERRORS, &line["api_error"]) {
                    self.api_error = Some(kind);
                }
                if let Some(status) = status(line) {
                    self.status = Some(status);
                }
                if line["is_error"] == true {
                    self.ending = of(ENDINGS, &line["terminal_reason"]).or_else(|| of(ENDINGS, &line["subtype"]));
                }
            }
            _ => {}
        }
    }

    /// The kind of cause the fields tell; none where they tell none, and the status and the words decide. The round's
    /// own error comes before a rate limit event that refused, and that before why the round ended.
    pub fn kind(&self) -> Option<Failure> {
        (self.api_error)
            .or(self.error)
            .or_else(|| self.refused.then_some(Failure::UsageLimit))
            .or(self.ending)
    }

    /// Whether the command line gave up on the round's return, as why the round ended says.
    pub fn gave_up(&self) -> bool {
        self.ending == Some(Failure::Unread)
    }
}

/// Whether a rate limit event refuses the account's requests: it says so, and the command line does not carry on past
/// it, on the account's overage or in a grace.
fn refuses(info: &Value) -> bool {
    let set = |key: &str| info[key] == true;
    let overage = matches!(info["overageStatus"].as_str(), Some("allowed" | "allowed_warning"));
    info["status"] == "rejected"
        && !overage
        && !set("isUsingOverage")
        && !set("overageInUse")
        && !set("rateLimitGraceActive")
}

/// When a limit resets, in milliseconds since the start of 1970, from what the command line says of it
/// (`rate_limit_info`), which gives it in seconds.
fn reset(info: &Value) -> Option<u64> {
    let at = info["resetsAt"].as_u64()?;
    // A time in seconds has at most eleven digits until the year 5138.
    Some(if at < 100_000_000_000 { at * 1000 } else { at })
}

/// The words that tell each kind apart in what the command line reports, in the order they are looked for: the first
/// kind whose words the report holds, in any case, is the kind. They decide only what the fields leave open, as for a
/// version of the command line that has none of them, and within the class a status tells.
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

/// The kind of cause of a failed round. A return the command line gave up on is sent back. Then the fields its stream
/// named the failure in tell the kind ([`Said::kind`]); then the status the service answered with tells its class, the
/// words telling the kind within it ([`Class`]); then, where neither says anything, the words of what it reported and
/// what the round took. Where nothing tells the kind, a round whose command line printed an error is no failure in
/// passing: run again, it would meet again what nobody has told apart, unseen, and its work is paused, with what it
/// said. A round that printed no result, as when the command line crashed or was killed, fails in passing unless its
/// words say otherwise, where it reached the service or said it began: it is tried again, what it spent counted, and
/// the bound on tries in a row pauses its work. One that did neither is the command line itself failing to run, which
/// no work's round runs past.
pub fn of(report: &str, said: &Said, spent: Option<&work::Spent>) -> Failure {
    let words = words(report);
    // A return the command line gave up on is sent back, whatever the service said of the calls before it.
    if gave_up(report, said) {
        return Failure::Unread;
    }
    let reached = spent.is_some_and(|s| s.uncached_tokens + s.cached_tokens + s.output_tokens > 0);
    let class = (said.status.and_then(Class::of)).or_else(|| said.limited.then_some(Class::Limit));
    let told = (said.kind()).or_else(|| class.map(|class| class.kind(words)));
    match (told, words) {
        // A connection that failed after the round had spent tokens is that round's own, cut or ended early, and no
        // sign the network has gone.
        (Some(Failure::Offline), _) if reached => Failure::Passing,
        (Some(kind), _) => kind,
        // A round that spent tokens reached the service: whatever words it quotes, its command line runs and its
        // network is there.
        (None, Some(Failure::CannotRun | Failure::Offline)) if reached => Failure::Passing,
        (None, Some(kind)) => kind,
        (None, None) if said.printed => Failure::Other,
        // It ended before it began and reached nothing: the command line itself fails to run.
        (None, None) if !reached && !said.began => Failure::CannotRun,
        (None, None) => Failure::Passing,
    }
}

/// Whether the command line gave up on the round's return, having refused every attempt at it: as why the round ended
/// says, or, for a version of the command line that does not say it, as the words of what it reported do.
pub fn gave_up(report: &str, said: &Said) -> bool {
    said.gave_up() || (said.ending.is_none() && words(report) == Some(Failure::Unread))
}

/// The kind of cause of a failed round that the words of what the command line reported tell, where they tell one.
fn words(report: &str) -> Option<Failure> {
    let text = report.to_lowercase();
    (KINDS.iter())
        .find(|(kind, words)| {
            words.iter().any(|w| text.contains(w)) || (*kind == Failure::UsageLimit && hit_a_limit(&text))
        })
        .map(|(kind, _)| *kind)
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

/// How long after a time it said a reset is still taken as just passed, and so as now: the time read and the clock
/// it is read by may be a little apart.
const JUST_PASSED: i64 = 60 * 60;

/// The furthest ahead a reset is taken: past it, it is read as said by mistake, and is not known. The longest limit
/// is a week's.
const FURTHEST: u64 = 8 * 24 * 60 * 60 * 1000;

/// When a usage limit resets, in milliseconds since the start of 1970: as the command line's stream said it
/// ([`Said`]), and where it did not, as the words of its report say ([`resets_at`]); one just passed is now, and one
/// further ahead than a reset can be is not known.
pub fn resets(report: &str, said: &Said, now: u64, offset: &dyn Fn(Option<&str>) -> Option<i64>) -> Option<u64> {
    let at = said.resets.or_else(|| resets_at(report, now, offset))?;
    let passed = now.saturating_sub(at);
    if passed > 0 {
        return (passed <= JUST_PASSED as u64 * 1000).then_some(now);
    }
    (at - now <= FURTHEST).then_some(at)
}

/// When a usage limit the command line reported resets, in milliseconds since the start of 1970: from the time it
/// gives, as seconds since then after a bar (`limit reached|1760000000`), or as a time after "resets", with or without
/// a day before it: `resets 5pm`, `resets 11:30am (Europe/Paris)`, `resets Fri 5pm`, `resets Nov 4 at 11pm`; the next
/// time the clock shows it there, but one passed within [`JUST_PASSED`] being now. `offset` gives a time zone's
/// distance from UTC in seconds, the local one for none, as it is now: a reset beyond a change of the clocks there is
/// read an hour off. None where the report gives no time it can read.
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
    let (hour, minute) = clock_of(word)?;
    // A time zone named in brackets, as the command line names it, with its own case.
    let zone = (report.rfind('('))
        .and_then(|i| report[i + 1..].split_once(')').map(|(z, _)| z.trim()))
        .filter(|z| z.contains('/'));
    let off = offset(zone)?;
    let local = i64::try_from(now / 1000).ok()? + off;
    let today = local.div_euclid(86_400);
    let time = hour * 3600 + minute * 60;
    // What is still to come, or just passed: the clock showing it this time, as against the next.
    let ahead = |at: i64| at > local - JUST_PASSED;
    let at = match (date, weekday) {
        // A date: this year's, or next year's where this year's has passed.
        (Some((month, day)), _) => {
            let (year, ..) = civil(today);
            let this = days(year, month, day)? * 86_400 + time;
            if ahead(this) {
                this
            } else {
                days(year + 1, month, day)? * 86_400 + time
            }
        }
        // A day of the week: this week's, or the next one's where this week's has passed.
        (None, Some(weekday)) => {
            let back = (weekday_of_day(today) - weekday).rem_euclid(7);
            let at = (today - back) * 86_400 + time;
            if ahead(at) { at } else { at + 7 * 86_400 }
        }
        // A time of day alone: today's, or tomorrow's where it has passed.
        (None, None) => {
            let at = today * 86_400 + time;
            if ahead(at) { at } else { at + 86_400 }
        }
    };
    // One just passed is now.
    let at = at.max(local);
    u64::try_from((at - off) * 1000).ok()
}

/// The hour and minute a word gives as a time of day, with am or pm at its end where it has one: `5pm`, `11:30am`,
/// `12:30`, `17:00`.
fn clock_of(word: &str) -> Option<(i64, i64)> {
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
    if rest.starts_with("pm") && hour < 12 {
        hour += 12;
    } else if rest.starts_with("am") && hour == 12 {
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
    fn the_words_of_a_report_tell_a_kind_where_they_hold_one() {
        // What the command line reports of a return it could not parse may quote any words of the return.
        assert_eq!(
            words(
                "the command line's result is an error (error_max_structured_output_retries): Failed to provide \
                 valid structured output after 3 attempts: please run /login"
            ),
            Some(Failure::Unread)
        );
        let limit = Some(Failure::UsageLimit);
        assert_eq!(words("Claude AI usage limit reached|1760000000"), limit);
        assert_eq!(words("You've hit your limit · resets 5pm"), limit);
        assert_eq!(words("You've hit your weekly limit · resets Nov 15 at 6pm"), limit);
        assert_eq!(words("You've hit your Opus 5-hour limit"), limit);
        // "Hit your" alone, or with a limit far off, is no usage limit.
        assert_eq!(
            words("API Error: 429 You hit your rate of requests, retry in a moment; the limit is per minute"),
            None
        );
        assert_eq!(words("Invalid API key · Please run /login"), Some(Failure::CannotRun));
        assert_eq!(words("Prompt is too long"), Some(Failure::PromptTooLong));
        assert_eq!(
            words("claude cannot be started: No such file or directory (os error 2)"),
            Some(Failure::CannotRun)
        );
        assert_eq!(
            words("API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)"),
            Some(Failure::Offline)
        );
        assert_eq!(
            words("API Error: Opus 5.5's safeguards flagged this message (https://www.anthropic.com/legal/aup)."),
            Some(Failure::Flagged)
        );
        // Words it knows none of tell nothing: neither a passing error nor one never met before is told by them.
        assert_eq!(words("API Error: 529 Overloaded"), None);
        assert_eq!(words("claude exited with signal: 9 and no result"), None);
    }

    /// What the stream says of a round that printed its result as an error, with these fields of the result besides.
    fn printed(fields: Value) -> Said {
        let mut result = serde_json::json!({ "type": "result", "subtype": "success", "is_error": true });
        for (key, value) in fields.as_object().into_iter().flatten() {
            result[key] = value.clone();
        }
        said(&[result])
    }

    // An error the command line printed whose kind neither its fields, the status the service answered with, nor its
    // words tell is no failure in passing: its work is paused, with what it said. A round that printed no result
    // crashed, and is tried again.
    #[test]
    fn an_error_nothing_tells_the_kind_of_is_not_taken_as_passing() {
        let none = Some(&work::Spent::default());
        let unknown = "the command line's result is an error: Something no version said before";
        assert_eq!(of(unknown, &printed(serde_json::json!({})), none), Failure::Other);
        let typed_unknown =
            printed(serde_json::json!({ "api_error": "dlp_request_denied", "terminal_reason": "api_error" }));
        assert_eq!(of(unknown, &typed_unknown, none), Failure::Other);
        assert_eq!(of(unknown, &began(), None), Failure::Passing);
        // A result that is no error and holds no return is not taken as passing either.
        let bare = said(&[serde_json::json!({ "type": "result", "subtype": "success", "is_error": false })]);
        let empty = "the command line's result holds no structured return";
        assert_eq!(of(empty, &bare, none), Failure::Other);
    }

    // The status the service answered with tells the class of the failure, whatever the words; within a refusal and a
    // rate limit, the words tell the kind.
    #[test]
    fn the_status_the_service_answered_with_tells_the_class_and_the_words_the_kind_within_it() {
        let none = Some(&work::Spent::default());
        let at = |status: u16| printed(serde_json::json!({ "api_error_status": status }));
        let api = "the command line's result is an error: API Error";
        for status in [408, 500, 502, 503, 504, 529] {
            assert_eq!(of(api, &at(status), none), Failure::Unavailable, "{status}");
            // Whatever words it quotes, a server's fault is the service's, and every work's.
            assert_eq!(
                of("Prompt is too long", &at(status), none),
                Failure::Unavailable,
                "{status}"
            );
        }
        assert_eq!(of(api, &at(401), none), Failure::CannotRun);
        assert_eq!(of(api, &at(403), none), Failure::CannotRun);
        // A rate limit is the account's usage limit where the words say so, else a throttle of every work's requests.
        assert_eq!(of("API Error: 429 slow down", &at(429), none), Failure::Unavailable);
        assert_eq!(
            of("You've hit your weekly limit · resets Nov 15 at 6pm", &at(429), none),
            Failure::UsageLimit
        );
        // A refused request is refused again: it pauses its work, as the kind its words tell or as one no kind tells.
        assert_eq!(of(api, &at(400), none), Failure::Other);
        assert_eq!(of(api, &at(422), none), Failure::Other);
        assert_eq!(
            of("API Error: 400 Prompt is too long", &at(400), none),
            Failure::PromptTooLong
        );
        assert_eq!(of("API Error: 400 Connection error.", &at(400), none), Failure::Other);
        // The fields come before the status.
        let unanswered = printed(serde_json::json!({ "api_error_status": 400, "api_error": "no_response" }));
        assert_eq!(of(api, &unanswered, none), Failure::Unavailable);
        let slept = printed(serde_json::json!({ "api_error_status": 529, "api_error": "computer_slept" }));
        assert_eq!(of(api, &slept, none), Failure::Passing);
    }

    // Why the round ended tells the kind where it says one: the return given up on, a context past the model's, a
    // crash of the command line as it ran, a bound reached.
    #[test]
    fn why_the_round_ended_tells_its_kind() {
        let none = Some(&work::Spent::default());
        let ended = |reason: &str| printed(serde_json::json!({ "terminal_reason": reason }));
        let api = "the command line's result is an error: API Error";
        // Given up on, the return is sent back whatever the words, and the words of another kind are not taken.
        assert_eq!(
            of(api, &ended("structured_output_retry_exhausted"), none),
            Failure::Unread
        );
        let gave_up = printed(serde_json::json!({ "subtype": "error_max_structured_output_retries" }));
        assert_eq!(of(api, &gave_up, none), Failure::Unread);
        assert_eq!(of(api, &ended("prompt_too_long"), none), Failure::PromptTooLong);
        assert_eq!(of(api, &ended("blocking_limit"), none), Failure::PromptTooLong);
        assert_eq!(of(api, &ended("malformed_tool_use_exhausted"), none), Failure::Passing);
        let crashed = printed(serde_json::json!({ "subtype": "error_during_execution" }));
        assert_eq!(of(api, &crashed, none), Failure::Passing);
        let bound = printed(serde_json::json!({ "subtype": "error_max_turns", "terminal_reason": "max_turns" }));
        assert_eq!(of(api, &bound, none), Failure::Other);
        // A reason that says only that it failed leaves the kind to the status.
        let failed = printed(serde_json::json!({ "terminal_reason": "api_error", "api_error_status": 529 }));
        assert_eq!(of(api, &failed, none), Failure::Unavailable);
        // A result that is no error says nothing of how a round ended.
        let fine =
            said(&[serde_json::json!({ "type": "result", "is_error": false, "subtype": "error_during_execution" })]);
        assert_eq!(fine.kind(), None);
    }

    #[test]
    fn a_round_that_reached_the_service_neither_cannot_run_nor_is_offline() {
        let spent = work::Spent {
            usd_micros: 1000,
            uncached_tokens: 10,
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

    // A command line that crashed once it began is tried again, as most of its crashes pass: the bound on failures in
    // passing in a row pauses its work where it goes on crashing. One that ended before it began, having reached
    // nothing, is the command line itself failing to run, whatever its words, as a program that cannot be started is.
    #[test]
    fn a_round_that_printed_no_result_fails_in_passing_where_it_began_unless_its_words_say_otherwise() {
        let began = began();
        assert_eq!(of("claude exited with signal: 9", &began, None), Failure::Passing);
        assert_eq!(of("API Error: 529 Overloaded", &began, None), Failure::Passing);
        assert_eq!(of("claude: command not found", &began, None), Failure::CannotRun);
        assert_eq!(
            of("getaddrinfo ENOTFOUND api.anthropic.com", &began, None),
            Failure::Offline
        );
        assert_eq!(of("Prompt is too long", &began, None), Failure::PromptTooLong);
        let spent = work::Spent {
            uncached_tokens: 10,
            ..work::Spent::default()
        };
        let none = Said::default();
        assert_eq!(
            of("claude exited with signal: 9", &none, Some(&spent)),
            Failure::Passing
        );
        assert_eq!(of("claude exited with signal: 9", &none, None), Failure::CannotRun);
        assert_eq!(
            of("claude cannot be started: Permission denied (os error 13)", &none, None),
            Failure::CannotRun
        );
        assert_eq!(
            of("getaddrinfo ENOTFOUND api.anthropic.com", &none, None),
            Failure::Offline
        );
    }

    /// What the stream says of a round whose command line said it began, and printed nothing more.
    fn began() -> Said {
        said(&[serde_json::json!({ "type": "system", "subtype": "init", "claude_code_version": "2.1.323" })])
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
    // usage limit, a throttle that shares its error makes the service unavailable, and an expired sign-in cannot run.
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
        assert_eq!(of("API Error: 429 slow down", &throttled, none), Failure::Unavailable);
        // The words still tell a limit the fields leave open.
        assert_eq!(
            of("You've hit your limit · resets 5pm", &throttled, none),
            Failure::UsageLimit
        );
        let overloaded = said(&[refusal("overloaded", None, Value::Null)]);
        assert_eq!(of("Invalid API key", &overloaded, none), Failure::Unavailable);
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

    // A refusing event the command line carries on past, on the account's overage, refuses nothing; and the round's
    // own error, read after it, tells the kind.
    #[test]
    fn a_rate_limit_event_the_command_line_carries_on_past_refuses_nothing() {
        let rejected = |extra: Value| {
            let mut info = serde_json::json!({ "status": "rejected", "resetsAt": 1_792_976_400_u64 });
            for (key, value) in extra.as_object().into_iter().flatten() {
                info[key] = value.clone();
            }
            serde_json::json!({ "type": "rate_limit_event", "rate_limit_info": info })
        };
        let none = Some(&work::Spent::default());
        for carried in [
            serde_json::json!({ "overageStatus": "allowed" }),
            serde_json::json!({ "overageStatus": "allowed_warning" }),
            serde_json::json!({ "isUsingOverage": true }),
            serde_json::json!({ "overageInUse": true }),
            serde_json::json!({ "rateLimitGraceActive": true }),
        ] {
            let on = rejected(carried.clone());
            assert!(!said(std::slice::from_ref(&on)).refused, "{carried}");
            let overloaded = said(&[on, refusal("overloaded", None, Value::Null)]);
            assert_eq!(
                of("API Error: 529", &overloaded, none),
                Failure::Unavailable,
                "{carried}"
            );
        }
        // A refusing event comes after the round's own error.
        let refused = rejected(serde_json::json!({ "overageStatus": "rejected" }));
        let overloaded = said(&[refused.clone(), refusal("overloaded", None, Value::Null)]);
        assert_eq!(of("API Error: 529", &overloaded, none), Failure::Unavailable);
        assert_eq!(of("API Error", &said(&[refused]), none), Failure::UsageLimit);
    }

    // A connection that failed once the round had spent tokens is the round's own; before any spend, the network's.
    #[test]
    fn a_connection_that_failed_after_the_round_spent_is_in_passing() {
        let failed = serde_json::json!({ "type": "result", "is_error": true, "api_error": "connection_failed" });
        let said = said(&[failed]);
        let spent = work::Spent {
            uncached_tokens: 10,
            output_tokens: 5,
            ..work::Spent::default()
        };
        assert_eq!(
            of("API Error: Connection error.", &said, Some(&spent)),
            Failure::Passing
        );
        assert_eq!(
            of("API Error: Connection error.", &said, Some(&work::Spent::default())),
            Failure::Offline
        );
    }

    // What is one session's or one model's holds no other work: its work pauses, saying why.
    #[test]
    fn a_cause_of_one_session_or_one_model_is_no_wait_every_work_shares() {
        let none = Some(&work::Spent::default());
        let long = said(&[refusal(
            "rate_limit",
            Some("long_context_credits_required"),
            Value::Null,
        )]);
        assert_eq!(of("API Error", &long, none), Failure::PromptTooLong);
        let credits = said(&[refusal("rate_limit", Some("model_requires_usage_credits"), Value::Null)]);
        assert_eq!(of("API Error", &credits, none), Failure::Other);
        let missing = said(&[refusal("model_not_found", None, Value::Null)]);
        assert_eq!(
            of("There's an issue with the selected model", &missing, none),
            Failure::NoModel
        );
        for kind in [Failure::PromptTooLong, Failure::NoModel, Failure::Other] {
            assert!(!kind.shared());
        }
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
        // A time just passed is now; one passed longer ago, tomorrow's.
        assert_eq!(resets_at("5-hour limit reached ∙ resets 7:30pm", now, &utc), Some(now));
        assert_eq!(
            resets_at("5-hour limit reached ∙ resets 6:30pm", now, &utc),
            Some(now + 22 * 3_600_000 + 1_800_000)
        );
        // Am or pm is the time's own: a time without it is on the clock of twenty-four hours.
        let chicago = |z: Option<&str>| (z == Some("America/Chicago")).then_some(-5 * 3600);
        assert_eq!(
            resets_at("You've hit your limit · resets 12:30 (America/Chicago)", now, &chicago),
            Some(now + 21 * 3_600_000 + 1_800_000)
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
        // Today's day at a time still to come is today; at one just passed, now; at one passed before, a week on.
        assert_eq!(resets_at("resets Sun 11pm", now, &utc), Some(hours(3)));
        assert_eq!(resets_at("resets Sun 7:30pm", now, &utc), Some(now));
        assert_eq!(resets_at("resets Sunday 5pm", now, &utc), Some(hours(7 * 24 - 3)));
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
        // A date passed this year is next year's, and one just passed is now.
        assert_eq!(
            resets_at("resets Oct 25 at 12am", now, &utc),
            Some(hours(365 * 24 - 20))
        );
        assert_eq!(resets_at("resets Oct 25 at 7:30pm", now, &utc), Some(now));
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
        // A reset further ahead than a week's limit can be is not known; one just passed is now, and one long passed
        // is not known.
        assert_eq!(resets("resets Oct 25 at 12am", &Said::default(), now, &utc), None);
        let at = |ms: u64| Said {
            resets: Some(ms),
            ..Said::default()
        };
        assert_eq!(resets("", &at(now - 600_000), now, &utc), Some(now));
        assert_eq!(resets("", &at(now - 2 * 3_600_000), now, &utc), None);
        assert_eq!(resets("", &at(hours(9 * 24)), now, &utc), None);
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
