//! The situations an entry of a cell of the small flow can reach: the entries, what ran it of what it watches, how its
//! session stands with each datum, and what is counted of every datum's own state.

mod support;

use context::Stands;
use context::situations::{Named, combinations, situations};
use flow::CellSpec;
use support::{compared, flow, words};

fn cell<'a>(flow: &'a flow::Compiled, id: &str) -> &'a CellSpec {
    flow.cells.iter().find(|c| c.id.as_str() == id).expect("the cell")
}

fn of(cell_id: &str) -> Vec<Named> {
    let (flow, words) = (flow(), words());
    situations(&flow, &words, cell(&flow, cell_id), &compared())
}

fn holding(named: &Named) -> bool {
    let held = [Stands::HoldsLast, Stands::HoldsLatest, Stands::HoldsNew, Stands::Holds];
    named.situation.data.values().any(|s| held.contains(s))
}

#[test]
fn the_first_cell_of_a_line_is_entered_first_holding_nothing_and_run_by_all_the_work_starts_with() {
    // The plan is the first of its line, and the request and the opinions, which people write, run it: the work starts
    // with them, so that both are new to a plan that worked from nothing.
    let all = of("plan");
    let first: Vec<&Named> = all.iter().filter(|n| n.name.starts_with("first-")).collect();
    assert!(!first.is_empty());
    for named in first {
        assert!(
            named.situation.holds.is_empty() && !named.situation.after_hand_in,
            "{}",
            named.name
        );
        assert!(!holding(named), "{}", named.name);
        assert_eq!(named.situation.new, ["request", "opinions"], "{}", named.name);
    }
}

#[test]
fn a_cell_after_another_of_its_line_is_entered_first_after_its_hand_in_and_may_hold_what_it_read_or_wrote() {
    let all = of("build");
    let first: Vec<&Named> = all.iter().filter(|n| n.name.starts_with("first-")).collect();
    // The plan, before the build on its line, writes a block.
    assert!(first.iter().all(|n| n.situation.after_hand_in));
    // The session may hold the request and the plan, which the plan read and wrote, as data the build never worked
    // from; never the code, which nothing before the build on its line reads or writes.
    let held = |block: &str| first.iter().any(|n| n.situation.data[block] == Stands::HoldsNew);
    assert!(held("request") && held("plan"));
    assert!(!held("code"));
}

#[test]
fn a_datum_certainly_written_when_the_cell_runs_is_never_unwritten() {
    // The request starts the work, and the plan, which the plan cell writes on no condition, comes before the build.
    for named in of("build") {
        for block in ["request", "plan"] {
            assert_ne!(
                named.situation.data[block],
                Stands::Unwritten,
                "{}: {block}",
                named.name
            );
        }
    }
    // The tree, which only people write, may be.
    assert!(
        of("build")
            .iter()
            .any(|n| n.situation.data["tree"] == Stands::Unwritten)
    );
}

#[test]
fn a_reopened_session_holds_nothing_and_a_return_that_asked_or_was_sent_back_handed_nothing_in() {
    for named in of("build") {
        if named.name.starts_with("reopened-") {
            assert!(!holding(&named) && named.situation.holds.is_empty(), "{}", named.name);
        }
        if !named.situation.results.is_empty() || named.situation.sent_back {
            assert!(!named.situation.after_hand_in, "{}", named.name);
        }
    }
}

#[test]
fn a_datum_watched_that_did_not_run_the_cell_stands_as_nothing_new_to_it_does() {
    let all = of("plan");
    let opinions = |name: &str| {
        let named = all
            .iter()
            .find(|n| n.name == name)
            .unwrap_or_else(|| panic!("no {name}"));
        named.situation.data["opinions"]
    };
    assert_eq!(opinions("later-request-ran_holds_last-rest_lacks"), Stands::Holds);
    assert_eq!(opinions("reopened-request-ran_lacks-rest_lacks"), Stands::Lacks);
}

#[test]
fn every_situation_is_written_once_and_every_datum_s_own_state_is_counted() {
    let (flow, words) = (flow(), words());
    for id in ["plan", "build", "review", "close"] {
        let all = situations(&flow, &words, cell(&flow, id), &compared());
        for (n, named) in all.iter().enumerate() {
            let again = all[..n].iter().find(|o| o.situation == named.situation);
            assert!(
                again.is_none(),
                "{id}: {} is {}",
                named.name,
                again.map_or("", |o| &o.name)
            );
        }
        assert!(
            combinations(&flow, &words, cell(&flow, id), &compared()) > 0,
            "{id}"
        );
    }
}

// A block the cell writes that nothing else gives it is given only where the session lacks its latest: held or lacking,
// never a line that it is held; and the tree, whose versions the carrier compares, is unread where the session lacks it
// (design document `data-and-context`, 21, 23).
#[test]
fn what_the_cell_only_writes_stands_held_or_lacking_and_the_tree_lacking_stands_unread() {
    let review = of("review");
    for named in &review {
        let verdict = named.situation.data["verdict"];
        let can = if named.name.starts_with("reopened") {
            vec![Stands::Lacks, Stands::Unwritten]
        } else {
            vec![Stands::Holds, Stands::Lacks, Stands::Unwritten]
        };
        assert!(can.contains(&verdict), "{}: {verdict:?}", named.name);
    }
    // Lacking where the session holds nothing, a reopened one standing as a first entry does; held on a later entry.
    let stands: Vec<Stands> = review.iter().map(|n| n.situation.data["verdict"]).collect();
    assert!(
        stands.contains(&Stands::Lacks) && stands.contains(&Stands::Holds),
        "{stands:?}"
    );
    let plan = of("plan");
    assert!(plan.iter().any(|n| n.situation.data["tree"] == Stands::Unread));
    assert!(plan.iter().all(|n| n.situation.data["tree"] != Stands::Lacks));
}
