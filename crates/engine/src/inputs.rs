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

/// The engineer's cell that fixes what the checks after implementing found, answering a numbered table.
const FIX: &str = "T6";

/// The cell of adversarial testing, which a re-check is told what the branch did since its last answer.
const ADVERSARIAL: &str = "T1";

/// The engineer's cell that writes the design, whose returns are the design's versions.
const DESIGN: &str = "D2";

/// The input that gives a check after design what the design changed since it last answered.
const DESIGN_CHANGE: &str = "design_change";

/// The design fields a check after design judges, outside the design's own: the way chosen, why, and the estimate.
const DESIGN_FIELDS: [&str; 3] = ["chosen", "why", "estimate"];

/// What the design changed since the check `cell` last answered: each of its fields that changed, as a difference of
/// lines between the design the check was given then (the latest design written before that answer) and the design
/// now, the fields that did not change by name, and the check's answer then with why; none before its first answer,
/// and none when no design was written since.
fn design_change(e: &Engine, w: &Work, g: &CompiledGraph, cell: &str) -> Result<Option<Value>, String> {
    let Some(last) = answered(e, w, g, cell)?.into_iter().next() else {
        return Ok(None);
    };
    let designs = returns_holding(e, w, g, DESIGN, "design")?;
    let Some((now_round, now)) = designs.first() else {
        return Ok(None);
    };
    let Some((then_round, then)) = designs.iter().find(|(r, _)| *r < last.id) else {
        return Ok(None);
    };
    if now_round == then_round {
        return Ok(None);
    }
    // Every field of the design, then those of the return beside it that a check reads.
    let mut fields: Vec<(String, Value, Value)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for d in [&now["design"], &then["design"]] {
        for k in d.as_object().into_iter().flatten().map(|(k, _)| k) {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
    }
    for k in &names {
        fields.push((
            format!("design.{k}"),
            then["design"][k].clone(),
            now["design"][k].clone(),
        ));
    }
    for k in DESIGN_FIELDS {
        fields.push((k.to_string(), then[k].clone(), now[k].clone()));
    }
    let text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    let mut changed = Map::new();
    let mut unchanged = Vec::new();
    // How long the design now is, and its difference, counted alike: a difference no shorter than the design is no
    // help, and the check judges the whole design, as the first time.
    let (mut whole, mut diffs) = (0, 0);
    for (name, was, is) in fields {
        whole += text(&is).len();
        if was == is {
            unchanged.push(Value::String(name));
        } else {
            let d = line_diff(&text(&was), &text(&is));
            diffs += d.len();
            changed.insert(name, Value::String(d));
        }
    }
    if diffs >= whole {
        return Ok(None);
    }
    let answer = last
        .output
        .as_ref()
        .map(|o| g.unwrap(o))
        .and_then(|o| o.get("answer").cloned())
        .unwrap_or(Value::Null);
    Ok(Some(json!({
        "changed": changed,
        "unchanged": unchanged,
        "your_last_answer": answer,
    })))
}

/// A text's sentences, each line split after every full stop, question or exclamation mark that ends a sentence: the
/// units a difference of a design's text is counted in, since a paragraph is one line.
fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut start = 0;
        for (i, c) in line.char_indices() {
            let end = i + c.len_utf8();
            if matches!(c, '.' | '?' | '!') && line[end..].starts_with(' ') {
                out.push(line[start..end].trim());
                start = end;
            }
        }
        out.push(line[start..].trim());
    }
    out
}

/// The sentences of `now` that differ from `then`, each one a line: each removed one after `-` and each added one after
/// `+`, with the sentence before and after a change as it stands, after a space, and `...` where unchanged ones are left
/// out.
pub fn line_diff(then: &str, now: &str) -> String {
    let a: Vec<&str> = sentences(then);
    let b: Vec<&str> = sentences(now);
    // The longest common subsequence's lengths, from the ends.
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // Each line, kept (' '), removed ('-') or added ('+').
    let mut ops: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] > lcs[i + 1][j]) {
            ops.push(('+', b[j]));
            j += 1;
        } else {
            ops.push(('-', a[i]));
            i += 1;
        }
    }
    let near = |k: usize| (k.saturating_sub(1)..=(k + 1).min(ops.len().saturating_sub(1))).any(|m| ops[m].0 != ' ');
    let mut out: Vec<String> = Vec::new();
    let mut skipped = false;
    for (k, (op, line)) in ops.iter().enumerate() {
        if *op != ' ' || near(k) {
            if skipped && !out.is_empty() {
                out.push("...".to_string());
            }
            skipped = false;
            out.push(format!("{op}{line}"));
        } else {
            skipped = true;
        }
    }
    out.join("\n")
}

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
    let rounds = work::rounds_at(e.db.conn(), w.id, cell)?;
    let mut out = Vec::new();
    for r in rounds {
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

/// Whether a cell has an input that reads the project's files on its main ([`crate::architect::MAIN_INPUTS`]).
pub fn reads_main(cell: &CompiledCell) -> bool {
    cell.inputs
        .iter()
        .flat_map(|x| &x.from)
        .any(|s| matches!(s, Src::Engine(n) if crate::architect::MAIN_INPUTS.contains(&n.as_str())))
}

/// What a round is pinned to: the commit of the project's main its inputs that read the project's files on main are
/// read at, and why main could not be fetched for it when it could not, with the commit it is read at all the same.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pin {
    /// The commit; none for a cell with no input that reads main.
    pub commit: Option<String>,
    /// Why the fetch of main failed, when it did and the commit is the one the checkout had from an earlier fetch.
    pub fetch_failed: Option<String>,
}

impl Pin {
    /// Writes what the round's record keeps of it into its `engine` part: `main`, the commit, and, when the fetch
    /// failed, `main_fetch_failed`, why.
    pub fn record(&self, engine: &mut Value) {
        engine["main"] = json!(self.commit);
        if let Some(why) = &self.fetch_failed {
            engine["main_fetch_failed"] = json!(why);
        }
    }
}

/// The commit of the project's main that a round of a cell reads the project at, which every input of the cell that
/// reads the project's files on its main is read at ([`crate::architect::MAIN_INPUTS`]), so that the round is given
/// one version of the project, and which the round's record keeps ([`Pin::record`]): with `fetch`, after bringing
/// `origin/<main>` in the project's checkout to the remote's main now, and else as the checkout has it. A fetch that
/// fails does not stop the round while `origin/<main>` still has a commit from an earlier fetch: the round goes on at
/// that commit, with why the fetch failed in the pin, and the engine says so. Nothing is retried. No commit of main is
/// an error that names the project, the failed fetch first: the inputs cannot be computed, and the step that asked is
/// paused or, with the network down, waits. A cell with no such input neither fetches nor reads main. A round that
/// runs a cell on a project's architect's line is pinned by `steps::pin_round` instead: it brings the architect's
/// worktree to main with the one fetch the round makes and pins the round to the commit the worktree is at.
pub fn pin_main(e: &Engine, w: &Work, cell: &CompiledCell, item: Option<&Item>, fetch: bool) -> Result<Pin, String> {
    if !reads_main(cell) {
        return Ok(Pin::default());
    }
    let named = item.map(|i| i.project.as_str()).or(w.project.as_deref());
    let Some((name, p)) = named.and_then(|n| e.config.config.projects.get(n).map(|p| (n, p))) else {
        return Ok(Pin::default());
    };
    let fetch_failed = if fetch {
        config::fetch_main(p).err().map(|x| format!("project {name}: {x}"))
    } else {
        None
    };
    match (config::main_commit(p), fetch_failed) {
        (Ok(commit), failed) => {
            if let Some(why) = &failed {
                e.say(&format!(
                    "work {} cell {}: {why}; the round goes on at {commit}, which an earlier fetch left",
                    w.id, cell.id
                ));
            }
            Ok(Pin {
                commit: Some(commit),
                fetch_failed: failed,
            })
        }
        (Err(x), Some(failed)) => Err(format!("{failed}; and then {x}")),
        (Err(x), None) => Err(format!("project {name}: {x}")),
    }
}

/// Gathers every input of a cell. A work of the architect is on no item but for a project, so an input read for the
/// item has nothing for it, and one read for the project is its project's. `main` is the commit of the project's main
/// the round is pinned to ([`Pin::commit`] of [`pin_main`]), which the inputs that read the project's files on main are
/// read at; a step that starts no round gives none, and they are then read at the commit `origin/<main>` is at now.
pub fn gather(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
    main: Option<&str>,
) -> Result<Vec<Gathered>, String> {
    let mut out = Vec::new();
    // The cell's own last answer, which an input that reads only what it has not answered reads after.
    let last_answer = answered(e, w, g, &cell.id)?.first().map(|r| r.id);
    for x in &cell.inputs {
        let mut parts: Vec<Part> = Vec::new();
        let after = if x.unanswered { last_answer } else { None };
        for s in &x.from {
            match s {
                // What the cell before this one answered of the matter, and for a group of matters, of the others too:
                // one place of its return for each.
                Src::Engine(n) if n == crate::architect::BEFORE => parts.extend(crate::architect::before(e, w, g)?),
                _ => parts.extend(source(e, w, g, cell, item, s, x.newest, after, main)?),
            }
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

/// The cell `@from` points at for a place `path`: the cell the work came from, unless that is one of the cells every
/// agent's cell shares that answer the cell that went to them (the tools, the search, the owner asked, the wait for the
/// owner's Start) and its returns hold nothing at the path, when it is the cell before them, as the work's rounds have
/// it, which the row that brought the work to its cell came from: a round after the tools still reads what that cell
/// handed in, and the tools' results are still read from the tools.
fn came_from(e: &Engine, w: &Work, g: &CompiledGraph, path: &str) -> Result<Option<String>, String> {
    let shared = |id: &str| g.cells.iter().any(|c| c.rows.iter().any(|r| r.common && r.to == id)) && g.is_called(id);
    let Some(from) = w.pos.from.as_deref() else {
        return Ok(None);
    };
    let holds = returns_holding(e, w, g, from, path)?
        .iter()
        .any(|(_, ret)| at_path(ret, path).is_some());
    let at = w.pos.at.as_str();
    if !shared(from) || holds {
        return Ok(Some(from.to_string()));
    }
    let cells: Vec<String> = work::rounds(e.db.conn(), w.id)?
        .into_iter()
        .filter_map(|r| r.cell)
        .collect();
    Ok(cells.into_iter().rev().find(|c| c != at && !shared(c)))
}

/// What one source of an input gives: none when it has nothing. A place is read only from the returns handed in
/// after the round `after`, when one is given.
#[allow(clippy::too_many_arguments)]
fn source(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
    s: &Src,
    newest: bool,
    after: Option<RoundId>,
    main: Option<&str>,
) -> Result<Option<Part>, String> {
    Ok(match s {
        Src::Place(c, p) => {
            let id = match c.as_str() {
                "@from" => came_from(e, w, g, p)?,
                _ if c.starts_with('@') => w.pos.pointer(c).map(str::to_string),
                _ => Some(c.clone()),
            };
            let Some(id) = id else { return Ok(None) };
            let mut held = returns_holding(e, w, g, &id, p)?;
            if let Some(a) = after {
                held.retain(|(r, _)| *r > a);
            }
            let mut values = held.iter().filter_map(|(r, ret)| at_path(ret, p).map(|v| (*r, v)));
            let Some((round, value)) = values.next() else {
                return Ok(None);
            };
            // A value of one branch, such as `result[conflict]` or `columns[findings]`, is the cell as its newest
            // return left it: when a newer return took another branch, the older value no longer holds; and so is
            // every place of an input that reads the newest answer only.
            if (newest || p.contains('[')) && held.first().map(|(r, _)| *r) != Some(round) {
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
        // A setting of the item's project, or for a work of an architect, of the work's.
        Src::Config(n) => {
            let named = item.map(|i| i.project.as_str()).or(w.project.as_deref());
            let project = named.and_then(|p| e.config.config.projects.get(p));
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
        // item, is the work's project's.
        Src::Db { by: Some(col), .. }
            if col != "work" && col != "matter" && item.is_none() && (col != "project" || w.project.is_none()) =>
        {
            None
        }
        Src::Db { relation, by } => {
            let rows = db_rows(e, w, item, relation, by.as_deref())?;
            // The interjections an agent is given are those for its own line.
            let mut rows: Vec<Value> = match (relation.as_str(), &cell.line) {
                ("waiting_interjections", Some(line)) => {
                    rows.into_iter().filter(|r| r["line"] == line.as_str()).collect()
                }
                _ => rows,
            };
            // Rows read for the work, or the matter in hand, are its own: what they are read by is the engine's
            // bookkeeping, not given.
            if let Some(col @ ("work" | "matter")) = by.as_deref() {
                for r in &mut rows {
                    if let Some(o) = r.as_object_mut() {
                        o.remove(col);
                    }
                }
            }
            if rows.is_empty() && relation == "waiting_interjections" {
                None
            } else {
                Some(Part {
                    origin: Origin::Db(relation.clone()),
                    value: Value::Array(rows),
                })
            }
        }
        Src::Engine(n) if n == DESIGN_CHANGE => design_change(e, w, g, &cell.id)?.map(|v| Part {
            origin: Origin::Engine(n.clone()),
            value: v,
        }),
        Src::Engine(n) => engine_input(e, w, g, item, n, main)?.map(|v| Part {
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
/// (red after the change, or a result before it that does not fit its kind, a test not run before it fitting either
/// kind, for the table review to judge); for a result with checks, each check that
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
                !matches!(base, Some("green" | "not_run"))
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

/// The rows of a table or view for the item, work or project at hand: for a work of an architect, which is on no item,
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
                // The matter a work of the architect has in hand.
                ("matter", _, _) => match crate::architect::picked(conn, w.id)? {
                    Some(m) => Box::new(m),
                    None => return Ok(Vec::new()),
                },
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

/// The paths of a project's design: the directory of its records.
fn design_paths(e: &Engine, item: &Item) -> Vec<String> {
    e.config
        .config
        .projects
        .get(&item.project)
        .map(config::Project::design_paths)
        .unwrap_or_default()
}

/// What the engine computes for an input; none when there is nothing. What the architect's cells read is computed by
/// [`crate::architect`]; what is about an item has nothing for a work on no item. `main` is the commit of the
/// project's main the round is pinned to, for the inputs that read it ([`crate::architect::MAIN_INPUTS`]).
pub fn engine_input(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    item: Option<&Item>,
    name: &str,
    main: Option<&str>,
) -> Result<Option<Value>, String> {
    // What the architect knows of the project, for a cell of an engineer's work on one of its items.
    if w.project.is_none()
        && let Some(item) = item
        && crate::architect::KNOWLEDGE.contains(&name)
    {
        return crate::architect::knowledge(e, &item.project, item.id, name, main);
    }
    if crate::architect::INPUTS.contains(&name) {
        return crate::architect::input(e, w, g, name, main);
    }
    // The reasoning system's rules and check, for the project of the work, an architect's included.
    if crate::reasoning::is_input(name) {
        let named = item.map(|i| i.project.as_str()).or(w.project.as_deref());
        let Some(project) = named.and_then(|p| e.config.config.projects.get(p)) else {
            return Ok(None);
        };
        return crate::reasoning::input(e, project, name);
    }
    if name == HISTORY_SOURCES {
        let named = item.map(|i| i.project.as_str()).or(w.project.as_deref());
        return Ok(Some(history_sources(e, named)));
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
        // The table the latest fix answered, as its round was given it; its answers are the fix's own return.
        "fix_table" => answered(e, w, g, FIX)?
            .into_iter()
            .next()
            .and_then(|r| r.input.get("engine").and_then(|x| x.get("table")).cloned())
            .filter(|t| t.as_array().is_some_and(|rows| !rows.is_empty())),
        // What a review reviews, by commits the worktree has: main as it has it fetched, the pull request's base, the
        // commit of main the branch was cut from or last took main in at, the head under review, the merges of main
        // into the branch (none, said so, when it never took main in), and at a re-review the head the last table
        // review looked at with the merges since.
        "review_base" => match wt {
            Some(wt) => {
                let fork = git::fork_point(wt, &main)?;
                let head = w
                    .pushed_head
                    .clone()
                    .or_else(|| git::resolve(wt, "HEAD"))
                    .unwrap_or_default();
                let last = crate::values::reviewed_head(e, w, g)?;
                Some(review_base_json(wt, &main, &fork, &head, last.as_deref())?)
            }
            None => None,
        },
        // What the branch did since adversarial testing last answered; none before its first answer.
        "since_adversarial" => match wt {
            Some(wt) => since_adversarial(e, w, g, wt, &main)?,
            None => None,
        },
        // A flow version stored before the records named this input `design_tree_diff`.
        "records_diff" | "design_tree_diff" => match wt {
            Some(wt) => {
                let fork = git::fork_point(wt, &main)?;
                let d = git::diff(wt, &fork, "HEAD", &design_paths(e, item))?;
                (!d.trim().is_empty()).then_some(Value::String(d))
            }
            None => None,
        },
        other => return Err(format!("no such engine input: {other}")),
    })
}

/// What a review reviews: see the input `review_base`. `last` is the head the last table review looked at, if any. A
/// head git cannot read there is said so, with why, since the reader goes on without the list it would have been
/// given; and when it is no ancestor of `head`, the branch was rewritten since, which a note says.
fn review_base_json(wt: &Path, main: &str, fork: &str, head: &str, last: Option<&str>) -> Result<Value, String> {
    let (main_merges, _) = git::main_merges_since(wt, main, &format!("origin/{main}"), head)?;
    let mut v = json!({
        "main": format!("origin/{main}"),
        "fork_point": fork,
        "head": head,
        "change": format!("git diff {fork}..{head}"),
        "main_merges": merges_json(&main_merges),
    });
    if let Some(last) = last {
        v["last_reviewed"] = json!(last);
        match git::main_merges_since(wt, main, last, head) {
            Ok((merges, diverged)) => {
                v["merges_since_last_review"] = merges_json(&merges);
                if diverged {
                    v["merges_since_last_review_note"] = json!(
                        "The head the last table review looked at is not an ancestor of the head now: the branch was \
                         rewritten since. The merges are those among the commits the head has that it lacked."
                    );
                }
            }
            Err(why) => v["merges_since_last_review"] = json!({ "unreadable": why }),
        }
    }
    Ok(v)
}

/// The merges of a branch as an input gives them, newest first: each commit with the first line of its message. An
/// empty list says the branch had none.
fn merges_json(merges: &[git::Commit]) -> Value {
    Value::Array(
        merges
            .iter()
            .map(|c| json!({ "commit": c.id, "subject": c.subject }))
            .collect(),
    )
}

/// What happened on the branch since `base` as an input gives it, with the numbers of the rounds of the fix that
/// answered in the meantime. A base git cannot read is said so, with why, since the reader goes on without the list
/// it would have been given.
fn since_json(wt: &Path, main: &str, base: &str, head: &str, fixes: &[String]) -> Value {
    let changes = match git::changes_since(wt, main, base, head) {
        Ok(c) => c,
        Err(why) => return json!({ "since": base, "head": head, "unreadable": why, "fix_rounds": fixes }),
    };
    let mut v = json!({
        "since": base,
        "head": head,
        // The first parents only; a merge is one commit, marked, and what it brought in is not listed.
        "commits": changes.commits.iter()
            .map(|c| json!({ "commit": c.id, "subject": c.subject, "merge": c.merge }))
            .collect::<Vec<_>>(),
        // The files the work itself changed in them; what merging main brought is left out.
        "own_files": changes.own_files,
        "fix_rounds": fixes,
    });
    if changes.diverged {
        v["note"] = json!(
            "The head adversarial testing last answered at is not an ancestor of the head now: the branch was \
             rewritten since. The commits are those the head has that it lacked, and the files are counted from the \
             commit they share."
        );
    }
    v
}

/// What the branch did since adversarial testing last answered, for its next round: the commits after the head it
/// ended at (the last it wrote tests in, and else the head it began at, since it changed nothing), the merges among
/// them marked, the files the work itself changed in them, and the rounds of the fix that answered after it. None
/// before its first answer.
fn since_adversarial(e: &Engine, w: &Work, g: &CompiledGraph, wt: &Path, main: &str) -> Result<Option<Value>, String> {
    let Some(last) = answered(e, w, g, ADVERSARIAL)?.into_iter().next() else {
        return Ok(None);
    };
    let wrote = last
        .output
        .as_ref()
        .and_then(|o| at_path(&g.unwrap(o), "wrote.head"))
        .and_then(|h| h.as_str().and_then(|h| git::resolve(wt, h)));
    let Some(base) = wrote.or_else(|| last.input["engine"]["head"].as_str().map(str::to_string)) else {
        return Ok(None);
    };
    let head = git::resolve(wt, "HEAD").unwrap_or_default();
    let mut fixes: Vec<String> = answered(e, w, g, FIX)?
        .into_iter()
        .filter(|r| r.id > last.id)
        .map(|r| r.id.to_string())
        .collect();
    // Oldest first, as the rounds were.
    fixes.reverse();
    Ok(Some(since_json(wt, main, &base, &head, &fixes)))
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
             return of its round R{id}-<n>: `SELECT output FROM rounds WHERE work = {id} AND n = <n>`; its decision \
             cards and asks of the owner: the views `item_design_cards` and `item_asks` with `work = {id}`.",
            x["flow_version"],
            x["rounds"],
            x["at"].as_str().unwrap_or("?"),
        ));
        // What it asked the architect and had no answer to when it ended: the question left the queue with it.
        let withdrawn = work::query_json(
            e.db.conn(),
            "SELECT id, what FROM architect_matters
             WHERE work = ?1 AND withdrawn IS NOT NULL AND kind IN ('engineer_question', 'failure') ORDER BY id",
            &[&id],
            100,
        )?;
        for m in withdrawn {
            out.push(format!(
                "- Work {id} asked the architect, and had no answer when it ended, so the question was withdrawn: {}. \
                 The whole question: `SELECT body FROM architect_matters WHERE id = {}`. Ask again if you still need \
                 to.",
                m["what"].as_str().unwrap_or_default().trim_end_matches('.'),
                m["id"]
            ));
        }
    }
    if let Some(from) = &item.imported_from {
        out.push(format!("- The item was imported from outside this flow: {from}."));
    }
    // What the start cell took over: a branch when an earlier work had one or the item was imported with one, and
    // the pull request that came with it.
    let start = g
        .cells
        .iter()
        .find(|c| c.step.as_deref() == Some("start_work"))
        .map_or(g.start.as_str(), |c| c.id.as_str());
    let place = work::rounds_at(e.db.conn(), w.id, start)?
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

/// The input that says where the history a gatherer reads is.
const HISTORY_SOURCES: &str = "history_sources";

/// Where the history is, for the agents that gather it: the database's address, with the command that reads it, and the
/// project's checkout, for git. The database is live, in WAL mode, and the engine holds it open while a round runs: a
/// plain read-only open sees what its write-ahead log holds, where an immutable open would miss it, and needs the log's
/// files that the engine's connection keeps there.
fn history_sources(e: &Engine, project: Option<&str>) -> Value {
    let database = config::database(&e.data);
    let checkout = project
        .and_then(|p| e.config.config.projects.get(p))
        .map(|p| p.checkout.display().to_string());
    json!({
        "database": database.display().to_string(),
        "read": format!("sqlite3 -safe -readonly '{}' \"<query>\"", database.display()),
        "checkout": checkout,
    })
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
/// records or to AGENTS.md never make the guide a new one to give again: the agent knows what it edited.
fn guide_base(wt: &Path, main: &str) -> Result<String, String> {
    git::fork_point(wt, main)
}

/// The project guide for the item as a commit of the project has it, read with git from the work's worktree: what
/// the project says of itself, with the full text of the records' files the item names.
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

    // What a design changed is the sentences removed and added, each with the sentence beside it as it stands, the
    // unchanged ones between two changes far apart left out; a paragraph is split into its sentences.
    #[test]
    fn a_change_of_the_design_is_its_sentences_removed_and_added() {
        let then = "one\ntwo\nthree\nfour\nfive\nsix\nseven";
        let now = "one\n2\nthree\nfour\nfive\nsix\nseven\neight";
        assert_eq!(line_diff(then, now), " one\n-two\n+2\n three\n...\n seven\n+eight");
        assert_eq!(line_diff("same", "same"), "");
        assert_eq!(line_diff("", "new"), "+new");
        assert_eq!(
            line_diff("It reads. It writes. It ends.", "It reads. It checks. It ends."),
            " It reads.\n-It writes.\n+It checks.\n It ends."
        );
    }

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
                common: false,
                numbered: Some("F".into()),
                what: "The table to answer".into(),
                from: Vec::new(),
                maybe: true,
                newest: false,
                unanswered: false,
            },
            parts: vec![
                place(
                    "T4",
                    "columns[findings]",
                    json!([{ "id": "2", "verdict": "findings", "evidence": {}, "findings": [finding("R1"), finding("R2")] }]),
                ),
                place(
                    "T5",
                    "columns[findings]",
                    json!([{ "id": "2", "verdict": "findings", "evidence": {}, "findings": [finding("R3")] }]),
                ),
                place(
                    "T4",
                    "columns[cannot_verify]",
                    json!([{ "id": "3", "verdict": "cannot_verify", "reason": "r" }]),
                ),
                place(
                    "T1",
                    "new_tests",
                    json!([
                        { "name": "fails_before", "on_head": "green", "on_base": "red" },
                        { "name": "passes_before", "on_head": "green", "on_base": "green" },
                        { "name": "not_reached", "on_head": "green", "on_base": "not_run" }
                    ]),
                ),
                place(
                    "T1",
                    "guard_tests",
                    json!([
                        { "name": "guard", "on_head": "green", "on_base": "green" },
                        { "name": "guard_not_reached", "on_head": "green", "on_base": "not_run" }
                    ]),
                ),
                place(
                    "T2",
                    "result[conflict]",
                    json!({ "kind": "conflict", "files": ["a.rs"] }),
                ),
                place(
                    "T2",
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
            (&json!("T4·columns[findings]"), &json!("2"), &json!("R1"))
        );
        assert_eq!(rows[2]["from"], "T5·columns[findings]");
        assert_eq!((&rows[3]["column"], rows[3].get("id")), (&json!("3"), None));
        assert_eq!(rows[4]["name"], "passes_before");
        assert_eq!(rows[5]["files"], json!(["a.rs"]));
        assert_eq!((&rows[6]["kind"], &rows[6]["name"]), (&json!("red"), &json!("check")));
        assert_eq!(row_key(&rows[0]), Some("R1"));
        assert_eq!(row_key(&rows[4]), Some("passes_before"));
    }

    // What the branch did since a head is given with its merges marked, the files the work itself changed and the
    // rounds of the fix in between; a head git cannot read, and a branch rewritten since, are each said so, not
    // answered as if all were well.
    #[test]
    fn what_the_branch_did_since_a_head_is_given_and_a_head_gone_or_rewritten_is_said() {
        let dir = std::env::temp_dir().join(format!("wf-agents-since-json-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let who = [
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "init.defaultBranch=main",
            ];
            git::git(&dir, &[&who[..], args].concat()).unwrap().trim().to_string()
        };
        let commit = |file: &str, message: &str| {
            std::fs::write(dir.join(file), message).unwrap();
            run(&["add", "-A"]);
            run(&["commit", "-q", "-m", message]);
            run(&["rev-parse", "HEAD"])
        };
        run(&["init", "-q"]);
        let start = commit("a.txt", "Start");
        run(&["update-ref", "refs/remotes/origin/main", "main"]);
        run(&["checkout", "-q", "-b", "work"]);
        let first = commit("b.txt", "First");
        let second = commit("c.txt", "Second");
        let fixes = vec!["R7-30".to_string(), "R7-34".to_string()];
        let v = since_json(&dir, "main", &first, &second, &fixes);
        assert_eq!((&v["since"], &v["head"]), (&json!(first), &json!(second)));
        assert_eq!(
            v["commits"],
            json!([{ "commit": second, "subject": "Second", "merge": false }])
        );
        assert_eq!(v["own_files"], json!(["c.txt"]));
        assert_eq!(v["fix_rounds"], json!(["R7-30", "R7-34"]));
        assert!(v.get("note").is_none() && v.get("unreadable").is_none(), "{v}");
        // A head git has no commit for: the round is told, not stopped.
        let gone = since_json(&dir, "main", &"0".repeat(40), &second, &[]);
        assert!(
            gone["unreadable"].is_string() && gone.get("commits").is_none(),
            "{gone}"
        );
        // The branch rebuilt on the start since: the head it last answered at is no ancestor of the head.
        run(&["checkout", "-q", "-b", "rebuilt", &start]);
        let rebuilt = commit("c.txt", "Rebuilt");
        let v = since_json(&dir, "main", &second, &rebuilt, &[]);
        assert!(v["note"].as_str().is_some_and(|n| n.contains("was rewritten")), "{v}");
        assert_eq!(v["own_files"], json!(["c.txt"]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // What a review reviews lists the merges that took main in and no other merge; the head the last table review
    // looked at is given with the merges since it, and one git cannot read, or one the branch was rewritten away
    // from, is said so, not answered as if all were well and not failing the message.
    #[test]
    fn a_review_is_given_the_merges_of_main_and_a_last_head_gone_or_rewritten_is_said() {
        let dir = std::env::temp_dir().join(format!("wf-agents-review-base-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let who = [
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "init.defaultBranch=main",
            ];
            git::git(&dir, &[&who[..], args].concat()).unwrap().trim().to_string()
        };
        let commit = |file: &str, message: &str| {
            std::fs::write(dir.join(file), message).unwrap();
            run(&["add", "-A"]);
            run(&["commit", "-q", "-m", message]);
            run(&["rev-parse", "HEAD"])
        };
        run(&["init", "-q"]);
        let start = commit("a.txt", "Start");
        run(&["update-ref", "refs/remotes/origin/main", "main"]);
        run(&["checkout", "-q", "-b", "side"]);
        commit("s.txt", "Side");
        run(&["checkout", "-q", "-b", "work", &start]);
        let first = commit("b.txt", "First");
        // Main goes on, and the branch takes it in; then the branch merges a branch of its own, which is no main.
        run(&["checkout", "-q", "main"]);
        commit("m.txt", "Main goes on");
        run(&["update-ref", "refs/remotes/origin/main", "main"]);
        run(&["checkout", "-q", "work"]);
        run(&["merge", "-q", "--no-ff", "-m", "Take main", "main"]);
        let took_main = run(&["rev-parse", "HEAD"]);
        run(&["merge", "-q", "--no-ff", "-m", "Take the side", "side"]);
        let head = run(&["rev-parse", "HEAD"]);
        let commits = |v: &Value| -> Vec<String> {
            v.as_array()
                .unwrap_or_else(|| panic!("{v}"))
                .iter()
                .map(|c| c["commit"].as_str().unwrap().to_string())
                .collect()
        };

        let v = review_base_json(&dir, "main", "fork", &head, Some(&first)).unwrap();
        assert_eq!(commits(&v["main_merges"]), [took_main.clone()], "{v}");
        assert_eq!(commits(&v["merges_since_last_review"]), [took_main], "{v}");
        assert_eq!(v["last_reviewed"], json!(first));
        assert!(v.get("merges_since_last_review_note").is_none(), "{v}");
        // No table review before: no last head, and no merges said since it.
        let first_review = review_base_json(&dir, "main", "fork", &head, None).unwrap();
        assert!(
            first_review.get("last_reviewed").is_none() && first_review.get("merges_since_last_review").is_none(),
            "{first_review}"
        );

        // A last head git has no commit for: said so, with why, and the rest of the input is still given.
        let gone = review_base_json(&dir, "main", "fork", &head, Some(&"0".repeat(40))).unwrap();
        assert!(gone["merges_since_last_review"]["unreadable"].is_string(), "{gone}");
        assert_eq!(gone["last_reviewed"], json!("0".repeat(40)));
        assert_eq!(gone["main_merges"], v["main_merges"]);

        // A last head the branch was rewritten away from: the merges are those among the commits the head has that it
        // lacked, and a note says so.
        run(&["checkout", "-q", "-b", "other", &start]);
        let other = commit("o.txt", "Other");
        let rewritten = review_base_json(&dir, "main", "fork", &head, Some(&other)).unwrap();
        assert!(
            rewritten["merges_since_last_review_note"]
                .as_str()
                .is_some_and(|n| n.contains("rewritten")),
            "{rewritten}"
        );
        assert_eq!(commits(&rewritten["merges_since_last_review"]).len(), 1, "{rewritten}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The inputs that read the project's files on its main are listed by the architect's module, and the flow's cells
    // declare them: a cell triggers a fetch and a commit of main for its round if and only if it declares one, and only
    // an agent's cell does, since only a round of an agent is pinned to a commit.
    #[test]
    fn a_cell_reads_main_if_and_only_if_it_declares_an_input_that_does() {
        let cx = flow::Context {
            relations: db::relations(db::SCHEMA).unwrap(),
            tables: db::index_of(db::SCHEMA).unwrap(),
        };
        let compiled = flow::compile(flow::FLOW, &cx).unwrap_or_else(|e| panic!("{e}"));
        let main_inputs = crate::architect::MAIN_INPUTS;
        let declares = |c: &CompiledCell, name: &str| {
            c.inputs
                .iter()
                .any(|x| x.from.iter().any(|s| matches!(s, Src::Engine(n) if n.as_str() == name)))
        };
        let mut declared: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        let mut agent = None;
        for g in &compiled.graphs {
            for c in &g.cells {
                let has = main_inputs.iter().any(|n| declares(c, n));
                assert_eq!(reads_main(c), has, "{} {}", g.name, c.id);
                assert!(!has || c.kind == "agent", "{} {} is no agent's cell", g.name, c.id);
                declared.extend(main_inputs.iter().copied().filter(|n| declares(c, n)));
                if has && agent.is_none() {
                    agent = Some(c.clone());
                }
            }
        }
        // Each is declared by some cell of the flow, and computed by the module that takes the commit of main.
        assert_eq!(declared.len(), main_inputs.len(), "{declared:?}");
        assert!(main_inputs.iter().all(|n| crate::architect::INPUTS.contains(n)));
        // A cell with none of them reads no main.
        let mut bare = agent.expect("some agent's cell reads main");
        bare.inputs.retain(|x| {
            !x.from
                .iter()
                .any(|s| matches!(s, Src::Engine(n) if main_inputs.contains(&n.as_str())))
        });
        assert!(!reads_main(&bare));
    }
}
