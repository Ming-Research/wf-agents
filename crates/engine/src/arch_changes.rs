//! The changes to the items a project's architect hands in, the flow's `item_change`: what makes them valid, which the
//! effects of an answer are checked against as they are handed in, and making them, in order, in the transaction of
//! the round that hands them in or of the second eye's that finds them sound.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::Engine;
use crate::work::{RoundId, query_json};

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
    /// The dependencies there are then, which a change may remove.
    links: BTreeSet<(i64, i64)>,
    /// The dependencies a cancel took out of `links`. The write may leave them: a cancel is only asked of the loop when
    /// a work on the item goes on, so they still count for a cycle, though no change can remove them.
    cancelled_links: BTreeSet<(i64, i64)>,
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

    /// Whether the items' dependencies close a cycle, those a cancel took away as far as the write may leave them
    /// included.
    fn links_cycle(&self) -> bool {
        let edges: Vec<(i64, i64)> = self.links.union(&self.cancelled_links).copied().collect();
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

/// Whether a dependency a change makes has its kind, blocking or parallel; otherwise says so.
fn dependency(kind: &Value, bad: &mut dyn FnMut(String)) {
    if !matches!(kind.as_str(), Some("blocking" | "parallel")) {
        bad(format!(
            "the dependency it makes is of no kind, blocking or parallel: {kind}"
        ));
    }
}

/// What is said of a line of a decision that a change writes by hand.
fn hand_written(line: &str) -> String {
    format!(
        "it writes the line \"{line}\" of a decision, which the item's text does not have: an item holds no new \
         decision line; what is to go to the project's decisions is written as a decision point, under the head \
         `Decision point:`, and a line the item has stands letter for letter as its history"
    )
}

/// The route a change that creates an item gives it: the normal route when it gives none.
fn route_of(c: &Value) -> &str {
    match text(c, "route").trim() {
        "" => "normal",
        r => r,
    }
}

/// Why a route a change gives an item does not fit it: no route there is, or the light route of a bug fix or a feature
/// for an item that delivers a report, which only a change goes; none when it fits.
fn route_unfit(route: &str, deliver: &str) -> Option<String> {
    match route {
        "normal" => None,
        "bugfix" | "feature" if deliver == "change" => None,
        "bugfix" | "feature" => Some(format!(
            "it gives the route {route} to an item that delivers a {deliver}: only a change goes the light route"
        )),
        other => Some(format!(
            "there is no route \"{other}\": a route is normal, bugfix or feature"
        )),
    }
}

/// Whether a title a change gives, an item's or an observation's, is within `max`, the installation's `items.title_max`,
/// as [`config::title_width`] counts it; otherwise says how wide it is.
pub(crate) fn title_fits(title: &str, max: usize, bad: &mut dyn FnMut(String)) {
    if let Err(why) = config::title_fits(title, max) {
        bad(why);
    }
}

/// Whether the changes a project's architect handed in are valid, in order: every change has its reason; an item
/// created is of the project and has what it needs and a key no other change uses; a title an item is created or
/// retitled with is within the installation's `items.title_max` ([`config::title_width`]); a route an item is created
/// with or set to is one there is, the light route of a bug fix or a feature only for an item that delivers a change; a
/// split gives each acceptance condition of the item split to one new item at most, one of its locked conditions when
/// its acceptance is locked; every item a change names is there, or made by an earlier change, and an item it changes
/// is of the project (an item of the project may depend on another project's); every dependency a change makes has its
/// kind; every dependency a change removes is one the items have then, stored or made by an earlier change and not
/// removed or cancelled away since; an item is moved first, last, under another item, or next to another item of its
/// own project; observations folded in are the project's and open; a merge condition has its sentence, and its quotation
/// is found letter for letter in its record, which is not the architect's own words; and after them the items'
/// dependencies close no cycle. Otherwise why not, every reason at once.
pub fn validate(e: &Engine, project: &str, changes: &[Value]) -> Result<(), String> {
    validate_from(e, project, changes, 0)
}

/// [`validate`], saying what is wrong with the changes from the `first`-th on only: those before it stand as the
/// changes that come after must find them, and are not judged again. No change writes a line of a decision by hand:
/// an item holds no new decision line, and keeps those it has as its history.
pub fn validate_from(e: &Engine, project: &str, changes: &[Value], first: usize) -> Result<(), String> {
    let mut s = State::load(e.db.conn())?;
    let cfg = &e.config.config;
    let mut problems: Vec<String> = Vec::new();
    let mut next_new = -1;
    // The acceptance conditions each item split gives to its new items so far.
    let mut taken: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    // What each item the changes create delivers, which the route a later change gives it must fit.
    let mut delivers: BTreeMap<i64, String> = BTreeMap::new();
    // The `approach` of the items the changes make so far.
    let mut approaches = crate::decisions::Approaches::default();
    for (i, c) in changes.iter().enumerate() {
        let n = i + 1;
        let mut bad = |p: String| {
            if i >= first {
                problems.push(format!("change {n} ({}): {p}", text(c, "kind")));
            }
        };
        if text(c, "reason").trim().is_empty() {
            bad("it has no reason".into());
        }
        let item = |v: &Value, s: &State, bad: &mut dyn FnMut(String)| -> Option<i64> {
            match known(v, s) {
                Some(id) if s.items.contains_key(&id) => Some(id),
                _ => {
                    bad(format!("there is no item {v}"));
                    None
                }
            }
        };
        // An item a change changes is of the project; one it only waits for may be another's.
        let own = |v: &Value, s: &State, bad: &mut dyn FnMut(String)| -> Option<i64> {
            let id = item(v, s, bad)?;
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
                title_fits(text(c, "title"), cfg.install.items.title_max as usize, &mut bad);
                for l in crate::decisions::new_decision_lines("", text(c, "approach")) {
                    bad(hand_written(&l));
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
                if let Some(why) = route_unfit(route_of(c), text(c, "deliver")) {
                    bad(why);
                }
                let id = next_new;
                next_new -= 1;
                delivers.insert(id, text(c, "deliver").to_string());
                let under = c.get("under").filter(|v| !v.is_null());
                if let Some(u) = under {
                    dependency(&u["dependency"], &mut bad);
                    if let Some(parent) = item(&u["item"], &s, &mut bad) {
                        s.links.insert((parent, id));
                    }
                }
                // A split gives each acceptance condition of the item split to one new item at most; the rest stay
                // with that item.
                let takes = texts(c, "takes");
                if !takes.is_empty() {
                    match under.and_then(|u| number(&u["item"])).filter(|split| *split > 0) {
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
                dependency(&c["dependency"], &mut bad);
                if let (Some(a), Some(b)) = (own(&c["item"], &s, &mut bad), item(&c["on"], &s, &mut bad)) {
                    if a == b {
                        bad("an item does not depend on itself".into());
                    }
                    s.links.insert((a, b));
                }
            }
            "remove_dependency" => {
                if let (Some(a), Some(b)) = (own(&c["item"], &s, &mut bad), item(&c["on"], &s, &mut bad)) {
                    // The links are the stored ones and what the earlier changes of the batch made of them: one that
                    // an earlier change added is there to remove, one that an earlier change removed or cancelled
                    // away is not.
                    if !s.links.remove(&(a, b)) {
                        bad(format!("item {a} does not depend on item {b}"));
                    }
                }
            }
            "move" => {
                if let Some(a) = own(&c["item"], &s, &mut bad) {
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
                                dependency(&to["dependency"], &mut bad);
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
                if let Some(keep) = own(&c["keep"], &s, &mut bad) {
                    for v in items(c.get("absorbed")) {
                        let Some(a) = known(&v, &s).filter(|a| *a != keep && s.items.contains_key(a)) else {
                            bad(format!("item {v} is not another item to merge into item {keep}"));
                            continue;
                        };
                        approaches.merged(e.db.conn(), keep, a);
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
            "cancel" => {
                // A cancelled item no longer depends on any item or holds any back, as `cancel_now` leaves it when no
                // work goes on the item, so no later change removes those dependencies. When a work goes on, they stay
                // until the loop cancels the item, and close a cycle with what the later changes add.
                if let Some(a) = own(&c["item"], &s, &mut bad) {
                    let away: Vec<(i64, i64)> = s.links.iter().filter(|(x, d)| *x == a || *d == a).copied().collect();
                    for l in away {
                        s.links.remove(&l);
                        s.cancelled_links.insert(l);
                    }
                }
            }
            "hold" | "release" | "edit_acceptance" => {
                own(&c["item"], &s, &mut bad);
            }
            "edit_item" => {
                let id = own(&c["item"], &s, &mut bad);
                if text(c, "text").trim().is_empty() {
                    bad("it changes the text to nothing".into());
                }
                if text(c, "part") == "title" {
                    title_fits(text(c, "text"), cfg.install.items.title_max as usize, &mut bad);
                }
                if text(c, "part") == "route"
                    && let Some(id) = id
                {
                    // An item the batch creates delivers what its change says; any other, what it holds.
                    let deliver = match delivers.get(&id) {
                        Some(d) => d.clone(),
                        None => {
                            e.db.conn()
                                .query_row("SELECT deliver FROM items WHERE id = ?1", [id], |r| r.get(0))
                                .map_err(sql)?
                        }
                    };
                    if let Some(why) = route_unfit(text(c, "text").trim(), &deliver) {
                        bad(why);
                    }
                }
                if text(c, "part") == "approach" {
                    // The lines of decisions the text had stand; a line it did not have is a decision written by hand.
                    // The text it has now is the one the batch has made of it: a merge of an earlier change carried the
                    // lines of the absorbed item's decisions into it.
                    let now = id.map(|id| approaches.of(e.db.conn(), id)).unwrap_or_default();
                    let now = now.as_str();
                    for l in crate::decisions::new_decision_lines(now, text(c, "text")) {
                        bad(hand_written(&l));
                    }
                    for why in crate::decisions::unkept(now, text(c, "text")) {
                        bad(format!(
                            "{why}: the lines of the item's decisions stand letter for letter, in their order, under \
                             the head {}",
                            crate::decisions::HEAD
                        ));
                    }
                    if let Some(id) = id {
                        approaches.written(id, text(c, "text"));
                    }
                }
            }
            "set_level" => {
                own(&c["item"], &s, &mut bad);
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
            "add_merge_condition" => {
                own(&c["item"], &s, &mut bad);
                if text(c, "what").trim().is_empty() {
                    bad("it has no sentence of what must be done before the merge".into());
                }
                if let Err(why) = crate::conditions::source_said_by(e.db.conn(), project, &c["source"]) {
                    bad(why);
                }
            }
            "fold_observations" => {
                own(&c["item"], &s, &mut bad);
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

/// What writing a batch of changes came to.
#[derive(Debug, Default)]
pub struct Written {
    /// Each item created, by its key, with its number.
    pub created: Vec<Value>,
    /// The changes the write refused, each by its place in the batch, with why. The review and the check of the reply
    /// found them valid in the state they read, but the changes left out since (the review drops some) leave a state in
    /// which they are not: a dependency that would close a cycle. A refused change leaves nothing of itself written.
    pub refused: Vec<(usize, String)>,
}

/// Why a change was not written.
enum Fault {
    /// The change cannot be made in the state the earlier changes left: it alone is left out.
    Refused(String),
    /// The write cannot go on: the batch is given up.
    Failed(String),
}

impl From<String> for Fault {
    fn from(why: String) -> Fault {
        Fault::Failed(why)
    }
}

impl From<&str> for Fault {
    fn from(why: &str) -> Fault {
        Fault::Failed(why.to_string())
    }
}

/// Writes every change, in order, each so that a change refused leaves nothing of itself, and gives what was created and
/// what was refused. `projects` are the configuration's, for the works a cancellation ends; `round` is the architect's
/// round that makes them, which a merge condition records.
pub fn write_all(
    tx: &Transaction<'_>,
    changes: &[Value],
    projects: &BTreeMap<String, config::Project>,
    round: RoundId,
) -> Result<Written, String> {
    let mut keys: BTreeMap<String, i64> = BTreeMap::new();
    let mut written = Written::default();
    for (i, c) in changes.iter().enumerate() {
        tx.execute_batch("SAVEPOINT change").map_err(sql)?;
        match write_one(tx, c, &mut keys, &mut written.created, projects, round) {
            Ok(()) => tx.execute_batch("RELEASE change").map_err(sql)?,
            Err(Fault::Refused(why)) => {
                tx.execute_batch("ROLLBACK TO change; RELEASE change").map_err(sql)?;
                written.refused.push((i, why));
            }
            Err(Fault::Failed(why)) => return Err(why),
        }
    }
    Ok(written)
}

/// The item a change names by its number, or by the key an earlier change created it under.
fn item_of(v: &Value, keys: &BTreeMap<String, i64>) -> Result<i64, String> {
    number(v)
        .or_else(|| v.as_str().and_then(|k| keys.get(k.trim()).copied()))
        .ok_or_else(|| format!("there is no item {v}"))
}

/// Makes one item depend on another by a dependency of the kind given, unless it does already; refused when the other
/// already depends on the item, directly or not.
fn depend(tx: &Transaction<'_>, item: i64, on: i64, needs: &str, kind: &str) -> Result<(), Fault> {
    let has: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM item_links WHERE item = ?1 AND depends_on = ?2",
            [item, on],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    if has.is_none() && item != on {
        match db::add_dependency(tx, item, on, needs, kind) {
            Ok(()) => {}
            Err(e @ db::Error::Cycle { .. }) => return Err(Fault::Refused(e.to_string())),
            Err(e) => return Err(Fault::Failed(e.to_string())),
        }
    }
    Ok(())
}

fn write_one(
    tx: &Transaction<'_>,
    c: &Value,
    keys: &mut BTreeMap<String, i64>,
    created: &mut Vec<Value>,
    projects: &BTreeMap<String, config::Project>,
    round: RoundId,
) -> Result<(), Fault> {
    // What came of it reaches the architect by itself: an item whose merge came first is not cancelled, and a report
    // says so; the rest of the batch stands either way, as its links moved before the cancel do.
    let cancel = |item: i64, why: &str| crate::ending::cancel_item(tx, item, why, "architect", projects).map(|_| ());
    let reason = text(c, "reason");
    let json_of = |k: &str| c.get(k).cloned().unwrap_or(Value::Null).to_string();
    match text(c, "kind") {
        "create_item" => {
            // How to do it, when the creator knows; none when it gave none. The decision points a discussion settled,
            // each with the owner's choice, which take the item to deep research; none when it carries none.
            let approach = Some(text(c, "approach").trim()).filter(|a| !a.is_empty());
            let points = c["decision_points"]
                .as_array()
                .filter(|p| !p.is_empty())
                .map(|p| Value::Array(p.clone()).to_string());
            tx.execute(
                "INSERT INTO items (project, title, problem, why, approach, deliver, level, wanted, route, decision_points)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    text(c, "project"),
                    text(c, "title"),
                    text(c, "problem"),
                    text(c, "why"),
                    approach,
                    text(c, "deliver"),
                    text(c, "level"),
                    json_of("acceptance"),
                    route_of(c),
                    points
                ],
            )
            .map_err(sql)?;
            let id = tx.last_insert_rowid();
            if !c["under"].is_null() {
                let parent = item_of(&c["under"]["item"], keys)?;
                let takes = texts(c, "takes");
                let needs = if takes.is_empty() {
                    text(c, "title").to_string()
                } else {
                    format!("{} (its acceptance conditions {})", text(c, "title"), takes.join(", "))
                };
                depend(tx, parent, id, &needs, text(&c["under"], "dependency"))?;
            }
            keys.insert(text(c, "key").trim().to_string(), id);
            created.push(json!({ "key": text(c, "key").trim(), "id": id.to_string() }));
        }
        "add_dependency" => depend(
            tx,
            item_of(&c["item"], keys)?,
            item_of(&c["on"], keys)?,
            reason,
            text(c, "dependency"),
        )?,
        // No row to remove is no fault: the review found the dependency there, and the owner may have removed it since,
        // which leaves the state the change is to bring about.
        "remove_dependency" => {
            tx.execute(
                "DELETE FROM item_links WHERE item = ?1 AND depends_on = ?2",
                [item_of(&c["item"], keys)?, item_of(&c["on"], keys)?],
            )
            .map_err(sql)?;
        }
        "move" => {
            let item = item_of(&c["item"], keys)?;
            let to = &c["to"];
            match text(to, "place") {
                "under" => {
                    let under = item_of(&to["item"], keys)?;
                    tx.execute("DELETE FROM item_links WHERE depends_on = ?1", [item])
                        .map_err(sql)?;
                    depend(tx, under, item, reason, text(to, "dependency"))?;
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
                    "SELECT item, needs, kind FROM item_links WHERE depends_on = ?1",
                    &[&a],
                    100_000,
                )?;
                let parts = query_json(
                    tx,
                    "SELECT depends_on, needs, kind FROM item_links WHERE item = ?1",
                    &[&a],
                    100_000,
                )?;
                tx.execute("DELETE FROM item_links WHERE item = ?1 OR depends_on = ?1", [a])
                    .map_err(sql)?;
                for d in dependents {
                    depend(
                        tx,
                        d["item"].as_i64().unwrap_or_default(),
                        keep,
                        text(&d, "needs"),
                        text(&d, "kind"),
                    )?;
                }
                for p in parts {
                    depend(
                        tx,
                        keep,
                        p["depends_on"].as_i64().unwrap_or_default(),
                        text(&p, "needs"),
                        text(&p, "kind"),
                    )?;
                }
                // The decisions that bear on the absorbed item bear on the one kept: its lines go there, as they are.
                let approach_of = |id: i64| -> Result<String, String> {
                    let a: Option<String> = tx
                        .query_row("SELECT approach FROM items WHERE id = ?1", [id], |r| r.get(0))
                        .map_err(sql)?;
                    Ok(a.unwrap_or_default())
                };
                let (kept, absorbed) = (approach_of(keep)?, approach_of(a)?);
                let carried = crate::decisions::carry(&kept, &absorbed);
                if carried != kept {
                    tx.execute("UPDATE items SET approach = ?2 WHERE id = ?1", params![keep, carried])
                        .map_err(sql)?;
                }
                cancel(a, &format!("merged into item {keep}: {reason}"))?;
            }
        }
        "cancel" => cancel(item_of(&c["item"], keys)?, text(c, "goes"))?,
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
                "route" => "route",
                other => return Err(format!("an item has no part {other}").into()),
            };
            // A route is a word, which the check found to be one there is.
            let value = if column == "route" {
                text(c, "text").trim()
            } else {
                text(c, "text")
            };
            tx.execute(
                &format!("UPDATE items SET {column} = ?2 WHERE id = ?1"),
                params![item_of(&c["item"], keys)?, value],
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
        "add_merge_condition" => {
            let item = item_of(&c["item"], keys)?;
            let project: String = tx
                .query_row("SELECT project FROM items WHERE id = ?1", [item], |r| r.get(0))
                .map_err(sql)?;
            let source = &c["source"];
            let said_by = crate::conditions::source_said_by(tx, &project, source).map_err(Fault::Refused)?;
            db::record_merge_condition(
                tx,
                &db::MergeCondition {
                    item,
                    what: text(c, "what"),
                    source: text(source, "from"),
                    source_id: source["id"].as_i64().unwrap_or_default(),
                    quote: text(source, "quote"),
                    said_by,
                    recorded_by: "architect",
                    round: Some((round.work, round.n)),
                },
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
        other => return Err(format!("there is no kind of change {other}").into()),
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

    /// The architect's round that makes the changes of a test.
    const ROUND: RoundId = RoundId { work: 1, n: 1 };

    // A merge condition the architect records keeps the sentence it wrote and says who the record says said the words: the
    // owner, a part a delegate declared the owner's own as relayed, or the delegate's own words as the delegate's. A
    // quotation its record does not hold, and a record of the architect's own words, are refused and write nothing.
    #[test]
    fn a_merge_condition_is_recorded_saying_who_said_its_words_and_is_refused_when_its_record_does_not_hold_them() {
        let mut db = Db::in_memory().unwrap();
        let change = |from: &str, id: i64, quote: &str| {
            json!({ "kind": "add_merge_condition", "item": 1, "what": "Run it twice before the merge.",
                    "source": { "from": from, "id": id, "quote": quote }, "reason": "r" })
        };
        let written = db
            .write(Actor::Architect, |tx| {
                tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', 't', 'x', 'y', 'change', '1')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'M1')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at)
                     VALUES (1, 1, 'I1', '{}', 'c', 1)",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO owner_asks (work, round, body, answer) VALUES (1, 1, '{}',
                     '{\"by\":\"owner\",\"at\":1,\"text\":\"OK\",\"speaker\":\"owner\"}')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, detail, what, body, at)
                     VALUES ('p', 'owner_words', 'said', 'w', ?1, 1)",
                    [
                        json!({ "text": "Run it twice first. Then ship.", "where": "page", "speaker": "delegate",
                             "relayed": ["Run it twice first."] })
                        .to_string(),
                    ],
                )?;
                tx.execute(
                    "INSERT INTO interjections (work, speaker, text) VALUES (1, 'architect', 'Run it once.')",
                    [],
                )?;
                let changes = [
                    change("ask", 1, "OK"),
                    change("matter", 1, "Run it twice first."),
                    change("matter", 1, "Then ship."),
                    change("ask", 1, "Never said."),
                    change("interjection", 1, "Run it once."),
                ];
                write_all(tx, &changes, &BTreeMap::new(), ROUND).map_err(db::Error::Abort)
            })
            .unwrap();
        let refused: Vec<(usize, bool)> = written
            .refused
            .iter()
            .map(|(i, why)| (*i, why.contains("not found") || why.contains("architect's own words")))
            .collect();
        assert_eq!(refused, [(3, true), (4, true)], "{:?}", written.refused);
        let rows = query_json(
            db.conn(),
            "SELECT what, source, source_id, quote, said_by, recorded_by, recorded_work, recorded_round, status
             FROM item_merge_conditions ORDER BY id",
            &[],
            10,
        )
        .unwrap();
        let row = |source: &str, id: i64, quote: &str, said_by: &str| {
            json!({ "what": "Run it twice before the merge.", "source": source, "source_id": id, "quote": quote,
                    "said_by": said_by, "recorded_by": "architect", "recorded_work": 1, "recorded_round": 1,
                    "status": "open" })
        };
        assert_eq!(
            rows,
            [
                row("ask", 1, "OK", "owner"),
                row("matter", 1, "Run it twice first.", "relayed"),
                row("matter", 1, "Then ship.", "delegate"),
            ]
        );
    }

    #[test]
    fn a_cycle_is_found_and_a_chain_is_not() {
        assert!(cycle(&[(1, 2), (2, 3), (3, 1)]));
        assert!(!cycle(&[(1, 2), (2, 3), (1, 3)]));
        assert!(!cycle(&[]));
    }

    // Only an item that delivers a change goes the light route of a bug fix or a feature; a report goes the normal
    // route, and a route that is none of the three is refused saying which there are. An item made with no route goes
    // the normal route, and the database refuses a report on the light route however it is written.
    #[test]
    fn only_a_change_goes_the_light_route() {
        for route in ["normal", "bugfix", "feature"] {
            assert_eq!(route_unfit(route, "change"), None, "{route}");
        }
        assert_eq!(route_unfit("normal", "report"), None);
        assert_eq!(
            route_unfit("bugfix", "report").as_deref(),
            Some("it gives the route bugfix to an item that delivers a report: only a change goes the light route")
        );
        assert_eq!(
            route_unfit("light", "change").as_deref(),
            Some("there is no route \"light\": a route is normal, bugfix or feature")
        );
        assert_eq!(route_of(&json!({ "kind": "create_item" })), "normal");
        assert_eq!(
            route_of(&json!({ "kind": "create_item", "route": "feature" })),
            "feature"
        );
        let mut db = Db::in_memory().unwrap();
        let made = |db: &mut Db, deliver: &str, route: &str| {
            db.write(Actor::Architect, |tx| {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level, route)
                     VALUES ('p', 't', 'x', 'y', ?1, 'l', ?2)",
                    params![deliver, route],
                )?;
                Ok(())
            })
        };
        assert!(made(&mut db, "change", "bugfix").is_ok());
        assert!(made(&mut db, "report", "normal").is_ok());
        assert!(made(&mut db, "report", "feature").is_err());
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
                write_all(tx, &[change], &BTreeMap::new(), ROUND).map_err(db::Error::Abort)
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
        moved(
            &mut db,
            json!({ "place": "under", "item": 1, "dependency": "blocking" }),
        );
        let links: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM item_links WHERE item = 1 AND depends_on = 5 AND kind = 'blocking'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 1);
    }

    // A change that removes a dependency deletes that row and no other. One whose row is gone already, removed from the
    // owner's page after the review found it, leaves the dependencies as they are and is no fault.
    #[test]
    fn a_dependency_is_removed_and_one_that_is_gone_already_is_no_fault() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Owner, |tx| {
            for title in ["parser", "lexer", "tokens"] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
                    [title],
                )?;
            }
            db::add_dependency(tx, 1, 2, "its lexer", "blocking")?;
            db::add_dependency(tx, 1, 3, "its tokens", "parallel")?;
            db::add_dependency(tx, 3, 2, "its lexer", "parallel")?;
            Ok(())
        })
        .unwrap();
        let removed = |db: &mut Db, item: &str, on: &str| {
            let change = json!({ "kind": "remove_dependency", "item": item, "on": on, "reason": "r" });
            db.write(Actor::Architect, |tx| {
                write_all(tx, &[change], &BTreeMap::new(), ROUND).map_err(db::Error::Abort)
            })
            .unwrap();
        };
        let links = |db: &Db| -> Vec<(i64, i64)> {
            let mut st = db
                .conn()
                .prepare("SELECT item, depends_on FROM item_links ORDER BY item, depends_on")
                .unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        removed(&mut db, "1", "2");
        assert_eq!(links(&db), [(1, 3), (3, 2)]);
        removed(&mut db, "1", "2");
        assert_eq!(links(&db), [(1, 3), (3, 2)]);
    }

    // A change that cannot be made in the state the earlier changes leave, as when the review dropped the removal that
    // made room for a reversing dependency, is refused alone, with why: nothing of it stays, not what it did before it
    // found the fault (a move under another item takes the links to the item first), and the changes after it are made.
    #[test]
    fn a_dependency_that_closes_a_cycle_is_refused_alone_and_leaves_nothing() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Owner, |tx| {
            for title in ["parser", "lexer", "tokens"] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
                    [title],
                )?;
            }
            // The parser depends on the lexer, and the lexer on the tokens.
            db::add_dependency(tx, 1, 2, "its lexer", "blocking")?;
            db::add_dependency(tx, 2, 3, "its tokens", "blocking")?;
            Ok(())
        })
        .unwrap();
        let changes = [
            // The tokens depend on the parser: a cycle, the parser reaching the tokens.
            json!({ "kind": "add_dependency", "item": "3", "on": "1", "dependency": "blocking", "reason": "r" }),
            // The lexer goes under the tokens: the links to the lexer go first, then the tokens depend on the lexer,
            // which depends on the tokens.
            json!({ "kind": "move", "item": "2", "to": { "place": "under", "item": "3", "dependency": "blocking" },
                    "reason": "r" }),
            json!({ "kind": "hold", "item": "3", "reason": "Held." }),
        ];
        let written = db
            .write(Actor::Architect, |tx| {
                write_all(tx, &changes, &BTreeMap::new(), ROUND).map_err(db::Error::Abort)
            })
            .unwrap();
        let refused: Vec<(usize, bool)> = written
            .refused
            .iter()
            .map(|(i, why)| (*i, why.contains("cannot depend on")))
            .collect();
        assert_eq!(refused, [(0, true), (1, true)], "{written:?}");
        let mut st = db
            .conn()
            .prepare("SELECT item, depends_on FROM item_links ORDER BY item, depends_on")
            .unwrap();
        let links: Vec<(i64, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        assert_eq!(links, [(1, 2), (2, 3)]);
        let held: Option<String> = db
            .conn()
            .query_row("SELECT held FROM items WHERE id = 3", [], |r| r.get(0))
            .unwrap();
        assert_eq!(held.as_deref(), Some("Held."));
    }

    // The architect's cancellation of an item, and its merge of one item into another, ask the engine's loop to cancel
    // the item whose work goes on, which it does once the work has no step in flight; the work on the item kept goes on.
    #[test]
    fn a_cancelled_or_merged_item_with_a_work_going_on_is_asked_of_the_loop() {
        let mut db = Db::in_memory().unwrap();
        let p: config::Project = serde_json::from_value(json!({
            "repo": "o/p", "checkout": "/checkout", "design_goal": "simple",
            "test_paths": ["tests/"], "ci": { "fast": "ci", "full": ["ci"], "merge_checks": ["check"] }
        }))
        .unwrap();
        let projects = BTreeMap::from([("p".to_string(), p)]);
        db.write(Actor::Architect, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for (title, pr) in [("kept", 1), ("cancelled", 2), ("merged", 3)] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
                    [title],
                )?;
                let item = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at, worktree, pr)
                     VALUES (?1, 1, 'engineer', 'implement', ?2, ?3)",
                    params![item, format!("/w/{item}"), pr],
                )?;
                let work = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, 'engineer', 'claude', ?2)",
                    params![work, title],
                )?;
            }
            let changes = [
                json!({ "kind": "cancel", "item": "2", "goes": "nowhere", "reason": "r" }),
                json!({ "kind": "merge", "keep": "1", "absorbed": ["3"], "reason": "the same" }),
            ];
            write_all(tx, &changes, &projects, ROUND).map_err(db::Error::Abort)?;
            Ok(())
        })
        .unwrap();
        // Nothing has ended: the transaction of a batch cannot know what the loop has in flight.
        let going: Vec<i64> = query_json(db.conn(), "SELECT work FROM sessions WHERE ended IS NULL", &[], 10)
            .unwrap()
            .iter()
            .filter_map(|r| r["work"].as_i64())
            .collect();
        assert_eq!(going, [1, 2, 3]);
        let actions: Vec<(String, Value)> = query_json(
            db.conn(),
            "SELECT kind, json_extract(args, '$.item') AS item FROM outbox ORDER BY id",
            &[],
            10,
        )
        .unwrap()
        .iter()
        .map(|r| (text(r, "kind").to_string(), r["item"].clone()))
        .collect();
        assert_eq!(
            actions,
            [
                (db::CANCEL_ITEM.to_string(), json!(2)),
                (db::CANCEL_ITEM.to_string(), json!(3)),
            ]
        );
    }

    // A merge keeps the decisions that bear on the absorbed item: their lines go into the kept item's approach, under its
    // head and in their order, each once, and the absorbed item keeps its own.
    #[test]
    fn a_merge_carries_the_decisions_of_the_absorbed_item_to_the_one_kept() {
        let (a, b) = ("Decision (2026-11-01, owner): A.", "Decision (2026-11-02, owner): B.");
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Architect, |tx| {
            for approach in [
                Some(format!("Approach:\n- Keep.\nDecisions:\n{a}\nLearned:\nx")),
                Some(format!("Approach:\n- Gone.\nDecisions:\n{a}\n{b}")),
                None,
                Some(format!("Approach:\n- Gone.\nDecisions:\n{a}\n{b}")),
            ] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, approach, deliver, level)
                     VALUES ('p', 't', 'x', 'y', ?1, 'change', 'l')",
                    [approach],
                )?;
            }
            let merge = |keep: &str, gone: &str| {
                json!({ "kind": "merge", "keep": keep, "absorbed": [gone], "reason": "the same" })
            };
            write_all(tx, &[merge("1", "2"), merge("3", "4")], &BTreeMap::new(), ROUND).map_err(db::Error::Abort)?;
            Ok(())
        })
        .unwrap();
        let approach = |id: i64| -> Option<String> {
            db.conn()
                .query_row("SELECT approach FROM items WHERE id = ?1", [id], |r| r.get(0))
                .unwrap()
        };
        // The first has the first line already: the second comes after it, before `Learned:`.
        assert_eq!(
            approach(1).unwrap(),
            format!("Approach:\n- Keep.\nDecisions:\n{a}\n{b}\nLearned:\nx")
        );
        assert_eq!(
            approach(2).unwrap(),
            format!("Approach:\n- Gone.\nDecisions:\n{a}\n{b}")
        );
        // The third had no text: it has the head and both lines.
        assert_eq!(approach(3).unwrap(), format!("Decisions:\n{a}\n{b}"));
    }

    // An observation folded into an item the batch creates, named by its key, is folded into that item once it is made.
    #[test]
    fn an_observation_is_folded_into_an_item_the_batch_creates() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Architect, |tx| {
            tx.execute(
                "INSERT INTO observations (project, what, source) VALUES ('p', 'Parsing is slow.', 'A report.')",
                [],
            )?;
            let changes = [
                json!({ "kind": "create_item", "key": "faster", "project": "p", "title": "Parse faster",
                        "problem": "Slow.", "why": "Time.", "acceptance": ["Fast."], "level": "l",
                        "deliver": "change", "reason": "r" }),
                json!({ "kind": "fold_observations", "item": "faster", "observations": [1], "reason": "r" }),
            ];
            write_all(tx, &changes, &BTreeMap::new(), ROUND).map_err(db::Error::Abort)?;
            Ok(())
        })
        .unwrap();
        let folded: (Option<i64>, i64) = db
            .conn()
            .query_row(
                "SELECT folded_into, (SELECT id FROM items WHERE title = 'Parse faster') FROM observations WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(folded.0, Some(folded.1));
    }

    // An item made keeps how to do it when its creator knew, as the item's approach, and has none when it gave none.
    #[test]
    fn a_made_item_keeps_the_approach_its_creator_knew() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Architect, |tx| {
            let made = |key: &str, approach: Value| {
                json!({ "kind": "create_item", "key": key, "project": "p", "title": key, "problem": "Slow.",
                        "why": "Time.", "approach": approach, "acceptance": ["Fast."], "level": "l",
                        "deliver": "change", "reason": "r" })
            };
            let changes = [made("known", json!("Cache the parse.")), made("unknown", Value::Null)];
            write_all(tx, &changes, &BTreeMap::new(), ROUND).map_err(db::Error::Abort)?;
            Ok(())
        })
        .unwrap();
        let approaches: Vec<Value> = query_json(db.conn(), "SELECT approach FROM items ORDER BY id", &[], 10)
            .unwrap()
            .iter()
            .map(|r| r["approach"].clone())
            .collect();
        assert_eq!(approaches, [json!("Cache the parse."), Value::Null]);
    }

    // Item #217, R232-45: an architect's edit of an item's acceptance changes the text the item was written with
    // (`wanted`), and the conditions locked after design stay as the design, the advisor or the owner's ruling left
    // them. The inputs of the engineer cells say so; this is the engine doing as they say.
    #[test]
    fn editing_the_acceptance_of_an_item_leaves_the_locked_conditions_as_they_are() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Architect, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level, wanted)
                 VALUES ('p', 't', 'x', 'y', 'change', 'l', '[\"Old wording.\"]')",
                [],
            )?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'D2', '{}', 'c', 0)",
                [],
            )?;
            tx.execute(
                "UPDATE items SET acceptance = '[{\"id\":\"A1\"}]', acceptance_work = 1, acceptance_round = 1",
                [],
            )?;
            let edit = json!({ "kind": "edit_acceptance", "item": 1, "acceptance": ["New wording."], "reason": "r" });
            write_all(tx, &[edit], &BTreeMap::new(), ROUND).map_err(db::Error::Abort)?;
            Ok(())
        })
        .unwrap();
        let (wanted, locked): (String, String) = db
            .conn()
            .query_row("SELECT wanted, acceptance FROM items WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(
            (wanted.as_str(), locked.as_str()),
            ("[\"New wording.\"]", "[{\"id\":\"A1\"}]")
        );
    }
}
