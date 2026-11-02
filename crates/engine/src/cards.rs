//! Decision cards: the questions a design puts to the owner, each a revision of the project's records in the shape of
//! the reasoning system's card extension. A design hands its cards in beside its text, each standing in the text as a
//! line `[[card:<key>]]`, and lists the changes to the records that change no decision as edits; the engine checks the
//! anchors against the cards, and each card's revision and each edit against the records the branch changed
//! ([`check`]), posts the cards for the owner, a card handed in again unchanged keeping its row and its ruling
//! ([`post`]), and holds the merge until the owner confirmed every current card and the records to merge are the words
//! the owner ruled on.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::{Connection, Transaction, params};
use serde_json::{Value, json};

use crate::reasoning::{self, Revised};

/// How a card stands in a design's text: a line of its own, `[[card:<key>]]`.
const ANCHOR: &str = "[[card:";

/// A work's current card of a key, open or ruled.
#[derive(Debug, Clone)]
pub struct Card {
    pub id: i64,
    pub key: String,
    pub body: Value,
    pub revision: Value,
    pub rulings: Vec<Value>,
}

impl Card {
    /// The owner's latest ruling on the card, if any.
    pub fn owner_ruling(&self) -> Option<&Value> {
        self.rulings.iter().rev().find(|r| r["by"] == "owner")
    }
}

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn list(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(xs)) => xs.clone(),
        _ => Vec::new(),
    }
}

/// A work's current cards, open or ruled, in the order they were posted.
pub fn current(conn: &Connection, work: i64) -> Result<Vec<Card>, String> {
    let mut st = conn
        .prepare(
            "SELECT id, key, body, revision, rulings FROM design_cards
             WHERE work = ?1 AND status IN ('open', 'ruled') ORDER BY id",
        )
        .map_err(sql)?;
    let rows = st
        .query_map([work], |r| {
            let json = |i: usize| -> db::rusqlite::Result<Value> {
                Ok(serde_json::from_str(&r.get::<_, String>(i)?).unwrap_or(Value::Null))
            };
            Ok(Card {
                id: r.get(0)?,
                key: r.get(1)?,
                body: json(2)?,
                revision: json(3)?,
                rulings: list(Some(&json(4)?)),
            })
        })
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// The keys of the cards a design's text stands them at, each a line of its own; an error for `[[card:` anywhere else.
pub fn anchors(solution: &str) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    for line in solution.lines() {
        let l = line.trim();
        if !l.contains(ANCHOR) {
            continue;
        }
        match l.strip_prefix(ANCHOR).and_then(|k| k.strip_suffix("]]")) {
            Some(key) if !key.contains(ANCHOR) && !key.contains(']') => keys.push(key.to_string()),
            _ => {
                return Err(format!(
                    "a card stands in the design's text as a line of its own, `[[card:<key>]]`, and nowhere else: {l:?}"
                ));
            }
        }
    }
    Ok(keys)
}

/// The records a card revises, as the branch has them: each reference it names with what the branch did to it.
pub fn revision(card: &Value, revised: &BTreeMap<String, Revised>) -> Value {
    Value::Array(
        list(card.get("revises"))
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|r| revised.get(r).map(|x| x.json(r)))
            .collect(),
    )
}

/// What a card says, to tell whether a version handed in again is the same card: every field but what changed, and the
/// records it revises as the branch has them, compared by what was done to each and its words.
fn content(card: &Value, revision: &Value) -> Value {
    let mut c = card.clone();
    if let Some(o) = c.as_object_mut() {
        o.remove("changed");
    }
    let records: Vec<Value> = list(Some(revision))
        .iter()
        .map(|r| json!([r["reference"], r["change"], r["text"]]))
        .collect();
    json!({ "card": c, "records": records })
}

/// What a design hands in of its cards: the cards, the changes to the records that change no decision, the keys it
/// withdraws, and its text.
pub struct Handed<'a> {
    pub cards: &'a [Value],
    pub edits: &'a [Value],
    pub withdrawn: &'a [Value],
    pub solution: &'a str,
}

/// The records a design's edits list: changed in wording, form or place, no decision with them.
pub fn edited(edits: &[Value]) -> BTreeSet<String> {
    edits
        .iter()
        .flat_map(|e| list(e.get("records")))
        .filter_map(|r| r.as_str().map(str::to_string))
        .collect()
}

/// The engine's check of the cards a design hands in, against the records the branch revised and the work's current
/// cards: keys, options and the recommendation, the anchors in the text, every card's revision and every edit a record
/// the branch revised and every record revised on a card or an edit, the keys withdrawn, what changed said exactly when
/// a card changed, and the owner's rulings followed. Whether an edit changes no decision is the coherence review's to
/// judge. `earlier` is the owner's latest choice of an option on each key in the item's other works. Gives why it fails.
pub fn check(
    h: &Handed,
    revised: &BTreeMap<String, Revised>,
    current: &[Card],
    earlier: &BTreeMap<String, String>,
) -> Result<(), String> {
    let mut keys = BTreeSet::new();
    for c in h.cards {
        let key = text(c, "key");
        if !reasoning::is_id(key) {
            return Err(format!(
                "card key {key:?}: lowercase letters and digits, words joined by hyphens"
            ));
        }
        if !keys.insert(key.to_string()) {
            return Err(format!("two cards have the key {key}"));
        }
        let options = list(c.get("options"));
        let names: Vec<&str> = options.iter().map(|o| text(o, "name")).collect();
        if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
            return Err(format!("card {key}: two options have one name"));
        }
        let recommended = text(c, "recommended");
        if !names.contains(&recommended) {
            return Err(format!(
                "card {key}: the recommended option {recommended:?} is not one of its options"
            ));
        }
    }
    let anchored = anchors(h.solution)?;
    let mut seen = BTreeSet::new();
    for k in &anchored {
        if !seen.insert(k.clone()) {
            return Err(format!("[[card:{k}]] stands in the design's text more than once"));
        }
        if !keys.contains(k) {
            return Err(format!(
                "[[card:{k}]] stands in the design's text, and no card has the key {k}"
            ));
        }
    }
    if let Some(k) = keys.iter().find(|k| !seen.contains(*k)) {
        return Err(format!(
            "card {k} does not stand in the design's text: put `[[card:{k}]]` on a line of its own where the design reaches its question"
        ));
    }
    let mut named = BTreeSet::new();
    for c in h.cards {
        let key = text(c, "key");
        for r in list(c.get("revises")) {
            let r = r.as_str().unwrap_or_default();
            if !reasoning::is_reference(r) {
                return Err(format!(
                    "card {key} revises {r:?}: a record is named by its reference, such as D:<id> or F:<id>"
                ));
            }
            if !revised.contains_key(r) {
                return Err(format!(
                    "card {key} revises {r}, which the branch did not add, change or remove"
                ));
            }
            named.insert(r.to_string());
        }
    }
    for (i, e) in h.edits.iter().enumerate() {
        let n = i + 1;
        if text(e, "why").trim().is_empty() {
            return Err(format!("edit {n} says no why"));
        }
        for r in list(e.get("records")) {
            let r = r.as_str().unwrap_or_default();
            if !reasoning::is_reference(r) {
                return Err(format!(
                    "edit {n} lists {r:?}: a record is named by its reference, such as D:<id> or F:<id>"
                ));
            }
            if !revised.contains_key(r) {
                return Err(format!(
                    "edit {n} lists {r}, which the branch did not add, change or remove"
                ));
            }
            if named.contains(r) {
                return Err(format!(
                    "{r} is on a card and listed as an edit: a change on a card changes a decision, an edit none"
                ));
            }
        }
    }
    let edited: BTreeSet<String> = edited(h.edits);
    let unnamed: Vec<&String> = revised
        .keys()
        .filter(|r| !named.contains(*r) && !edited.contains(*r))
        .collect();
    if !unnamed.is_empty() {
        let list: Vec<String> = unnamed
            .iter()
            .map(|r| format!("{r} ({})", revised[*r].change))
            .collect();
        return Err(format!(
            "the branch revised records no card or edit names: {}; every revision is put to the owner on a card, and \
             a change that changes no decision is listed as an edit, saying why",
            list.join(", ")
        ));
    }
    let now: BTreeMap<&str, &Card> = current.iter().map(|c| (c.key.as_str(), c)).collect();
    let mut gone = BTreeSet::new();
    for w in h.withdrawn {
        let key = text(w, "key");
        if keys.contains(key) {
            return Err(format!("card {key} is handed in and withdrawn at once"));
        }
        if !now.contains_key(key) {
            return Err(format!("this work has no card {key} to withdraw"));
        }
        gone.insert(key);
    }
    if let Some(k) = now.keys().find(|k| !keys.contains(**k) && !gone.contains(*k)) {
        return Err(format!(
            "this work put card {k} to the owner, and this design neither hands it in nor withdraws it: hand it in \
             again, changed or not, or withdraw it saying why"
        ));
    }
    for c in h.cards {
        let key = text(c, "key");
        let said = !text(c, "changed").trim().is_empty();
        let recommended = text(c, "recommended");
        if let Some(o) = earlier.get(key)
            && o != recommended
        {
            return Err(format!(
                "the owner chose \"{o}\" on card {key}: the card with that key recommends it"
            ));
        }
        let Some(was) = now.get(key) else {
            if said {
                return Err(format!(
                    "card {key} is new in this work: leave out what changed, which only a later version of a card says"
                ));
            }
            continue;
        };
        let same = content(&was.body, &was.revision) == content(c, &revision(c, revised));
        if let Some(r) = was.owner_ruling() {
            let verdict = text(r, "verdict");
            if verdict == "choose" && text(r, "option") != recommended {
                return Err(format!(
                    "the owner chose \"{}\" on card {key} ({}): the card with that key recommends it",
                    text(r, "option"),
                    was.id
                ));
            }
            if db::ruling_disagrees(r) && same {
                return Err(format!(
                    "the owner disagreed with card {key} ({}): hand in a new version of it that follows the ruling, \
                     saying what changed, or withdraw it",
                    was.id
                ));
            }
        }
        match (same, said) {
            (true, true) => {
                return Err(format!(
                    "card {key} is the same as the one put to the owner before ({}): leave out what changed",
                    was.id
                ));
            }
            (false, false) => {
                return Err(format!(
                    "card {key} differs from the one put to the owner before ({}): say in changed what changed and why",
                    was.id
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// The cards a posting made, replaced and withdrew.
#[derive(Debug, Default)]
pub struct Posted {
    /// Each new card, with the card of its key it replaced and whether the owner had ruled that one.
    pub new: Vec<(i64, Option<(i64, bool)>)>,
    /// Each card withdrawn, and whether the owner had ruled it.
    pub withdrawn: Vec<(i64, bool)>,
}

/// A new open card of a key at the round `round`, with the records it revises and what changed from the card it
/// replaces, which is marked replaced by it; gives its number.
fn new_version(
    tx: &Transaction<'_>,
    round: crate::work::RoundId,
    key: &str,
    body: &Value,
    revision: &Value,
    changed: Option<&str>,
    was: Option<i64>,
) -> Result<i64, String> {
    if let Some(w) = was {
        tx.execute("UPDATE design_cards SET status = 'replaced' WHERE id = ?1", [w])
            .map_err(sql)?;
    }
    tx.execute(
        "INSERT INTO design_cards (work, round, key, body, revision, status, changed)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6)",
        params![
            round.work,
            round.n,
            key,
            body.to_string(),
            revision.to_string(),
            changed
        ],
    )
    .map_err(sql)?;
    let id = tx.last_insert_rowid();
    if let Some(w) = was {
        tx.execute("UPDATE design_cards SET replaced_by = ?2 WHERE id = ?1", params![w, id])
            .map_err(sql)?;
    }
    Ok(id)
}

/// Posts the cards a design handed in, at the round `round` of `work`: a card the same as the work's current card of its
/// key keeps that card, its number and its ruling; another is a new open card that replaces it; a key withdrawn
/// withdraws its card, saying why.
pub fn post(
    tx: &Transaction<'_>,
    round: crate::work::RoundId,
    handed: &Handed,
    revised: &BTreeMap<String, Revised>,
) -> Result<Posted, String> {
    let work = round.work;
    let now = current(tx, work)?;
    let mut posted = Posted::default();
    for c in handed.cards {
        let key = text(c, "key");
        let rev = revision(c, revised);
        let was = now.iter().find(|x| x.key == key);
        if was.is_some_and(|w| content(&w.body, &w.revision) == content(c, &rev)) {
            continue;
        }
        let changed = Some(text(c, "changed").trim()).filter(|s| !s.is_empty());
        let id = new_version(tx, round, key, c, &rev, changed, was.map(|w| w.id))?;
        posted.new.push((id, was.map(|w| (w.id, w.owner_ruling().is_some()))));
    }
    for g in handed.withdrawn {
        let key = text(g, "key");
        let Some(w) = now.iter().find(|x| x.key == key) else {
            continue;
        };
        let why = Some(text(g, "why").trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("no longer asked");
        tx.execute(
            "UPDATE design_cards SET status = 'withdrawn', withdrawn = ?2 WHERE id = ?1",
            params![w.id, why],
        )
        .map_err(sql)?;
        posted.withdrawn.push((w.id, w.owner_ruling().is_some()));
    }
    Ok(posted)
}

/// The owner's latest choice of an option on each key of the item's cards in works other than `work`, by key: what a
/// card of the same key in this work recommends.
pub fn chosen_elsewhere(conn: &Connection, item: i64, work: i64) -> Result<BTreeMap<String, String>, String> {
    let mut st = conn
        .prepare(
            "SELECT c.key, c.rulings FROM design_cards c JOIN works w ON w.id = c.work
             WHERE w.item = ?1 AND c.work <> ?2 ORDER BY c.id",
        )
        .map_err(sql)?;
    let rows: Vec<(String, String)> = st
        .query_map(params![item, work], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    let mut out = BTreeMap::new();
    for (key, rulings) in rows {
        let rulings: Value = serde_json::from_str(&rulings).unwrap_or(Value::Null);
        if let Some(r) = list(Some(&rulings)).iter().rev().find(|r| r["by"] == "owner") {
            if text(r, "verdict") == "choose" {
                out.insert(key, text(r, "option").to_string());
            } else {
                out.remove(&key);
            }
        }
    }
    Ok(out)
}

/// Numbers joined as a sentence lists them: "15", "15 and 16", "15, 16 and 17".
pub fn and_list(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str, revises: &[&str]) -> Value {
        json!({
            "key": key,
            "question": "Which?",
            "options": [
                { "name": "A", "does": "a", "cost": "c", "risks": "r" },
                { "name": "B", "does": "b", "cost": "c", "risks": "r" }
            ],
            "not_workable": [],
            "recommended": "A",
            "confidence": { "level": "3", "why": "w", "overturn": "o" },
            "revises": revises
        })
    }

    fn revised(refs: &[(&str, &'static str)]) -> BTreeMap<String, Revised> {
        refs.iter()
            .map(|(r, change)| {
                (
                    r.to_string(),
                    Revised {
                        change,
                        text: format!("Decision {}: text.", r.trim_start_matches("D:")),
                        before: None,
                    },
                )
            })
            .collect()
    }

    // A card stands in the design's text by its key on a line of its own: every card once, and no anchor without a card.
    #[test]
    fn the_anchors_in_the_text_are_the_cards() {
        let cards = [card("red-run", &["D:a"])];
        let r = revised(&[("D:a", "changed")]);
        let check_text = |solution: &str| {
            check(
                &Handed {
                    cards: &cards,
                    edits: &[],
                    withdrawn: &[],
                    solution,
                },
                &r,
                &[],
                &BTreeMap::new(),
            )
        };
        assert_eq!(check_text("Why.\n\n[[card:red-run]]\n\nThen."), Ok(()));
        assert!(
            check_text("Why.")
                .unwrap_err()
                .contains("does not stand in the design's text")
        );
        assert!(
            check_text("[[card:red-run]]\n[[card:red-run]]")
                .unwrap_err()
                .contains("more than once")
        );
        assert!(
            check_text("[[card:red-run]]\n[[card:other]]")
                .unwrap_err()
                .contains("no card has the key other")
        );
        assert!(
            check_text("See [[card:red-run]] here.")
                .unwrap_err()
                .contains("a line of its own")
        );
    }

    // Every card revises records the branch revised, and every record the branch revised is on a card.
    #[test]
    fn a_card_revises_what_the_branch_revised_and_nothing_is_left_off() {
        let check_cards = |cards: &[Value], r: &BTreeMap<String, Revised>| {
            check(
                &Handed {
                    cards,
                    edits: &[],
                    withdrawn: &[],
                    solution: "[[card:k]]",
                },
                r,
                &[],
                &BTreeMap::new(),
            )
        };
        let r = revised(&[("D:a", "added"), ("F:b", "changed")]);
        assert_eq!(check_cards(&[card("k", &["D:a", "F:b"])], &r), Ok(()));
        assert!(
            check_cards(&[card("k", &["D:a"])], &r)
                .unwrap_err()
                .contains("F:b (changed)")
        );
        assert!(
            check_cards(&[card("k", &["D:a", "F:b", "D:c"])], &r)
                .unwrap_err()
                .contains("D:c, which the branch did not")
        );
        assert!(
            check_cards(&[card("k", &["D:a", "F:b", "a"])], &r)
                .unwrap_err()
                .contains("by its reference")
        );
    }

    // A key the work put to the owner comes back the same, changed saying what changed, or withdrawn; the owner's choice
    // of another option is what the card with that key recommends.
    #[test]
    fn a_key_comes_back_or_is_withdrawn_and_follows_the_ruling() {
        let r = revised(&[("D:a", "changed")]);
        let before = card("k", &["D:a"]);
        let was = Card {
            id: 7,
            key: "k".into(),
            body: before.clone(),
            revision: revision(&before, &r),
            rulings: Vec::new(),
        };
        let none = BTreeMap::new();
        let run = |cards: &[Value], withdrawn: &[Value], was: &Card| {
            check(
                &Handed {
                    cards,
                    edits: &[],
                    withdrawn,
                    solution: if cards.is_empty() { "" } else { "[[card:k]]" },
                },
                if cards.is_empty() { &none } else { &r },
                std::slice::from_ref(was),
                &BTreeMap::new(),
            )
        };
        assert_eq!(run(std::slice::from_ref(&before), &[], &was), Ok(()));
        let mut said = before.clone();
        said["changed"] = json!("Nothing.");
        assert!(run(&[said], &[], &was).unwrap_err().contains("leave out what changed"));
        let mut other = before.clone();
        other["question"] = json!("Which one?");
        assert!(
            run(std::slice::from_ref(&other), &[], &was)
                .unwrap_err()
                .contains("say in changed")
        );
        other["changed"] = json!("The question is plainer.");
        assert_eq!(run(std::slice::from_ref(&other), &[], &was), Ok(()));
        assert!(
            run(&[], &[], &was)
                .unwrap_err()
                .contains("neither hands it in nor withdraws it")
        );
        assert_eq!(run(&[], &[json!({ "key": "k", "why": "Gone." })], &was), Ok(()));
        let chose = Card {
            rulings: vec![json!({ "by": "owner", "verdict": "choose", "option": "B" })],
            ..was.clone()
        };
        assert!(
            run(std::slice::from_ref(&other), &[], &chose)
                .unwrap_err()
                .contains("recommends it")
        );
        let mut follows = other.clone();
        follows["recommended"] = json!("B");
        assert_eq!(run(&[follows], &[], &chose), Ok(()));
        let disagreed = Card {
            rulings: vec![json!({ "by": "owner", "verdict": "disagree", "text": "No." })],
            ..was.clone()
        };
        assert!(
            run(&[before], &[], &disagreed)
                .unwrap_err()
                .contains("follows the ruling")
        );
    }

    // A change to the records that changes no decision is listed as an edit beside the cards, with why, and needs no
    // card: an edit lists records the branch revised, none of them on a card.
    #[test]
    fn a_change_that_changes_no_decision_is_an_edit_with_no_card() {
        let r = revised(&[("D:a", "changed"), ("F:b", "changed")]);
        let check_edits = |cards: &[Value], edits: &[Value]| {
            check(
                &Handed {
                    cards,
                    edits,
                    withdrawn: &[],
                    solution: if cards.is_empty() { "" } else { "[[card:k]]" },
                },
                &r,
                &[],
                &BTreeMap::new(),
            )
        };
        let reworded = json!({ "records": ["F:b"], "why": "Reworded, the same fact." });
        assert_eq!(
            check_edits(&[card("k", &["D:a"])], std::slice::from_ref(&reworded)),
            Ok(())
        );
        let both = json!({ "records": ["D:a", "F:b"], "why": "Moved." });
        assert_eq!(check_edits(&[], std::slice::from_ref(&both)), Ok(()));
        assert!(
            check_edits(&[card("k", &["D:a", "F:b"])], std::slice::from_ref(&reworded))
                .unwrap_err()
                .contains("on a card and listed as an edit")
        );
        let elsewhere = json!({ "records": ["F:b", "D:c"], "why": "Moved." });
        assert!(
            check_edits(&[card("k", &["D:a"])], &[elsewhere])
                .unwrap_err()
                .contains("edit 1 lists D:c, which the branch did not")
        );
        let silent = json!({ "records": ["F:b"], "why": " " });
        assert!(
            check_edits(&[card("k", &["D:a"])], &[silent])
                .unwrap_err()
                .contains("says no why")
        );
        assert!(
            check_edits(&[card("k", &["D:a"])], &[])
                .unwrap_err()
                .contains("no card or edit names: F:b (changed)")
        );
    }

    #[test]
    fn numbers_are_listed_as_a_sentence_lists_them() {
        let s = |xs: &[&str]| and_list(&xs.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(s(&["15"]), "15");
        assert_eq!(s(&["15", "16"]), "15 and 16");
        assert_eq!(s(&["15", "16", "17"]), "15, 16 and 17");
    }
}
