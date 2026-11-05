//! What a session already holds: what it was given and what it handed back since it was last compacted, worked out
//! from the record of its rounds. A conditional input it holds is only referred to by its address; one it lacks is
//! given.

use std::collections::{BTreeSet, HashMap, HashSet};

use db::rusqlite::Connection;
use serde_json::Value;

use crate::round::{self, Round, RoundId};

/// The round an address names, for an address of a round's value.
pub fn round_of(address: &str) -> Option<RoundId> {
    RoundId::parse(address.split('·').next()?)
}

/// One conditional input a round recorded.
struct Recorded {
    address: String,
    hash: String,
    /// The key its whole version is kept under, for an input of the standard context recorded since rounds kept it.
    text: Option<String>,
}

/// The conditional inputs a round recorded; none for a round recorded before they were. A round recorded before they
/// were called so keeps them under `materials`.
fn recorded(input: &Value) -> Vec<Recorded> {
    input
        .get("conditional")
        .or_else(|| input.get("materials"))
        .unwrap_or(&Value::Null)
        .as_array()
        .map(|xs| {
            xs.iter()
                .filter_map(|m| {
                    Some(Recorded {
                        address: m["address"].as_str()?.to_string(),
                        hash: m["hash"].as_str()?.to_string(),
                        text: m["text"].as_str().map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// How the address of a version of the project guide ends, and the input of the standard context it is.
const GUIDE_ADDRESS: &str = "·engine·project_guide";
const GUIDE_INPUT: &str = "engine·project_guide";

/// What the engine computes that is standard context: large and much the same from round to round, so that a session
/// is given it once and after that only what changed in it. For the architect's cells, what they read of the project,
/// what came since the base, which grows by its entries, and the queue. For the reviews, the CI on the work, whose runs
/// change one at a time (`work_ci`).
const ENGINE_STANDARD: [&str; 11] = [
    "constitution",
    "guide",
    "policy",
    "policy_signals",
    "board",
    "numbers",
    "observations_overview",
    "observation_tags",
    "appended",
    "queued",
    "work_ci",
];

/// The entries of an input of the standard context that every version of it given as what changed carries whole,
/// changed or not: the numbers of the matters the architect's queue can be picked from and of those its batch handled,
/// so that a session given only what changed, round after round, loses track of none.
pub const INDEXES: [&str; 2] = ["pickable", "handled_matters"];

/// Which input of the standard context an address is a version of: the project guide and what the engine computes as
/// standard context, whose versions have addresses of their own, or a return format, a list of inputs, a rule, a
/// setting, the worktree, a cell's task or the columns of its review table, whose versions share one; none for an
/// address of anything else.
pub fn standard(address: &str) -> Option<&str> {
    if address.ends_with(GUIDE_ADDRESS) {
        return Some(GUIDE_INPUT);
    }
    if let Some(at) = address.find("·engine·")
        && round_of(address).is_some()
    {
        let input = &address[at + '·'.len_utf8()..];
        let name = &input["engine·".len()..];
        if ENGINE_STANDARD.contains(&name) {
            return Some(input);
        }
    }
    ["format:", "inputs:", "rule:", "config:", "work:", "task:", "columns:"]
        .iter()
        .any(|p| address.starts_with(p))
        .then_some(address)
}

/// A version of an input of the standard context a session holds: its address, the hash of its content, the round
/// the session got it in, and the key `contents` keeps it under, none for a version recorded before rounds kept it.
#[derive(Debug, Clone)]
pub struct Version {
    pub address: String,
    pub hash: String,
    pub round: RoundId,
    pub text: Option<String>,
}

/// What a session already holds: what it was given and what it handed back since it was last compacted.
#[derive(Debug, Default)]
pub struct Held {
    /// The conditional inputs it was given or referred to, each as its address and the hash of its content.
    pub conditional: HashSet<(String, String)>,
    /// The rounds whose returns it holds: every value they produced is in the session under its address.
    pub rounds: BTreeSet<RoundId>,
    /// The last round in which it was compacted, or after which a keep-warm ping compacted it while it waited: it
    /// holds nothing from before that round's end.
    pub compacted: Option<RoundId>,
    /// The last compaction came in a keep-warm ping while the session waited after the round `compacted` names.
    pub compacted_waiting: bool,
    /// Where it first got each content it holds, by the content's hash: the round, and the address it got it under.
    pub got: HashMap<String, (RoundId, String)>,
    /// The latest version it holds of each input of the standard context, by which input it is.
    pub standard: HashMap<String, Version>,
    /// The parts of its system prompt ([`crate::system`]), each as the round that started the session recorded it: it
    /// holds them from its start to its end, compacted or not.
    pub system: Vec<Version>,
}

/// The parts of a session's system prompt, as the round that started it recorded them (`system`); none for a session
/// started without one.
fn system_of(session_rounds: &[Round]) -> Vec<Version> {
    session_rounds
        .iter()
        .find_map(|r| {
            let parts = r.input.get("system")?.as_array()?;
            Some(
                parts
                    .iter()
                    .filter_map(|x| {
                        Some(Version {
                            address: x["address"].as_str()?.to_string(),
                            hash: x["hash"].as_str()?.to_string(),
                            round: r.id,
                            text: x["text"].as_str().map(str::to_string),
                        })
                    })
                    .collect(),
            )
        })
        .unwrap_or_default()
}

impl Held {
    /// What the session whose rounds these are, oldest first, holds now. A round that handed in a return holds what
    /// it was given and what it produced; one whose run ended without a return holds what it was given, which the
    /// command line wrote into the session before it called the service, unless the command line could not run at
    /// all (`wrote_message`).
    pub fn of(session_rounds: &[Round]) -> Held {
        Held::with_pings(session_rounds, &[])
    }

    /// What a session holds that holds only its system prompt, these parts: a session that starts with them, or one
    /// just compacted, which keeps its system prompt.
    pub fn with_system(parts: &[Version]) -> Held {
        let mut h = Held::default();
        for v in parts {
            h.got.entry(v.hash.clone()).or_insert((v.round, v.address.clone()));
            h.conditional.insert((v.address.clone(), v.hash.clone()));
            if let Some(input) = standard(&v.address) {
                h.standard.insert(input.to_string(), v.clone());
            }
        }
        h.system = parts.to_vec();
        h
    }

    /// What the session holds, its rounds oldest first and the times keep-warm pings compacted it: a ping's compaction,
    /// like a round's, leaves it holding nothing from before but its system prompt.
    pub fn with_pings(session_rounds: &[Round], compacted_at: &[i64]) -> Held {
        let system = system_of(session_rounds);
        let mut h = Held::with_system(&system);
        let mut last: Option<&Round> = None;
        let waited = |after: Option<&Round>, before: Option<i64>| {
            after.is_some_and(|a| {
                compacted_at
                    .iter()
                    .any(|t| *t >= a.started_at && before.is_none_or(|b| *t < b))
            })
        };
        for r in session_rounds {
            if waited(last, Some(r.started_at))
                && let Some(a) = last
            {
                h = Held {
                    compacted: Some(a.id),
                    compacted_waiting: true,
                    ..Held::with_system(&system)
                };
            }
            last = Some(r);
            if r.compactions > 0 {
                h = Held {
                    compacted: Some(r.id),
                    ..Held::with_system(&system)
                };
                continue;
            }
            if r.output.is_some() {
                h.rounds.insert(r.id);
            } else if !wrote_message(r) {
                continue;
            }
            for x in recorded(&r.input) {
                let round = h.got.entry(x.hash.clone()).or_insert((r.id, x.address.clone())).0;
                // What the engine computes is a version of its input only where the round kept it whole: one recorded
                // before it was standard context was given whole, and may not be of the kind its versions are now.
                let unkept = x.text.is_none()
                    && ENGINE_STANDARD
                        .iter()
                        .any(|n| x.address.ends_with(&format!("·engine·{n}")));
                if let Some(input) = standard(&x.address).filter(|_| !unkept) {
                    let text = x.text.clone().or_else(|| {
                        h.standard
                            .get(input)
                            .filter(|v| v.hash == x.hash)
                            .and_then(|v| v.text.clone())
                    });
                    let v = Version {
                        address: x.address.clone(),
                        hash: x.hash.clone(),
                        round,
                        text,
                    };
                    h.standard.insert(input.to_string(), v);
                }
                h.conditional.insert((x.address, x.hash));
            }
        }
        if waited(last, None)
            && let Some(a) = last
        {
            h = Held {
                compacted: Some(a.id),
                compacted_waiting: true,
                ..Held::with_system(&system)
            };
        }
        h
    }

    /// Whether the session holds a conditional input: a value one of its own returns produced, or the same content under the
    /// same address as one it was given. One content has one address, so the same content is held however it came.
    pub fn holds(&self, address: &str, hash: &str) -> bool {
        let own = round_of(address).is_some_and(|r| self.rounds.contains(&r)) && !address.contains("·engine·");
        own || self.conditional.contains(&(address.to_string(), hash.to_string()))
    }
}

/// Whether a round that handed in no return still put its message in its session: the command line writes the
/// message into the session before it calls the service, so a run the engine cut off, and one that failed reaching
/// the service (no network, a usage limit, a server error, a message the service refused), left it there; only a
/// command line that could not run at all (`cannot_run`: not logged in, not installed) did not. Giving such a round's
/// message again would put a second copy of it in the session at every attempt.
pub fn wrote_message(r: &Round) -> bool {
    let Some(rej) = r.rejected.as_ref() else {
        return false;
    };
    rej.get("interrupted").is_some()
        || rej
            .get("failed")
            .and_then(|f| f.get("kind"))
            .and_then(Value::as_str)
            .is_some_and(|k| k != "cannot_run")
}

/// A CLI session's rounds, oldest first, and what it holds now, the compactions of the keep-warm pings it holds counted
/// ([`crate::warm::held_pings`]): a fork holds those of its base's from before it was made.
pub fn of_session(conn: &Connection, cli_session: &str) -> Result<(Vec<Round>, Held), String> {
    let rounds = round::of_cli_session(conn, cli_session)?;
    let at: Vec<i64> = crate::warm::held_pings(conn, cli_session)?
        .into_iter()
        .filter(|p| p.compactions > 0)
        .map(|p| p.started_at)
        .collect();
    let held = Held::with_pings(&rounds, &at);
    Ok((rounds, held))
}

/// Every address each content was given or referred to by in the work, in the order they first appear, by its hash.
pub fn registry(work_rounds: &[Round]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for r in work_rounds {
        for x in recorded(&r.input) {
            let seen = out.entry(x.hash).or_default();
            if !seen.contains(&x.address) {
                seen.push(x.address);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow::formats::hash;
    use serde_json::json;

    fn round(n: i64, input: Value, output: Option<Value>, compactions: i64) -> Round {
        Round {
            id: RoundId::new(1, n),
            cell: Some("R1".into()),
            session: Some(1),
            input,
            output,
            transition: Some("done".into()),
            rejected: None,
            compactions,
            started_at: n * 100,
            ended_at: Some(n * 100 + 50),
            usage: None,
        }
    }

    #[test]
    fn a_round_is_named_by_its_work_and_number() {
        assert_eq!(round_of("R5-28·I1·end.test_changes[added]"), Some(RoundId::new(5, 28)));
        assert_eq!(RoundId::new(5, 28).to_string(), "R5-28");
        assert_eq!(round_of("rule:ground"), None);
        // An address written before rounds were named by their work names no round.
        assert_eq!(round_of("R28·I1·end.test_changes[added]"), None);
    }

    // What the engine computes as the architect's standard context is known by its input, whichever round gave it; what
    // else the engine computes for a round is not standard context.
    #[test]
    fn the_architect_s_standard_context_is_known_by_its_input() {
        assert_eq!(standard("R3-4·engine·board"), Some("engine·board"));
        assert_eq!(standard("R3-9·engine·queued"), Some("engine·queued"));
        assert_eq!(standard("R3-4·engine·appended"), Some("engine·appended"));
        assert_eq!(standard("R3-4·engine·matter"), None);
        assert_eq!(standard("x·engine·board"), None);
        assert_eq!(standard("R3-4·engine·project_guide"), Some("engine·project_guide"));
        assert_eq!(standard("rule:ground"), Some("rule:ground"));
    }

    // A session holds what it was given, referred to and handed back since its last compaction, and nothing from a
    // run that handed nothing in.
    #[test]
    fn a_session_holds_what_it_was_given_and_returned_since_its_last_compaction() {
        let given = |a: &str, v: &Value| json!({ "conditional": [{ "address": a, "hash": hash(v), "given": true }] });
        let rule = json!("Facts first.");
        let rounds = [
            round(
                2,
                given("rule:ground", &rule),
                Some(json!({ "outcome": {}, "progress": "p" })),
                0,
            ),
            round(3, json!({}), Some(json!({ "outcome": { "design": "d" } })), 0),
            round(4, given("ij:1", &json!("From the owner: x")), None, 0),
        ];
        let h = Held::of(&rounds);
        assert!(h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("rule:ground", &hash(&json!("Facts first, then the rest."))));
        assert!(h.holds("R1-3·D2·design", "any"));
        assert!(!h.holds("R2-3·D2·design", "any"));
        assert!(!h.holds("R1-3·engine·project_guide", "any"));
        assert!(!h.holds("ij:1", &hash(&json!("From the owner: x"))));
        let mut later = rounds.to_vec();
        later.push(round(5, json!({}), Some(json!({ "outcome": {} })), 1));
        let h = Held::of(&later);
        assert!(!h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("R1-3·D2·design", "any"));
        assert_eq!(h.compacted, Some(RoundId::new(1, 5)));
        assert!(!h.compacted_waiting);
    }

    // A run that ended without a return left its message in the session, the command line having written it there
    // before it called the service: the session holds what it was given, not anything it produced, so that running
    // it again does not give it all once more; a command line that could not run at all left nothing.
    #[test]
    fn a_failed_run_left_its_message_in_the_session() {
        let task = json!("Implement the scheduler.");
        let given = json!({ "conditional": [{ "address": "task:I1", "hash": hash(&task), "given": true }] });
        let failed = |n: i64, kind: &str| Round {
            transition: None,
            rejected: Some(json!({ "failed": { "kind": kind, "what": "API Error" } })),
            ..round(n, given.clone(), None, 0)
        };
        for kind in ["offline", "transient", "flagged", "usage_limit"] {
            let h = Held::of(&[failed(2, kind)]);
            assert!(h.holds("task:I1", &hash(&task)), "{kind}");
            assert!(!h.rounds.contains(&RoundId::new(1, 2)), "{kind}");
        }
        let cut = Round {
            transition: None,
            rejected: Some(json!({ "interrupted": "the engine stopped" })),
            ..round(2, given.clone(), None, 0)
        };
        assert!(Held::of(&[cut]).holds("task:I1", &hash(&task)));
        assert!(!Held::of(&[failed(2, "cannot_run")]).holds("task:I1", &hash(&task)));
    }

    // A session holds the parts of its system prompt, as the round that started it recorded them, from its start to
    // its end: a compaction, in a round or in a keep-warm ping, leaves it holding them and nothing else from before.
    #[test]
    fn a_session_holds_its_system_prompt_through_a_compaction() {
        let common = json!("The forms.");
        let rule = json!("Facts first.");
        let started = json!({
            "system": [{ "address": "format:common", "hash": hash(&common), "text": "k1" }],
            "conditional": [{ "address": "rule:ground", "hash": hash(&rule), "given": true }],
        });
        let rounds = [round(2, started, Some(json!({})), 0)];
        let h = Held::of(&rounds);
        assert!(h.holds("format:common", &hash(&common)) && h.holds("rule:ground", &hash(&rule)));
        assert_eq!(h.standard["format:common"].round, RoundId::new(1, 2));
        assert_eq!(h.standard["format:common"].text.as_deref(), Some("k1"));
        let mut later = rounds.to_vec();
        later.push(round(3, json!({}), Some(json!({})), 1));
        let h = Held::of(&later);
        assert!(h.holds("format:common", &hash(&common)) && !h.holds("rule:ground", &hash(&rule)));
        assert_eq!(h.got[&hash(&common)].1, "format:common");
        let h = Held::with_pings(&rounds, &[300]);
        assert!(h.compacted_waiting && h.holds("format:common", &hash(&common)));
        assert_eq!(h.system.len(), 1);
    }

    // A keep-warm ping that compacted the session while it waited leaves it holding nothing from before; a round after
    // the ping is held again.
    #[test]
    fn a_ping_that_compacted_the_session_leaves_it_holding_nothing_from_before() {
        let rule = json!("Facts first.");
        let given = json!({ "conditional": [{ "address": "rule:ground", "hash": hash(&rule), "given": true }] });
        let rounds = [round(2, given.clone(), Some(json!({})), 0)];
        // Round 2 started at 200 and ended at 250; the ping compacted the session at 300.
        let h = Held::with_pings(&rounds, &[300]);
        assert!(!h.holds("rule:ground", &hash(&rule)));
        assert_eq!(h.compacted, Some(RoundId::new(1, 2)));
        assert!(h.compacted_waiting);
        let mut later = rounds.to_vec();
        later.push(round(4, given, Some(json!({})), 0));
        let h = Held::with_pings(&later, &[300]);
        assert!(h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("R1-2·R1·outcome", "any") && h.holds("R1-4·R1·outcome", "any"));
        // A ping that compacted nothing changes nothing.
        assert!(Held::with_pings(&rounds, &[]).holds("rule:ground", &hash(&rule)));
    }
}
