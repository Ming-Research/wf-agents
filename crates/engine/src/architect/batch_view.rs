//! The batch of item changes as its review is given it, and the architect's revision of it is held to it. Before each
//! look of the review the engine tries the batch out ([`stage`]): it writes the changes in order in a savepoint it takes
//! back, reads what each wrote from `changes`, and keeps in `architect_batch_views` what each change did, how the items
//! the batch names stand after them and where they stand in their project's order. The review is given that, and each
//! change as it stands against the look before: new, changed (with what changed), with another effect than it had, or
//! the same. The revision names every change by its line and says why it leaves one out, and the checks of both hold
//! them to it.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Map, Value, json};

use super::{
    CLOSE_OBSERVATION, Change, REVIEW, ROWS, batch_start, closing, collected, ids_of, items, kept_change, names_key,
    sql, text,
};
use crate::arch_changes::{self, Trial};
use crate::inputs::line_diff;
use crate::work::{RoundId, query_json, query_json_counted};

/// How many characters a text keeps whole when a field changes; one longer is shown as the sentences that differ.
const WHOLE: usize = 240;

/// The most items the list of the ones ready to start gives.
const READY_MOST: usize = 100;

/// What the latest look of the review found of a change: the round, the verdict (`ok`, `fix`, `drop` or
/// `close_observation`) and the review's row.
#[derive(Debug, Clone)]
pub(super) struct Verdict {
    pub round: i64,
    pub verdict: String,
    pub row: Value,
}

/// What the review found of each change of an architect work's batch at the latest look that gave a row for it, by line.
/// A change the review gave no row at a look stands as the look before found it.
pub(super) fn last_verdicts(conn: &Connection, work: i64) -> Result<BTreeMap<i64, Verdict>, String> {
    let rounds = query_json(
        conn,
        "SELECT n, json_extract(output, '$.outcome.review') AS review FROM rounds
         WHERE work = ?1 AND cell = ?2 AND n > ?3 AND transition IS NOT NULL ORDER BY n",
        &[&work, &REVIEW, &batch_start(conn, work)?],
        ROWS,
    )?;
    let mut out = BTreeMap::new();
    for r in rounds {
        let round = r["n"].as_i64().unwrap_or_default();
        for row in items(r.get("review")) {
            if let Some(line) = row["change"].as_i64() {
                let verdict = text(&row["verdict"]).to_string();
                out.insert(line, Verdict { round, verdict, row });
            }
        }
    }
    Ok(out)
}

/// What a look at a batch left in `architect_batch_views`.
#[derive(Debug, Clone)]
pub(super) struct Stored {
    /// By line: the change and its effect at that look, and the digest of the effect.
    pub lines: Value,
    /// Where each change stands against the look before, by line, and the changes that look had and this has not.
    pub states: Value,
    /// What the changes make of the items.
    pub after: Value,
}

/// The look at an architect work's batch made for its latest review; none before the first, and none for one made in an
/// earlier batch of the work.
pub(super) fn stored(conn: &Connection, work: i64) -> Result<Option<Stored>, String> {
    let row = query_json(
        conn,
        "SELECT round, lines, states, after FROM architect_batch_views WHERE work = ?1",
        &[&work],
        1,
    )?;
    let Some(r) = row.into_iter().next() else {
        return Ok(None);
    };
    let round = r["round"].as_i64().unwrap_or_default();
    if round <= batch_start(conn, work)? {
        return Ok(None);
    }
    Ok(Some(Stored {
        lines: r["lines"].clone(),
        states: r["states"].clone(),
        after: r["after"].clone(),
    }))
}

/// Forgets the look at an architect work's batch, once the batch is over.
pub(super) fn forget(tx: &Transaction<'_>, work: i64) -> Result<(), String> {
    tx.execute("DELETE FROM architect_batch_views WHERE work = ?1", [work])
        .map_err(sql)?;
    Ok(())
}

// Trying the batch out.

/// Tries the batch out and keeps what came of it for the review `round` goes to: each change's effect, how each stands
/// against the look before, and how the items stand after them all. Nothing the changes write stays.
pub(super) fn stage(
    tx: &Transaction<'_>,
    round: RoundId,
    project: &str,
    projects: &BTreeMap<String, config::Project>,
) -> Result<(), String> {
    let work = round.work;
    let rows = collected(tx, work)?;
    let changes: Vec<Value> = rows.iter().map(|r| r.change.clone()).collect();
    let (trials, after) = arch_changes::tried(tx, &changes, projects, |tx, trials| {
        after_state(tx, project, &changes, trials)
    })?;
    let made: BTreeMap<i64, String> = trials
        .iter()
        .flat_map(|t| t.created.iter().map(|(key, id)| (*id, key.clone())))
        .collect();
    let previous = stored(tx, work)?;
    let was = previous.as_ref().map(|p| &p.lines);
    let mut lines = Map::new();
    let mut states = Map::new();
    for (row, trial) in rows.iter().zip(&trials) {
        let (effect, digest) = effect_of(trial, &row.change, &made);
        let key = row.line.to_string();
        let state = match was.and_then(|w| w.get(&key)) {
            None => json!({ "state": "new" }),
            Some(w) if w["change"] != row.change => {
                json!({ "state": "changed", "diff": change_diff(&w["change"], &row.change) })
            }
            Some(w) if w["digest"] != json!(digest) => json!({ "state": "effect_changed", "effect_was": w["effect"] }),
            Some(_) => json!({ "state": "unchanged" }),
        };
        states.insert(key.clone(), state);
        lines.insert(key, json!({ "change": row.change, "effect": effect, "digest": digest }));
    }
    let mut removed = Vec::new();
    for (key, w) in was.and_then(Value::as_object).into_iter().flatten() {
        if lines.contains_key(key) {
            continue;
        }
        let line: i64 = key.parse().unwrap_or_default();
        removed.push(json!({
            "change": line,
            "state": "removed",
            "kind": w["change"]["kind"],
            "why": why_removed(tx, work, line)?,
        }));
    }
    let states = json!({ "lines": states, "removed": removed });
    tx.execute(
        "INSERT INTO architect_batch_views (work, round, lines, states, after) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (work) DO UPDATE SET round = ?2, lines = ?3, states = ?4, after = ?5",
        params![
            work,
            round.n,
            Value::Object(lines).to_string(),
            states.to_string(),
            after.to_string()
        ],
    )
    .map_err(sql)?;
    Ok(())
}

/// Why a change the look before had is not in the batch now, as the revision or the engine said it when it left it out.
fn why_removed(conn: &Connection, work: i64, line: i64) -> Result<String, String> {
    let said: Option<Option<String>> = conn
        .query_row(
            "SELECT json_extract(outcome, '$.what') FROM architect_item_changes
             WHERE work = ?1 AND line = ?2 AND round > ?3 AND state IN ('revised', 'dropped') ORDER BY id DESC LIMIT 1",
            params![work, line, batch_start(conn, work)?],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(said
        .flatten()
        .unwrap_or_else(|| "The revision left it out, and said nothing of why".to_string()))
}

/// A number or a key as a change names an item, as the item's number, by the keys the batch's creates made.
fn id_named(v: &Value, keys: &BTreeMap<String, i64>) -> Option<i64> {
    let mut ids = BTreeSet::new();
    ids_of(v, &mut ids);
    ids.into_iter()
        .next()
        .or_else(|| v.as_str().and_then(|k| keys.get(k.trim()).copied()))
}

/// The items a change names, as numbers, in the order its fields come: the item it changes, the one it waits for, keeps
/// or absorbs, splits from, moves next to or under, or records a decision in, and the one that fixes a known red check.
fn named_in(c: &Value, keys: &BTreeMap<String, i64>) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    let mut add = |v: &Value| {
        if let Some(id) = id_named(v, keys).filter(|id| !out.contains(id)) {
            out.push(id);
        }
    };
    for k in ["item", "keep", "on", "fix_item"] {
        add(&c[k]);
    }
    add(&c["under"]["item"]);
    add(&c["to"]["item"]);
    add(&c["decision"]["item"]);
    for a in items(c.get("absorbed")) {
        add(&a);
    }
    out
}

/// An item as the changes name it: its number, or the key the batch makes it under.
fn label(id: i64, made: &BTreeMap<i64, String>) -> Value {
    made.get(&id).map_or_else(|| json!(id), |key| json!(key))
}

/// A text, or a list of texts as lines of one, which a field's change is shown by when it is long.
fn as_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(xs) if xs.iter().all(Value::is_string) => {
            Some(xs.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
        }
        _ => None,
    }
}

/// How a field changed: `[was, now]`, or, for a long text, the sentences that differ.
fn shown(was: &Value, now: &Value) -> Value {
    match (as_text(was), as_text(now)) {
        (Some(a), Some(b)) if a.chars().count().max(b.chars().count()) > WHOLE => {
            json!({ "diff": line_diff(&a, &b) })
        }
        _ => json!([was, now]),
    }
}

/// The fields of a row that differ between two versions of it, each as [`shown`] has it.
fn differing(old: &Value, new: &Value) -> Map<String, Value> {
    let empty = Map::new();
    let (o, n) = (old.as_object().unwrap_or(&empty), new.as_object().unwrap_or(&empty));
    let mut out = Map::new();
    for (k, v) in n {
        let was = o.get(k).unwrap_or(&Value::Null);
        if was != v {
            out.insert(k.clone(), shown(was, v));
        }
    }
    out
}

/// What differs between two versions of a change, by field: the sentences that differ for a text, else what it was and
/// what it is.
fn change_diff(then: &Value, now: &Value) -> Value {
    let empty = Map::new();
    let (a, b) = (then.as_object().unwrap_or(&empty), now.as_object().unwrap_or(&empty));
    let mut out = Map::new();
    for k in a.keys().chain(b.keys()) {
        let (x, y) = (a.get(k).unwrap_or(&Value::Null), b.get(k).unwrap_or(&Value::Null));
        if x == y || out.contains_key(k) {
            continue;
        }
        let d = match (as_text(x), as_text(y)) {
            (Some(p), Some(q)) => json!(line_diff(&p, &q)),
            _ => json!({ "was": x, "now": y }),
        };
        out.insert(k.clone(), d);
    }
    Value::Object(out)
}

/// What one change did, as the review is shown it, with the digest of everything it wrote: `nothing` when it wrote no
/// row (a move to where the item is, a hold with the text it has), `changes` with the rows it wrote, `refused` or
/// `failed` with why. Of the items it only moved along, by renumbering the rank of its project's items, the count is
/// shown; the digest holds them all, so that a change whose effect moved them otherwise is found to have another one.
fn effect_of(trial: &Trial, change: &Value, made: &BTreeMap<i64, String>) -> (Value, String) {
    if trial.result != "made" {
        let effect = json!({ "result": trial.result, "why": trial.why });
        let digest = db::content_hash(&effect.to_string());
        return (effect, digest);
    }
    let mut about: BTreeSet<i64> = trial.created.iter().map(|(_, id)| *id).collect();
    for k in ["item", "keep"] {
        ids_of(&change[k], &mut about);
    }
    let mut wrote: Vec<Value> = Vec::new();
    let mut reranked: Vec<Value> = Vec::new();
    for row in &trial.rows {
        let (old, new) = (&row["old_row"], &row["new_row"]);
        match text(&row["table_name"]) {
            "items" => {
                let id = row["row_key"]["id"].as_i64().unwrap_or_default();
                if old.is_null() {
                    let mut item = Map::new();
                    for k in ["title", "level", "deliver", "route"] {
                        item.insert(k.to_string(), new[k].clone());
                    }
                    wrote.push(json!({ "item": label(id, made), "created": item }));
                } else if new.is_null() {
                    wrote.push(json!({ "item": label(id, made), "deleted": true }));
                } else {
                    let fields = differing(old, new);
                    if fields.len() == 1 && fields.contains_key("rank") && !about.contains(&id) {
                        reranked.push(json!([id, old["rank"], new["rank"]]));
                    } else {
                        wrote.push(json!({ "item": label(id, made), "fields": fields }));
                    }
                }
            }
            "item_links" => {
                let (link, of) = if old.is_null() {
                    ("added", new)
                } else {
                    ("removed", old)
                };
                let (item, on) = (
                    of["item"].as_i64().unwrap_or_default(),
                    of["depends_on"].as_i64().unwrap_or_default(),
                );
                wrote.push(json!({
                    "link": link, "item": label(item, made), "depends_on": label(on, made), "kind": of["kind"],
                }));
            }
            table => {
                let key = &row["row_key"];
                wrote.push(if old.is_null() {
                    json!({ "table": table, "key": key, "added": new })
                } else if new.is_null() {
                    json!({ "table": table, "key": key, "removed": old })
                } else {
                    json!({ "table": table, "key": key, "fields": differing(old, new) })
                });
            }
        }
    }
    let mut effect = Map::new();
    let result = if wrote.is_empty() && reranked.is_empty() {
        "nothing"
    } else {
        "changes"
    };
    effect.insert("result".into(), json!(result));
    if !wrote.is_empty() {
        effect.insert("wrote".into(), json!(wrote));
    }
    if !reranked.is_empty() {
        effect.insert("others_reranked".into(), json!(reranked.len()));
    }
    let effect = Value::Object(effect);
    let digest = db::content_hash(&json!({ "effect": effect, "reranked": reranked }).to_string());
    (effect, digest)
}

/// The items of a project that wait to start, in the order the architect gave: not cancelled, and no work has started on
/// them. Of each: its number, title, whether it is held, and its rank.
fn waiting(conn: &Connection, project: &str) -> Result<Vec<Value>, String> {
    query_json(
        conn,
        "SELECT i.id, i.title, i.held IS NOT NULL AS held, i.rank FROM items i
         WHERE i.project = ?1 AND i.cancelled IS NULL AND NOT EXISTS (SELECT 1 FROM works w WHERE w.item = i.id)
         ORDER BY i.rank NULLS LAST, i.id",
        &[&project],
        1_000_000,
    )
}

/// An item of the order as its neighbours are shown: its number, title and whether it is held.
fn neighbour(x: Option<&Value>) -> Value {
    match x {
        Some(x) => json!({ "item": x["id"], "title": x["title"], "held": x["held"].as_i64() == Some(1) }),
        None => Value::Null,
    }
}

/// How the items the changes name stand after them all, called in the savepoint that takes them back: each item they
/// name or make, with the fields that differ from before (and what they were), its rank before and after, and, among
/// the items of its project that wait to start, how many stand ahead of it and how many of those are held, and its
/// neighbours; and the project's items ready to start, in the order they start.
fn after_state(tx: &Transaction<'_>, project: &str, changes: &[Value], trials: &[Trial]) -> Result<Value, String> {
    let made: BTreeMap<i64, String> = trials
        .iter()
        .flat_map(|t| t.created.iter().map(|(key, id)| (*id, key.clone())))
        .collect();
    let keys: BTreeMap<String, i64> = made.iter().map(|(id, key)| (key.clone(), *id)).collect();
    // The items the changes are about, in the order they come up.
    let mut named: Vec<i64> = Vec::new();
    for (c, t) in changes.iter().zip(trials) {
        let created = t.created.iter().map(|(_, id)| *id);
        for id in created.chain(named_in(c, &keys)) {
            if !named.contains(&id) {
                named.push(id);
            }
        }
    }
    // What each item was before the changes and is after, by the first and the last row written of it.
    let mut first: BTreeMap<i64, Value> = BTreeMap::new();
    for row in trials.iter().flat_map(|t| &t.rows) {
        if text(&row["table_name"]) == "items"
            && let Some(id) = row["row_key"]["id"].as_i64()
        {
            first.entry(id).or_insert_with(|| row["old_row"].clone());
        }
    }
    let mut out = Map::new();
    let mut orders: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for id in named {
        let now = query_json(
            tx,
            "SELECT project, title, rank, held, cancelled, level, route FROM items WHERE id = ?1",
            &[&id],
            1,
        )?;
        let Some(now) = now.into_iter().next() else { continue };
        let was = first.get(&id).cloned().unwrap_or_else(|| now.clone());
        let mut item = Map::new();
        item.insert("title".into(), now["title"].clone());
        if was.is_null() {
            // Made by the batch: its texts are in the change that makes it.
            item.insert(
                "created".into(),
                json!({ "level": now["level"], "route": now["route"] }),
            );
        } else {
            let mut fields = differing(&was, &now);
            fields.remove("rank");
            if !fields.is_empty() {
                item.insert("changed".into(), Value::Object(fields));
            }
        }
        item.insert("rank".into(), json!({ "was": was["rank"], "now": now["rank"] }));
        for k in ["held", "cancelled"] {
            if !now[k].is_null() {
                item.insert(k.into(), now[k].clone());
            }
        }
        let order = match orders.get(text(&now["project"])) {
            Some(o) => o.clone(),
            None => {
                let o = waiting(tx, text(&now["project"]))?;
                orders.insert(text(&now["project"]).to_string(), o.clone());
                o
            }
        };
        match order.iter().position(|x| x["id"].as_i64() == Some(id)) {
            Some(at) => {
                let ahead = &order[..at];
                item.insert(
                    "queue".into(),
                    json!({
                        "ahead": ahead.len(),
                        "ahead_held": ahead.iter().filter(|x| x["held"].as_i64() == Some(1)).count(),
                        "previous": neighbour(at.checked_sub(1).and_then(|p| order.get(p))),
                        "next": neighbour(order.get(at + 1)),
                    }),
                );
            }
            None => {
                item.insert(
                    "queue".into(),
                    json!("not waiting to start: cancelled, or a work has started on it"),
                );
            }
        }
        out.insert(label_text(id, &made), Value::Object(item));
    }
    // The items that start next, in the order the view gives them: the owner's queue first, then the architect's order.
    let counted = query_json_counted(
        tx,
        "SELECT item, title, level, position, rank FROM ready_items WHERE project = ?1",
        &[&project],
        READY_MOST,
    )?;
    Ok(json!({
        "items": out,
        "ready": { "items": counted.rows, "more": counted.left_out },
    }))
}

/// An item as its place in the view is named: the key the batch makes it under, else `#` and its number.
fn label_text(id: i64, made: &BTreeMap<i64, String>) -> String {
    made.get(&id).cloned().unwrap_or_else(|| format!("#{id}"))
}

// What the review is given.

/// The items a change names, as it names them, for a change the review is shown by name only: the key it makes an item
/// under, else the items it names.
fn named_as_given(c: &Value) -> Vec<Value> {
    if c["kind"] == "create_item" {
        return vec![c["key"].clone()];
    }
    let mut out: Vec<Value> = Vec::new();
    for v in ["item", "keep", "on", "fix_item"]
        .iter()
        .map(|k| c[*k].clone())
        .chain([
            c["under"]["item"].clone(),
            c["to"]["item"].clone(),
            c["decision"]["item"].clone(),
        ])
        .chain(items(c.get("absorbed")))
    {
        if !v.is_null() && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// What the review is given of one change of the batch beyond the change itself: how it stands against the look before,
/// what it did when the batch was tried out, and what the review found of it at the look before. A change that is the
/// same as at the look before, with the same effect, and that look found sound, is given by its line, its kind and the
/// items it names alone, and the round that found it sound: the review gives no row for it.
pub(super) fn described(stored: &Stored, row: &Change, verdicts: &BTreeMap<i64, Verdict>, full: Value) -> Value {
    let key = row.line.to_string();
    let state = &stored.states["lines"][&key];
    let last = verdicts.get(&row.line);
    if state["state"] == "unchanged"
        && let Some(v) = last.filter(|v| v.verdict == "ok")
    {
        return json!({
            "change": row.line,
            "state": "kept",
            "kind": row.change["kind"],
            "items": named_as_given(&row.change),
            "found_sound_in": format!("R{}", v.round),
        });
    }
    let mut out = full;
    if let Some(o) = out.as_object_mut() {
        for (k, v) in state.as_object().into_iter().flatten() {
            o.insert(k.clone(), v.clone());
        }
        o.insert("effect".into(), stored.lines[&key]["effect"].clone());
        if let Some(v) = last {
            o.insert(
                "found_at_look_before".into(),
                json!({ "round": v.round, "verdict": v.verdict }),
            );
        }
    }
    out
}

/// The changes the look before had that this one has not, each with its line, its kind and why it was left out.
pub(super) fn removed(stored: &Stored) -> Vec<Value> {
    items(stored.states.get("removed"))
}

/// The changes of an architect work's batch the review must give a row for: each new, changed or with another effect
/// than at the look before, and each that look found wrong and is the same now. All of them when the batch was not
/// tried out for this look.
pub(super) fn needing_a_row(conn: &Connection, work: i64, rows: &[Change]) -> Result<BTreeSet<i64>, String> {
    let all: BTreeSet<i64> = rows.iter().map(|r| r.line).collect();
    let Some(stored) = stored(conn, work)? else {
        return Ok(all);
    };
    let verdicts = last_verdicts(conn, work)?;
    Ok(rows
        .iter()
        .filter(|r| {
            let same = stored.states["lines"][r.line.to_string()]["state"] == "unchanged";
            !(same && verdicts.get(&r.line).is_some_and(|v| v.verdict == "ok"))
        })
        .map(|r| r.line)
        .collect())
}

/// The lines of changes that name an item a change the review drops (`drop`, or `close_observation`) was to create,
/// or one that such a change was to create: dropped with it, the review gives no row for them.
pub(super) fn dropped_with(rows: &[Change], review: &[Value]) -> BTreeSet<i64> {
    let mut gone: BTreeSet<i64> = rows
        .iter()
        .filter(|r| {
            review.iter().any(|x| {
                x["change"].as_i64() == Some(r.line)
                    && matches!(x["verdict"].as_str(), Some("drop" | CLOSE_OBSERVATION))
            })
        })
        .map(|r| r.line)
        .collect();
    let mut with = BTreeSet::new();
    loop {
        let keys: Vec<String> = rows
            .iter()
            .filter(|r| gone.contains(&r.line) && r.change["kind"] == "create_item")
            .map(|r| text(&r.change["key"]).trim().to_string())
            .collect();
        let next: Vec<i64> = rows
            .iter()
            .filter(|r| !gone.contains(&r.line) && keys.iter().any(|k| names_key(&r.change, k)))
            .map(|r| r.line)
            .collect();
        if next.is_empty() {
            return with;
        }
        gone.extend(&next);
        with.extend(next);
    }
}

// What the revision says of the batch.

/// Why a change the revision leaves out needs no reason from the architect: the review dropped it (or found it
/// replaced by closing an observation), or it names an item a change left out that way was to create. The text says so,
/// for the record of the change and for the review's next look.
pub(super) fn left_out_as_found(
    before: &[Change],
    kept: &BTreeSet<i64>,
    verdicts: &BTreeMap<i64, Verdict>,
) -> BTreeMap<i64, String> {
    let mut out: BTreeMap<i64, String> = BTreeMap::new();
    for r in before.iter().filter(|r| !kept.contains(&r.line)) {
        let Some(v) = verdicts.get(&r.line) else { continue };
        match v.verdict.as_str() {
            "drop" => {
                out.insert(
                    r.line,
                    format!("Left out as the review dropped it: {}", text(&v.row["why"])),
                );
            }
            CLOSE_OBSERVATION => {
                let (id, _, why) = closing(&v.row).unwrap_or_default();
                out.insert(
                    r.line,
                    format!("Left out as the review asked; observation #{id} closes when the batch is made: {why}"),
                );
            }
            _ => {}
        }
    }
    loop {
        let created: Vec<(i64, String)> = before
            .iter()
            .filter(|r| out.contains_key(&r.line) && r.change["kind"] == "create_item")
            .map(|r| (r.line, text(&r.change["key"]).trim().to_string()))
            .collect();
        let next: Vec<(i64, String)> = before
            .iter()
            .filter(|r| !kept.contains(&r.line) && !out.contains_key(&r.line))
            .filter_map(|r| {
                created.iter().find(|(_, k)| names_key(&r.change, k)).map(|(l, _)| {
                    (
                        r.line,
                        format!("Left out with the change on line {l}, which was to create an item it names"),
                    )
                })
            })
            .collect();
        if next.is_empty() {
            return out;
        }
        out.extend(next);
    }
}

/// Every thing wrong with how a revision accounts for the batch it revises, all at once. `given` are the changes it
/// hands in (a `line` names one of the batch's, none a new one), `left` what it says of changes it leaves out or does
/// not make as the review asked, each `{line, why}`; `before` is the batch as it stands and `verdicts` what the review
/// found at its latest look.
///
/// Every change of the batch is handed in again, or left out with a reason: unless the review dropped it, or it names an
/// item a change the review dropped was to create, which the engine says for it. A change the review found wrong
/// (`fix`, or dropped) and that is handed in as it was has a reason too, of why it is not changed.
pub(super) fn accounting_problems(
    before: &[Change],
    given: &[Value],
    left: &[Value],
    verdicts: &BTreeMap<i64, Verdict>,
) -> Vec<String> {
    let known: BTreeSet<i64> = before.iter().map(|r| r.line).collect();
    let mut problems = Vec::new();
    let mut kept: BTreeMap<i64, &Value> = BTreeMap::new();
    for (i, x) in given.iter().enumerate() {
        let Some(line) = x["line"].as_i64() else { continue };
        if !known.contains(&line) {
            problems.push(format!(
                "change {}: line {line} is no change of the batch, whose lines are {}: leave `line` out for a new change",
                i + 1,
                lines_text(&known)
            ));
        } else if kept.insert(line, x).is_some() {
            problems.push(format!(
                "line {line} is handed in more than once: one change for each line, the whole set as it should be made"
            ));
        }
    }
    let mut said: BTreeSet<i64> = BTreeSet::new();
    for x in left {
        let line = x["line"].as_i64().unwrap_or_default();
        if !known.contains(&line) {
            problems.push(format!(
                "left says of line {line}, which is no change of the batch, whose lines are {}",
                lines_text(&known)
            ));
        } else if !said.insert(line) {
            problems.push(format!("left says of line {line} more than once: say it once"));
        }
        if text(&x["why"]).trim().is_empty() {
            problems.push(format!("left says nothing of why for line {line}"));
        }
    }
    let kept_lines: BTreeSet<i64> = kept.keys().copied().collect();
    let by_review = left_out_as_found(before, &kept_lines, verdicts);
    let mut unsaid: Vec<String> = Vec::new();
    let mut unchanged: Vec<String> = Vec::new();
    for r in before {
        let found = verdicts.get(&r.line).map(|v| v.verdict.as_str());
        match kept.get(&r.line) {
            None if !said.contains(&r.line) && !by_review.contains_key(&r.line) => unsaid.push(r.line.to_string()),
            Some(x) if !said.contains(&r.line) && matches!(found, Some("fix" | "drop" | CLOSE_OBSERVATION)) => {
                let same = if x["decision"].is_object() {
                    x["decision"] == r.change["decision"]
                } else {
                    kept_change(&x["change"]) == r.change
                };
                if same {
                    unchanged.push(r.line.to_string());
                }
            }
            _ => {}
        }
    }
    if !unsaid.is_empty() {
        problems.push(format!(
            "line{} {} {} left out of the revision, and the review did not drop {}: say in `left` why each is left out",
            if unsaid.len() > 1 { "s" } else { "" },
            unsaid.join(", "),
            if unsaid.len() > 1 { "are" } else { "is" },
            if unsaid.len() > 1 { "them" } else { "it" },
        ));
    }
    if !unchanged.is_empty() {
        problems.push(format!(
            "line{} {} {} handed in as {} was, though the review found {} wrong: change {} as the review asked, or say \
             in `left` why not",
            if unchanged.len() > 1 { "s" } else { "" },
            unchanged.join(", "),
            if unchanged.len() > 1 { "are" } else { "is" },
            if unchanged.len() > 1 { "they" } else { "it" },
            if unchanged.len() > 1 { "them" } else { "it" },
            if unchanged.len() > 1 { "each" } else { "it" },
        ));
    }
    problems
}

/// The lines of a batch as a sentence names them.
fn lines_text(lines: &BTreeSet<i64>) -> String {
    lines.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")
}
