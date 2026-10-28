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

/// What the engine computes for the architect's cells that is standard context: large and much the same from round to
/// round, so that its line's session is given it once and after that only what changed in it.
const ARCHITECT_STANDARD: [&str; 10] = [
    "constitution",
    "guide",
    "policy",
    "policy_signals",
    "board",
    "numbers",
    "observations_overview",
    "observation_tags",
    "queue",
    "batch",
];

/// The entries of an input of the standard context that every version of it given as what changed carries whole,
/// changed or not: the numbers of the matters the architect's queue can be picked from and of those its batch handled,
/// so that a session given only what changed, round after round, loses track of none.
pub const INDEXES: [&str; 2] = ["pickable", "handled_matters"];

/// Which input of the standard context an address is a version of: the project guide and what the engine computes as
/// the architect's standard context, whose versions have addresses of their own, or a return format, a list of
/// inputs, a rule, a setting or the worktree, whose versions share one; none for an address of anything else.
pub fn standard(address: &str) -> Option<&str> {
    if address.ends_with(GUIDE_ADDRESS) {
        return Some(GUIDE_INPUT);
    }
    if let Some(at) = address.find("·engine·")
        && round_of(address).is_some()
    {
        let input = &address[at + '·'.len_utf8()..];
        let name = &input["engine·".len()..];
        if ARCHITECT_STANDARD.contains(&name) {
            return Some(input);
        }
    }
    ["format:", "inputs:", "rule:", "config:", "work:"]
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
}

impl Held {
    /// What the session whose rounds these are, oldest first, holds now. A round counts once it handed in a return:
    /// a run that failed may never have reached the session.
    pub fn of(session_rounds: &[Round]) -> Held {
        Held::with_pings(session_rounds, &[])
    }

    /// What the session holds, its rounds oldest first and the times keep-warm pings compacted it: a ping's compaction,
    /// like a round's, leaves it holding nothing from before.
    pub fn with_pings(session_rounds: &[Round], compacted_at: &[i64]) -> Held {
        let mut h = Held::default();
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
                    ..Held::default()
                };
            }
            last = Some(r);
            if r.compactions > 0 {
                h = Held {
                    compacted: Some(r.id),
                    ..Held::default()
                };
                continue;
            }
            if r.output.is_none() {
                continue;
            }
            h.rounds.insert(r.id);
            for x in recorded(&r.input) {
                let round = h.got.entry(x.hash.clone()).or_insert((r.id, x.address.clone())).0;
                if let Some(input) = standard(&x.address) {
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
                ..Held::default()
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

/// A CLI session's rounds, oldest first, and what it holds now, its keep-warm pings' compactions counted.
pub fn of_session(conn: &Connection, cli_session: &str) -> Result<(Vec<Round>, Held), String> {
    let rounds = round::of_cli_session(conn, cli_session)?;
    let mut st = conn
        .prepare(
            "SELECT p.started_at FROM pings p JOIN sessions s ON s.id = p.session
             WHERE s.cli_session = ?1 AND p.compactions > 0 ORDER BY p.started_at",
        )
        .map_err(|e| e.to_string())?;
    let at: Vec<i64> = st
        .query_map([cli_session], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
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
            cell: Some("1".into()),
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
        assert_eq!(round_of("R5-28·4·end.test_changes[added]"), Some(RoundId::new(5, 28)));
        assert_eq!(RoundId::new(5, 28).to_string(), "R5-28");
        assert_eq!(round_of("rule:ground"), None);
        // An address written before rounds were named by their work names no round.
        assert_eq!(round_of("R28·4·end.test_changes[added]"), None);
    }

    // What the engine computes as the architect's standard context is known by its input, whichever round gave it; what
    // else the engine computes for a round is not standard context.
    #[test]
    fn the_architect_s_standard_context_is_known_by_its_input() {
        assert_eq!(standard("R3-4·engine·board"), Some("engine·board"));
        assert_eq!(standard("R3-9·engine·queue"), Some("engine·queue"));
        assert_eq!(standard("R3-4·engine·batch"), None);
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
        assert!(h.holds("R1-3·2a·design", "any"));
        assert!(!h.holds("R2-3·2a·design", "any"));
        assert!(!h.holds("R1-3·engine·project_guide", "any"));
        assert!(!h.holds("ij:1", &hash(&json!("From the owner: x"))));
        let mut later = rounds.to_vec();
        later.push(round(5, json!({}), Some(json!({ "outcome": {} })), 1));
        let h = Held::of(&later);
        assert!(!h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("R1-3·2a·design", "any"));
        assert_eq!(h.compacted, Some(RoundId::new(1, 5)));
        assert!(!h.compacted_waiting);
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
        assert!(!h.holds("R1-2·1·outcome", "any") && h.holds("R1-4·1·outcome", "any"));
        // A ping that compacted nothing changes nothing.
        assert!(Held::with_pings(&rounds, &[]).holds("rule:ground", &hash(&rule)));
    }
}
