//! The changes to the items a project's architect hands in, the flow's `item_change`: what makes them valid, which the
//! effects of an answer and a revision of a batch are checked against, and making a batch of them once its review
//! finds them sound, in the transaction of the review's round.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::Engine;
use crate::work::query_json;

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

/// The item a change names by its number, or by the key an earlier change of the batch created it under.
fn known(v: &Value, s: &State) -> Option<i64> {
    number(v).or_else(|| v.as_str().and_then(|key| s.keys.get(key.trim()).copied()))
}

/// The items, each with its project, and their dependencies, as the changes find them and as each change leaves them.
/// An item the changes create is known by its key until it is written, here as a number below zero.
#[derive(Debug, Default)]
struct State {
    items: BTreeMap<i64, String>,
    links: BTreeSet<(i64, i64)>,
    red: BTreeSet<(String, String)>,
    keys: BTreeMap<String, i64>,
}

impl State {
    fn load(conn: &Connection) -> Result<State, String> {
        let mut s = State::default();
        for r in query_json(conn, "SELECT id, project FROM items", &[], 1_000_000)? {
            s.items
                .insert(r["id"].as_i64().unwrap_or_default(), text(&r, "project").to_string());
        }
        for r in query_json(conn, "SELECT item, depends_on FROM item_links", &[], 1_000_000)? {
            s.links.insert((
                r["item"].as_i64().unwrap_or_default(),
                r["depends_on"].as_i64().unwrap_or_default(),
            ));
        }
        for r in query_json(conn, "SELECT project, check_name FROM known_red_checks", &[], 100_000)? {
            s.red
                .insert((text(&r, "project").to_string(), text(&r, "check_name").to_string()));
        }
        Ok(s)
    }

    /// Whether the items' dependencies close a cycle.
    fn links_cycle(&self) -> bool {
        let edges: Vec<(i64, i64)> = self.links.iter().copied().collect();
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

/// Whether the changes a project's architect handed in are valid, in order: every change has its reason; an item
/// created is of the project and has what it needs and a key no other change uses; a split gives each acceptance
/// condition of the item split to one new item at most, one of its locked conditions when its acceptance is locked;
/// every item a change names is there, or made by an earlier change, and an item it changes is of the project (an item
/// of the project may depend on another project's); an item is moved first, last, under another item, or next to
/// another item of its own project; observations folded in are the project's and open; and after them the items'
/// dependencies close no cycle. Otherwise why not, every reason at once.
pub fn validate(e: &Engine, project: &str, changes: &[Value]) -> Result<(), String> {
    let mut s = State::load(e.db.conn())?;
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
            match known(&c[k], s) {
                Some(id) if s.items.contains_key(&id) => Some(id),
                _ => {
                    bad(format!("there is no item {}", c[k]));
                    None
                }
            }
        };
        // An item a change changes is of the project; one it only waits for may be another's.
        let own = |k: &str, s: &State, bad: &mut dyn FnMut(String)| -> Option<i64> {
            let id = item(k, s, bad)?;
            if s.items.get(&id).map(String::as_str) != Some(project) {
                bad(format!("item {id} is not of {project}: its architect changes it"));
                return None;
            }
            Some(id)
        };
        match text(c, "kind") {
            "create_item" => {
                let key = text(c, "key").trim().to_string();
                if key.is_empty() || s.keys.contains_key(&key) {
                    bad(format!("its key \"{key}\" is empty, or another change uses it"));
                }
                for k in ["project", "title", "problem", "why", "level", "deliver"] {
                    if text(c, k).trim().is_empty() {
                        bad(format!("it has no {k}"));
                    }
                }
                if text(c, "project") != project {
                    bad(format!(
                        "it is of {}, and you make items of {project} only",
                        text(c, "project")
                    ));
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
                let under = c.get("under").filter(|v| !v.is_null());
                if under.is_some()
                    && let Some(parent) = item("under", &s, &mut bad)
                {
                    s.links.insert((parent, id));
                }
                // A split gives each acceptance condition of the item split to one new item at most; the rest stay
                // with that item.
                let takes = texts(c, "takes");
                if !takes.is_empty() {
                    match under.and_then(number).filter(|split| *split > 0) {
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
                s.items.insert(id, text(c, "project").to_string());
                s.keys.insert(key, id);
            }
            "add_dependency" => {
                if let (Some(a), Some(b)) = (own("item", &s, &mut bad), item("on", &s, &mut bad)) {
                    if a == b {
                        bad("an item does not depend on itself".into());
                    }
                    s.links.insert((a, b));
                }
            }
            "move" => {
                if let Some(a) = own("item", &s, &mut bad) {
                    let to = &c["to"];
                    let other = known(&to["item"], &s).filter(|b| s.items.contains_key(b) && *b != a);
                    match text(to, "place") {
                        "first" | "last" => {}
                        "before" | "after" => {
                            if other.is_none_or(|b| s.items.get(&b) != s.items.get(&a)) {
                                bad(format!(
                                    "it moves next to item {}, which is not another item of its project",
                                    to["item"]
                                ));
                            }
                        }
                        "under" => match other {
                            Some(b) => {
                                s.links.retain(|(_, d)| *d != a);
                                s.links.insert((b, a));
                            }
                            None => bad(format!(
                                "it moves under item {}, which there is not, or is itself",
                                to["item"]
                            )),
                        },
                        place => bad(format!("there is no place {place}")),
                    }
                }
            }
            "merge" => {
                if let Some(keep) = own("keep", &s, &mut bad) {
                    for v in items(c.get("absorbed")) {
                        let Some(a) = known(&v, &s).filter(|a| *a != keep && s.items.contains_key(a)) else {
                            bad(format!("item {v} is not another item to merge into item {keep}"));
                            continue;
                        };
                        let links: Vec<(i64, i64)> = s.links.iter().copied().collect();
                        s.links.clear();
                        for (x, d) in links {
                            let (x, d) = (if x == a { keep } else { x }, if d == a { keep } else { d });
                            if x != d {
                                s.links.insert((x, d));
                            }
                        }
                    }
                }
            }
            "cancel" | "hold" | "release" | "edit_acceptance" => {
                own("item", &s, &mut bad);
            }
            "edit_item" => {
                own("item", &s, &mut bad);
                if text(c, "text").trim().is_empty() {
                    bad("it changes the text to nothing".into());
                }
            }
            "set_level" => {
                own("item", &s, &mut bad);
                if cfg.level(text(c, "level")).is_none() {
                    bad(format!("there is no level {}", text(c, "level")));
                }
                if let Err(why) = cfg.item_level(text(c, "level")) {
                    bad(why);
                }
            }
            "add_known_red_check" => {
                if text(c, "project") != project {
                    bad(format!("{} is not this project, {project}", text(c, "project")));
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
            "fold_observations" => {
                own("item", &s, &mut bad);
                for o in items(c.get("observations")) {
                    let id = number(&o).unwrap_or_default();
                    let open: i64 =
                        e.db.conn()
                            .query_row(
                                "SELECT count(*) FROM observations_open WHERE id = ?1 AND project = ?2",
                                params![id, project],
                                |r| r.get(0),
                            )
                            .map_err(sql)?;
                    if open == 0 {
                        bad(format!("{o} is no open observation of {project}"));
                    }
                }
            }
            other => bad(format!("there is no kind of change {other}")),
        }
    }
    if s.links_cycle() {
        problems.push("after the changes, the items' dependencies close a cycle".into());
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Writes every change, in order, and gives what was created: each item, by its key, with its number.
pub fn write_all(tx: &Transaction<'_>, changes: &[Value]) -> Result<Vec<Value>, String> {
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
        .or_else(|| v.as_str().and_then(|k| keys.get(k.trim()).copied()))
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
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level, wanted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    text(c, "project"),
                    text(c, "title"),
                    text(c, "problem"),
                    text(c, "why"),
                    text(c, "deliver"),
                    text(c, "level"),
                    json_of("acceptance")
                ],
            )
            .map_err(sql)?;
            let id = tx.last_insert_rowid();
            if !c["under"].is_null() {
                let parent = item_of(&c["under"], keys)?;
                let takes = texts(c, "takes");
                let needs = if takes.is_empty() {
                    text(c, "title").to_string()
                } else {
                    format!("{} (its acceptance conditions {})", text(c, "title"), takes.join(", "))
                };
                depend(tx, parent, id, &needs)?;
            }
            keys.insert(text(c, "key").trim().to_string(), id);
            created.push(json!({ "key": text(c, "key").trim(), "id": id.to_string() }));
        }
        "add_dependency" => depend(tx, item_of(&c["item"], keys)?, item_of(&c["on"], keys)?, reason)?,
        "move" => {
            let item = item_of(&c["item"], keys)?;
            let to = &c["to"];
            match text(to, "place") {
                "under" => {
                    let under = item_of(&to["item"], keys)?;
                    tx.execute("DELETE FROM item_links WHERE depends_on = ?1", [item])
                        .map_err(sql)?;
                    depend(tx, under, item, reason)?;
                }
                place => {
                    let other = match place {
                        "before" | "after" => Some(item_of(&to["item"], keys)?),
                        _ => None,
                    };
                    rank(tx, item, place, other)?;
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
        "fold_observations" => {
            let item = item_of(&c["item"], keys)?;
            for o in items(c.get("observations")) {
                tx.execute(
                    "UPDATE observations SET folded_into = ?2 WHERE id = ?1",
                    params![number(&o).unwrap_or_default(), item],
                )
                .map_err(sql)?;
            }
        }
        other => return Err(format!("there is no kind of change {other}")),
    }
    Ok(())
}

/// Puts an item at its place among its project's items in the architect's order and ranks every item of the project
/// afresh by that order, 1 the first; the order it starts from is the items' ranks, those with none after, in the order
/// they were made.
fn rank(tx: &Transaction<'_>, item: i64, place: &str, other: Option<i64>) -> Result<(), String> {
    let order: Vec<i64> = query_json(
        tx,
        "SELECT id FROM items WHERE project = (SELECT project FROM items WHERE id = ?1) ORDER BY rank NULLS LAST, id",
        &[&item],
        1_000_000,
    )?
    .iter()
    .filter_map(|r| r["id"].as_i64())
    .collect();
    for (i, id) in placed(&order, item, place, other)?.into_iter().enumerate() {
        tx.execute(
            "UPDATE items SET rank = ?2 WHERE id = ?1 AND rank IS NOT ?2",
            params![id, i as i64 + 1],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// An order of items with one put at its place: first, last, or just before or after another item of the order.
fn placed(order: &[i64], item: i64, place: &str, other: Option<i64>) -> Result<Vec<i64>, String> {
    let mut out: Vec<i64> = order.iter().copied().filter(|x| *x != item).collect();
    let next_to = || -> Result<usize, String> {
        let o = other.ok_or_else(|| format!("item {item} moves {place} another item, which it does not name"))?;
        out.iter()
            .position(|x| *x == o)
            .ok_or_else(|| format!("item {o} is not another item of item {item}'s project"))
    };
    let at = match place {
        "first" => 0,
        "last" => out.len(),
        "before" => next_to()?,
        "after" => next_to()? + 1,
        p => return Err(format!("there is no place {p}")),
    };
    out.insert(at, item);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use db::{Actor, Db};

    use super::*;

    #[test]
    fn a_cycle_is_found_and_a_chain_is_not() {
        assert!(cycle(&[(1, 2), (2, 3), (3, 1)]));
        assert!(!cycle(&[(1, 2), (2, 3), (1, 3)]));
        assert!(!cycle(&[]));
    }

    #[test]
    fn an_item_is_put_first_last_or_next_to_another() {
        let order = [1, 2, 3, 4];
        assert_eq!(placed(&order, 3, "first", None), Ok(vec![3, 1, 2, 4]));
        assert_eq!(placed(&order, 1, "last", None), Ok(vec![2, 3, 4, 1]));
        assert_eq!(placed(&order, 4, "before", Some(2)), Ok(vec![1, 4, 2, 3]));
        assert_eq!(placed(&order, 1, "after", Some(3)), Ok(vec![2, 3, 1, 4]));
        assert!(placed(&order, 1, "after", Some(9)).is_err());
        assert!(placed(&order, 1, "after", Some(1)).is_err());
        assert!(placed(&order, 1, "before", None).is_err());
        assert!(placed(&order, 1, "between", None).is_err());
    }

    // Moving an item ranks every item of its project afresh, those not ranked yet after the others in the order they
    // were made, and leaves the items of another project as they were.
    #[test]
    fn a_move_ranks_the_items_of_the_project_afresh() {
        let mut db = Db::in_memory().unwrap();
        let ranks = |db: &Db| -> Vec<(i64, Option<i64>)> {
            let mut st = db.conn().prepare("SELECT id, rank FROM items ORDER BY id").unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        db.write(Actor::Owner, |tx| {
            for (project, rank) in [("p", Some(2)), ("q", Some(1)), ("p", Some(1)), ("p", None), ("p", None)] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level, rank)
                     VALUES (?1, 't', 'x', 'y', 'change', 'l', ?2)",
                    params![project, rank],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let moved = |db: &mut Db, to: Value| {
            let change = json!({ "kind": "move", "item": 5, "to": to, "reason": "r" });
            db.write(Actor::Architect, |tx| {
                write_all(tx, &[change]).map_err(db::Error::Abort)
            })
            .unwrap();
        };
        moved(&mut db, json!({ "place": "before", "item": 1 }));
        assert_eq!(
            ranks(&db),
            [(1, Some(3)), (2, Some(1)), (3, Some(1)), (4, Some(4)), (5, Some(2))]
        );
        moved(&mut db, json!({ "place": "first" }));
        assert_eq!(
            ranks(&db),
            [(1, Some(3)), (2, Some(1)), (3, Some(2)), (4, Some(4)), (5, Some(1))]
        );
        moved(&mut db, json!({ "place": "under", "item": 1 }));
        let links: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM item_links WHERE item = 1 AND depends_on = 5",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 1);
    }
}
