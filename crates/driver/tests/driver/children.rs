//! How many open children an act leaves an item with: an item's children are the items whose main dependency is on it,
//! and a child is open while its work has neither merged nor been cancelled. An act that leaves an item with more than
//! it had, and more than the warning, is done and says so; more than the most, it is refused and changes nothing. An
//! act that lowers a count, or leaves an item past a limit no higher, is never refused. Every act that can move a
//! child is held to it: an item added with its dependencies, a dependency added, taken away or made the main one, a
//! cancel and a reopen, each alone or asked together with others.

use driver::{Asked, Children, Crowded};
use store::ItemChange;

use super::*;

/// Past two open children an act is warned of, and past three it is refused: small, so that a test needs few items.
const LIMITS: Children = Children { warn: 2, most: 3 };

/// The driver, holding every act to [`LIMITS`].
fn held() -> H {
    let mut h = H::new();
    h.hold_children(LIMITS);
    h
}

/// Items of these titles, each added depending on nothing.
fn free(h: &mut H, titles: &[&str]) -> Vec<ItemNumber> {
    (titles.iter())
        .map(|title| h.add(new_item(title, &[], "short")).0)
        .collect()
}

/// An item left with this many open children, past this limit.
fn crowded(item: Option<ItemNumber>, children: usize, limit: usize) -> Crowded {
    Crowded { item, children, limit }
}

/// Why changes asked together are refused; the driver and the store are left as they were.
fn refused(h: &mut H, asked: Vec<(ItemNumber, Asked)>) -> Failed {
    h.refused(|driver, now| driver.change(now, Some("why not"), asked).map(drop))
}

/// Changes asked together, which the driver takes: what they leave past the warning.
fn taken(h: &mut H, asked: Vec<(ItemNumber, Asked)>) -> Vec<Crowded> {
    h.changes_crowding(asked).expect("the changes are taken").2
}

#[test]
fn an_item_added_on_its_dependencies_is_warned_past_the_warning_and_refused_past_the_most() {
    let mut h = held();
    let a = free(&mut h, &["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9"]);
    // Two children are no more than the warning.
    let (_, _, warned) = h.add_crowding(new_item("two", &a[0..2], "short"));
    assert_eq!(warned, []);
    // Three are, and the add names the item it made.
    let (three, _, warned) = h.add_crowding(new_item("three", &a[2..5], "short"));
    assert_eq!(warned, [crowded(Some(three), 3, LIMITS.warn)]);
    // Four are more than the most: refused, the item that was to be added named as new, and nothing kept.
    let why = h.refused(|d, now| d.add(now, new_item("four", &a[5..9], "short")).map(drop));
    assert_eq!(why, Failed::Crowded(vec![crowded(None, 4, LIMITS.most)]));
    // Items another item's main dependency is on already stay under it, and are no children of the new one.
    let (_, _, warned) = h.add_crowding(new_item("shadows", &a[0..5], "short"));
    assert_eq!(warned, [], "the items it depends on are drawn under the others");
}

#[test]
fn a_dependency_added_or_made_the_main_one_is_held_to_the_children_it_leaves() {
    let mut h = held();
    let a = free(&mut h, &["a1", "a2", "a3", "b"]);
    let (p, _, _) = h.add_crowding(new_item("p", &a[0..3], "short"));
    let (q, _) = h.add(new_item("q", &[a[3]], "short"));
    // b's main dependency is q's, so p coming to depend on it gives p no child.
    assert_eq!(taken(&mut h, vec![(p, Asked::Item(ItemChange::DependsOn(a[3])))]), []);
    // Made p's main one, b would be p's fourth child.
    assert_eq!(
        refused(&mut h, vec![(p, Asked::Item(ItemChange::Main(a[3])))]),
        Failed::Crowded(vec![crowded(Some(p), 4, LIMITS.most)])
    );
    // A new item p comes to depend on, whose main dependency is then p's, would be its fourth child too.
    let c = free(&mut h, &["c"])[0];
    assert_eq!(
        refused(&mut h, vec![(p, Asked::Item(ItemChange::DependsOn(c)))]),
        Failed::Crowded(vec![crowded(Some(p), 4, LIMITS.most)])
    );
    // Asked together with a change that takes one of p's children away, p is left with no more than it had.
    let asked = vec![
        (p, Asked::Item(ItemChange::DependsOn(c))),
        (p, Asked::Item(ItemChange::NoLongerDependsOn(a[0]))),
    ];
    assert_eq!(
        taken(&mut h, asked),
        [],
        "an item past the warning left no higher is not warned of"
    );
    // q, left with b, takes c's main dependency: two, no more than the warning.
    assert_eq!(taken(&mut h, vec![(q, Asked::Item(ItemChange::DependsOn(c)))]), []);
    assert_eq!(taken(&mut h, vec![(q, Asked::Item(ItemChange::Main(c)))]), []);
    assert_eq!(h.d().main_dependent(c), Some(q));
}

#[test]
fn a_dependency_taken_away_or_a_cancel_that_moves_children_is_held_to_where_they_go() {
    let mut h = held();
    let b = free(&mut h, &["b1", "b2", "b3", "a"]);
    let (q, _, warned) = h.add_crowding(new_item("q", &b[0..3], "short"));
    assert_eq!(warned, [crowded(Some(q), 3, LIMITS.warn)]);
    // a is p's child; q depends on it too, under p.
    let (p, _) = h.add(new_item("p", &[b[3]], "short"));
    assert_eq!(taken(&mut h, vec![(q, Asked::Item(ItemChange::DependsOn(b[3])))]), []);
    // Taken from p, or p cancelled, a would go to q, its fourth child.
    let to_q = Failed::Crowded(vec![crowded(Some(q), 4, LIMITS.most)]);
    assert_eq!(
        refused(&mut h, vec![(p, Asked::Item(ItemChange::NoLongerDependsOn(b[3])))]),
        to_q
    );
    assert_eq!(refused(&mut h, vec![(p, Asked::Cancel)]), to_q);
    // With one of q's own taken away in the same act, q is left with three, as it had.
    let asked = vec![
        (p, Asked::Cancel),
        (q, Asked::Item(ItemChange::NoLongerDependsOn(b[0]))),
    ];
    assert_eq!(taken(&mut h, asked), []);
    assert_eq!(h.d().main_dependent(b[3]), Some(q));
}

#[test]
fn a_reopen_is_held_to_the_children_it_opens_again() {
    let mut h = held();
    let b = free(&mut h, &["b1", "b2", "b3", "b4"]);
    let (q, _, _) = h.add_crowding(new_item("q", &b[0..3], "short"));
    // A cancelled child is not open: q has two, and a fourth item makes three.
    let (_, todos) = h.changes(vec![(b[2], Asked::Cancel)]).expect("b3 is cancelled");
    h.released(todos);
    assert_eq!(
        taken(&mut h, vec![(q, Asked::Item(ItemChange::DependsOn(b[3])))]),
        [crowded(Some(q), 3, LIMITS.warn)]
    );
    // Reopened, b3 would be its fourth open child.
    assert_eq!(
        refused(&mut h, vec![(b[2], Asked::Reopen)]),
        Failed::Crowded(vec![crowded(Some(q), 4, LIMITS.most)])
    );
    // Reopened as b4 is cancelled, q is left with three.
    let (_, todos, warned) =
        (h.changes_crowding(vec![(b[3], Asked::Cancel), (b[2], Asked::Reopen)])).expect("the changes are taken");
    assert_eq!(warned, []);
    h.released(todos);
}

#[test]
fn an_act_that_lowers_a_count_or_leaves_one_past_the_most_no_higher_is_never_refused() {
    let mut h = H::new();
    let b = free(&mut h, &["b1", "b2", "b3", "b4"]);
    let (b5, wb5) = h.add(new_item("b5", &[], "short"));
    // Added before any act was held to a number, q has five open children, past the most.
    let (q, _) = h.add(new_item("q", &[b[0], b[1], b[2], b[3], b5], "short"));
    h.hold_children(LIMITS);
    // A change that moves no child, and one of another item's, are done and warn of nothing.
    assert_eq!(
        taken(&mut h, vec![(q, Asked::Item(ItemChange::Title("q, again".into())))]),
        []
    );
    let other = free(&mut h, &["other"])[0];
    assert_eq!(
        taken(&mut h, vec![(other, Asked::Item(ItemChange::Tagged("x".into())))]),
        []
    );
    // Fewer, each still past the most or the warning: a dependency taken away, a child cancelled, a child merged.
    assert_eq!(
        taken(&mut h, vec![(q, Asked::Item(ItemChange::NoLongerDependsOn(b[0])))]),
        []
    );
    let (_, todos, warned) = h
        .changes_crowding(vec![(b[1], Asked::Cancel)])
        .expect("b2 is cancelled");
    assert_eq!(warned, []);
    h.released(todos);
    h.let_on(wb5);
    h.through(wb5);
    // b3 and b4 are left open: b1 again is the third, past the warning and no more than the most.
    assert_eq!(
        taken(&mut h, vec![(q, Asked::Item(ItemChange::DependsOn(b[0])))]),
        [crowded(Some(q), 3, LIMITS.warn)]
    );
}
