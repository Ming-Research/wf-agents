//! The situations an entry of an agent's cell has been found in, for whoever reads what the cell is sent in each: what
//! the search of [`crate::witness`] found of the rounds of works of the flow, as the situations that together cover each
//! line a round stood in, each with a name that says it.
//!
//! A round stands in lines, each of one thing the message depends on ([`stood`]): how the session stands with a datum of
//! the cell's context and whether the datum ran the cell, what ran the cell, what the session holds as the entry begins
//! and whether the last return in it handed anything in, what else the round is given. The text of a datum is the
//! builder's from its kind and from how the session stands with it alone, so that covering the lines covers the texts.
//! A situation is one real round's, which a script of the search runs again to the same situation; the cover picks, for
//! a cell, situations in turn, the one that stands in most lines not yet covered first, the earliest found where
//! two stand in as many, until every line the search found for the cell is covered. Nothing here knows what a flow's
//! cells, blocks or lines are called or when a cell can be entered in a state: a line that no round the search carried
//! stood in is not written, and a line a round stood in is.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use flow::CellSpec;

use crate::message::Situation;
use crate::witness::{Entry, Found};

/// A situation, with a name that says it, its parts joined by `-`, and the witness whose round stood in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub entry: Entry,
    pub situation: Situation,
    /// The place of the witness in [`Found::witnesses`].
    pub witness: usize,
}

/// What a situation of the cell says, as lines each of one thing the tool writes a situation for, apart: how the
/// session stands with each datum, and whether the datum ran the cell; what ran the cell; what the session holds as the
/// entry begins; what else is given. A real round and a written situation are held to the same lines, so that the state
/// of a datum is compared by the cell and whether it ran it, and a state of one datum is not asked of another. A
/// situation does not say its entry: two entries that stand alike are one.
pub fn stood(cell: &CellSpec, s: &Situation) -> Vec<String> {
    let id = cell.id.as_str();
    // A datum ran the cell where it is watched by itself and new; a member of a group stands by its own state alone,
    // the text of a datum being the builder's from its kind and its state, and the group that ran the cell is a line of
    // its own.
    let ran = |block: &str| (cell.watches.iter()).any(|t| !t.is_group() && t.name == block && s.new.contains(&t.name));
    let mut lines: Vec<String> = (s.data.iter())
        .map(|(block, stands)| {
            let ran = if ran(block) { "ran it" } else { "did not run it" };
            format!("{id}: {block} {} and {ran}", stands.name())
        })
        .collect();
    lines.extend(s.new.iter().map(|name| format!("{id}: run by {name}")));
    lines.push(format!(
        "{id}: the session holds {:?}, after a hand-in {}",
        s.holds, s.after_hand_in
    ));
    lines.extend(
        s.results
            .iter()
            .map(|key| format!("{id}: given the result of a call, {key}")),
    );
    // A return given whole again ends its reasons in words of its own, so it is a line of its own, not one beside the line
    // of any other.
    lines.extend(s.sent_back.then(|| match s.whole {
        true => format!("{id}: given why a return was sent back, to give it whole again"),
        false => format!("{id}: given why a return was sent back"),
    }));
    lines.extend(
        s.main
            .iter()
            .map(|key| format!("{id}: given what came of taking main in, {key}")),
    );
    lines
}

/// The name of a situation: how its entry was reached, what ran it, and what else it says apart from how the session
/// stands with each datum, which its lines say.
fn name(entry: Entry, s: &Situation) -> String {
    let mut parts = vec![entry.name().to_string()];
    parts.push(match s.new.is_empty() {
        true => "nothing_new".to_string(),
        false => s.new.join("+"),
    });
    parts.push(match s.holds.is_empty() {
        true => "session_empty".to_string(),
        false => format!("session_holds_{}", s.holds.len()),
    });
    if s.after_hand_in {
        parts.push("after_hand_in".to_string());
    }
    parts.extend(s.main.iter().map(|key| format!("main_{key}")));
    parts.extend(s.results.iter().map(|key| format!("result_{key}")));
    if s.sent_back {
        parts.push(if s.whole { "sent_back_whole" } else { "sent_back" }.to_string());
    }
    parts.join("-")
}

/// The lines of the cell the search found a round stand in.
pub fn witnessed(found: &Found, cell: &CellSpec) -> usize {
    let prefix = format!("{}: ", cell.id);
    found.lines.keys().filter(|line| line.starts_with(&prefix)).count()
}

/// The situations of the cell, as the module says: the fewest the cover finds that stand in every line the search found
/// a round of the cell stand in, each a round the search carried.
pub fn situations(found: &Found, cell: &CellSpec) -> Vec<Named> {
    let own: Vec<usize> = (0..found.witnesses.len())
        .filter(|&w| found.witnesses[w].cell == cell.id)
        .collect();
    let lines: Vec<BTreeSet<String>> = (own.iter())
        .map(|&w| stood(cell, &found.witnesses[w].situation).into_iter().collect())
        .collect();
    let mut left: BTreeSet<&String> = lines.iter().flatten().collect();
    let mut taken: Vec<usize> = Vec::new();
    while !left.is_empty() {
        let covering = |i: usize| lines[i].iter().filter(|line| left.contains(line)).count();
        let best = (0..own.len())
            .filter(|i| !taken.contains(i))
            .max_by_key(|&i| (covering(i), Reverse(i)));
        let Some(best) = best.filter(|&i| covering(i) > 0) else {
            break;
        };
        for line in &lines[best] {
            left.remove(line);
        }
        taken.push(best);
    }
    (taken.into_iter())
        .map(|i| {
            let witness = &found.witnesses[own[i]];
            Named {
                name: name(witness.entry, &witness.situation),
                entry: witness.entry,
                situation: witness.situation.clone(),
                witness: own[i],
            }
        })
        .collect()
}
