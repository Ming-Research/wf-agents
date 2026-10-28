//! The changes to the database the architect collects, the flow's `arch_changes`: what makes them valid, which the
//! eighth question's return is checked against, and writing those the owner approved, which the tool db_write does in
//! one transaction.

use std::collections::{BTreeMap, BTreeSet};

use db::Actor;
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::work::{Work, query_json};
use crate::{ENGINEER, Engine, actions};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(xs)) => xs.clone(),
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other.clone()],
    }
}

fn text<'a>(c: &'a Value, k: &str) -> &'a str {
    c[k].as_str().unwrap_or_default()
}

fn texts(c: &Value, k: &str) -> Vec<String> {
    items(c.get(k))
        .iter()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

/// An item's number as a change gives it, a number or a text of one.
fn number(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().trim_start_matches('#').parse().ok(),
        _ => None,
    }
}

/// The first number a text holds, as a budget's line or share writes it, such as "-2 KB" or "+1500 bytes".
fn amount(text: &str) -> Option<f64> {
    let start = text.find(|c: char| c.is_ascii_digit() || c == '-' || c == '+')?;
    let rest = &text[start..];
    let end = rest
        .char_indices()
        .skip(1)
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map_or(rest.len(), |(i, _)| i);
    rest[..end].parse().ok()
}

/// What an item is to the rules the changes keep: the segment it hangs on, and whether it is still to do.
#[derive(Debug, Clone, Default)]
struct It {
    segment: Option<String>,
    open: bool,
}

/// A budget's line and its shares: each segment's, by its key.
type Budget = (String, Vec<(String, String)>);

/// The items, their dependencies and the plan, as the changes find them and as each change leaves them. An item the
/// changes create is known by its key until it is written, here as a number below zero.
#[derive(Debug, Default)]
struct State {
    items: BTreeMap<i64, It>,
    links: BTreeSet<(i64, i64)>,
    segments: BTreeMap<String, Vec<String>>,
    gates: BTreeMap<String, (String, Vec<String>)>,
    budgets: BTreeMap<String, Budget>,
    red: BTreeSet<(String, String)>,
    keys: BTreeMap<String, i64>,
}

impl State {
    fn load(e: &Engine, conn: &Connection) -> Result<State, String> {
        let end = e
            .flows
            .get(&e.flow_now)
            .and_then(|f| f.graph(ENGINEER))
            .map(|g| g.end.clone())
            .unwrap_or_default();
        let mut s = State::default();
        for r in query_json(
            conn,
            "SELECT i.id, i.segment, i.cancelled IS NULL AND NOT EXISTS (
                SELECT 1 FROM works w WHERE w.item = i.id AND w.graph = 'engineer' AND w.ended IS NULL AND w.at = ?1)
                AS open
             FROM items i",
            &[&end],
            1_000_000,
        )? {
            s.items.insert(
                r["id"].as_i64().unwrap_or_default(),
                It {
                    segment: r["segment"].as_str().map(str::to_string),
                    open: r["open"].as_i64() == Some(1),
                },
            );
        }
        for r in query_json(conn, "SELECT item, depends_on FROM item_links", &[], 1_000_000)? {
            s.links.insert((
                r["item"].as_i64().unwrap_or_default(),
                r["depends_on"].as_i64().unwrap_or_default(),
            ));
        }
        for r in query_json(conn, "SELECT key, after FROM plan_segments", &[], 100_000)? {
            s.segments.insert(text(&r, "key").to_string(), texts(&r, "after"));
        }
        for r in query_json(conn, "SELECT name, what, segments FROM plan_gates", &[], 100_000)? {
            s.gates.insert(
                text(&r, "name").to_string(),
                (text(&r, "what").to_string(), texts(&r, "segments")),
            );
        }
        for r in query_json(conn, "SELECT name, line, split FROM plan_budgets", &[], 100_000)? {
            let split = items(r.get("split"))
                .iter()
                .map(|x| (text(x, "segment").to_string(), text(x, "amount").to_string()))
                .collect();
            s.budgets
                .insert(text(&r, "name").to_string(), (text(&r, "line").to_string(), split));
        }
        for r in query_json(conn, "SELECT project, check_name FROM known_red_checks", &[], 100_000)? {
            s.red
                .insert((text(&r, "project").to_string(), text(&r, "check_name").to_string()));
        }
        Ok(s)
    }

    /// The segment an item reaches: its own, or that of an item that depends on it, nearest first.
    fn reaches(&self, item: i64) -> Option<String> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![item];
        while let Some(i) = todo.pop() {
            if !seen.insert(i) {
                continue;
            }
            if let Some(s) = self.items.get(&i).and_then(|x| x.segment.clone()) {
                return Some(s);
            }
            todo.extend(self.links.iter().filter(|(_, d)| *d == i).map(|(x, _)| *x));
        }
        None
    }

    /// Whether the items' dependencies close a cycle.
    fn links_cycle(&self) -> bool {
        let edges: Vec<(i64, i64)> = self.links.iter().copied().collect();
        cycle(&edges)
    }

    /// Whether the segments' order closes a cycle.
    fn order_cycle(&self) -> bool {
        let names: Vec<&String> = self.segments.keys().collect();
        let at = |k: &str| names.iter().position(|n| n.as_str() == k).map(|i| i as i64);
        let mut edges = Vec::new();
        for (k, after) in &self.segments {
            for a in after {
                if let (Some(x), Some(y)) = (at(k), at(a)) {
                    edges.push((x, y));
                }
            }
        }
        cycle(&edges)
    }
}

/// Whether directed edges close a cycle.
fn cycle(edges: &[(i64, i64)]) -> bool {
    let nodes: BTreeSet<i64> = edges.iter().flat_map(|(a, b)| [*a, *b]).collect();
    let mut state: BTreeMap<i64, u8> = BTreeMap::new();
    fn visit(n: i64, edges: &[(i64, i64)], state: &mut BTreeMap<i64, u8>) -> bool {
        match state.get(&n) {
            Some(1) => return true,
            Some(2) => return false,
            _ => {}
        }
        state.insert(n, 1);
        for (_, b) in edges.iter().filter(|(a, _)| *a == n) {
            if visit(*b, edges, state) {
                return true;
            }
        }
        state.insert(n, 2);
        false
    }
    nodes.into_iter().any(|n| visit(n, edges, &mut state))
}

/// The numbers of an item's locked acceptance conditions; none while its acceptance is not locked.
fn locked_conditions(conn: &Connection, item: i64) -> BTreeSet<String> {
    let text: Option<String> = conn
        .query_row("SELECT acceptance FROM items WHERE id = ?1", [item], |r| r.get(0))
        .ok()
        .flatten();
    let rows: Vec<Value> = text.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    rows.iter()
        .filter_map(|r| r["id"].as_str().map(str::to_string))
        .collect()
}

/// Whether the changes the architect collected are valid: every change has its reason; an item created has what it
/// needs and a key no other change uses; a split gives each acceptance condition of the item split to one new item at
/// most, one of its locked conditions when its acceptance is locked; every name a change gives is there, or made by an
/// earlier change; and what the rules fix holds after them: the items' dependencies and the segments' order close no
/// cycle, an item created touches only what the segment it reaches may touch, each budget's shares add up to no more
/// than its line, and a segment removed has no unfinished item. Otherwise why not, every reason at once.
pub fn validate(e: &Engine, changes: &[Value]) -> Result<(), String> {
    let mut s = State::load(e, e.db.conn())?;
    let cfg = &e.config.config;
    let mut problems: Vec<String> = Vec::new();
    let mut next_new = -1;
    // The acceptance conditions each item split gives to its new items so far.
    let mut taken: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    for (i, c) in changes.iter().enumerate() {
        let n = i + 1;
        let mut bad = |p: String| problems.push(format!("change {n} ({}): {p}", text(c, "kind")));
        if text(c, "reason").trim().is_empty() {
            bad("it has no reason".into());
        }
        let item = |k: &str, s: &State, bad: &mut dyn FnMut(String)| -> Option<i64> {
            let id = number(&c[k]);
            match id {
                Some(id) if s.items.contains_key(&id) => Some(id),
                _ => {
                    bad(format!("there is no item {}", c[k]));
                    None
                }
            }
        };
        let segment = |key: &str, s: &State, bad: &mut dyn FnMut(String)| -> bool {
            let ok = s.segments.contains_key(key);
            if !ok {
                bad(format!("there is no segment {key}"));
            }
            ok
        };
        match text(c, "kind") {
            "create_item" => {
                let key = text(c, "key").trim().to_string();
                if key.is_empty() || s.keys.contains_key(&key) || s.segments.contains_key(&key) {
                    bad(format!("its key \"{key}\" is empty, or another change uses it"));
                }
                for k in ["project", "title", "problem", "why", "level", "deliver", "under"] {
                    if text(c, k).trim().is_empty() {
                        bad(format!("it has no {k}"));
                    }
                }
                if !cfg.projects.contains_key(text(c, "project")) {
                    bad(format!("there is no project {}", text(c, "project")));
                }
                if cfg.level(text(c, "level")).is_none() {
                    bad(format!("there is no level {}", text(c, "level")));
                }
                if let Err(why) = cfg.item_level(text(c, "level")) {
                    bad(why);
                }
                if texts(c, "acceptance").iter().all(|a| a.trim().is_empty()) {
                    bad("it has no acceptance".into());
                }
                let id = next_new;
                next_new -= 1;
                let under = text(c, "under").trim().to_string();
                let mut it = It {
                    segment: None,
                    open: true,
                };
                match number(&c["under"]) {
                    Some(parent) if s.items.contains_key(&parent) => {
                        s.links.insert((parent, id));
                    }
                    Some(parent) => bad(format!("it hangs under item {parent}, which there is not")),
                    None if s.segments.contains_key(&under) => it.segment = Some(under.clone()),
                    None => bad(format!("it hangs on segment {under}, which there is not")),
                }
                // A split gives each acceptance condition of the item split to one new item at most; the rest stay
                // with that item.
                let takes = texts(c, "takes");
                if !takes.is_empty() {
                    match number(&c["under"]).filter(|split| *split > 0) {
                        None => bad("it takes acceptance conditions, but it is split from no item".into()),
                        Some(split) => {
                            let locked = locked_conditions(e.db.conn(), split);
                            for t in takes {
                                if !locked.is_empty() && !locked.contains(&t) {
                                    bad(format!("item {split} has no acceptance condition {t}"));
                                }
                                if !taken.entry(split).or_default().insert(t.clone()) {
                                    bad(format!(
                                        "acceptance condition {t} of item {split} goes to two new items"
                                    ));
                                }
                            }
                        }
                    }
                }
                s.items.insert(id, it);
                s.keys.insert(key, id);
                // What it touches, its segment may touch.
                if let Some(seg) = s.reaches(id) {
                    for t in texts(c, "touches") {
                        for (name, (what, may)) in &s.gates {
                            if *what == t && !may.contains(&seg) {
                                bad(format!(
                                    "it touches {t}, which the gate {name} lets segment {seg} not touch"
                                ));
                            }
                        }
                    }
                }
            }
            "add_dependency" => {
                if let (Some(a), Some(b)) = (item("item", &s, &mut bad), item("on", &s, &mut bad)) {
                    if a == b {
                        bad("an item does not depend on itself".into());
                    }
                    s.links.insert((a, b));
                }
            }
            "move" => {
                if let Some(a) = item("item", &s, &mut bad) {
                    match number(&c["to"]) {
                        Some(to) if s.items.contains_key(&to) && to != a => {
                            s.links.retain(|(_, d)| *d != a);
                            s.links.insert((to, a));
                            if let Some(x) = s.items.get_mut(&a) {
                                x.segment = None;
                            }
                        }
                        Some(to) => bad(format!("it moves under item {to}, which there is not, or is itself")),
                        None => {
                            let key = text(c, "to").to_string();
                            if segment(&key, &s, &mut bad)
                                && let Some(x) = s.items.get_mut(&a)
                            {
                                x.segment = Some(key);
                            }
                        }
                    }
                }
            }
            "merge" => {
                if let Some(keep) = item("keep", &s, &mut bad) {
                    let absorbed: Vec<i64> = items(c.get("absorbed")).iter().filter_map(number).collect();
                    for a in absorbed {
                        if a == keep || !s.items.contains_key(&a) {
                            bad(format!("item {a} is not another item to merge into item {keep}"));
                            continue;
                        }
                        let links: Vec<(i64, i64)> = s.links.iter().copied().collect();
                        s.links.clear();
                        for (x, d) in links {
                            let (x, d) = (if x == a { keep } else { x }, if d == a { keep } else { d });
                            if x != d {
                                s.links.insert((x, d));
                            }
                        }
                        if let Some(x) = s.items.get_mut(&a) {
                            x.open = false;
                        }
                    }
                }
            }
            "cancel" => {
                if let Some(a) = item("item", &s, &mut bad)
                    && let Some(x) = s.items.get_mut(&a)
                {
                    x.open = false;
                }
            }
            "hold" | "release" | "edit_acceptance" => {
                item("item", &s, &mut bad);
            }
            "edit_item" => {
                item("item", &s, &mut bad);
                if text(c, "text").trim().is_empty() {
                    bad("it changes the text to nothing".into());
                }
            }
            "set_level" => {
                item("item", &s, &mut bad);
                if cfg.level(text(c, "level")).is_none() {
                    bad(format!("there is no level {}", text(c, "level")));
                }
                if let Err(why) = cfg.item_level(text(c, "level")) {
                    bad(why);
                }
            }
            "add_segment" => {
                let key = text(c, "key").trim().to_string();
                if key.is_empty() || s.segments.contains_key(&key) || s.keys.contains_key(&key) {
                    bad(format!("the segment's key \"{key}\" is empty, or taken"));
                }
                if !cfg.projects.contains_key(text(c, "project")) {
                    bad(format!("there is no project {}", text(c, "project")));
                }
                let after = texts(c, "after");
                for a in &after {
                    segment(a, &s, &mut bad);
                }
                s.segments.insert(key, after);
            }
            "edit_segment" => {
                let key = text(c, "segment").to_string();
                if segment(&key, &s, &mut bad) && c.get("after").is_some() {
                    let after = texts(c, "after");
                    for a in &after {
                        segment(a, &s, &mut bad);
                    }
                    s.segments.insert(key, after);
                }
            }
            "edit_done_when" | "defer_segment" => {
                segment(text(c, "segment"), &s, &mut bad);
            }
            "delete_segment" => {
                let key = text(c, "segment").to_string();
                if segment(&key, &s, &mut bad) {
                    let unfinished: Vec<i64> = s
                        .items
                        .iter()
                        .filter(|(_, x)| x.open && x.segment.as_deref() == Some(key.as_str()))
                        .map(|(i, _)| *i)
                        .collect();
                    if !unfinished.is_empty() {
                        bad(format!("segment {key} still has unfinished items: {unfinished:?}"));
                    }
                    s.segments.remove(&key);
                }
            }
            "order_segments" => {
                for k in texts(c, "order") {
                    segment(&k, &s, &mut bad);
                }
            }
            "add_gate" | "edit_gate" => {
                let adding = text(c, "kind") == "add_gate";
                let name = text(c, if adding { "name" } else { "gate" }).to_string();
                if adding == s.gates.contains_key(&name) {
                    bad(format!("the gate {name} is taken, or there is none"));
                }
                let may = texts(c, "segments");
                for k in &may {
                    segment(k, &s, &mut bad);
                }
                let what = match s.gates.get(&name) {
                    Some((w, _)) if !adding => w.clone(),
                    _ => text(c, "what").to_string(),
                };
                s.gates.insert(name, (what, may));
            }
            "add_budget" | "edit_budget" => {
                let adding = text(c, "kind") == "add_budget";
                let name = text(c, if adding { "name" } else { "budget" }).to_string();
                if adding == s.budgets.contains_key(&name) {
                    bad(format!("the budget {name} is taken, or there is none"));
                }
                let (mut line, mut split) = s.budgets.get(&name).cloned().unwrap_or_default();
                if let Some(l) = c["line"].as_str() {
                    line = l.to_string();
                }
                if c.get("split").is_some() {
                    split = items(c.get("split"))
                        .iter()
                        .map(|x| (text(x, "segment").to_string(), text(x, "amount").to_string()))
                        .collect();
                }
                for (k, _) in &split {
                    segment(k, &s, &mut bad);
                }
                let shares: Option<Vec<f64>> = split.iter().map(|(_, a)| amount(a)).collect();
                if let (Some(line_n), Some(shares)) = (amount(&line), shares)
                    && shares.iter().filter(|x| **x > 0.0).sum::<f64>() > line_n
                {
                    bad(format!(
                        "the shares of budget {name} add up to more than its line, {line}"
                    ));
                }
                s.budgets.insert(name, (line, split));
            }
            "add_known_red_check" => {
                if !cfg.projects.contains_key(text(c, "project")) {
                    bad(format!("there is no project {}", text(c, "project")));
                }
                let fix = match number(&c["fix_item"]) {
                    Some(i) => s.items.contains_key(&i),
                    None => s.keys.contains_key(text(c, "fix_item")),
                };
                if !fix {
                    bad(format!("there is no item {} to fix it", c["fix_item"]));
                }
                s.red
                    .insert((text(c, "project").to_string(), text(c, "check").to_string()));
            }
            "remove_known_red_check" => {
                let k = (text(c, "project").to_string(), text(c, "check").to_string());
                if !s.red.remove(&k) {
                    bad(format!("{} is no known red check of {}", k.1, k.0));
                }
            }
            other => bad(format!("there is no kind of change {other}")),
        }
    }
    if s.links_cycle() {
        problems.push("after the changes, the items' dependencies close a cycle".into());
    }
    if s.order_cycle() {
        problems.push("after the changes, the segments' order closes a cycle".into());
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// A change in a few words, for a reason it was refused.
fn short(c: &Value) -> String {
    let t = c.to_string();
    if t.chars().count() > 200 {
        format!("{}…", t.chars().take(200).collect::<String>())
    } else {
        t
    }
}

/// Writes the changes handed in, when every one is a change the owner approved and that is not written yet, word for
/// word, and they are valid: all in one transaction, each item created given its number. Otherwise nothing is written,
/// and why is said.
pub fn db_write(e: &mut Engine, _w: &Work, req: &Value) -> Value {
    let refused = |why: String| json!({ "written": { "verdict": "refused", "why": why } });
    let handed = items(req.get("changes"));
    if handed.is_empty() {
        return refused("no change was handed in".into());
    }
    let mut open = match crate::architect::approved_unwritten(e.db.conn()) {
        Ok(x) => x,
        Err(why) => return refused(why),
    };
    for (i, c) in handed.iter().enumerate() {
        match open.iter().position(|x| x == c) {
            Some(k) => {
                open.remove(k);
            }
            None => {
                return refused(format!(
                    "change {} is not one the owner approved and that is not written yet, word for word: {}",
                    i + 1,
                    short(c)
                ));
            }
        }
    }
    if let Err(why) = validate(e, &handed) {
        return refused(why);
    }
    match e
        .db
        .write(Actor::Architect, |tx| write_all(tx, &handed).map_err(db::Error::Abort))
    {
        Ok(created) => json!({ "written": { "verdict": "written", "created": created } }),
        Err(why) => refused(why.to_string()),
    }
}

/// Writes every change, in order, and gives what was created: each item and segment, by its key, with what it is now.
fn write_all(tx: &Transaction<'_>, changes: &[Value]) -> Result<Vec<Value>, String> {
    let mut keys: BTreeMap<String, i64> = BTreeMap::new();
    let mut created = Vec::new();
    for c in changes {
        write_one(tx, c, &mut keys, &mut created)?;
    }
    Ok(created)
}

/// The item a change names by its number, or by the key an earlier change created it under.
fn item_of(v: &Value, keys: &BTreeMap<String, i64>) -> Result<i64, String> {
    number(v)
        .or_else(|| v.as_str().and_then(|k| keys.get(k).copied()))
        .ok_or_else(|| format!("there is no item {v}"))
}

/// Ends the works of an item that is not to be done, and takes it out of the owner's queue.
fn cancel(tx: &Transaction<'_>, item: i64, why: &str) -> Result<(), String> {
    tx.execute("UPDATE items SET cancelled = ?2 WHERE id = ?1", params![item, why])
        .map_err(sql)?;
    tx.execute("DELETE FROM dispatch_queue WHERE item = ?1", [item])
        .map_err(sql)?;
    tx.execute(
        "UPDATE works SET ended = ?2 WHERE item = ?1 AND ended IS NULL",
        params![item, format!("item cancelled: {why}")],
    )
    .map_err(sql)?;
    Ok(())
}

/// Makes one item depend on another, unless it does already.
fn depend(tx: &Transaction<'_>, item: i64, on: i64, needs: &str) -> Result<(), String> {
    let has: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM item_links WHERE item = ?1 AND depends_on = ?2",
            [item, on],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    if has.is_none() && item != on {
        db::add_dependency(tx, item, on, needs).map_err(sql)?;
    }
    Ok(())
}

fn write_one(
    tx: &Transaction<'_>,
    c: &Value,
    keys: &mut BTreeMap<String, i64>,
    created: &mut Vec<Value>,
) -> Result<(), String> {
    let reason = text(c, "reason");
    let json_of = |k: &str| c.get(k).cloned().unwrap_or(Value::Null).to_string();
    match text(c, "kind") {
        "create_item" => {
            let segment = number(&c["under"]).is_none().then(|| text(c, "under").to_string());
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level, touches, wanted, segment)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    text(c, "project"),
                    text(c, "title"),
                    text(c, "problem"),
                    text(c, "why"),
                    text(c, "deliver"),
                    text(c, "level"),
                    json_of("touches"),
                    json_of("acceptance"),
                    segment
                ],
            )
            .map_err(sql)?;
            let id = tx.last_insert_rowid();
            if let Some(parent) = number(&c["under"]) {
                let takes = texts(c, "takes");
                let needs = if takes.is_empty() {
                    text(c, "title").to_string()
                } else {
                    format!("{} (its acceptance conditions {})", text(c, "title"), takes.join(", "))
                };
                depend(tx, parent, id, &needs)?;
            }
            keys.insert(text(c, "key").to_string(), id);
            created.push(json!({ "key": text(c, "key"), "id": id.to_string() }));
        }
        "add_dependency" => depend(tx, item_of(&c["item"], keys)?, item_of(&c["on"], keys)?, reason)?,
        "move" => {
            let item = item_of(&c["item"], keys)?;
            match number(&c["to"]) {
                Some(to) => {
                    tx.execute("DELETE FROM item_links WHERE depends_on = ?1", [item])
                        .map_err(sql)?;
                    tx.execute("UPDATE items SET segment = NULL WHERE id = ?1", [item])
                        .map_err(sql)?;
                    depend(tx, to, item, reason)?;
                }
                None => {
                    tx.execute(
                        "UPDATE items SET segment = ?2 WHERE id = ?1",
                        params![item, text(c, "to")],
                    )
                    .map_err(sql)?;
                }
            }
        }
        "merge" => {
            let keep = item_of(&c["keep"], keys)?;
            for a in items(c.get("absorbed")) {
                let a = item_of(&a, keys)?;
                let dependents = query_json(
                    tx,
                    "SELECT item, needs FROM item_links WHERE depends_on = ?1",
                    &[&a],
                    100_000,
                )?;
                let parts = query_json(
                    tx,
                    "SELECT depends_on, needs FROM item_links WHERE item = ?1",
                    &[&a],
                    100_000,
                )?;
                tx.execute("DELETE FROM item_links WHERE item = ?1 OR depends_on = ?1", [a])
                    .map_err(sql)?;
                for d in dependents {
                    depend(tx, d["item"].as_i64().unwrap_or_default(), keep, text(&d, "needs"))?;
                }
                for p in parts {
                    depend(
                        tx,
                        keep,
                        p["depends_on"].as_i64().unwrap_or_default(),
                        text(&p, "needs"),
                    )?;
                }
                cancel(tx, a, &format!("merged into item {keep}: {reason}"))?;
            }
        }
        "cancel" => cancel(tx, item_of(&c["item"], keys)?, text(c, "goes"))?,
        "hold" => {
            tx.execute(
                "UPDATE items SET held = ?2 WHERE id = ?1",
                params![item_of(&c["item"], keys)?, reason],
            )
            .map_err(sql)?;
        }
        "release" => {
            tx.execute(
                "UPDATE items SET held = NULL WHERE id = ?1",
                [item_of(&c["item"], keys)?],
            )
            .map_err(sql)?;
        }
        "edit_item" => {
            let column = match text(c, "part") {
                "title" => "title",
                "problem" => "problem",
                "why" => "why",
                "approach" => "approach",
                other => return Err(format!("an item has no part {other}")),
            };
            tx.execute(
                &format!("UPDATE items SET {column} = ?2 WHERE id = ?1"),
                params![item_of(&c["item"], keys)?, text(c, "text")],
            )
            .map_err(sql)?;
        }
        "edit_acceptance" => {
            tx.execute(
                "UPDATE items SET wanted = ?2 WHERE id = ?1",
                params![item_of(&c["item"], keys)?, json_of("acceptance")],
            )
            .map_err(sql)?;
        }
        "set_level" => {
            tx.execute(
                "UPDATE items SET level = ?2 WHERE id = ?1",
                params![item_of(&c["item"], keys)?, text(c, "level")],
            )
            .map_err(sql)?;
        }
        "add_segment" => {
            tx.execute(
                "INSERT INTO plan_segments (key, project, title, stage, delivers, done_when, after, assumptions,
                                            indicators, size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    text(c, "key"),
                    text(c, "project"),
                    text(c, "title"),
                    text(c, "stage"),
                    text(c, "delivers"),
                    text(c, "done_when"),
                    json_of("after"),
                    json_of("assumptions"),
                    json_of("indicators"),
                    text(c, "size")
                ],
            )
            .map_err(sql)?;
            created.push(json!({ "key": text(c, "key"), "id": text(c, "key") }));
        }
        "edit_segment" => {
            let key = text(c, "segment");
            for (field, column, is_json) in [
                ("title", "title", false),
                ("delivers", "delivers", false),
                ("after", "after", true),
                ("assumptions", "assumptions", true),
                ("indicators", "indicators", true),
                ("size", "size", false),
            ] {
                if c.get(field).is_none() {
                    continue;
                }
                let v = if is_json {
                    json_of(field)
                } else {
                    text(c, field).to_string()
                };
                tx.execute(
                    &format!("UPDATE plan_segments SET {column} = ?2 WHERE key = ?1"),
                    params![key, v],
                )
                .map_err(sql)?;
            }
        }
        "edit_done_when" => {
            tx.execute(
                "UPDATE plan_segments SET done_when = ?2 WHERE key = ?1",
                params![text(c, "segment"), text(c, "done_when")],
            )
            .map_err(sql)?;
        }
        "defer_segment" => {
            let key = text(c, "segment");
            let until = text(c, "until");
            tx.execute(
                "UPDATE plan_segments SET deferred = ?2 WHERE key = ?1",
                params![key, until],
            )
            .map_err(sql)?;
            // The works of its items pause too: those of the items that hang on it, and of the items they depend on.
            let works = query_json(
                tx,
                "WITH RECURSIVE hung(item) AS (
                   SELECT id FROM items WHERE segment = ?1
                   UNION SELECT l.depends_on FROM item_links l JOIN hung h ON l.item = h.item)
                 SELECT w.id FROM works w JOIN hung h ON h.item = w.item WHERE w.ended IS NULL AND w.paused IS NULL",
                &[&key],
                100_000,
            )?;
            let why = format!("{}: segment {key} is deferred until {until}", db::STOPPED_BY_ARCHITECT);
            for w in works {
                let id = w["id"].as_i64().unwrap_or_default();
                tx.execute(
                    "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
                    params![id, why],
                )
                .map_err(sql)?;
                actions::notify(tx, Some(id), &format!("Work {id} is {why}"))?;
            }
        }
        "delete_segment" => {
            let key = text(c, "segment");
            tx.execute("UPDATE items SET segment = NULL WHERE segment = ?1", [key])
                .map_err(sql)?;
            tx.execute(
                "UPDATE plan_segments SET after = (SELECT json_group_array(a.value) FROM json_each(after) a
                                                   WHERE a.value <> ?1)
                 WHERE EXISTS (SELECT 1 FROM json_each(after) a WHERE a.value = ?1)",
                [key],
            )
            .map_err(sql)?;
            tx.execute("DELETE FROM plan_segments WHERE key = ?1", [key])
                .map_err(sql)?;
        }
        "order_segments" => {
            tx.execute("UPDATE plan_segments SET position = NULL", [])
                .map_err(sql)?;
            for (i, k) in texts(c, "order").iter().enumerate() {
                tx.execute(
                    "UPDATE plan_segments SET position = ?2 WHERE key = ?1",
                    params![k, (i + 1) as f64],
                )
                .map_err(sql)?;
            }
        }
        "add_gate" => {
            tx.execute(
                "INSERT INTO plan_gates (name, basis, what, segments) VALUES (?1, ?2, ?3, ?4)",
                params![text(c, "name"), text(c, "basis"), text(c, "what"), json_of("segments")],
            )
            .map_err(sql)?;
        }
        "edit_gate" => {
            tx.execute(
                "UPDATE plan_gates SET segments = ?2 WHERE name = ?1",
                params![text(c, "gate"), json_of("segments")],
            )
            .map_err(sql)?;
        }
        "add_budget" => {
            tx.execute(
                "INSERT INTO plan_budgets (name, basis, measure, line, split) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    text(c, "name"),
                    text(c, "basis"),
                    text(c, "measure"),
                    text(c, "line"),
                    json_of("split")
                ],
            )
            .map_err(sql)?;
        }
        "edit_budget" => {
            if c.get("line").is_some() {
                tx.execute(
                    "UPDATE plan_budgets SET line = ?2 WHERE name = ?1",
                    params![text(c, "budget"), text(c, "line")],
                )
                .map_err(sql)?;
            }
            if c.get("split").is_some() {
                tx.execute(
                    "UPDATE plan_budgets SET split = ?2 WHERE name = ?1",
                    params![text(c, "budget"), json_of("split")],
                )
                .map_err(sql)?;
            }
        }
        "add_known_red_check" => {
            tx.execute(
                "INSERT INTO known_red_checks (project, check_name, why, fix_item) VALUES (?1, ?2, ?3, ?4)",
                params![
                    text(c, "project"),
                    text(c, "check"),
                    text(c, "why"),
                    item_of(&c["fix_item"], keys)?
                ],
            )
            .map_err(sql)?;
        }
        "remove_known_red_check" => {
            tx.execute(
                "DELETE FROM known_red_checks WHERE project = ?1 AND check_name = ?2",
                params![text(c, "project"), text(c, "check")],
            )
            .map_err(sql)?;
        }
        other => return Err(format!("there is no kind of change {other}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_budget_s_amount_is_its_first_number() {
        assert_eq!(amount("+1500 bytes"), Some(1500.0));
        assert_eq!(amount("-2.5 KB"), Some(-2.5));
        assert_eq!(amount("at most 40 rules"), Some(40.0));
        assert_eq!(amount("none"), None);
    }

    #[test]
    fn a_cycle_is_found_and_a_chain_is_not() {
        assert!(cycle(&[(1, 2), (2, 3), (3, 1)]));
        assert!(!cycle(&[(1, 2), (2, 3), (1, 3)]));
        assert!(!cycle(&[]));
    }
}
