//! Every situation an entry of a cell of the small flow can be in: the entries, the triggers that ran it, how its
//! session stands with each datum, and what is counted of every datum's own state.

mod support;

use context::Stands;
use context::situations::{combinations, situations};
use flow::CellSpec;
use support::{flow, words};

/// The source no block of the small flow is written by.
const REOPENED: &str = "session";

fn plan(flow: &flow::Compiled) -> &CellSpec {
    flow.cells.iter().find(|c| c.id.as_str() == "plan").expect("the plan")
}

#[test]
fn every_datum_s_own_state_is_counted_and_each_class_of_data_is_written_in_each_of_its_states() {
    let (flow, words) = (flow(), words());
    // The plan is run by `request` and `opinions`, each alone or both, and is given those two, `notes`, its own
    // `plan` and `tree`. Run by one, the datum of the other is never new; the three of the rest stand any way. On a
    // first entry a new datum stands 2 ways and any other 3; on a later one 3 and 5, one never new 2; on one reopened
    // 2 and 5, one never new 3.
    let alone = 2 * 27 + 3 * 2 * 125 + 2 * 3 * 125;
    let both = 2 * 2 * 27 + 3 * 3 * 125 + 2 * 2 * 125;
    assert_eq!(combinations(&flow, plan(&flow), REOPENED), 2 * alone + both);
    // Written, what ran it stands one way and the rest another: 2 by 3, 3 by 5 and 2 by 5 for each of the three sets
    // of triggers; then, on a later entry, each kind of result, a return sent back and nothing handed in.
    let all = situations(&flow, &words, plan(&flow), REOPENED);
    assert_eq!(all.len(), 3 * (6 + 15 + 10) + 5 + 1 + 1);
    let names: Vec<&str> = all.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names[..7],
        [
            "first-request-ran_lacks-rest_unwritten",
            "first-request-ran_lacks-rest_lacks",
            "first-request-ran_lacks-rest_holds",
            "first-request-ran_holds-rest_unwritten",
            "first-request-ran_holds-rest_lacks",
            "first-request-ran_holds-rest_holds",
            "first-opinions-ran_lacks-rest_unwritten",
        ]
    );
    assert!(names.contains(&"later-all-ran_holds_last-rest_holds_latest"));
    assert_eq!(names.last(), Some(&"later-request-nothing_handed_in"));
}

#[test]
fn a_datum_of_a_trigger_that_did_not_run_the_cell_stands_as_nothing_new_to_it_does() {
    let (flow, words) = (flow(), words());
    let all = situations(&flow, &words, plan(&flow), REOPENED);
    let opinions = |name: &str| {
        let named = all.iter().find(|n| n.name == name).expect(name);
        named.situation.data["opinions"]
    };
    assert_eq!(opinions("first-request-ran_lacks-rest_lacks"), Stands::Unwritten);
    assert_eq!(opinions("later-request-ran_holds_last-rest_lacks"), Stands::Holds);
    assert_eq!(opinions("reopened-request-ran_lacks-rest_holds"), Stands::Lacks);
    // Run by both, it is new with the request.
    assert_eq!(opinions("later-all-ran_holds_latest-rest_holds"), Stands::HoldsLatest);
}

#[test]
fn a_later_entry_holds_every_part_the_session_keeps_and_a_first_or_reopened_one_none() {
    let (flow, words) = (flow(), words());
    let all = situations(&flow, &words, plan(&flow), REOPENED);
    for named in &all {
        let later = named.name.starts_with("later-");
        assert_eq!(!named.situation.holds.is_empty(), later, "{}", named.name);
        let handed_in = later && !named.name.ends_with("nothing_handed_in");
        assert_eq!(named.situation.after_hand_in, handed_in, "{}", named.name);
    }
}
