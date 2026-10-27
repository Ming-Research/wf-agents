//! What a cell is given: each input gathered from its sources, the returns of earlier cells, the database, the
//! configuration, what the engine computes and what rows recorded. Each value keeps where it came from, which is
//! what an agent's message addresses it by.

use std::path::Path;

use db::rusqlite::ToSql;
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph, CompiledInput};
use serde_json::{Map, Value, json};

use crate::work::{self, Item, Work};
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
        round: i64,
        first: i64,
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
) -> Result<Vec<(i64, Value)>, String> {
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

/// Gathers every input of a cell.
pub fn gather(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
) -> Result<Vec<Gathered>, String> {
    let mut out = Vec::new();
    for x in &cell.inputs {
        let mut parts: Vec<Part> = Vec::new();
        for s in &x.from {
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
    item: &Item,
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
            let project = e.config.config.projects.get(&item.project);
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
        Src::Engine(n) => engine_input(e, w, g, item, n)?.map(|v| Part {
            origin: Origin::Engine(n.clone()),
            value: v,
        }),
    })
}

/// The rows of a table or view for the item, work or project at hand.
pub fn db_rows(e: &Engine, w: &Work, item: &Item, relation: &str, by: Option<&str>) -> Result<Vec<Value>, String> {
    let conn = e.db.conn();
    match by {
        None => work::query_json(conn, &format!("SELECT * FROM {relation}"), &[], ROWS),
        Some(col) => {
            let key: Box<dyn ToSql> = match col {
                "item" => Box::new(item.id),
                "work" => Box::new(w.id),
                _ => Box::new(item.project.clone()),
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

/// What the engine computes for an input; none when there is nothing.
pub fn engine_input(e: &Engine, w: &Work, g: &CompiledGraph, item: &Item, name: &str) -> Result<Option<Value>, String> {
    let wt = w.worktree.as_deref();
    let main = e
        .config
        .config
        .projects
        .get(&item.project)
        .map(|p| p.main.clone())
        .unwrap_or_else(|| "main".into());
    Ok(match name {
        "free_slots" => {
            let hosts = &e.config.config.install.machines.hosts;
            Some(
                json!({ "machines": hosts.iter().map(|(n, h)| json!({ "name": n, "slots": h.slots })).collect::<Vec<_>>() }),
            )
        }
        "project_guide" => wt.map(|wt| project_guide(wt, item, &design_paths(e, item))),
        "local_commits" => wt.and_then(|wt| {
            let range = match &w.pushed_head {
                Some(p) => format!("{p}..HEAD"),
                None => format!("origin/{main}..HEAD"),
            };
            git::git(wt, &["log", "--format=%H %s", &range]).ok().map(Value::String)
        }),
        "plan_segment" | "plan_usage" => None,
        "existing_work" => existing_work(e, w, item, &main)?.map(Value::String),
        "design_tree_diff" | "tree_changes_to_review" => match wt {
            Some(wt) => {
                let base = if name == "tree_changes_to_review" {
                    crate::values::reviewed_head(e, w, g)?
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
                    Some(Value::String(String::from_utf8_lossy(&out.stdout).into_owned()))
                }
                _ => None,
            }
        }
        other => return Err(format!("no such engine input: {other}")),
    })
}

/// What already exists for the item, as pointers the agent follows itself: its earlier works here, with how far each
/// got and why it ended, and the queries that read their rounds; what an import recorded of work outside this flow;
/// the branch and the pull request the work took over. None for an item nothing was done on.
fn existing_work(e: &Engine, w: &Work, item: &Item, main: &str) -> Result<Option<String>, String> {
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
            "- Work {id} (flow version {}, {} rounds) was at cell {} when it ended: {ended}. Its rounds, with the cell, \
             the row taken and what each said it did: query `SELECT id, cell, transition, json_extract(output, \
             '$.progress') AS progress FROM rounds WHERE work = {id} ORDER BY id`; a round's whole return: `SELECT \
             output FROM rounds WHERE id = <round>`; its design cards and asks of the owner: the views \
             `item_design_cards` and `item_asks` with `work = {id}`.",
            x["flow_version"],
            x["rounds"],
            x["at"].as_str().unwrap_or("?"),
        ));
    }
    if let Some(from) = &item.imported_from {
        out.push(format!("- The item was imported from outside this flow: {from}."));
    }
    if let Some(branch) = &w.branch {
        out.push(format!(
            "- Your worktree is on the branch {branch}, taken over as it was: what it adds beyond main is `git log \
             --oneline origin/{main}..HEAD` and `git diff origin/{main}...HEAD`; what is not committed is `git status`."
        ));
    }
    if let Some(pr) = w.pr {
        out.push(format!(
            "- The pull request #{pr}, kept: its description and comments with the tool gh, `pr view {pr} --comments`; \
             its reviews, `pr view {pr} --json reviews`; its checks, `pr checks {pr}`."
        ));
    }
    Ok(Some(out.join("\n")))
}

/// The project's AGENTS.md, the index of its design tree (each node's path and first line), and the full text of the
/// nodes the item names.
fn project_guide(wt: &Path, item: &Item, design: &[String]) -> Value {
    let agents = std::fs::read_to_string(wt.join("AGENTS.md")).unwrap_or_default();
    let mut index = Map::new();
    let mut cited = Map::new();
    let text = format!(
        "{} {} {}",
        item.title,
        item.problem,
        item.approach.clone().unwrap_or_default()
    );
    for d in design.iter().filter(|d| d.ends_with('/')) {
        for f in files_under(&wt.join(d)) {
            let rel = f.strip_prefix(wt).unwrap_or(&f).to_string_lossy().into_owned();
            let body = std::fs::read_to_string(&f).unwrap_or_default();
            let first = body
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or_default()
                .to_string();
            if text.contains(&rel) {
                cited.insert(rel.clone(), Value::String(body));
            }
            index.insert(rel, Value::String(first));
        }
    }
    json!({ "agents_md": agents, "design_tree": index, "cited_nodes": cited })
}

fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            out.extend(files_under(&p));
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
    out
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
}
