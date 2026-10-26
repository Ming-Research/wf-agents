//! What a cell is given: each input gathered from its sources, the returns of earlier cells, the database, the
//! configuration, what the engine computes and what rows recorded.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use db::rusqlite::ToSql;
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph, CompiledInput, Held};
use serde_json::{Map, Value, json};

use crate::work::{self, Item, Work};
use crate::{Engine, form_of, git};

/// The most rows an input read from the database gives.
const ROWS: usize = 200;

/// The least length, as JSON, of a value the history gives again by the round that first held it.
const REPEATED: usize = 300;

/// The view of the work's rounds. A round's input does not hold it again, and the history gives each round without
/// the history it was given: those are the rounds before it, there already, and holding them would give every
/// earlier round once more with each round.
pub const HISTORY: &str = "work_history";

/// What stands for the history in a round's input.
pub const EARLIER: &str = "The rounds before this one";

/// One input as gathered.
#[derive(Debug, Clone)]
pub struct Gathered {
    pub input: CompiledInput,
    /// None when no source has anything: the input is missing.
    pub value: Option<Value>,
}

impl Gathered {
    /// Whether the agent reads it itself.
    pub fn itself(&self) -> bool {
        self.input.from.iter().all(|s| *s == Src::Itself)
    }

    pub fn held(&self) -> bool {
        self.input.held == Some(Held::Session)
    }

    /// Whether it is the work's history.
    pub fn history(&self) -> bool {
        is_history(&self.input)
    }
}

pub fn is_history(x: &CompiledInput) -> bool {
    x.from
        .iter()
        .any(|s| matches!(s, Src::Db { relation, .. } if relation == HISTORY))
}

/// A round as the history gives it: what the engine gave, without the prompt it wrote that into and without the
/// history the round was given.
fn compact(row: &mut Value, history: &BTreeSet<&str>) {
    let Some(input) = row.get_mut("input").and_then(Value::as_object_mut) else {
        return;
    };
    input.remove("prompt");
    if let Some(Value::Array(xs)) = input.get_mut("inputs") {
        for x in xs {
            if x["what"].as_str().is_some_and(|w| history.contains(w)) {
                x["value"] = json!(EARLIER);
            }
        }
    }
}

/// Gives each value of the history's rounds that an earlier round already held by that round: most of what a round
/// is given is what the rounds before it were given or returned.
fn refer_back(rows: &mut [Value]) {
    let mut seen = HashMap::new();
    for r in rows {
        let round = r["round"].as_i64().unwrap_or_default();
        for field in ["input", "output", "tools"] {
            if let Some(v) = r.get_mut(field) {
                refer(v, round, &mut seen);
            }
        }
    }
}

fn refer(v: &mut Value, round: i64, seen: &mut HashMap<String, i64>) {
    let text = v.to_string();
    if text.len() < REPEATED {
        return;
    }
    if let Some(r) = seen.get(&text) {
        *v = json!(format!("The same as in round {r}"));
        return;
    }
    match v {
        Value::Array(xs) => xs.iter_mut().for_each(|x| refer(x, round, seen)),
        Value::Object(o) => o.values_mut().for_each(|x| refer(x, round, seen)),
        _ => {}
    }
    seen.insert(text, round);
}

/// The history as a session that holds its rounds up to one is given it: the rounds after that one.
pub fn since(x: &mut Gathered, upto: i64) {
    let Some(Value::Array(rows)) = x.value.take() else {
        return;
    };
    let after: Vec<Value> = rows
        .into_iter()
        .filter(|r| r["round"].as_i64().is_some_and(|n| n > upto))
        .collect();
    x.value = Some(json!({
        "earlier": format!("The rounds up to round {upto} were given to you earlier in this session"),
        "since": after,
    }));
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

/// The last return of a cell in the work that holds a place: for an agent's own answer, the last return of its own
/// form; for the fields every return has, the last return that has them; for any other cell, its last return. The
/// return is given as the rows read it, an agent's form taken out of the field it sits in.
pub fn last_return(e: &Engine, w: &Work, g: &CompiledGraph, cell: &str, path: &str) -> Result<Option<Value>, String> {
    let Some(c) = g.cell(cell) else { return Ok(None) };
    let top = path.split(['.', '[']).next().unwrap_or_default();
    for r in work::rounds_at(e.db.conn(), w.id, cell)? {
        let Some(out) = r.output.filter(|_| r.transition.is_some()) else {
            continue;
        };
        if c.kind != "agent" {
            return Ok(Some(out));
        }
        let form = form_of(g, c, &out);
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
            return Ok(Some(g.unwrap(&out)));
        }
    }
    Ok(None)
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
        let mut parts: Vec<(String, Value)> = Vec::new();
        for s in &x.from {
            if let Some((label, v)) = source(e, w, g, cell, item, s)? {
                parts.push((label, v));
            }
        }
        let value = match parts.len() {
            0 => None,
            1 => parts.pop().map(|p| p.1),
            _ => Some(Value::Object(parts.into_iter().collect())),
        };
        out.push(Gathered {
            input: x.clone(),
            value,
        });
    }
    Ok(out)
}

fn source(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    s: &Src,
) -> Result<Option<(String, Value)>, String> {
    Ok(match s {
        Src::Place(c, p) => {
            let id = if c.starts_with('@') {
                w.pos.pointer(c).map(str::to_string)
            } else {
                Some(c.clone())
            };
            let Some(id) = id else { return Ok(None) };
            last_return(e, w, g, &id, p)?
                .and_then(|r| at_path(&r, p))
                .map(|v| (format!("{id}.{p}"), v))
        }
        Src::Itself => None,
        Src::Config(n) => {
            let project = e.config.config.projects.get(&item.project);
            project
                .and_then(|p| p.setting(n))
                .filter(|v| !v.is_null())
                .map(|v| (format!("configuration {n}"), v))
        }
        Src::Recorded(n) => w
            .pos
            .recorded
            .get(n)
            .map(|v| (format!("recorded {n}"), Value::String(v.clone()))),
        Src::Db { relation, by } => {
            let mut rows = db_rows(e, w, item, relation, by.as_deref())?;
            if relation == HISTORY {
                let history: BTreeSet<&str> = g
                    .cells
                    .iter()
                    .flat_map(|c| &c.inputs)
                    .filter(|x| is_history(x))
                    .map(|x| x.what.as_str())
                    .collect();
                for r in &mut rows {
                    compact(r, &history);
                }
                refer_back(&mut rows);
            }
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
                Some((format!("database {relation}"), Value::Array(rows)))
            }
        }
        Src::Engine(n) => engine_input(e, w, g, item, n)?.map(|v| (format!("engine {n}"), v)),
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

    #[test]
    fn the_history_gives_a_round_without_its_prompt_or_its_history() {
        let what = "This work's history";
        let mut row = json!({ "round": 3, "input": { "prompt": "p", "inputs": [
            { "what": what, "value": [{ "round": 1 }] },
            { "what": "The plan", "value": "x" },
        ] } });
        compact(&mut row, &BTreeSet::from([what]));
        assert_eq!(
            row["input"],
            json!({ "inputs": [{ "what": what, "value": EARLIER }, { "what": "The plan", "value": "x" }] })
        );
    }

    #[test]
    fn the_history_gives_a_value_again_by_the_round_that_held_it() {
        let design = json!({ "design": "d".repeat(REPEATED) });
        let mut rows = vec![
            json!({ "round": 2, "output": { "design": design.clone(), "short": "s" } }),
            json!({ "round": 5, "input": { "inputs": [{ "what": "The design", "value": design.clone() }] },
                    "output": { "short": "s" } }),
        ];
        refer_back(&mut rows);
        assert_eq!(rows[0]["output"]["design"], design);
        assert_eq!(rows[1]["input"]["inputs"][0]["value"], "The same as in round 2");
        assert_eq!(rows[1]["output"]["short"], "s");
    }

    #[test]
    fn a_session_is_given_the_rounds_after_those_it_holds() {
        let mut x = Gathered {
            input: CompiledInput {
                id: None,
                what: "This work's history".into(),
                from: vec![Src::Db {
                    relation: HISTORY.into(),
                    by: Some("work".into()),
                }],
                maybe: false,
                held: Some(Held::Given),
            },
            value: Some(json!([{ "round": 3 }, { "round": 7 }, { "round": 9 }])),
        };
        since(&mut x, 7);
        let v = x.value.unwrap();
        assert_eq!(v["since"], json!([{ "round": 9 }]));
        assert!(v["earlier"].as_str().unwrap().contains("round 7"));
    }
}
