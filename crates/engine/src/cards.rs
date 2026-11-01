//! Decision cards: the questions a design puts to the owner, each a revision of the project's records in the shape of
//! the reasoning system's card extension. A design hands its cards in beside its text, each standing in the text as a
//! line `[[card:<key>]]`, and lists the changes to the records that change no decision as edits; the engine checks the
//! anchors against the cards, and each card's revision and each edit against the records the branch changed
//! ([`check`]), posts the cards for the owner, a card handed in again unchanged keeping its row and its ruling
//! ([`post`]), and holds the merge until the owner confirmed every current card and the records to merge are the words
//! the owner ruled on ([`waiting_for`]). In wf-agents' own repository, one card of a work whose branch changes the
//! flow carries the flow drawn from the branch, and a new version of it is posted when the flow changed since
//! ([`carry_flow`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::git;
use crate::reasoning::{self, Revised};

/// The file of wf-agents' repository that defines its flow, and the database definition the flow is checked against:
/// a branch that changes the flow is drawn from them.
pub const FLOW_FILE: &str = "flow/flow.json";
const SCHEMA_FILE: &str = "db/schema.sql";

/// How a card stands in a design's text: a line of its own, `[[card:<key>]]`.
const ANCHOR: &str = "[[card:";

/// A work's current card of a key, open or ruled.
#[derive(Debug, Clone)]
pub struct Card {
    pub id: i64,
    pub key: String,
    pub status: String,
    pub body: Value,
    pub revision: Value,
    pub rulings: Vec<Value>,
    /// The git object of the flow file the card carries the flow of, when it carries one.
    pub flow_source: Option<String>,
}

impl Card {
    /// The owner's latest ruling on the card, if any.
    pub fn owner_ruling(&self) -> Option<&Value> {
        self.rulings.iter().rev().find(|r| r["by"] == "owner")
    }

    /// Whether the owner's latest ruling confirms it.
    pub fn confirmed(&self) -> bool {
        self.status == "ruled" && self.owner_ruling().is_some_and(|r| db::ruling_confirms(r, 1))
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
            "SELECT id, key, status, body, revision, rulings, flow_source FROM design_cards
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
                status: r.get(2)?,
                body: json(3)?,
                revision: json(4)?,
                rulings: list(Some(&json(5)?)),
                flow_source: r.get(6)?,
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

/// The records the work's latest design listed as edits, which change no decision and need no card.
fn design_edits(conn: &Connection, work: i64) -> Result<BTreeSet<String>, String> {
    let edits: Option<String> = conn
        .query_row(
            &format!(
                "SELECT json_extract(output, '$.outcome.design_tree.edits') FROM rounds
                 WHERE work = ?1 AND cell IN {} AND transition IS NOT NULL ORDER BY n DESC LIMIT 1",
                db::cells_sql(crate::ENGINEER, &["D4"])
            ),
            [work],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?
        .flatten();
    let edits: Value = edits.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    Ok(edited(&list(Some(&edits))))
}

/// What the merge waits for of the work's cards, in words; none when the branch revised no record, or when the owner
/// confirmed every current card and the records to merge are what those cards say, word for word, or listed by the
/// latest design as edits that change no decision; and, in wf-agents' own repository, when the card that carries the
/// work's flow carries it as the branch has it (`own`).
pub fn waiting_for(
    conn: &Connection,
    work: i64,
    wt: &Path,
    project: &config::Project,
) -> Result<Option<String>, String> {
    let fork = git::fork_point(wt, &project.main)?;
    if own_repository(project)
        && let Some((card, _)) = flow_due(conn, work, wt, &fork)?
    {
        return Ok(Some(format!(
            "a new version of decision card {}, carrying the flow as the branch now has it",
            card.id
        )));
    }
    let Some(records) = &project.records else {
        return Ok(None);
    };
    let revised = reasoning::revised(wt, &fork, "HEAD", &records.root)?;
    if revised.is_empty() {
        return Ok(None);
    }
    let cards = current(conn, work)?;
    let edits = design_edits(conn, work)?;
    if cards.is_empty() && revised.keys().any(|r| !edits.contains(r)) {
        return Ok(Some("decision cards for the change to the records".to_string()));
    }
    let open: Vec<String> = cards
        .iter()
        .filter(|c| !c.confirmed())
        .map(|c| c.id.to_string())
        .collect();
    if !open.is_empty() {
        return Ok(Some(format!(
            "your ruling on decision card{} {}",
            if open.len() == 1 { "" } else { "s" },
            and_list(&open)
        )));
    }
    let mut named = BTreeSet::new();
    for c in &cards {
        for r in list(Some(&c.revision)) {
            let reference = text(&r, "reference").to_string();
            let now = revised.get(&reference);
            if now.is_none_or(|x| x.change != text(&r, "change") || x.text != text(&r, "text")) {
                return Ok(Some(format!(
                    "a new design: {reference} on the branch is not as decision card {} put it to you",
                    c.id
                )));
            }
            named.insert(reference);
        }
    }
    Ok(revised
        .keys()
        .find(|r| !named.contains(*r) && !edits.contains(*r))
        .map(|r| format!("a new design: no decision card puts {r} to you")))
}

/// Numbers joined as a sentence lists them: "15", "15 and 16", "15, 16 and 17".
pub fn and_list(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A repository's name as `owner/name`, whichever way it is written: a GitHub address or the name itself.
fn repository_name(repo: &str) -> String {
    let r = repo.trim().trim_end_matches('/');
    let r = r.strip_suffix(".git").unwrap_or(r);
    let r = [
        "https://github.com/",
        "http://github.com/",
        "git@github.com:",
        "github.com/",
    ]
    .iter()
    .find_map(|p| r.strip_prefix(p))
    .unwrap_or(r);
    r.to_ascii_lowercase()
}

/// Whether a project's repository is wf-agents' own, the repository this program was built from: the only one whose
/// flow file is the flow. A file at that path in another project's repository is that project's own.
pub fn own_repository(project: &config::Project) -> bool {
    repository_name(&project.repo) == repository_name(env!("CARGO_PKG_REPOSITORY"))
}

/// The card that carries a work's flow, and the git object of the flow file as the branch has it, when the branch
/// changes the flow and that card does not carry it as it is now: the current card that carries a flow, else the
/// work's first current card. None when the branch does not change the flow, the work has no card, or the card carries
/// the flow as it is.
pub fn flow_due(conn: &Connection, work: i64, wt: &Path, fork: &str) -> Result<Option<(Card, String)>, String> {
    if !git::changed(wt, fork, "HEAD")?.iter().any(|f| f == FLOW_FILE) {
        return Ok(None);
    }
    let cards = current(conn, work)?;
    let carrier = cards
        .iter()
        .rev()
        .find(|c| c.flow_source.is_some())
        .or_else(|| cards.first());
    let Some(card) = carrier else { return Ok(None) };
    let source = git::git(wt, &["rev-parse", &format!("HEAD:{FLOW_FILE}")])?
        .trim()
        .to_string();
    Ok((card.flow_source.as_deref() != Some(source.as_str())).then(|| (card.clone(), source)))
}

/// What the engine says on a card it posts again because the flow changed in implementation.
pub const FLOW_CHANGED: &str = "The flow changed in implementation: this version carries the flow as the branch now has it; the rest is as before.";

/// Has the card that carries a work's flow carry it as the branch has it now, `in_force` being the flow version in force:
/// a card posted in this same round (`fresh`) takes it as it is posted; another is posted again under its key as a new
/// version, saying the flow changed in implementation, which the owner rules before the merge. Gives the card that
/// carries it and, when the flow does not compile, why, the card then carrying no drawing. Nothing when no card is due.
pub fn carry_flow(
    tx: &Transaction<'_>,
    round: crate::work::RoundId,
    wt: &Path,
    fork: &str,
    in_force: i64,
    fresh: &[i64],
) -> Result<Option<(i64, Option<String>)>, String> {
    let Some((card, source)) = flow_due(tx, round.work, wt, fork)? else {
        return Ok(None);
    };
    let (drawn, failed) = match snapshot(tx, wt, fork) {
        Ok(s) => (s, None),
        Err(why) => (None, Some(why)),
    };
    let id = if fresh.contains(&card.id) {
        card.id
    } else {
        let mut body = card.body.clone();
        body["changed"] = json!(FLOW_CHANGED);
        new_version(
            tx,
            round,
            &card.key,
            &body,
            &card.revision,
            Some(FLOW_CHANGED),
            Some(card.id),
        )?
    };
    tx.execute(
        "UPDATE design_cards SET flow_snapshot = ?2, flow_in_force = ?3, flow_source = ?4 WHERE id = ?1",
        params![id, drawn, drawn.map(|_| in_force), source],
    )
    .map_err(sql)?;
    Ok(Some((id, failed)))
}

/// The flow as the branch has it at its head, when the branch changed it against the fork point `fork`: compiled from
/// its flow file against its database definition, as the flow page draws a flow at a ref, and stored once as a
/// snapshot. Gives the snapshot; none when the branch does not change the flow; an error when its flow does not compile.
pub fn snapshot(tx: &Transaction<'_>, wt: &Path, fork: &str) -> Result<Option<i64>, String> {
    if !git::changed(wt, fork, "HEAD")?.iter().any(|f| f == FLOW_FILE) {
        return Ok(None);
    }
    let text = git::git(wt, &["show", &format!("HEAD:{FLOW_FILE}")])?;
    let schema = git::git(wt, &["show", &format!("HEAD:{SCHEMA_FILE}")])?;
    let at = |e| format!("{SCHEMA_FILE} on the branch: {e}");
    let relations = db::relations(&schema).map_err(at)?;
    let tables = db::index_of(&schema).map_err(at)?;
    let compiled = flow::compile_shown(&text, &flow::Context { relations, tables })
        .map_err(|e| format!("{FLOW_FILE} on the branch does not compile:\n{e}"))?;
    let digest = compiled.digest();
    tx.execute(
        "INSERT INTO flow_snapshots (digest, compiled) VALUES (?1, ?2) ON CONFLICT (digest) DO NOTHING",
        params![digest, compiled.json()],
    )
    .map_err(sql)?;
    tx.query_row("SELECT id FROM flow_snapshots WHERE digest = ?1", [&digest], |r| {
        r.get(0)
    })
    .optional()
    .map_err(sql)
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
            status: "open".into(),
            body: before.clone(),
            revision: revision(&before, &r),
            rulings: Vec::new(),
            flow_source: None,
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
            status: "ruled".into(),
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
            status: "ruled".into(),
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

    // In wf-agents' own repository, one card of a work whose branch changes the flow carries the flow as the branch had
    // it, compiled and kept in the database beside the version in force, so that the owner's page draws it after the
    // branch is gone; when the flow changed since, as in implementation, a new version of that card carries it as the
    // branch now has it, saying so, and waits for the owner's ruling. A branch that does not change the flow gives none.
    #[test]
    fn the_card_that_carries_the_flow_is_posted_again_when_the_flow_changed() {
        use db::{Actor, Db};
        let dir = std::env::temp_dir().join(format!("wf-agents-snapshot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |path: &str, text: &str| {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let run = |args: &[&str]| -> String {
            let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
            all.extend(args);
            git::git(&dir, &all).unwrap().trim().to_string()
        };
        put("README.md", "A project.\n");
        run(&["init", "-q"]);
        run(&["add", "-A"]);
        run(&["commit", "-qm", "main"]);
        let fork = run(&["rev-parse", "HEAD"]);
        let mut db = Db::in_memory().unwrap();
        let card = json!({ "key": "red-run", "question": "Which?", "options": [{ "name": "A" }, { "name": "B" }],
                           "recommended": "A", "revises": [] });
        let round = |n: i64| crate::work::RoundId::new(1, n);
        let posted = db
            .write(Actor::Engine, |tx| {
                tx.execute("INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')", [])?;
                tx.execute(
                    "INSERT INTO works (id, item, flow_version, graph, at) VALUES (1, NULL, 1, 'engineer', 'D15')",
                    [],
                )?;
                for n in [1, 2] {
                    tx.execute(
                        "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'D4', '{}', 'c', 1)",
                        [n],
                    )?;
                }
                let handed = Handed {
                    cards: std::slice::from_ref(&card),
                    edits: &[],
                    withdrawn: &[],
                    solution: "[[card:red-run]]",
                };
                post(tx, round(1), &handed, &BTreeMap::new()).map_err(db::Error::Abort)
            })
            .unwrap();
        let fresh: Vec<i64> = posted.new.iter().map(|x| x.0).collect();
        let carried = |db: &mut Db, n: i64, fresh: &[i64]| {
            db.write(Actor::Engine, |tx| {
                carry_flow(tx, round(n), &dir, &fork, 1, fresh).map_err(db::Error::Abort)
            })
            .unwrap()
        };
        // The branch does not change the flow: no card carries one.
        assert_eq!(carried(&mut db, 1, &fresh), None);
        // The branch renames the title of cell R1: the card just posted takes the flow as it is posted.
        let renamed = |title: &str| {
            let to = format!("\"title\": \"{title}\"");
            let r = flow::FLOW.replacen("\"title\": \"Research the problem itself\"", &to, 1);
            assert_ne!(r, flow::FLOW, "the flow has cell R1's title");
            r
        };
        put(FLOW_FILE, &renamed("Research the problem, drawn"));
        put(SCHEMA_FILE, db::SCHEMA);
        run(&["add", "-A"]);
        run(&["commit", "-qm", "the flow drawn"]);
        assert_eq!(carried(&mut db, 1, &fresh), Some((fresh[0], None)));
        assert!(flow_due(db.conn(), 1, &dir, &fork).unwrap().is_none());
        // Implementation changes the flow again: the card is posted again under its key, saying so.
        put(FLOW_FILE, &renamed("Research the problem, redrawn"));
        run(&["commit", "-qam", "the flow changed in implementation"]);
        assert_eq!(
            flow_due(db.conn(), 1, &dir, &fork).unwrap().map(|x| x.0.id),
            Some(fresh[0])
        );
        let (again, failed) = carried(&mut db, 2, &[]).unwrap();
        assert_eq!(failed, None);
        assert_ne!(again, fresh[0]);
        let now = current(db.conn(), 1).unwrap();
        assert_eq!((now.len(), now[0].id, now[0].key.as_str()), (1, again, "red-run"));
        assert_eq!(now[0].body["changed"], FLOW_CHANGED);
        assert!(flow_due(db.conn(), 1, &dir, &fork).unwrap().is_none());
        // Both flows are drawn from the database once the branch is gone.
        let _ = std::fs::remove_dir_all(&dir);
        let drawn = |card: i64| -> String {
            let (snapshot, in_force): (i64, i64) = db
                .conn()
                .query_row(
                    "SELECT flow_snapshot, flow_in_force FROM design_cards WHERE id = ?1",
                    [card],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(in_force, 1);
            let d = api::flow_snapshot(db.conn(), snapshot).unwrap();
            d.compiled.graph("engineer").unwrap().cell("R1").unwrap().title.clone()
        };
        assert_eq!(drawn(fresh[0]), "Research the problem, drawn");
        assert_eq!(drawn(again), "Research the problem, redrawn");
    }

    // The flow is wf-agents' own: only a project whose repository is the one this program was built from has its flow
    // file drawn.
    #[test]
    fn only_wf_agents_own_repository_has_its_flow_drawn() {
        let project = |repo: &str| -> config::Project {
            serde_json::from_value(json!({
                "repo": repo, "checkout": "/c", "design_goal": "simple", "test_paths": ["tests/"],
                "ci": { "fast": "ci", "full": ["ci"], "merge_checks": ["check"] }
            }))
            .unwrap()
        };
        assert!(own_repository(&project("Ming-Research/wf-agents")));
        assert!(own_repository(&project(
            "https://github.com/Ming-Research/wf-agents.git"
        )));
        assert!(!own_repository(&project("Ming-Research/whitefoot")));
        assert!(!own_repository(&project("o/p")));
    }

    // The merge waits while the branch revised records and a current card of the work is not confirmed by the owner,
    // naming the cards, and while the records to merge are not word for word what the cards put to the owner; a branch
    // that revised none waits for nothing.
    #[test]
    fn the_merge_waits_for_every_card_confirmed_and_the_records_as_ruled() {
        use db::{Actor, Db};
        let dir = std::env::temp_dir().join(format!("wf-agents-gate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |path: &str, text: &str| {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let run = |args: &[&str]| -> String {
            let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
            all.extend(args);
            git::git(&dir, &all).unwrap().trim().to_string()
        };
        put("r/derivations/runs.md", "Decision kept: K.\n");
        run(&["init", "-q", "-b", "main"]);
        run(&["add", "-A"]);
        run(&["commit", "-qm", "main"]);
        // The fork point is main's head as the remote has it.
        run(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        run(&["checkout", "-q", "-b", "work"]);
        let project: config::Project = serde_json::from_value(json!({
            "repo": "o/p", "checkout": dir, "design_goal": "simple", "test_paths": ["tests/"],
            "ci": { "fast": "ci", "full": ["ci"], "merge_checks": ["check"] }, "records": { "root": "r" }
        }))
        .unwrap();
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (id, item, flow_version, graph, at) VALUES (1, NULL, 1, 'engineer', 'M1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'D4', '{}', 'c', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let waits = |db: &Db| waiting_for(db.conn(), 1, &dir, &project).unwrap();
        assert_eq!(waits(&db), None);
        put(
            "r/derivations/runs.md",
            "Decision kept: K.\n\nDecision red-run: Run every test.\n",
        );
        put("r/facts/ci.md", "Fact cancelled: A cancelled test leaves no line.\n");
        run(&["add", "-A"]);
        run(&["commit", "-qm", "revise"]);
        assert_eq!(
            waits(&db).as_deref(),
            Some("decision cards for the change to the records")
        );
        let revised = reasoning::revised(&dir, "main", "HEAD", "r").unwrap();
        let cards = [card("red-run", &["D:red-run"]), card("cancelled", &["F:cancelled"])];
        db.write(Actor::Engine, |tx| {
            let handed = Handed {
                cards: &cards,
                edits: &[],
                withdrawn: &[],
                solution: "",
            };
            post(tx, crate::work::RoundId::new(1, 1), &handed, &revised).map_err(db::Error::Abort)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(waits(&db).as_deref(), Some("your ruling on decision cards 1 and 2"));
        let confirm = |db: &mut Db, id: i64| {
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "UPDATE design_cards SET status = 'ruled', rulings = '[{\"by\":\"owner\",\"verdict\":\"confirm\"}]'
                     WHERE id = ?1",
                    [id],
                )?;
                Ok(())
            })
            .unwrap();
        };
        confirm(&mut db, 1);
        assert_eq!(waits(&db).as_deref(), Some("your ruling on decision card 2"));
        confirm(&mut db, 2);
        assert_eq!(waits(&db), None);
        // A record reworded after the ruling is not what the owner ruled on.
        put(
            "r/facts/ci.md",
            "Fact cancelled: A cancelled test leaves no line at all.\n",
        );
        run(&["commit", "-qam", "reword"]);
        assert_eq!(
            waits(&db).as_deref(),
            Some("a new design: F:cancelled on the branch is not as decision card 2 put it to you")
        );
        // A record the design lists as an edit, changing no decision, needs no card.
        run(&["checkout", "-q", "HEAD~1", "--", "r/facts/ci.md"]);
        put(
            "r/derivations/runs.md",
            "Decision kept: K, worded better.\n\nDecision red-run: Run every test.\n",
        );
        run(&["commit", "-qam", "reword kept"]);
        assert_eq!(
            waits(&db).as_deref(),
            Some("a new design: no decision card puts D:kept to you")
        );
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET transition = 'done', output = json_object('outcome', json_object('design_tree',
                   json_object('edits', json_array(json_object('records', json_array('D:kept'), 'why', 'Worded better.')))))
                 WHERE work = 1 AND n = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(waits(&db), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn numbers_are_listed_as_a_sentence_lists_them() {
        let s = |xs: &[&str]| and_list(&xs.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(s(&["15"]), "15");
        assert_eq!(s(&["15", "16"]), "15 and 16");
        assert_eq!(s(&["15", "16", "17"]), "15, 16 and 17");
    }
}
