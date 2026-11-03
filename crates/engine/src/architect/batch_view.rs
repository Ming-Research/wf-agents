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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use db::{Actor, Db};

    use super::*;
    use crate::architect::collect_change;

    /// A database with project p's architect's work 1 at `B2`, its rounds 1 to 8, matter 1 and `n` items of p, ranked in
    /// the order they were made.
    fn batch_db(n: i64) -> Db {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B2', 'p')",
                [],
            )?;
            for r in 1..=8 {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at)
                     VALUES (1, ?1, 'H8', '{}', 'c', ?1)",
                    [r],
                )?;
            }
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('p', 'owner_words', 'w', '{}', 1)",
                [],
            )?;
            for i in 1..=n {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level, rank)
                     VALUES ('p', ?1, 'x', 'y', 'change', 'l', ?2)",
                    params![format!("item {i}"), i],
                )?;
            }
            Ok(())
        })
        .unwrap();
        db
    }

    fn at(n: i64) -> RoundId {
        RoundId::new(1, n)
    }

    /// Puts a change in the batch as round `round` hands it in; gives its line.
    fn collect(db: &mut Db, round: i64, line: Option<i64>, change: Value) -> i64 {
        db.write(Actor::Architect, |tx| {
            collect_change(tx, at(round), 1, line, &change).map_err(db::Error::Abort)
        })
        .unwrap()
    }

    /// Tries the batch out for the review that round `round` goes to.
    fn stage_at(db: &mut Db, round: i64) {
        db.write(Actor::Engine, |tx| {
            stage(tx, at(round), "p", &BTreeMap::new()).map_err(db::Error::Abort)
        })
        .unwrap();
    }

    fn hold(item: i64, text: &str) -> Value {
        json!({ "kind": "hold", "item": item, "text": text, "reason": "r" })
    }

    fn put(item: i64, to: Value) -> Value {
        json!({ "kind": "move", "item": item, "to": to, "reason": "r" })
    }

    /// The review at round `n` found each change of the lines given as the verdicts say.
    fn reviewed(db: &mut Db, n: i64, verdicts: &[(i64, &str)]) {
        let rows: Vec<Value> = verdicts
            .iter()
            .map(|(line, verdict)| json!({ "change": line, "verdict": verdict, "why": "w" }))
            .collect();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET cell = 'B2', transition = 'fix', output = ?2, ended_at = ?1 WHERE work = 1 AND n = ?1",
                params![n, json!({ "outcome": { "review": rows } }).to_string()],
            )?;
            Ok(())
        })
        .unwrap();
    }

    // A change takes the batch's next line when it joins, a revised version of it keeps the line, and a line a revision
    // left out is not given again: the review and the revision name a change by it, so a number that meant one change
    // must never come to mean another. No two changes waiting for their review have one line.
    #[test]
    fn a_line_is_kept_by_every_version_of_a_change_and_never_given_twice() {
        let mut db = batch_db(2);
        let lines: Vec<i64> = (0..3).map(|_| collect(&mut db, 2, None, hold(1, "A"))).collect();
        assert_eq!(lines, [1, 2, 3]);
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE architect_item_changes SET state = 'revised' WHERE work = 1 AND line IN (2, 3)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        // Line 2 comes again as another version of the change; line 3 does not, and a new change takes line 4.
        assert_eq!(collect(&mut db, 4, Some(2), hold(1, "B")), 2);
        assert_eq!(collect(&mut db, 4, None, hold(1, "C")), 4);
        // Two changes waiting for their review on one line is refused by the database.
        let twice = db.write(Actor::Architect, |tx| {
            collect_change(tx, at(4), 1, Some(1), &hold(1, "D")).map_err(db::Error::Abort)
        });
        assert!(twice.is_err());
        let waiting: Vec<i64> = collected(db.conn(), 1).unwrap().iter().map(|r| r.line).collect();
        assert_eq!(waiting, [1, 2, 4]);
    }

    // Item #21: the review is told how each change stands against the look before. A change whose text is the same but
    // whose effect is not, because a change before it moved something, is not the same: a move to a place that was already
    // the item's did nothing, and does something once an earlier change has moved it. A change left out says why.
    #[test]
    fn each_change_stands_against_the_look_before_and_one_left_out_says_why() {
        let mut db = batch_db(4);
        collect(&mut db, 2, None, put(4, json!({ "place": "first" })));
        collect(&mut db, 2, None, put(3, json!({ "place": "after", "item": 2 })));
        collect(&mut db, 2, None, hold(4, "A"));
        collect(&mut db, 2, None, hold(1, "B"));
        collect(&mut db, 2, None, hold(3, "C"));
        stage_at(&mut db, 3);
        let state = |s: &Stored, line: i64| s.states["lines"][line.to_string()]["state"].clone();
        let first = stored(db.conn(), 1).unwrap().unwrap();
        let states: Vec<Value> = (1..=5).map(|l| state(&first, l)).collect();
        assert_eq!(states, vec![json!("new"); 5]);
        // The second move finds item 3 right after item 2 already.
        assert_eq!(first.lines["2"]["effect"]["result"], "nothing");
        assert_eq!(first.lines["3"]["effect"]["result"], "changes");
        // The review found the second wrong, the third sound, the fifth wrong, and the others sound.
        reviewed(&mut db, 4, &[(1, "ok"), (2, "ok"), (3, "ok"), (4, "fix"), (5, "fix")]);
        // The revision: line 1 changes, 2, 3 and 5 are as they were, 4 is left out, and a change joins.
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE architect_item_changes SET state = 'revised' WHERE work = 1 AND state = 'collected'",
                [],
            )?;
            tx.execute(
                "UPDATE architect_item_changes SET outcome = ?1 WHERE line = 4",
                ["{\"what\":\"Item 1 needs no hold.\"}"],
            )?;
            Ok(())
        })
        .unwrap();
        collect(&mut db, 5, Some(1), put(2, json!({ "place": "last" })));
        collect(&mut db, 5, Some(2), put(3, json!({ "place": "after", "item": 2 })));
        collect(&mut db, 5, Some(3), hold(4, "A"));
        collect(&mut db, 5, Some(5), hold(3, "C"));
        assert_eq!(
            collect(&mut db, 5, None, hold(2, "E")),
            6,
            "a line left out is not given again"
        );
        stage_at(&mut db, 6);
        let second = stored(db.conn(), 1).unwrap().unwrap();
        let states: Vec<Value> = [1, 2, 3, 5, 6].iter().map(|l| state(&second, *l)).collect();
        assert_eq!(
            states,
            [
                json!("changed"),
                json!("effect_changed"),
                json!("unchanged"),
                json!("unchanged"),
                json!("new")
            ]
        );
        assert_eq!(second.states["lines"]["1"]["diff"]["to"]["now"]["place"], "last");
        assert_eq!(second.states["lines"]["2"]["effect_was"]["result"], "nothing");
        assert_eq!(
            second.states["removed"],
            json!([{ "change": 4, "state": "removed", "kind": "hold", "why": "Item 1 needs no hold." }])
        );
        // The review gives a row for what is new, changed, with another effect, or was found wrong and is the same; line 3,
        // the same as at the look it was found sound, needs none, and is given by its line, kind and items alone.
        let rows = collected(db.conn(), 1).unwrap();
        let needing = needing_a_row(db.conn(), 1, &rows).unwrap();
        assert_eq!(needing, BTreeSet::from([1, 2, 5, 6]));
        let verdicts = last_verdicts(db.conn(), 1).unwrap();
        let given = |line: i64| {
            let row = rows.iter().find(|r| r.line == line).unwrap();
            described(
                &second,
                row,
                &verdicts,
                json!({ "change": line, "item_change": row.change }),
            )
        };
        assert_eq!(
            given(3),
            json!({ "change": 3, "state": "kept", "kind": "hold", "items": [4], "found_sound_in": "R4" })
        );
        assert_eq!(given(1)["state"], "changed");
        assert_eq!(given(5)["found_at_look_before"]["verdict"], "fix");
        assert_eq!(given(2)["effect"]["result"], "changes");
    }

    // The review is given what the batch comes to: each item the changes name as it stands after them, with the fields
    // that changed and what they were; where it stands among the items of its project that wait to start, how many stand
    // ahead of it and how many of those are held; its neighbours; and the items ready to start. Of the project's other
    // items it is given none: only those the changes name, and those next to them.
    #[test]
    fn the_review_is_given_the_items_the_changes_name_as_they_stand_after_them() {
        let mut db = batch_db(30);
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE items SET held = 'Later.' WHERE id > 7", [])?;
            Ok(())
        })
        .unwrap();
        collect(&mut db, 2, None, hold(5, "Wait for the lexer."));
        collect(&mut db, 2, None, put(4, json!({ "place": "after", "item": 3 })));
        collect(
            &mut db,
            2,
            None,
            json!({ "kind": "edit_item", "item": 2, "problem": "Changed.", "acceptance": ["It holds."], "reason": "r" }),
        );
        stage_at(&mut db, 3);
        let seen = stored(db.conn(), 1).unwrap().unwrap();
        // What each change did.
        assert_eq!(seen.lines["2"]["effect"], json!({ "result": "nothing" }));
        let wrote = &seen.lines["3"]["effect"]["wrote"][0];
        assert_eq!(wrote["item"], 2);
        assert_eq!(wrote["fields"]["problem"], json!(["x", "Changed."]));
        assert_eq!(wrote["fields"]["wanted"], json!([null, ["It holds."]]));
        // The items they name, after them.
        let items = &seen.after["items"];
        assert_eq!(items["#5"]["changed"]["held"], json!([null, "Wait for the lexer."]));
        assert_eq!(items["#5"]["rank"], json!({ "was": 5, "now": 5 }));
        assert_eq!(items["#5"]["queue"]["ahead"], 4);
        assert_eq!(items["#5"]["queue"]["ahead_held"], 0);
        assert_eq!(items["#5"]["queue"]["previous"]["item"], 4);
        assert_eq!(items["#5"]["queue"]["next"]["item"], 6);
        assert_eq!(items["#2"]["changed"]["problem"], json!(["x", "Changed."]));
        // The items ready to start: those not held, and not item 5, which the batch holds.
        let ready: Vec<i64> = seen.after["ready"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x["item"].as_i64())
            .collect();
        assert_eq!(ready, [1, 2, 3, 4, 6, 7]);
        assert_eq!(seen.after["ready"]["more"], 0);
        // Nothing is given of the items the changes do not name, but their neighbours.
        let all = seen.after.to_string();
        assert!(!all.contains("item 25") && !all.contains("item 12"), "{all}");
    }

    /// The tables a batch tried out must leave as they were, each as the rows it holds.
    fn snapshot(db: &Db) -> Vec<Value> {
        [
            "items",
            "item_links",
            "observations",
            "works",
            "sessions",
            "outbox",
            "changes",
            "dispatch_queue",
            "known_red_checks",
            "engine_notices",
            "interjections",
            "architect_matters",
        ]
        .iter()
        .map(|t| json!(query_json(db.conn(), &format!("SELECT * FROM {t} ORDER BY 1"), &[], 100_000).unwrap()))
        .collect()
    }

    // Trying a batch out changes nothing: not an item, a dependency, an observation or a work, not the outbox, not what
    // `changes` keeps, and the listener of the owner's page is told no row of it, though a write that takes its changes
    // back by hand does tell the rows it touched. This holds for a cancel of an item whose work goes on, which asks the
    // engine's loop to end the work and puts the asking in the outbox.
    #[test]
    fn a_batch_tried_out_leaves_nothing_and_tells_nobody() {
        let mut db = batch_db(3);
        let project: config::Project = serde_json::from_value(json!({
            "repo": "o/p", "checkout": "/checkout", "design_goal": "simple",
            "test_paths": ["tests/"], "ci": { "fast": "ci", "full": ["ci"], "merge_checks": ["check"] }
        }))
        .unwrap();
        let projects = BTreeMap::from([("p".to_string(), project)]);
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO observations (project, what, source) VALUES ('p', 'Slow.', 'A report.')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, worktree, pr) VALUES (3, 1, 'engineer', 'implement', '/w/3', 3)",
                [],
            )?;
            tx.execute(
                "INSERT INTO sessions (work, line, cli, cli_session) VALUES (2, 'engineer', 'claude', 's')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let told: Arc<Mutex<Vec<db::Changed>>> = Arc::default();
        let sink = Arc::clone(&told);
        db.on_commit(Arc::new(move |_: &Connection, rows: &[db::Changed]| {
            sink.lock().unwrap().extend(rows.iter().cloned());
        }))
        .unwrap();
        let before = snapshot(&db);
        let changes = [
            hold(1, "Held."),
            put(1, json!({ "place": "last" })),
            json!({ "kind": "create_item", "key": "cover", "project": "p", "title": "Cover it", "problem": "p", "why": "w",
                    "acceptance": ["Covered."], "level": "l", "deliver": "change",
                    "under": { "item": "2", "dependency": "blocking" }, "reason": "r" }),
            json!({ "kind": "fold_observations", "item": "cover", "observations": [1], "reason": "r" }),
            // The work on item 3 goes on: the cancel asks the loop to end it, and puts the asking in the outbox.
            json!({ "kind": "cancel", "item": "3", "goes": "nowhere", "reason": "r" }),
        ];
        let (trials, queued) = db
            .write(Actor::Architect, |tx| {
                crate::arch_changes::tried(tx, &changes, &projects, |tx, _| {
                    // Inside, the outbox holds the asking, so that the trial did write it.
                    crate::architect::count(tx, "SELECT count(*) FROM outbox", &[])
                })
                .map_err(db::Error::Abort)
            })
            .unwrap();
        assert_eq!(trials.iter().map(|t| t.result).collect::<Vec<_>>(), vec!["made"; 5]);
        // The four changes of the items wrote rows; the cancel wrote the asking.
        assert!(trials[..4].iter().all(|t| !t.rows.is_empty()), "{trials:?}");
        assert!(queued > 0);
        assert_eq!(snapshot(&db), before);
        assert!(told.lock().unwrap().is_empty(), "{:?}", told.lock().unwrap());
        // A write that takes its changes back to a savepoint by hand is told, which shows the listener would be told.
        db.write(Actor::Architect, |tx| {
            tx.execute_batch("SAVEPOINT by_hand")?;
            tx.execute("UPDATE items SET held = 'x' WHERE id = 1", [])?;
            tx.execute_batch("ROLLBACK TO by_hand; RELEASE by_hand")?;
            Ok(())
        })
        .unwrap();
        assert!(!told.lock().unwrap().is_empty());
    }

    fn row(line: i64, change: Value) -> Change {
        Change {
            id: line,
            round: 2,
            matter: 1,
            line,
            change,
        }
    }

    fn found(round: i64, verdict: &str) -> Verdict {
        Verdict {
            round,
            verdict: verdict.to_string(),
            row: json!({ "verdict": verdict, "why": "It is done already." }),
        }
    }

    // A revision accounts for every change of the batch: a change it leaves out has a reason unless the review dropped
    // it, or it names an item such a change was to create; a change the review found wrong and the revision hands in as
    // it was has one too; a line that is none of the batch's, one handed in twice and a reason for a line that is none of
    // them are wrong. Every problem is said at once.
    #[test]
    fn a_revision_must_say_why_it_leaves_a_change_out_or_does_not_make_what_was_asked() {
        let create = json!({ "kind": "create_item", "key": "new", "reason": "r" });
        let depends = json!({ "kind": "add_dependency", "item": 3, "on": "new", "reason": "r" });
        let before = vec![
            row(1, create),
            row(2, depends),
            row(3, hold(1, "A")),
            row(4, hold(2, "B")),
            row(5, hold(3, "C")),
        ];
        let verdicts = BTreeMap::from([
            (1, found(4, "drop")),
            (3, found(4, "fix")),
            (4, found(4, "ok")),
            (5, found(4, "fix")),
        ]);
        let as_was = json!({ "line": 3, "matter": 1, "change": hold(1, "A") });
        let wrongly = vec![
            as_was.clone(),
            as_was,
            json!({ "line": 9, "matter": 1, "change": hold(3, "Z") }),
        ];
        let left = vec![json!({ "line": 7, "why": "Not a line." })];
        let problems = accounting_problems(&before, &wrongly, &left, &verdicts);
        let said = problems.join("\n");
        assert!(said.contains("line 9 is no change of the batch"), "{said}");
        assert!(said.contains("line 3 is handed in more than once"), "{said}");
        assert!(
            said.contains("left says of line 7, which is no change of the batch"),
            "{said}"
        );
        // Line 4 was found sound and 5 wrong, and both are left out with no reason; 1 was dropped and 2 names the item
        // it was to create, which the engine says.
        assert!(said.contains("lines 4, 5 are left out of the revision"), "{said}");
        assert!(
            said.contains("line 3 is handed in as it was, though the review found it wrong"),
            "{said}"
        );
        assert!(
            !said.contains("line 1 ") && !said.contains("line 2 ") && !said.contains("lines 2"),
            "{said}"
        );
        assert_eq!(problems.len(), 5, "{said}");
        // With a reason for each, nothing is wrong.
        let right = vec![json!({ "line": 3, "matter": 1, "change": hold(1, "A") })];
        let left = vec![
            json!({ "line": 3, "why": "The hold stands: nothing else keeps the item back." }),
            json!({ "line": 4, "why": "Item 2 is held by the batch's other change." }),
            json!({ "line": 5, "why": "Item 3 is held already." }),
        ];
        assert_eq!(
            accounting_problems(&before, &right, &left, &verdicts),
            Vec::<String>::new()
        );
    }

    // A change that names an item a change the review drops was to create goes with it, and the change after that, which
    // names the item the second was to create; the review gives a row for neither. What the review drops is told to the
    // revision as it left the batch.
    #[test]
    fn a_change_that_names_an_item_a_dropped_change_creates_is_dropped_with_it() {
        let create = |key: &str, under: Option<&str>| {
            let mut c = json!({ "kind": "create_item", "key": key, "reason": "r" });
            if let Some(u) = under {
                c["under"] = json!({ "item": u, "dependency": "blocking" });
            }
            c
        };
        let rows = vec![
            row(1, create("one", None)),
            row(2, create("two", Some("one"))),
            row(
                3,
                json!({ "kind": "add_dependency", "item": 5, "on": "two", "reason": "r" }),
            ),
            row(4, hold(5, "A")),
        ];
        let drop_one = [json!({ "change": 1, "verdict": "drop", "why": "It is done." })];
        assert_eq!(dropped_with(&rows, &drop_one), BTreeSet::from([2, 3]));
        // A change the review asks to fix leaves the others to be judged.
        let fix_one = [json!({ "change": 1, "verdict": "fix", "what": "w", "how": "h" })];
        assert!(dropped_with(&rows, &fix_one).is_empty());
        let verdicts = BTreeMap::from([(1, found(4, "drop"))]);
        let said = left_out_as_found(&rows, &BTreeSet::from([4]), &verdicts);
        assert_eq!(said.keys().copied().collect::<Vec<_>>(), [1, 2, 3]);
        assert!(said[&1].contains("as the review dropped it: w") || said[&1].contains("It is done already."));
        assert!(said[&2].contains("line 1"), "{said:?}");
        assert!(said[&3].contains("line 2"), "{said:?}");
    }
}
