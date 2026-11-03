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
        "it writes the line \"{line}\" of a decision, which the item's text does not have: a decision is recorded by \
         the cell that records decisions, which makes its line from the records it quotes; a line the item has \
         stands letter for letter"
    )
}

/// The change that records a decision in the batch, before the engine reads its records: the decision as it was handed
/// in, with the project it is of and the matter of the batch it comes from. The engine adds what it read of the records
/// (`resolved`) when it collects the change, so that the review reads what is written.
pub fn record_decision_of(project: &str, matter: i64, decision: &Value) -> Value {
    json!({
        "kind": "record_decision",
        "reason": format!("Records the decision that the words of matter {matter} make, from the records it quotes"),
        "project": project,
        "matter": matter,
        "decision": decision,
    })
}

/// The most characters an item's title has, spaces not counted, and an observation's.
const TITLE_MOST: usize = 20;

/// How many characters a title has, spaces not counted, as the earlier board counted them.
fn title_length(title: &str) -> usize {
    title.chars().filter(|c| !c.is_whitespace()).count()
}

/// Whether a title a change gives, an item's or an observation's, is short enough; otherwise says how long it is.
pub(crate) fn title_fits(title: &str, bad: &mut dyn FnMut(String)) {
    let n = title_length(title);
    if n > TITLE_MOST {
        bad(format!(
            "the title it gives has {n} characters, spaces not counted, and a title has {TITLE_MOST} at most: \
             write it shorter"
        ));
    }
}

/// Whether the changes a project's architect handed in are valid, in order: every change has its reason; an item
/// created is of the project and has what it needs and a key no other change uses; a title an item is created or
/// retitled with has 20 characters at most, spaces not counted; a split gives each acceptance condition of the item
/// split to one new item at most, one of its locked conditions when its acceptance is locked; every item a change
/// names is there, or made by an earlier change, and an item it changes is of the project (an item of the project may
/// depend on another project's); every dependency a change makes has its kind; every dependency a change removes is
/// one the items have then, stored or made by an earlier change and not removed or cancelled away since; an item is
/// moved first, last, under another item, or next to another item of its own project; observations folded in are the
/// project's and open; and after them the items' dependencies close no cycle. Otherwise why not, every reason at once.
pub fn validate(e: &Engine, project: &str, changes: &[Value]) -> Result<(), String> {
    validate_from(e, project, changes, 0)
}

/// [`validate`], saying what is wrong with the changes from the `first`-th on only: those before it stand as the
/// changes that come after must find them, and are not judged again. A change that records a decision is held to what
/// a decision must be: the records it quotes hold its words, an item it names is the project's, and no edit of that
/// item's `approach` comes after it in the batch, which would write the text over again; and no change writes a line of
/// a decision by hand, which only a decision does.
pub fn validate_from(e: &Engine, project: &str, changes: &[Value], first: usize) -> Result<(), String> {
    let mut s = State::load(e.db.conn())?;
    let cfg = &e.config.config;
    let mut problems: Vec<String> = Vec::new();
    let mut next_new = -1;
    // The acceptance conditions each item split gives to its new items so far.
    let mut taken: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    // The items a decision of the batch is recorded in so far, and the words each has quoted.
    let mut decided: BTreeSet<i64> = BTreeSet::new();
    let mut quoted: BTreeSet<(i64, String, i64, String)> = BTreeSet::new();
    // The `approach` of the items changes of the batch make, and the items merged away so far, with the item each is
    // kept in.
    let mut approaches = crate::decisions::Approaches::default();
    let mut away: BTreeMap<i64, i64> = BTreeMap::new();
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
                title_fits(text(c, "title"), &mut bad);
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
                let id = next_new;
                next_new -= 1;
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
                        away.insert(a, keep);
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
                    title_fits(text(c, "text"), &mut bad);
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
                    if id.is_some_and(|id| decided.contains(&id)) {
                        bad(
                            "it writes the item's approach whole after a decision of this batch was recorded in it, \
                             which the new text would write over: put this change before the decision"
                                .into(),
                        );
                    }
                    if let Some(id) = id {
                        approaches.written(id, text(c, "text"));
                    }
                }
            }
            "record_decision" => {
                let d = &c["decision"];
                let id = match d.get("item").filter(|v| !v.is_null()) {
                    Some(v) => own(v, &s, &mut bad),
                    None => None,
                };
                if let Some(keep) = id.and_then(|id| away.get(&id)) {
                    bad(format!(
                        "item {} is merged into item {keep} earlier in this batch, and is cancelled with it: record \
                         the decision in item {keep}, or put it before the merge, which carries it",
                        id.unwrap_or_default()
                    ));
                }
                if let Some(id) = id {
                    decided.insert(id);
                }
                match crate::decisions::resolve(e.db.conn(), project, d, "architect", id.filter(|id| *id > 0)) {
                    Ok(r) => {
                        for w in &r.said {
                            if !quoted.insert((id.unwrap_or_default(), w.from.clone(), w.id, w.words.clone())) {
                                bad(format!(
                                    "\"{}\" of {} {} is quoted by an earlier decision of this batch",
                                    w.words, w.from, w.id
                                ));
                            }
                        }
                    }
                    Err(why) => why.into_iter().for_each(&mut bad),
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
/// what was refused. `projects` are the configuration's, for the works a cancellation ends.
pub fn write_all(
    tx: &Transaction<'_>,
    changes: &[Value],
    projects: &BTreeMap<String, config::Project>,
) -> Result<Written, String> {
    let mut keys: BTreeMap<String, i64> = BTreeMap::new();
    let mut written = Written::default();
    for (i, c) in changes.iter().enumerate() {
        tx.execute_batch("SAVEPOINT change").map_err(sql)?;
        match write_one(tx, c, &mut keys, &mut written.created, projects) {
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
) -> Result<(), Fault> {
    // What came of it reaches the architect by itself: an item whose merge came first is not cancelled, and a report
    // says so; the rest of the batch stands either way, as its links moved before the cancel do.
    let cancel = |item: i64, why: &str| crate::ending::cancel_item(tx, item, why, "architect", projects).map(|_| ());
    let reason = text(c, "reason");
    let json_of = |k: &str| c.get(k).cloned().unwrap_or(Value::Null).to_string();
    match text(c, "kind") {
        "create_item" => {
            // How to do it, when the creator knows; none when it gave none.
            let approach = Some(text(c, "approach").trim()).filter(|a| !a.is_empty());
            tx.execute(
                "INSERT INTO items (project, title, problem, why, approach, deliver, level, wanted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    text(c, "project"),
                    text(c, "title"),
                    text(c, "problem"),
                    text(c, "why"),
                    approach,
                    text(c, "deliver"),
                    text(c, "level"),
                    json_of("acceptance")
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
                other => return Err(format!("an item has no part {other}").into()),
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
        "record_decision" => {
            // The line is the one the review read, which the engine made from the records when it collected the change.
            let line = c["resolved"]["line"]
                .as_str()
                .ok_or("a decision to record has no line the review read")?;
            let d = &c["decision"];
            match d.get("item").filter(|v| !v.is_null()) {
                Some(v) => {
                    let id = item_of(v, keys)?;
                    let now: Option<String> = tx
                        .query_row("SELECT approach FROM items WHERE id = ?1", [id], |r| r.get(0))
                        .map_err(sql)?;
                    tx.execute(
                        "UPDATE items SET approach = ?2 WHERE id = ?1",
                        params![id, crate::decisions::insert(now.as_deref().unwrap_or_default(), line)],
                    )
                    .map_err(sql)?;
                }
                // No item holds it: the line is kept as an observation, for an item to take in.
                None => {
                    let refs: Vec<String> = items(c["resolved"].get("words"))
                        .iter()
                        .map(|w| format!("{} {}", text(w, "from"), w["id"]))
                        .collect();
                    let o = json!({
                        "kind": "new",
                        "title": text(d, "title").trim(),
                        "what": line,
                        "tags": d.get("tags").cloned().unwrap_or(Value::Null),
                        "source": "A decision found in the owner's words that no item holds, as the architect's \
                                   batch of item changes recorded it",
                        "refs": refs,
                    });
                    let from = format!("matter {}", c["matter"]);
                    crate::architect::observe(tx, text(c, "project"), &from, &o)?;
                }
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

    #[test]
    fn a_cycle_is_found_and_a_chain_is_not() {
        assert!(cycle(&[(1, 2), (2, 3), (3, 1)]));
        assert!(!cycle(&[(1, 2), (2, 3), (1, 3)]));
        assert!(!cycle(&[]));
    }

    // A title is counted as the earlier board counted it: every character but spaces of any kind, a backtick too, one
    // each however many bytes it takes; 20 are let through, and 21 are refused, saying how many.
    #[test]
    fn a_title_is_counted_without_spaces_and_refused_over_twenty() {
        // An em space and a no-break space are not counted, as an ordinary one is not.
        assert_eq!(title_length("Parse\u{2003}the `lexer`\u{a0}first"), 20);
        // An emoji, four bytes in UTF-8.
        let wide = "\u{1f680}";
        let refused = |title: &str| {
            let mut said: Vec<String> = Vec::new();
            title_fits(title, &mut |p: String| said.push(p));
            said
        };
        assert!(refused(format!("{} {}", wide.repeat(10), wide.repeat(10)).as_str()).is_empty());
        assert_eq!(
            refused(format!("{} x", wide.repeat(20)).as_str()),
            ["the title it gives has 21 characters, spaces not counted, and a title has 20 at most: write it shorter"]
        );
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
                write_all(tx, &[change], &BTreeMap::new()).map_err(db::Error::Abort)
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
                write_all(tx, &[change], &BTreeMap::new()).map_err(db::Error::Abort)
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
                write_all(tx, &changes, &BTreeMap::new()).map_err(db::Error::Abort)
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

    // A merge whose re-pointed dependencies close a cycle in the state the write sees (validation did not see it: a
    // cancel earlier in the batch had taken the links of an item with a work going on out of its state, but the write
    // leaves them) is refused whole: the absorbed item keeps its links, is not cancelled, and the kept item's
    // approach is not changed; the change after it is made.
    #[test]
    fn a_merge_that_closes_a_cycle_is_refused_whole_and_the_absorbed_item_is_left_as_it_was() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Owner, |tx| {
            for title in ["parser", "lexer", "tokens"] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, approach, deliver, level)
                     VALUES ('p', ?1, 'x', 'y', 'kept', 'change', 'l')",
                    [title],
                )?;
            }
            // The parser depends on the lexer, and the tokens on the parser.
            db::add_dependency(tx, 1, 2, "its lexer", "blocking")?;
            db::add_dependency(tx, 3, 1, "its parser", "parallel")?;
            Ok(())
        })
        .unwrap();
        let changes = [
            // The lexer is merged into the tokens: the parser would depend on the tokens, which depend on the parser.
            json!({ "kind": "merge", "keep": "3", "absorbed": ["2"], "reason": "r" }),
            json!({ "kind": "hold", "item": "1", "reason": "Held." }),
        ];
        let written = db
            .write(Actor::Architect, |tx| {
                write_all(tx, &changes, &BTreeMap::new()).map_err(db::Error::Abort)
            })
            .unwrap();
        let refused: Vec<(usize, bool)> = written
            .refused
            .iter()
            .map(|(i, why)| (*i, why.contains("cannot depend on")))
            .collect();
        assert_eq!(refused, [(0, true)], "{written:?}");
        let mut st = db
            .conn()
            .prepare("SELECT item, depends_on FROM item_links ORDER BY item, depends_on")
            .unwrap();
        let links: Vec<(i64, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        assert_eq!(links, [(1, 2), (3, 1)]);
        let cancelled: Option<String> = db
            .conn()
            .query_row("SELECT cancelled FROM items WHERE id = 2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cancelled, None);
        let held: Option<String> = db
            .conn()
            .query_row("SELECT held FROM items WHERE id = 1", [], |r| r.get(0))
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
            write_all(tx, &changes, &projects).map_err(db::Error::Abort)?;
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
            write_all(tx, &[merge("1", "2"), merge("3", "4")], &BTreeMap::new()).map_err(db::Error::Abort)?;
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
            write_all(tx, &changes, &BTreeMap::new()).map_err(db::Error::Abort)?;
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
            write_all(tx, &changes, &BTreeMap::new()).map_err(db::Error::Abort)?;
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
}
