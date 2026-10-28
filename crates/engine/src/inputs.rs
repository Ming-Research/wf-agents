//! What a cell is given: each input gathered from its sources, the returns of earlier cells, the database, the
//! configuration, what the engine computes and what rows recorded. Each value keeps where it came from, which is
//! what an agent's message addresses it by.

use std::path::Path;

use db::rusqlite::ToSql;
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph, CompiledInput};
use serde_json::{Map, Value, json};

use crate::work::{self, Item, RoundId, Work};
use crate::{Engine, form_of, git};

/// The most rows an input read from the database gives.
const ROWS: usize = 200;

/// The view of the work's rounds that a flow version stored before the history was removed still names. It re-sent
/// every round's whole record, so no input gives it any longer.
const HISTORY: &str = "work_history";

/// Where a value an input gives comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// A place of a round's return: the round, its cell and the path, with the earliest round of the cell whose return
    /// gave the same value there.
    Place {
        round: RoundId,
        first: RoundId,
        cell: String,
        path: String,
    },
    /// A setting of the project's configuration.
    Config(String),
    /// A table or view of the database, for the item, work or project at hand.
    Db(String),
    /// What the engine computes.
    Engine(String),
    /// A value rows recorded.
    Recorded(String),
}

/// One value one source of an input gave.
#[derive(Debug, Clone)]
pub struct Part {
    pub origin: Origin,
    pub value: Value,
}

impl Part {
    /// How the value is named when an input of several sources is given as one object.
    fn label(&self) -> String {
        match &self.origin {
            Origin::Place { cell, path, .. } => format!("{cell}.{path}"),
            Origin::Config(n) => format!("configuration {n}"),
            Origin::Db(n) => format!("database {n}"),
            Origin::Engine(n) => format!("engine {n}"),
            Origin::Recorded(n) => format!("recorded {n}"),
        }
    }
}

/// One input as gathered.
#[derive(Debug, Clone)]
pub struct Gathered {
    pub input: CompiledInput,
    /// What each of its sources gave; empty when none has anything.
    pub parts: Vec<Part>,
    /// The value as one: the one part's, or an object of each part by where it came from; none when the input is
    /// missing.
    pub value: Option<Value>,
}

impl Gathered {
    /// Whether the agent reads it itself.
    pub fn itself(&self) -> bool {
        self.input.from.iter().all(|s| *s == Src::Itself)
    }
}

/// The value at a path of a return, as an input gives it: a branch keeps only what is of that branch, and the
/// values at a path inside a list are given as a list.
pub fn at_path(x: &Value, path: &str) -> Option<Value> {
    let is_branch = |v: &Value, b: &str| v.as_object().is_some_and(|o| o.values().any(|x| x == b));
    let mut cur = vec![x.clone()];
    for seg in path.split('.') {
        let (name, branch) = match seg.split_once('[') {
            Some((n, rest)) => (n, Some(rest.trim_end_matches(']'))),
            None => (seg, None),
        };
        let mut next = Vec::new();
        for c in cur {
            match c {
                Value::Array(xs) => next.extend(xs.iter().filter_map(|it| it.get(name).cloned())),
                o => next.extend(o.get(name).cloned()),
            }
        }
        if let Some(b) = branch {
            next = next
                .into_iter()
                .filter_map(|v| match v {
                    Value::Array(xs) => {
                        let keep: Vec<Value> = xs.into_iter().filter(|it| is_branch(it, b)).collect();
                        (!keep.is_empty()).then_some(Value::Array(keep))
                    }
                    o => is_branch(&o, b).then_some(o),
                })
                .collect();
        }
        cur = next;
    }
    match cur.len() {
        0 => None,
        1 => cur.pop(),
        _ => Some(Value::Array(cur)),
    }
}

/// The returns of a cell in the work that hold a place, newest first, each with its round: for an agent's own answer,
/// the returns of its own form; for the fields every return has, the returns that have them; for any other cell,
/// every return that took a row. Each is given as the rows read it, an agent's form taken out of the field it sits
/// in.
pub fn returns_holding(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &str,
    path: &str,
) -> Result<Vec<(RoundId, Value)>, String> {
    let Some(c) = g.cell(cell) else {
        return Ok(Vec::new());
    };
    let top = path.split(['.', '[']).next().unwrap_or_default();
    let mut out = Vec::new();
    for r in work::rounds_at(e.db.conn(), w.id, cell)? {
        let Some(ret) = r.output.filter(|_| r.transition.is_some()) else {
            continue;
        };
        if c.kind != "agent" {
            out.push((r.id, ret));
            continue;
        }
        let form = form_of(g, c, &ret);
        let fields: Vec<&str> = c.forms.iter().filter_map(|f| f.field.as_deref()).collect();
        let props = c
            .schema
            .as_ref()
            .and_then(|s| s.get("properties"))
            .and_then(Value::as_object);
        let common = top != g.within && props.is_some_and(|p| p.contains_key(top));
        let leaves_out = c
            .forms
            .iter()
            .find(|f| f.field.is_some() && f.field == form)
            .is_some_and(|f| f.without.iter().any(|x| x == top));
        let holds = if fields.contains(&top) {
            form.as_deref() == Some(top)
        } else if common {
            !leaves_out
        } else {
            form.is_none()
        };
        if holds {
            out.push((r.id, g.unwrap(&ret)));
        }
    }
    Ok(out)
}

/// The last return of a cell in the work that holds a place, as [`returns_holding`] finds it.
pub fn last_return(e: &Engine, w: &Work, g: &CompiledGraph, cell: &str, path: &str) -> Result<Option<Value>, String> {
    Ok(returns_holding(e, w, g, cell, path)?.into_iter().next().map(|x| x.1))
}

/// The rounds of a cell of the work that handed in the cell's own answer, newest first. A round that asked for
/// tools, asked someone or paused took a row too, but answered nothing.
pub fn answered(e: &Engine, w: &Work, g: &CompiledGraph, cell: &str) -> Result<Vec<work::Round>, String> {
    let c = g.cell(cell);
    Ok(work::rounds_at(e.db.conn(), w.id, cell)?
        .into_iter()
        .filter(|r| r.transition.is_some())
        .filter(|r| match (c, &r.output) {
            (Some(c), Some(o)) => form_of(g, c, o).is_none(),
            _ => true,
        })
        .collect())
}

/// Gathers every input of a cell. A wake of the architect is on no item but for a project, so an input read for the
/// item has nothing for it, and one read for the project is its project's.
pub fn gather(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
) -> Result<Vec<Gathered>, String> {
    let mut out = Vec::new();
    for x in &cell.inputs {
        let mut parts: Vec<Part> = Vec::new();
        for s in &x.from {
            // The state of things: the wake's own understanding as its return, with what came since; or the view.
            if matches!(s, Src::Engine(n) if n == crate::architect::STATE) {
                let view = Src::Db {
                    relation: "architect_state".into(),
                    by: Some("project".into()),
                };
                let whole = source(e, w, g, cell, item, &view)?;
                if answered(e, w, g, "U1")?.is_empty() {
                    parts.extend(whole);
                    continue;
                }
                for path in ["state", "direction"] {
                    parts.extend(source(e, w, g, cell, item, &Src::Place("U1".into(), path.into()))?);
                }
                if let Some(p) = whole {
                    let has = |v: &Value| v.as_array().is_some_and(|xs| xs.iter().any(|x| !x.is_null()));
                    let since: Vec<Value> = p
                        .value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|r| has(&r["learned"]) || has(&r["notes"]))
                        .map(|r| json!({ "learned": r["learned"], "notes": r["notes"] }))
                        .collect();
                    if !since.is_empty() {
                        parts.push(Part {
                            origin: p.origin,
                            value: Value::Array(since),
                        });
                    }
                }
                continue;
            }
            // The study of a new policy and the owner's answer to it, when that answer said the policy stays.
            if matches!(s, Src::Engine(n) if n == crate::architect::STAYS) {
                if crate::architect::policy_stays(e, w, g)? {
                    for (c, path) in crate::architect::STAYS_PLACES {
                        let place = Src::Place(c.to_string(), path.to_string());
                        parts.extend(source(e, w, g, cell, item, &place)?);
                    }
                }
                continue;
            }
            parts.extend(source(e, w, g, cell, item, s)?);
        }
        let value = match parts.as_slice() {
            [] => None,
            [one] => Some(one.value.clone()),
            many => Some(Value::Object(
                many.iter().map(|p| (p.label(), p.value.clone())).collect(),
            )),
        };
        out.push(Gathered {
            input: x.clone(),
            parts,
            value,
        });
    }
    Ok(out)
}

/// What one source of an input gives: none when it has nothing.
fn source(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
    s: &Src,
) -> Result<Option<Part>, String> {
    Ok(match s {
        Src::Place(c, p) => {
            let id = if c.starts_with('@') {
                w.pos.pointer(c).map(str::to_string)
            } else {
                Some(c.clone())
            };
            let Some(id) = id else { return Ok(None) };
            let held = returns_holding(e, w, g, &id, p)?;
            let mut values = held.iter().filter_map(|(r, ret)| at_path(ret, p).map(|v| (*r, v)));
            let Some((round, value)) = values.next() else {
                return Ok(None);
            };
            // A value of one branch, such as `result[conflict]` or `columns[findings]`, is the cell as its newest
            // return left it: when a newer return took another branch, the older value no longer holds.
            if p.contains('[') && held.first().map(|(r, _)| *r) != Some(round) {
                return Ok(None);
            }
            // The same value an earlier return gave is the one thing it was then.
            let first = values
                .filter(|(_, v)| *v == value)
                .map(|(r, _)| r)
                .next_back()
                .unwrap_or(round);
            Some(Part {
                origin: Origin::Place {
                    round,
                    first,
                    cell: id,
                    path: p.clone(),
                },
                value,
            })
        }
        Src::Itself => None,
        Src::Config(n) => {
            let project = item.and_then(|i| e.config.config.projects.get(&i.project));
            project
                .and_then(|p| p.setting(n))
                .filter(|v| !v.is_null())
                .map(|v| Part {
                    origin: Origin::Config(n.clone()),
                    value: v,
                })
        }
        Src::Recorded(n) => w.pos.recorded.get(n).map(|v| Part {
            origin: Origin::Recorded(n.clone()),
            value: Value::String(v.clone()),
        }),
        Src::Db { relation, .. } if relation == HISTORY => None,
        // What is read for the item has nothing for a work on no item; what is read for the project, for a work on no
        // item, is the wake's project's.
        Src::Db { by: Some(col), .. }
            if col != "work" && item.is_none() && (col != "project" || w.project.is_none()) =>
        {
            None
        }
        Src::Db { relation, by } => {
            let rows = db_rows(e, w, item, relation, by.as_deref())?;
            // The interjections an agent is given are those for its own line.
            let rows: Vec<Value> = match (relation.as_str(), &cell.line) {
                ("waiting_interjections", Some(line)) => {
                    rows.into_iter().filter(|r| r["line"] == line.as_str()).collect()
                }
                _ => rows,
            };
            if rows.is_empty() && relation == "waiting_interjections" {
                None
            } else {
                Some(Part {
                    origin: Origin::Db(relation.clone()),
                    value: Value::Array(rows),
                })
            }
        }
        // What the cell before this one on a matter's path answered is a place of that round's return.
        Src::Engine(n) if n == crate::architect::BEFORE => crate::architect::before(e, w, g)?,
        // The policy being set is the newest of the places of the returns that write it, addressed as the oldest of
        // them that holds it as it is: the text the engine put together unchanged from the text written is the
        // written one.
        Src::Engine(n) if crate::architect::draft_places(n).is_some() => {
            let round = |p: &Part| match p.origin {
                Origin::Place { round, .. } => Some(round),
                _ => None,
            };
            let mut parts: Vec<Part> = Vec::new();
            for (c, path) in crate::architect::draft_places(n).into_iter().flatten() {
                let place = Src::Place((*c).to_string(), (*path).to_string());
                parts.extend(source(e, w, g, cell, item, &place)?);
            }
            let newest = parts.iter().max_by_key(|p| round(p)).map(|p| p.value.clone());
            newest.and_then(|v| parts.iter().filter(|p| p.value == v).min_by_key(|p| round(p)).cloned())
        }
        Src::Engine(n) => engine_input(e, w, g, item, n)?.map(|v| Part {
            origin: Origin::Engine(n.clone()),
            value: v,
        }),
    })
}

/// The rows of a cell's numbered table: each finding, column, test, conflict, check and entry its sources hold now,
/// numbered with the table's prefix in the order of its sources, each saying where it came from (the cell and the
/// path) besides what it holds; none when the cell has no numbered table or nothing to put in it.
pub fn table(gathered: &[Gathered]) -> Vec<Value> {
    let Some(x) = gathered.iter().find(|x| x.input.numbered.is_some()) else {
        return Vec::new();
    };
    let prefix = x.input.numbered.clone().unwrap_or_default();
    let mut rows = Vec::new();
    for p in &x.parts {
        let Origin::Place { cell, path, .. } = &p.origin else {
            continue;
        };
        for entry in entries(path, &p.value) {
            let mut row = Map::new();
            row.insert("row".into(), Value::String(format!("{prefix}{}", rows.len() + 1)));
            row.insert("from".into(), Value::String(format!("{cell}·{path}")));
            row.extend(entry);
            rows.push(Value::Object(row));
        }
    }
    rows
}

/// What a row of a numbered table is known by outside it: a finding's number, or a test's name.
pub fn row_key(row: &Value) -> Option<&str> {
    ["fid", "test", "name"]
        .iter()
        .find_map(|k| row.get(*k).and_then(Value::as_str))
}

/// What one source of a table holds, one entry for each row: for a review's column with findings, each finding with
/// the column's number; for a column it could not verify, the column; for a test, the test only when it went wrong
/// (red after the change, or a result before it that does not fit its kind); for a result with checks, each check that
/// is not green with the rest of the result, or the whole result when every check is green, as a flaky one is once
/// run again; anything else as it is.
fn entries(path: &str, v: &Value) -> Vec<Map<String, Value>> {
    let items = match v {
        Value::Array(xs) => xs.clone(),
        other => vec![other.clone()],
    };
    let guards = path.ends_with("guard_tests");
    let tests = guards || path.ends_with("new_tests");
    let object = |v: Value| match v {
        Value::Object(o) => o,
        other => Map::from_iter([("value".to_string(), other)]),
    };
    let mut out = Vec::new();
    for it in items {
        let mut o = object(it);
        if let Some(Value::Array(findings)) = o.get("findings").cloned() {
            let column = o.get("id").cloned().unwrap_or(Value::Null);
            for f in findings {
                let mut m = Map::from_iter([("column".to_string(), column.clone())]);
                m.extend(object(f));
                out.push(m);
            }
        } else if o.contains_key("verdict") && o.contains_key("id") {
            let column = o.remove("id").unwrap_or(Value::Null);
            let mut m = Map::from_iter([("column".to_string(), column)]);
            m.extend(o);
            out.push(m);
        } else if tests {
            let at = |k: &str| o.get(k).and_then(Value::as_str);
            let (head, base) = (at("on_head"), at("on_base"));
            let unfit = if guards {
                base != Some("green")
            } else {
                base == Some("green")
            };
            if head == Some("red") || unfit {
                out.push(o);
            }
        } else if let Some(Value::Array(checks)) = o.get("checks").cloned() {
            let not_green: Vec<Value> = checks.into_iter().filter(|c| c["result"] != "green").collect();
            if not_green.is_empty() {
                out.push(o);
                continue;
            }
            o.remove("checks");
            for c in not_green {
                let mut m = o.clone();
                m.extend(object(c));
                out.push(m);
            }
        } else {
            out.push(o);
        }
    }
    out
}

/// The rows of a table or view for the item, work or project at hand: for a wake of an architect, which is on no item,
/// none of the item's, and its own project's.
pub fn db_rows(
    e: &Engine,
    w: &Work,
    item: Option<&Item>,
    relation: &str,
    by: Option<&str>,
) -> Result<Vec<Value>, String> {
    let conn = e.db.conn();
    match by {
        None => work::query_json(conn, &format!("SELECT * FROM {relation}"), &[], ROWS),
        Some(col) => {
            let key: Box<dyn ToSql> = match (col, item, &w.project) {
                ("work", _, _) => Box::new(w.id),
                ("project", None, Some(p)) => Box::new(p.clone()),
                (_, None, _) => return Ok(Vec::new()),
                ("item", Some(i), _) => Box::new(i.id),
                (_, Some(i), _) => Box::new(i.project.clone()),
            };
            work::query_json(
                conn,
                &format!("SELECT * FROM {relation} WHERE {col} = ?1"),
                &[key.as_ref()],
                ROWS,
            )
        }
    }
}

/// The paths of a project's design.
fn design_paths(e: &Engine, item: &Item) -> Vec<String> {
    e.config
        .config
        .projects
        .get(&item.project)
        .map(|p| p.design_paths.clone())
        .unwrap_or_default()
}

/// What the engine computes for an input; none when there is nothing. What the architect's cells read is computed by
/// [`crate::architect`]; what is about an item has nothing for a work on no item.
pub fn engine_input(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    item: Option<&Item>,
    name: &str,
) -> Result<Option<Value>, String> {
    if crate::architect::INPUTS.contains(&name) {
        return crate::architect::input(e, w, g, name);
    }
    if name == "free_slots" {
        let hosts = &e.config.config.install.machines.hosts;
        return Ok(Some(
            json!({ "machines": hosts.iter().map(|(n, h)| json!({ "name": n, "slots": h.slots })).collect::<Vec<_>>() }),
        ));
    }
    let Some(item) = item else { return Ok(None) };
    let wt = w.worktree.as_deref();
    let main = main_of(e, item);
    Ok(match name {
        "project_guide" => match wt {
            Some(wt) => Some(project_guide(e, item, wt, &guide_base(wt, &main)?)?),
            None => None,
        },
        "local_commits" => wt.and_then(|wt| {
            let range = match &w.pushed_head {
                Some(p) => format!("{p}..HEAD"),
                None => format!("origin/{main}..HEAD"),
            };
            git::git(wt, &["log", "--format=%H %s", &range]).ok().map(Value::String)
        }),
        "existing_work" => existing_work(e, w, g, item, &main)?.map(Value::String),
        "design_tree_diff" | "tree_changes_to_review" => match wt {
            Some(wt) => {
                // At a re-review of the design tree, the change since the last design tree review; the table review
                // runs just before it on the same head, so its head would leave nothing to review.
                let base = if name == "tree_changes_to_review" {
                    crate::values::head_reviewed_by(e, w, g, "5b")?
                } else {
                    None
                };
                let base = match base {
                    Some(b) => b,
                    None => git::fork_point(wt, &main)?,
                };
                let d = git::diff(wt, &base, "HEAD", &design_paths(e, item))?;
                (!d.trim().is_empty()).then_some(Value::String(d))
            }
            None => None,
        },
        "tree_check_numbers" => {
            let check = e
                .config
                .config
                .projects
                .get(&item.project)
                .and_then(|p| p.design_check.clone());
            match (wt, check) {
                (Some(wt), Some(cmd)) => {
                    let fork = git::fork_point(wt, &main)?;
                    let out = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!("{cmd} {fork}"))
                        .current_dir(wt)
                        .output()
                        .map_err(|e| format!("{cmd}: {e}"))?;
                    let printed = String::from_utf8_lossy(&out.stdout).into_owned();
                    (!printed.trim().is_empty()).then_some(Value::String(printed))
                }
                _ => None,
            }
        }
        other => return Err(format!("no such engine input: {other}")),
    })
}

/// What already exists for the item, as pointers the agent follows itself: its earlier works here, with how far each
/// got and why it ended, and the queries that read their rounds; what an import recorded of work outside this flow;
/// the branch and the pull request the work took over, as its start cell took them over, so that the pull request
/// the work opens itself later neither changes it nor is said to be one it took over. None for an item nothing was
/// done on.
fn existing_work(e: &Engine, w: &Work, g: &CompiledGraph, item: &Item, main: &str) -> Result<Option<String>, String> {
    let earlier = work::query_json(
        e.db.conn(),
        "SELECT w.id, w.flow_version, w.at, w.ended, w.branch, w.pr,
                (SELECT count(*) FROM rounds r WHERE r.work = w.id) AS rounds
         FROM works w WHERE w.item = ?1 AND w.id <> ?2 ORDER BY w.id",
        &[&item.id, &w.id],
        100,
    )?;
    if earlier.is_empty() && item.imported_from.is_none() && item.imported_branch.is_none() {
        return Ok(None);
    }
    let mut out = vec!["This item was worked on before. Read what exists yourself:".to_string()];
    for x in &earlier {
        let id = x["id"].as_i64().unwrap_or_default();
        let ended = x["ended"].as_str().unwrap_or("it reached the end of its graph");
        out.push(format!(
            "- Work {id} (flow version {}, {} rounds) was at cell {} when it ended: {ended}. Its rounds, R{id}-1 on, \
             with the cell, the row taken and what each said it did: query `SELECT n, cell, transition, \
             json_extract(output, '$.progress') AS progress FROM rounds WHERE work = {id} ORDER BY n`; the whole \
             return of its round R{id}-<n>: `SELECT output FROM rounds WHERE work = {id} AND n = <n>`; its design \
             cards and asks of the owner: the views `item_design_cards` and `item_asks` with `work = {id}`.",
            x["flow_version"],
            x["rounds"],
            x["at"].as_str().unwrap_or("?"),
        ));
    }
    if let Some(from) = &item.imported_from {
        out.push(format!("- The item was imported from outside this flow: {from}."));
    }
    // What the start cell took over: a branch when an earlier work had one or the item was imported with one, and
    // the pull request that came with it.
    let place = work::rounds_at(e.db.conn(), w.id, &g.start)?
        .into_iter()
        .find_map(|r| r.output.filter(|_| r.transition.is_some()))
        .map(|o| o["place"].clone())
        .unwrap_or_default();
    let took_branch = item.imported_branch.is_some() || earlier.iter().any(|x| x["branch"].is_string());
    if let Some(branch) = place["branch"].as_str().filter(|_| took_branch) {
        out.push(format!(
            "- Your worktree is on the branch {branch}, taken over as it was: what it adds beyond main is `git log \
             --oneline origin/{main}..HEAD` and `git diff origin/{main}...HEAD`; what is not committed is `git status`."
        ));
    }
    if let Some(pr) = place["pr"].as_i64() {
        out.push(format!(
            "- The pull request #{pr}, kept: its description and comments with the tool gh, `pr view {pr} --comments`; \
             its reviews, `pr view {pr} --json reviews`; its checks, `pr checks {pr}`."
        ));
    }
    Ok(Some(out.join("\n")))
}

/// The branch of the project the flow calls main.
fn main_of(e: &Engine, item: &Item) -> String {
    e.config
        .config
        .projects
        .get(&item.project)
        .map(|p| p.main.clone())
        .unwrap_or_else(|| "main".into())
}

/// The engine's value the project guide is.
pub const GUIDE: &str = "project_guide";

/// The commit the project guide of a work is read from: main as the work's branch has merged it, the merge base of
/// its head and the fetched main. What the work itself changes on its branch is never in it, so its own edits to the
/// design tree or to AGENTS.md never make the guide a new one to give again: the agent knows what it edited.
fn guide_base(wt: &Path, main: &str) -> Result<String, String> {
    git::fork_point(wt, main)
}

/// The project guide for the item as a commit of the project has it, read with git from the work's worktree: what
/// the project says of itself, with the full text of the nodes the item names.
fn project_guide(e: &Engine, item: &Item, wt: &Path, commit: &str) -> Result<Value, String> {
    let text = format!(
        "{} {} {}",
        item.title,
        item.problem,
        item.approach.clone().unwrap_or_default()
    );
    config::project_guide_at(wt, commit, &design_paths(e, item), &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_keeps_its_branch_and_lists_what_a_list_holds() {
        let x = json!({
            "better": { "verdict": "yes", "how": "h" },
            "rows": [{ "answer": "fixed", "id": "R1" }, { "answer": "test_wrong", "id": "R2" }],
            "end": { "test_changes": [{ "change": "added", "test": "t" }] }
        });
        assert_eq!(
            at_path(&x, "better[yes]"),
            Some(json!({ "verdict": "yes", "how": "h" }))
        );
        assert_eq!(at_path(&x, "better[no]"), None);
        assert_eq!(
            at_path(&x, "rows[test_wrong]"),
            Some(json!([{ "answer": "test_wrong", "id": "R2" }]))
        );
        assert_eq!(at_path(&x, "rows.id"), Some(json!(["R1", "R2"])));
        assert_eq!(
            at_path(&x, "end.test_changes[added]"),
            Some(json!([{ "change": "added", "test": "t" }]))
        );
        assert_eq!(at_path(&x, "nothing"), None);
    }

    // A numbered table holds a row for each finding, column not verified, test that went wrong, conflict and check not
    // green, numbered across its sources in their order, each saying where it came from.
    #[test]
    fn a_numbered_table_numbers_every_row_across_its_sources() {
        let place = |cell: &str, path: &str, value: Value| Part {
            origin: Origin::Place {
                round: RoundId::new(1, 9),
                first: RoundId::new(1, 9),
                cell: cell.into(),
                path: path.into(),
            },
            value,
        };
        let finding = |fid: &str| json!({ "fid": fid, "where": "w", "what": "x", "fix": "y" });
        let g = Gathered {
            input: CompiledInput {
                id: None,
                name: None,
                numbered: Some("F".into()),
                what: "The table to answer".into(),
                from: Vec::new(),
                maybe: true,
            },
            parts: vec![
                place(
                    "5a",
                    "columns[findings]",
                    json!([{ "id": "2", "verdict": "findings", "evidence": {}, "findings": [finding("R1"), finding("R2")] }]),
                ),
                place(
                    "5b",
                    "columns[findings]",
                    json!([{ "id": "2", "verdict": "findings", "evidence": {}, "findings": [finding("R3")] }]),
                ),
                place(
                    "5a",
                    "columns[cannot_verify]",
                    json!([{ "id": "3", "verdict": "cannot_verify", "reason": "r" }]),
                ),
                place(
                    "6",
                    "new_tests",
                    json!([
                        { "name": "fails_before", "on_head": "green", "on_base": "red" },
                        { "name": "passes_before", "on_head": "green", "on_base": "green" }
                    ]),
                ),
                place(
                    "6",
                    "guard_tests",
                    json!([{ "name": "guard", "on_head": "green", "on_base": "green" }]),
                ),
                place(
                    "7",
                    "result[conflict]",
                    json!({ "kind": "conflict", "files": ["a.rs"] }),
                ),
                place(
                    "7",
                    "result[red]",
                    json!({ "kind": "red", "checks": [
                        { "name": "check", "result": "red", "run": "u" },
                        { "name": "other", "result": "green", "run": "v" }
                    ] }),
                ),
            ],
            value: None,
        };
        let rows = table(std::slice::from_ref(&g));
        let ids: Vec<&str> = rows.iter().filter_map(|r| r["row"].as_str()).collect();
        assert_eq!(ids, ["F1", "F2", "F3", "F4", "F5", "F6", "F7"]);
        assert_eq!(
            (&rows[0]["from"], &rows[0]["column"], &rows[0]["fid"]),
            (&json!("5a·columns[findings]"), &json!("2"), &json!("R1"))
        );
        assert_eq!(rows[2]["from"], "5b·columns[findings]");
        assert_eq!((&rows[3]["column"], rows[3].get("id")), (&json!("3"), None));
        assert_eq!(rows[4]["name"], "passes_before");
        assert_eq!(rows[5]["files"], json!(["a.rs"]));
        assert_eq!((&rows[6]["kind"], &rows[6]["name"]), (&json!("red"), &json!("check")));
        assert_eq!(row_key(&rows[0]), Some("R1"));
        assert_eq!(row_key(&rows[4]), Some("passes_before"));
    }
}
