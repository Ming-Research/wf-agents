//! What a message says of a value of an input the engine computed that it only refers to by an address, when the value
//! went A, then B, then back to A and to B again: the round it names as having given another value since must come after
//! the round the address names, since "since" is what the architect reads it by.

use std::collections::HashMap;

use db::rusqlite::Connection;
use flow::formats::hash;
use serde_json::{Value, json};
use sessions::give::Giver;
use sessions::{Held, Round, RoundId};

/// A round of work 1 whose message recorded the conditional inputs `record` says.
fn round(n: i64, record: Value) -> Round {
    Round {
        id: RoundId::new(1, n),
        cell: Some("R1".into()),
        session: Some(1),
        input: record,
        output: Some(json!({})),
        transition: Some("done".into()),
        rejected: None,
        compactions: 0,
        started_at: 0,
        ended_at: Some(0),
        usage: None,
    }
}

/// The record of a message that gave (`given`) or only referred to the value `v` under `address`.
fn record(address: &str, v: &Value, given: bool) -> Value {
    json!({ "conditional": [{ "address": address, "hash": hash(v), "given": given }] })
}

/// The number of the work's round that a sentence such as "R1-6 gave another value since" names.
fn named_round(said: &str) -> i64 {
    let (named, _) = said.split_once(" gave another value since").unwrap_or_else(|| panic!("{said}"));
    named.rsplit('-').next().unwrap().parse().unwrap_or_else(|_| panic!("{said}"))
}

// The slots are A in round 2, B in round 4, and A again in round 6, which refers to round 2's address. In round 8 they
// are B: the message refers to round 4's address, and the other value, A, is the latest the session holds, put there by
// round 6. It must not name round 2 as the round that gave another value "since" round 4: round 2 is before round 4.
// The reference is the sentence itself: "since" names a round after the one the address names.
#[test]
fn the_round_named_as_having_given_another_value_since_comes_after_the_value_it_is_told_of() {
    let (a, b) = (
        json!({ "engine": { "runs": "running" } }),
        json!({ "engine": { "runs": "paused" } }),
    );
    let session = [
        round(2, record("R1-2·engine·slots", &a, true)),
        round(4, record("R1-4·engine·slots", &b, true)),
        round(6, record("R1-2·engine·slots", &a, false)),
    ];
    let held = Held::of(&session);
    let conn = Connection::open_in_memory().unwrap();
    let registry = HashMap::new();
    let mut giver = Giver::new(&held, &registry, &conn, &session);
    giver.add("R1-4·engine·slots".to_string(), b);
    let said = giver.current("R1-4·engine·slots").expect("a value the engine computed, referred to");
    assert!(said.starts_with("the value now, the same as given in R1-4"), "{said}");
    assert!(said.contains("no longer current"), "the other value is told as not current: {said}");
    let since = named_round(said.split("; ").nth(1).unwrap_or_else(|| panic!("{said}")));
    assert!(since > 4, "{since} is not after round 4, which the address names: {said}");
}
