//! A work on an older flow version, when it next goes on: what the version the engine runs does with it ([`plan`]),
//! judged only by what the work carries now: where it stands, the returns later cells will read, and, in the middle of a
//! cell, the session that cell has open. What every agent's cell shares (the engine's fallback rows, the rows of the
//! forms every return shares, the inputs every agent's cell has) takes no part: a version that changes it moves every
//! work all the same, and the session of a cell under way is given what changed, as any changed input is given.
//!
//! The cell a work serves is the one it is doing: where it stopped, else the first of the cells that wait for an
//! answer, else the cell it stands at. It has begun that cell once a round ran there since the work last took a row of
//! a cell off its detours (the asking part, and the cells every agent's cell shares that answer it: the tools, the
//! search, the owner's Start, the answers to who interjected): a round that asked for tools and got their results, or
//! a return sent back, has begun it; coming to a cell, or standing at an engine's cell that has not run, has not.
//!
//! The returns later cells will read are the newest own answer of each cell off the detours that the new version reads
//! from where the work stands ([`read_from`]): the cells the work can still come to, those they read by id, through an
//! input or a database view, and those the engine's own code reads by id; and the work's last return, which the cell it
//! goes to reads by where it came from. Earlier returns of the detours, and an agent's earlier returns in a form every
//! return shares (asking, running tools), are read only so and are not counted. A return is what the cell's schema in
//! the work's own version takes: what the engine records of a row it took itself, with no return (an ask withdrawn, a
//! turn waited for, a session begun afresh), is none. Each counts by what its cell takes in the new version from a
//! return handed in on an earlier one ([`CompiledCell::carried_schema`]): its outcome alone for an agent's
//! ([`outcome_only`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use flow::format::Src;
use flow::{CompiledCell, CompiledGraph, Position};
use serde_json::{Value, json};

use crate::work::Round;

/// What the version the engine runs does with a work on an older one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// It moves where it stands: the same work at the same cell in the same sessions, through no cell.
    Move,
    /// It comes to its graph's cell for a work on an older version and goes back from there to cell `to`, which it
    /// does again from its start, for the reason `why`.
    GoBack { to: String, why: String },
    /// The work stays at that cell, paused, for the reason `why`: the version has no cell it can go on from, or going
    /// back to `back_to` would do again a round whose effect cannot be undone, where the owner's Start takes it.
    Stuck { why: String, back_to: Option<String> },
}

/// What the version whose graph is `new` does with a work standing at `pos` on the same graph `old` of the version it
/// is on, having run `rounds`, oldest first. Both graphs are as the work's project's agents are asked them, which its
/// returns were checked against. A work never goes back past a round whose effect took hold and cannot be undone, as
/// `in_effect` says of each round in words (a merge, a reply that went out): it is stuck instead, for the owner to
/// decide. An engineer's work whose merge stands is judged by the rounds after its merge alone ([`after_merge`]), so
/// that no plan, and no Start of the owner's, takes it back before its merge.
pub fn plan(
    old: &CompiledGraph,
    new: &CompiledGraph,
    pos: &Position,
    rounds: &[Round],
    in_effect: impl Fn(&Round) -> Option<String>,
) -> Plan {
    // A merged work is judged by the rounds after its merge alone: their returns, the cells they walked and those they
    // would do again. Those before are never done again, and later cells read their returns as they are.
    let rounds = &rounds[after_merge(old, rounds)..];
    match decide(old, new, pos, rounds) {
        Plan::GoBack { to, why } => {
            let from = redone_from(old, rounds, &to);
            match rounds[from..]
                .iter()
                .find_map(|r| in_effect(r).map(|what| (r.id, what)))
            {
                Some((round, what)) => Plan::Stuck {
                    why: format!("{why}; going back to cell {to} would do round {round} again, which {what}"),
                    back_to: Some(to),
                },
                None => Plan::GoBack { to, why },
            }
        }
        other => other,
    }
}

/// Where the work would go, before the rounds it would do again are weighed ([`plan`]).
fn decide(old: &CompiledGraph, new: &CompiledGraph, pos: &Position, rounds: &[Round]) -> Plan {
    let serving = serving(pos);
    // The first return of the work's main way that does not fit: the cell it is the answer of, or for the last
    // return at a detour, the cell the detour serves, is done again.
    for (i, r) in read_later(old, new, pos, rounds) {
        let Some(why) = misfit(new, r) else { continue };
        let cell = r.cell.as_deref().unwrap_or_default();
        let main = if detour(old, cell) { serving } else { cell };
        return back(old, new, std::iter::once(main).chain(walked(old, &rounds[..i])), why);
    }
    // Where the work stands, then the cell it serves, then the cells it walked, the latest first.
    let here = || [pos.at.as_str(), serving].into_iter().chain(walked(old, rounds));
    if new.cell(&pos.at).is_none() {
        return back(old, new, here(), format!("cell {} is gone", pos.at));
    }
    // A work that could not go on in its own version, waiting at its cell for that with nowhere to go back to, looks
    // again for the latest cell it walked.
    if old.version.as_deref() == Some(pos.at.as_str()) && pos.stopped.is_none() {
        let why = "it could go on from none of the cells it walked in the version before".to_string();
        return back(old, new, walked(old, rounds), why);
    }
    // A cell under way whose session the version would take on in another kind, line or inputs is done again: the
    // cell it serves, and the agent's cell of a detour whose session is open, as the advisor answering: the one it
    // stands at, or the one the cell it stands at answers, as the tools the advisor asked for.
    let is_agent = |c: &str| old.cell(c).is_some_and(|c| c.kind == "agent");
    let open = if is_agent(&pos.at) {
        Some(pos.at.as_str())
    } else {
        pos.callers.last().map(String::as_str).filter(|c| is_agent(c))
    };
    let detour_at = open.filter(|c| *c != serving);
    for cell in std::iter::once(serving).chain(detour_at) {
        let inner = |c: &str| {
            if cell == serving {
                detour(old, c)
            } else {
                shared(old, c)
            }
        };
        if begun(old, rounds, cell, inner)
            && let Some(what) = changed(old, new, cell)
        {
            let why = format!("cell {cell}, which the work had begun, {what}");
            return back(old, new, std::iter::once(serving).chain(walked(old, rounds)), why);
        }
    }
    for to in pos.stopped.iter().chain(&pos.callers) {
        match (old.cell(to), new.cell(to)) {
            (Some(x), Some(y)) if x.kind == y.kind => {}
            _ => {
                let why = format!("cell {to}, which the work goes back to, is gone or of another kind");
                return back(old, new, std::iter::once(serving).chain(walked(old, rounds)), why);
            }
        }
    }
    Plan::Move
}

/// The first round the work does again when it goes back to cell `to`, as an index into `rounds`: the first of the
/// rounds of `to`'s latest stretch, its detours in between included, or, for a cell the work never ran, the first after
/// its last round off the detours; `rounds.len()` when none is done again. What those rounds collected and settled is
/// collected and settled again as the cell runs again.
pub fn redone_from(g: &CompiledGraph, rounds: &[Round], to: &str) -> usize {
    let at = |r: &Round| r.cell.as_deref() == Some(to);
    let Some(last) = rounds.iter().rposition(at) else {
        return rounds
            .iter()
            .rposition(|r| r.cell.as_deref().is_some_and(|c| !detour(g, c)))
            .map_or(0, |i| i + 1);
    };
    let mut first = last;
    for i in (0..last).rev() {
        let r = &rounds[i];
        if at(r) && !took_own_row(g, r) {
            first = i;
        } else if !r.cell.as_deref().is_some_and(|c| detour(g, c)) {
            break;
        }
    }
    first
}

/// The first of `rounds` after the merge of an engineer's work while it stands, as `db::merge_stands` says: the merge
/// cell took `merged`, and watching main has not taken `broken_by_merge` since; none (0) for a work whose merge does not
/// stand, or of another graph.
fn after_merge(g: &CompiledGraph, rounds: &[Round]) -> usize {
    let at = |r: &Round, cell: &str, row: &str| {
        r.transition.as_deref() == Some(row) && r.cell.as_deref().is_some_and(|c| db::is_cell(crate::ENGINEER, cell, c))
    };
    if g.name != crate::ENGINEER {
        return 0;
    }
    match rounds.iter().rposition(|r| at(r, crate::MERGE, "merged")) {
        Some(m) if !rounds[m + 1..].iter().any(|r| at(r, crate::WATCH, "broken_by_merge")) => m + 1,
        _ => 0,
    }
}

/// The cell a work standing at `pos` serves: where it stopped, else the first of the cells that wait for an answer,
/// else the cell it stands at.
fn serving(pos: &Position) -> &str {
    pos.stopped
        .as_deref()
        .or(pos.callers.first().map(String::as_str))
        .unwrap_or(pos.at.as_str())
}

/// Whether a cell of `g` is off the work's main way: in the asking part, or one every agent's cell shares that answers
/// the cell that went to it ([`shared`]).
fn detour(g: &CompiledGraph, id: &str) -> bool {
    g.asking.iter().any(|a| a == id) || shared(g, id)
}

/// Whether a cell of `g` is one every agent's cell shares that answers the cell that went to it: the tools, the
/// search, the owner's Start, the answers to who interjected.
fn shared(g: &CompiledGraph, id: &str) -> bool {
    g.is_called(id) && g.cells.iter().any(|c| c.rows.iter().any(|r| r.common && r.to == id))
}

/// The cells of the work's main way that `rounds` ran at, the latest first.
fn walked<'r>(g: &'r CompiledGraph, rounds: &'r [Round]) -> impl Iterator<Item = &'r str> {
    rounds
        .iter()
        .rev()
        .filter_map(|r| r.cell.as_deref())
        .filter(move |c| !detour(g, c))
}

/// Where the work goes back to, the first of `cells` the version has on its main way, for the reason `why`; stuck
/// when it has none of them.
fn back<'c>(old: &CompiledGraph, new: &CompiledGraph, mut cells: impl Iterator<Item = &'c str>, why: String) -> Plan {
    let fits = |c: &str| {
        new.cell(c).is_some()
            && !detour(new, c)
            && !detour(old, c)
            && [&new.version, &new.cancel].iter().all(|x| x.as_deref() != Some(c))
            && c != new.end
    };
    match cells.find(|c| fits(c)) {
        Some(to) => Plan::GoBack {
            to: to.to_string(),
            why,
        },
        None => Plan::Stuck {
            why: format!("{why}, and the version has none of the cells the work walked"),
            back_to: None,
        },
    }
}

/// Whether the work began `cell`: a round ran there since it last took a row elsewhere but at the cells `inner` says
/// are within the cell's own stretch, or a row of the cell's own that kept it there, as a wait.
fn begun(g: &CompiledGraph, rounds: &[Round], cell: &str, inner: impl Fn(&str) -> bool) -> bool {
    for r in rounds.iter().rev() {
        let Some(at) = r.cell.as_deref() else { continue };
        if at == cell {
            return !took_own_row(g, r);
        }
        if !inner(at) {
            return false;
        }
    }
    false
}

/// Whether a round took one of its cell's own rows, not one every agent's cell shares or one of the engine's fallback.
fn took_own_row(g: &CompiledGraph, r: &Round) -> bool {
    let (Some(c), Some(t)) = (r.cell.as_deref().and_then(|c| g.cell(c)), r.transition.as_deref()) else {
        return false;
    };
    c.rows.iter().any(|x| x.name == t && !x.common && !x.fallback)
}

/// What of a cell the session under way there holds changed between the versions, in words: it is gone, or of another
/// kind, on another line, or reads other inputs of its own (those every agent's cell has come with what every return
/// shares, which the session is given as it changes); none when none did.
fn changed(old: &CompiledGraph, new: &CompiledGraph, id: &str) -> Option<&'static str> {
    let (Some(was), Some(is)) = (old.cell(id), new.cell(id)) else {
        return Some("is gone");
    };
    let inputs = |c: &CompiledCell| -> Vec<(Option<String>, Option<String>, Vec<flow::format::Src>)> {
        c.inputs
            .iter()
            .filter(|x| !x.common)
            .map(|x| (x.id.clone(), x.name.clone(), x.from.clone()))
            .collect()
    };
    if was.kind != is.kind {
        Some("is of another kind")
    } else if crate::line_of(old, was) != crate::line_of(new, is) {
        Some("is on another line")
    } else if inputs(was) != inputs(is) {
        Some("reads other inputs")
    } else {
        None
    }
}

/// The returns later cells will read, each with its index in `rounds`, oldest first: the newest own answer of each
/// cell off its detours that the new version reads from where the work stands ([`read_from`]), and the work's last
/// return.
fn read_later<'r>(
    old: &CompiledGraph,
    new: &CompiledGraph,
    pos: &Position,
    rounds: &'r [Round],
) -> Vec<(usize, &'r Round)> {
    let read = read_from(new, pos);
    let returns: Vec<(usize, &Round)> = rounds.iter().enumerate().filter(|(_, r)| is_return(old, r)).collect();
    let mut out: Vec<(usize, &Round)> = returns.last().copied().into_iter().collect();
    let mut seen: Vec<&str> = Vec::new();
    for &(i, r) in returns.iter().rev() {
        let cell = r.cell.as_deref().unwrap_or_default();
        if seen.contains(&cell)
            || !read.contains(cell)
            || new.cell(cell).is_none()
            || detour(new, cell)
            || !own_answer(old, new, r)
        {
            continue;
        }
        seen.push(cell);
        if out.iter().all(|(j, _)| *j != i) {
            out.push((i, r));
        }
    }
    out.sort_by_key(|(i, _)| *i);
    out
}

/// The cells whose returns the engine's own code reads by their id, beside any input, by graph, whatever cell the work
/// is at: the engineer's start (where its branch came from), its research and its plan, its design, the review of it
/// taken in and the split of it, the first pass of the review, the fix, its merge and its end; the architect's look at
/// its queue and its pick, its understanding of the project, the owner's talk, its policy written and taken into
/// effect, and the cells whose answers say what a matter became.
const READ_BY_CODE: [(&str, &[&str]); 2] = [
    (crate::ENGINEER, &["S1", "R1", "D2", "D4", "D5", "T2", "T6", "M2", "E1"]),
    (crate::ARCHITECT, &["Q0", "Q1", "U1", "H8", "H12", "E1", "P2", "P12"]),
];

/// The cells whose answers the version whose graph is `g` reads from where a work stands at `pos` on: each cell the
/// work can still come to by the rows of `g`, from the cell it stands at and the cells it goes back to (those it
/// stopped at and those that wait for an answer, where every row to `@stopped`, `@caller`, `@asker` or `@self` leads);
/// each cell those cells read by its id, through an input's place or a database view that names it; and each cell the
/// engine's own code reads by its id ([`READ_BY_CODE`]). A place at `@stopped` or `@caller` reads the cells it goes
/// back to; one at `@from`, the work's last return, which counts wherever the work stands.
fn read_from(g: &CompiledGraph, pos: &Position) -> BTreeSet<String> {
    let back: Vec<&str> = pos.stopped.iter().chain(&pos.callers).map(String::as_str).collect();
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    let mut todo: Vec<&str> = std::iter::once(pos.at.as_str()).chain(back.iter().copied()).collect();
    while let Some(id) = todo.pop() {
        let Some(c) = g.cell(id) else { continue };
        if !reached.insert(c.id.as_str()) {
            continue;
        }
        for r in &c.rows {
            if r.to.starts_with('@') {
                todo.extend(back.iter().copied());
            } else {
                todo.push(r.to.as_str());
            }
        }
    }
    let mut read: BTreeSet<String> = reached.iter().map(|c| c.to_string()).collect();
    for c in reached.iter().filter_map(|id| g.cell(id)) {
        for src in c.inputs.iter().flat_map(|x| &x.from) {
            match src {
                Src::Place(at, _) if at == "@stopped" || at == "@caller" => {
                    read.extend(back.iter().map(|b| b.to_string()));
                }
                Src::Place(at, _) if !at.starts_with('@') => {
                    read.insert(at.clone());
                }
                Src::Db { relation, .. } => {
                    read.extend(NAMED.get(relation).into_iter().flatten().cloned());
                }
                _ => {}
            }
        }
    }
    let code = READ_BY_CODE
        .iter()
        .filter(|(graph, _)| *graph == g.name)
        .flat_map(|(_, cells)| cells.iter());
    read.extend(code.map(|c| c.to_string()));
    read
}

/// The words each view of the database names in quotes, through the views it reads, by its name: among them the cells
/// whose rounds it reads, as `cell IN ('M2', ...)`. Read once from the schema the program is built with; its comments,
/// which may hold a quote, are left out.
static NAMED: LazyLock<BTreeMap<String, BTreeSet<String>>> = LazyLock::new(|| {
    let conn = db::rusqlite::Connection::open_in_memory().expect("a database in memory opens");
    conn.execute_batch(db::SCHEMA)
        .expect("the schema the program is built with loads");
    let mut st = conn
        .prepare("SELECT name, sql FROM sqlite_schema WHERE type = 'view'")
        .expect("the schema's views are read");
    let views: BTreeMap<String, String> = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .expect("the schema's views are read")
        .filter_map(Result::ok)
        .map(|(name, sql)| {
            let code: Vec<&str> = sql.lines().map(|l| l.split("--").next().unwrap_or_default()).collect();
            (name, code.join("\n"))
        })
        .collect();
    let quoted = |sql: &str| -> Vec<String> { sql.split('\'').skip(1).step_by(2).map(str::to_string).collect() };
    let reads = |sql: &str| -> Vec<String> {
        sql.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| views.contains_key(*w))
            .map(str::to_string)
            .collect()
    };
    let mut named = BTreeMap::new();
    for name in views.keys() {
        let mut seen = BTreeSet::new();
        let mut words = BTreeSet::new();
        let mut todo = vec![name.clone()];
        while let Some(v) = todo.pop() {
            if !seen.insert(v.clone()) {
                continue;
            }
            let sql = &views[&v];
            words.extend(quoted(sql));
            todo.extend(reads(sql));
        }
        named.insert(name.clone(), words);
    }
    named
});

/// Whether a round handed in a return: it took a row and its cell in the work's own version takes its output, as one
/// handed in on that version or carried from an earlier one.
fn is_return(old: &CompiledGraph, r: &Round) -> bool {
    let (Some(cell), Some(output)) = (r.cell.as_deref(), r.output.as_ref()) else {
        return false;
    };
    r.transition.is_some()
        && old
            .cell(cell)
            .is_none_or(|c| c.carried_schema().is_none_or(|s| flow::valid(s, output)))
}

/// Whether a return is its cell's own answer, not a form every return shares, by the cell as the work's own version
/// has it, or as the new one does for a cell the work's version lacks.
fn own_answer(old: &CompiledGraph, new: &CompiledGraph, r: &Round) -> bool {
    let cell = r.cell.as_deref().unwrap_or_default();
    let (g, c) = match old.cell(cell) {
        Some(c) => (old, c),
        None => match new.cell(cell) {
            Some(c) => (new, c),
            None => return true,
        },
    };
    c.kind != "agent" || r.output.as_ref().is_none_or(|o| crate::form_of(g, c, o).is_none())
}

/// Why a return does not fit its cell in the new version, as a return carried from an earlier one, in words: its cell
/// is gone, or the places it does not fit; none when it fits.
fn misfit(new: &CompiledGraph, r: &Round) -> Option<String> {
    let cell = r.cell.as_deref()?;
    let Some(is) = new.cell(cell) else {
        return Some(format!("cell {cell}, which the work handed a return in at, is gone"));
    };
    let (schema, output) = outcome_only(is.carried_schema()?, r.output.as_ref()?);
    let places = flow::misfits(&schema, &output, "");
    (!places.is_empty()).then(|| {
        format!(
            "the return of cell {cell} in round {} no longer fits it ({})",
            r.id,
            places.join("; ")
        )
    })
}

/// The part of an agent's return a later round reads, its outcome, with the schema cut down to it: the fields every
/// agent's return holds beside it (what it told the owner, its progress, its side reports, its flow feedback) are acted
/// on as its round ends and read by no later row, so a version that changes them, as one that narrows the kinds of side
/// reports, moves a work all the same. A return of a tool or of the engine, which has no outcome, is whole.
fn outcome_only(schema: &Value, output: &Value) -> (Value, Value) {
    if schema["properties"].get("outcome").is_none() {
        return (schema.clone(), output.clone());
    }
    let mut schema = schema.clone();
    if let Some(p) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        p.retain(|k, _| k == "outcome");
    }
    if let Some(r) = schema.get_mut("required").and_then(Value::as_array_mut) {
        r.retain(|k| k == "outcome");
    }
    let outcome = output.get("outcome").cloned().unwrap_or(Value::Null);
    (schema, json!({ "outcome": outcome }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use flow::format::{Atom, Src};
    use serde_json::json;

    use super::*;
    use crate::RoundId;

    /// The plan for a work, each round weighed as the engine weighs it, by the graph of the work's own version.
    fn plan(old: &CompiledGraph, new: &CompiledGraph, pos: &Position, rounds: &[Round]) -> Plan {
        super::plan(old, new, pos, rounds, |r| crate::in_effect(old, r))
    }

    fn compiled(graph: &str) -> CompiledGraph {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        f.graph(graph).unwrap().clone()
    }

    fn engineer() -> CompiledGraph {
        compiled(crate::ENGINEER)
    }

    fn architect() -> CompiledGraph {
        compiled(crate::ARCHITECT)
    }

    fn cell<'g>(g: &'g mut CompiledGraph, id: &str) -> &'g mut CompiledCell {
        g.cells.iter_mut().find(|c| c.id == id).unwrap()
    }

    /// A return of an engineer's cell that its schema takes, from the flow's samples, by the row it takes.
    fn sample(cell: &str, row: &str) -> Value {
        let all: BTreeMap<String, Vec<Value>> =
            serde_json::from_str(include_str!("../../flow/tests/samples.json")).unwrap();
        all[cell].iter().find(|s| s["row"] == row).unwrap()["json"].clone()
    }

    /// Round `n` of work 1 at `cell`, which took row `took` and handed `output` in.
    fn ran(n: i64, cell: &str, took: &str, output: Value) -> Round {
        Round {
            id: RoundId::new(1, n),
            cell: Some(cell.into()),
            session: None,
            input: Value::Null,
            output: Some(output),
            transition: Some(took.into()),
            rejected: None,
            compactions: 0,
            started_at: n,
            ended_at: Some(n),
            usage: None,
        }
    }

    /// Round `n` of work 1 at `cell` whose return was sent back, or that is still open.
    fn took_none(n: i64, cell: &str) -> Round {
        Round {
            transition: None,
            output: None,
            rejected: Some(json!({ "problems": ["x"] })),
            ..ran(n, cell, "", Value::Null)
        }
    }

    fn at(cell: &str) -> Position {
        Position {
            at: cell.to_string(),
            ..Position::default()
        }
    }

    /// Where a work stands, by name, at each point of cell `I1` under way and before it, with the rounds that brought
    /// it there and whether it has begun `I1` there.
    fn positions() -> Vec<(&'static str, Position, Vec<Round>, bool)> {
        let gathered = ran(1, "R0", "done", sample("R0", "done"));
        let tools = ran(2, "I1", "tools", sample("I1", "tools"));
        let results = ran(3, "X1", "ran", sample("X1", "ran"));
        let done = ran(4, "I1", "done", sample("I1", "done"));
        let asked = ran(2, "I1", "ask_architect", sample("I1", "ask_architect"));
        let at_x1 = Position {
            at: "X1".into(),
            callers: vec!["I1".into()],
            ..Position::default()
        };
        let asking = Position {
            at: "A2".into(),
            stopped: Some("I1".into()),
            callers: vec!["I1".into()],
            ..Position::default()
        };
        vec![
            ("just come", at("I1"), vec![gathered.clone()], false),
            ("asked for tools", at_x1, vec![gathered.clone(), tools.clone()], true),
            (
                "tools ran",
                at("I1"),
                vec![gathered.clone(), tools.clone(), results.clone()],
                true,
            ),
            ("sent back", at("I1"), vec![gathered.clone(), took_none(2, "I1")], true),
            (
                "asking",
                asking,
                vec![gathered.clone(), asked, took_none(3, "A2")],
                true,
            ),
            (
                "at an engine's cell not run",
                at("M1"),
                vec![gathered, tools, results, done],
                false,
            ),
        ]
    }

    // Item #230: a work moves where it stands, at any point of a cell under way or before it, whatever the version
    // changed of what every agent's cell shares: a row of the engine's fallback added to each (as flow 35 added three),
    // every shared row's note, every shared input's name and sources, and a field added to every form.
    #[test]
    fn a_work_moves_whatever_the_version_changed_of_what_every_agent_s_cell_shares() {
        let old = engineer();
        let mut new = old.clone();
        for c in new.cells.iter_mut().filter(|c| c.kind == "agent") {
            let mut case = c.rows.iter().find(|r| r.fallback).unwrap().clone();
            case.name = "new_case".into();
            case.conditions = vec![Atom::Engine("new_case".into())];
            c.rows.push(case);
            for r in c.rows.iter_mut().filter(|r| r.common) {
                r.note = Some("Said otherwise.".into());
            }
            for x in c.inputs.iter_mut().filter(|x| x.common) {
                x.name = x.name.as_ref().map(|n| format!("{n}, renamed"));
                x.from.push(Src::Engine("another".into()));
            }
            // Read first, since indexing a schema to change it adds what it lacks.
            if let Some(s) = c.schema.as_mut()
                && s["properties"]["outcome"]["anyOf"].is_array()
            {
                for form in s["properties"]["outcome"]["anyOf"].as_array_mut().into_iter().flatten() {
                    if let Some(Value::Object(p)) = form.get_mut("properties") {
                        p.insert("more".into(), json!({ "type": "string" }));
                    }
                }
            }
        }
        for (name, pos, rounds, _) in positions() {
            assert_eq!(plan(&old, &new, &pos, &rounds), Plan::Move, "{name}");
        }
    }

    // A work has begun a cell once a round ran there since it came, its tools, its question and a return sent back
    // included; it has not when it only came there, or stands at an engine's cell that has not run. Begun, the cell it
    // serves is done again when the version gives it another line, kind or inputs of its own; not begun, the work moves.
    #[test]
    fn a_cell_begun_whose_line_kind_or_inputs_changed_is_done_again() {
        let old = engineer();
        let mut line = old.clone();
        cell(&mut line, "I1").line = Some("reviewer".into());
        let mut kind = old.clone();
        cell(&mut kind, "I1").kind = "engine".into();
        let mut inputs = old.clone();
        let own = cell(&mut inputs, "I1").inputs.iter_mut().find(|x| !x.common).unwrap();
        own.from = vec![Src::Place("R0".into(), "brief".into())];
        for (new, what) in [
            (line, "is on another line"),
            (kind, "is of another kind"),
            (inputs, "reads other inputs"),
        ] {
            for (name, pos, rounds, begun) in positions() {
                let want = if begun && serving(&pos) == "I1" {
                    Plan::GoBack {
                        to: "I1".into(),
                        why: format!("cell I1, which the work had begun, {what}"),
                    }
                } else {
                    Plan::Move
                };
                assert_eq!(plan(&old, &new, &pos, &rounds), want, "{name}: {what}");
            }
        }
    }

    // Item #230, W245: an architect's work at an engine's cell that has not run moves when the version gives that cell
    // a row of its own (B1's decisions, to B5).
    #[test]
    fn a_work_at_an_engine_s_cell_not_run_moves_though_its_rows_changed() {
        let old = architect();
        let mut new = old.clone();
        let b1 = cell(&mut new, "B1");
        let mut more = b1.rows[0].clone();
        more.name = "more".into();
        b1.rows.insert(0, more);
        let rounds = [ran(1, "H5", "done", json!({}))];
        assert_eq!(plan(&old, &new, &at("B1"), &rounds), Plan::Move);
    }

    /// The version with the tools form of cell `I1` asking for a field `reason` that the samples' returns lack.
    fn tools_need_a_reason(g: &mut CompiledGraph) {
        let forms = cell(g, "I1").schema.as_mut().unwrap()["properties"]["outcome"]["anyOf"]
            .as_array_mut()
            .unwrap();
        let tools = forms
            .iter_mut()
            .find(|f| f["properties"].get("tools").is_some())
            .unwrap();
        tools["properties"]["reason"] = json!({ "type": "string" });
        require(tools, "reason");
    }

    // A return in a form every return shares is read only as the work's last one, by the cell that answers it: a work
    // that asked for tools before moves when the version asks the tools form for more, and one that asks for them now,
    // at the tool cell, does the cell that asked again.
    #[test]
    fn an_earlier_return_in_a_shared_form_takes_no_part() {
        let old = engineer();
        let mut new = old.clone();
        tools_need_a_reason(&mut new);
        let tools = sample("I1", "tools");
        let schema = |g: &CompiledGraph| g.cell("I1").unwrap().schema.clone().unwrap();
        assert!(flow::valid(&schema(&old), &tools));
        assert!(!flow::valid(&schema(&new), &tools));
        let mut all = positions();
        let (_, _, before, _) = all.pop().unwrap();
        assert_eq!(plan(&old, &new, &at("M1"), &before), Plan::Move);
        let (_, at_x1, now, _) = all.swap_remove(1);
        match plan(&old, &new, &at_x1, &now) {
            Plan::GoBack { to, why } => {
                assert_eq!(to, "I1");
                assert!(
                    why.starts_with("the return of cell I1 in round R1-2 no longer fits it ("),
                    "{why}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    /// The version with a field `after` that every return of cell `id` must hold, in its own answer for an agent's, one
    /// handed in now and one carried from an earlier version alike.
    fn needs_after(g: &mut CompiledGraph, id: &str, own: &str) {
        let c = cell(g, id);
        for schema in [&mut c.schema, &mut c.carried].into_iter().flatten() {
            let form = own_form(schema, own);
            form["properties"]["after"] = json!({ "type": "string" });
            require(form, "after");
        }
    }

    /// The part of a cell's schema that takes its own answer: the form of its outcome whose fields hold `field`, its
    /// outcome when it has no forms, or the whole for a return with no outcome.
    fn own_form<'s>(schema: &'s mut Value, field: &str) -> &'s mut Value {
        // Read first, since indexing a schema to change it adds what it lacks.
        if schema["properties"]["outcome"]["anyOf"].is_array() {
            return schema["properties"]["outcome"]["anyOf"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|f| f["properties"].get(field).is_some())
                .unwrap();
        }
        if schema["properties"]["outcome"].is_object() {
            return &mut schema["properties"]["outcome"];
        }
        schema
    }

    /// Field `name` required of what `schema` takes, besides those it requires.
    fn require(schema: &mut Value, name: &str) {
        match schema["required"].as_array_mut() {
            Some(r) => r.push(json!(name)),
            None => schema["required"] = json!([name]),
        }
    }

    /// `schema` with field `name` no longer required anywhere in it.
    fn without_required(schema: &mut Value, name: &str) {
        match schema {
            Value::Object(o) => {
                if let Some(Value::Array(r)) = o.get_mut("required") {
                    r.retain(|x| x != name);
                }
                o.values_mut().for_each(|v| without_required(v, name));
            }
            Value::Array(xs) => xs.iter_mut().for_each(|v| without_required(v, name)),
            _ => {}
        }
    }

    // The newest own answer of a cell of the main way that no longer fits has the work go back to that cell, the first
    // such of its main way; an earlier answer of the cell, which a newer one replaced, takes no part; nor does an
    // earlier return of a detour, as the tools' results, which the cell that runs them reads again only as the last.
    #[test]
    fn the_first_own_answer_that_no_longer_fits_is_done_again() {
        let old = engineer();
        let mut new = old.clone();
        needs_after(&mut new, "R1", "understanding");
        needs_after(&mut new, "X1", "results");
        let understood = sample("R1", "start");
        let results = sample("X1", "ran");
        let gathered = ran(4, "R0", "done", sample("R0", "done"));
        let rounds = [
            ran(1, "R1", "start", understood.clone()),
            ran(2, "I1", "tools", sample("I1", "tools")),
            ran(3, "X1", "ran", results.clone()),
            gathered.clone(),
        ];
        match plan(&old, &new, &at("I1"), &rounds) {
            Plan::GoBack { to, why } => {
                assert_eq!(to, "R1");
                assert!(
                    why.starts_with("the return of cell R1 in round R1-1 no longer fits it ("),
                    "{why}"
                );
            }
            other => panic!("{other:?}"),
        }
        // A newer answer of R1 that fits stands for the cell: the old one, which the work's version took too, is not read
        // again.
        let mut fits = understood.clone();
        fits["outcome"]["after"] = json!("a");
        let mut old_too = old.clone();
        let mut r1 = new.cell("R1").unwrap().clone();
        let schema = r1.schema.as_mut().unwrap();
        without_required(schema, "after");
        *cell(&mut old_too, "R1") = r1;
        let took = old_too.cell("R1").unwrap().schema.as_ref().unwrap();
        assert!(flow::valid(took, &fits) && flow::valid(took, &understood));
        let again = [
            ran(1, "R1", "start", understood),
            ran(2, "R1", "start", fits),
            ran(3, "X1", "ran", results.clone()),
            gathered,
        ];
        assert_eq!(plan(&old_too, &new, &at("I1"), &again), Plan::Move);
        // The tools' results as the last return: the cell they answered is done again.
        let last = [
            ran(1, "I1", "tools", sample("I1", "tools")),
            ran(2, "X1", "ran", results),
        ];
        match plan(&old, &new, &at("I1"), &last) {
            Plan::GoBack { to, .. } => assert_eq!(to, "I1"),
            other => panic!("{other:?}"),
        }
    }

    // A view names the cells whose rounds it reads in quotes, as the item's problem names R1; its comments, which hold
    // quotes of their own, are left out.
    #[test]
    fn a_view_names_the_cells_it_reads() {
        let problem = &NAMED["item_problem"];
        assert!(problem.contains("R1"), "{problem:?}");
        assert!(problem.iter().all(|w| !w.contains(' ')), "{problem:?}");
    }

    // An agent's return whose fields beside the outcome no longer fit the new version counts all the same: what it told
    // the owner and its side reports were acted on as its round ended, and no later row reads them.
    #[test]
    fn a_return_that_differs_only_beside_its_outcome_still_fits() {
        let now = architect();
        let mut old = now.clone();
        let schema = cell(&mut old, "Q1").schema.as_mut().unwrap();
        schema["properties"]["tell_owner"] = json!({ "type": "array" });
        schema["properties"]["side_reports"] = json!({ "type": "array" });
        let told = json!({ "outcome": { "pick": { "matter": 4, "why": "w" } }, "progress": "p",
                           "flow_feedback": "none", "tell_owner": [], "side_reports": [{ "kind": "other", "what": "w" }] });
        let taken = old.cell("Q1").unwrap().schema.as_ref().unwrap();
        assert!(flow::valid(taken, &told), "{:?}", flow::misfits(taken, &told, ""));
        assert!(!flow::valid(now.cell("Q1").unwrap().schema.as_ref().unwrap(), &told));
        assert_eq!(plan(&old, &now, &at("Q2"), &[ran(1, "Q1", "picked", told)]), Plan::Move);
    }

    // What the engine records of a row it took itself, which its cell's schema does not take, is no return: the work
    // moves whatever the new version says of that cell, even that it is gone.
    #[test]
    fn a_row_the_engine_took_itself_is_no_return() {
        let old = engineer();
        let mut new = old.clone();
        new.cells.retain(|c| c.id != "A2");
        assert!(!flow::valid(
            old.cell("A2").unwrap().schema.as_ref().unwrap(),
            &json!({})
        ));
        let rounds = [
            ran(1, "R1", "start", sample("R1", "start")),
            ran(2, "A2", "holder_gone", json!({})),
        ];
        assert_eq!(plan(&old, &new, &at("I1"), &rounds), Plan::Move);
    }

    // A work whose cell is gone goes back to the latest cell of its main way the version has; with none, it is stuck.
    // One that waits to go back to a cell that is gone goes back the same way, and one that could go on from no cell of
    // its own version looks again in the new one.
    #[test]
    fn a_work_whose_cell_is_gone_goes_back_to_the_latest_it_walked() {
        let now = engineer();
        let mut old = now.clone();
        let mut gone = old.cell("M1").unwrap().clone();
        gone.id = "Z1".into();
        old.cells.push(gone);
        let rounds = vec![
            ran(1, "R1", "start", sample("R1", "start")),
            ran(2, "R0", "done", sample("R0", "done")),
            ran(3, "I1", "done", sample("I1", "done")),
        ];
        let back_to = |to: &str, why: &str| Plan::GoBack {
            to: to.into(),
            why: why.into(),
        };
        assert_eq!(plan(&old, &now, &at("Z1"), &rounds), back_to("I1", "cell Z1 is gone"));
        let mut none = now.clone();
        none.cells.retain(|c| !["R1", "R0", "I1"].contains(&c.id.as_str()));
        // Its last return is of a cell gone too, which the cell after it would read.
        assert_eq!(
            plan(&old, &none, &at("Z1"), &rounds),
            Plan::Stuck {
                why: "cell I1, which the work handed a return in at, is gone, and the version has none of the cells \
                      the work walked"
                    .into(),
                back_to: None,
            }
        );
        let mut d1_gone = now.clone();
        d1_gone.cells.retain(|c| c.id != "D1");
        let asking = Position {
            at: "A2".into(),
            stopped: Some("D1".into()),
            callers: vec!["D1".into()],
            ..Position::default()
        };
        assert_eq!(
            plan(&now, &d1_gone, &asking, &rounds[..1]),
            back_to("R1", "cell D1, which the work goes back to, is gone or of another kind")
        );
        let stuck = [rounds[0].clone(), ran(2, "V1", "stuck", json!({}))];
        assert_eq!(
            plan(&now, &now, &at("V1"), &stuck),
            back_to(
                "R1",
                "it could go on from none of the cells it walked in the version before"
            )
        );
    }

    // Item #230, A12: an engineer's work whose merge stands is judged by the rounds after its merge alone, so that
    // neither the engine nor the owner's Start ever takes it back before its merge: an answer from before the merge
    // that no longer fits moves it, a cell after the merge it had begun and the version changed is done again, and with
    // no cell walked since the merge it is stuck with nowhere to go back to. Once watching main took the merge back, as a
    // flow version before watched it, going back past the merge is stuck, with that cell for the owner's Start.
    #[test]
    fn a_merged_work_never_goes_back_before_its_merge() {
        let old = engineer();
        let rounds = [
            ran(1, "R1", "start", sample("R1", "start")),
            ran(2, "D2", "done", sample("D2", "done")),
            ran(3, "M2", "merged", sample("M2", "merged")),
        ];
        // An answer from before the merge that no longer fits, the understanding or the design L2 reads, moves the
        // work: the cells before its merge are never done again, and later ones read those answers as they are.
        let mut r1 = old.clone();
        needs_after(&mut r1, "R1", "understanding");
        assert_eq!(plan(&old, &r1, &at("L2"), &rounds), Plan::Move);
        let mut d2 = old.clone();
        needs_after(&mut d2, "D2", "examined");
        assert!(read_from(&d2, &at("L2")).contains("D2"));
        assert_eq!(plan(&old, &d2, &at("L2"), &rounds), Plan::Move);
        // A cell after the merge it had begun, on another line, is done again.
        let mut l2 = old.clone();
        cell(&mut l2, "L2").line = Some("reviewer".into());
        let begun = [rounds[2].clone(), took_none(4, "L2")];
        assert_eq!(
            plan(&old, &l2, &at("L2"), &begun),
            Plan::GoBack {
                to: "L2".into(),
                why: "cell L2, which the work had begun, is on another line".into()
            }
        );
        // The cell it stands at gone, with no cell walked since the merge, it is stuck with nowhere to go back to,
        // though the version has every cell from before the merge.
        let mut gone = old.clone();
        gone.cells.retain(|c| c.id != "L2");
        match plan(&old, &gone, &at("L2"), &rounds) {
            Plan::Stuck { back_to, .. } => assert_eq!(back_to, None),
            other => panic!("{other:?}"),
        }
        // Once watching main took the merge back, as a flow version before watched it, the merge no longer stands: going
        // back past it is stuck, for the owner's Start, as for any round that cannot be undone.
        let broken = json!({ "main": { "kind": "broken_by_merge", "checks": [] } });
        let reverted = [&rounds[..], &[ran(5, "L1", "broken_by_merge", broken)]].concat();
        match plan(&old, &d2, &at("T6"), &reverted) {
            Plan::Stuck { why, back_to } => {
                assert!(
                    why.ends_with("going back to cell D2 would do round R1-3 again, which merged the pull request"),
                    "{why}"
                );
                assert_eq!(back_to.as_deref(), Some("D2"));
            }
            other => panic!("{other:?}"),
        }
    }

    /// A return of an architect's cell that answers an engineer's question, as a round stores it, with `effects`.
    fn answered(n: i64, effects: Value) -> Round {
        let outcome = json!({ "reply": { "text": "Keep it." }, "effects": effects });
        ran(
            n,
            "H3",
            "done",
            json!({ "outcome": outcome, "progress": "p", "flow_feedback": "none", "tell_owner": [] }),
        )
    }

    // An architect's work never goes back past a round that did something at once: an interjection, a reply that went
    // out; a reply that waits for the batch's item changes, which is done again, holds it back from nothing.
    #[test]
    fn an_architect_work_never_goes_back_past_what_it_did_at_once() {
        let old = architect();
        let mut new = old.clone();
        // The version asks the pick for a field the round's return lacks.
        needs_after(&mut new, "Q1", "pick");
        let pick =
            json!({ "outcome": { "pick": { "matter": 4, "why": "w" } }, "progress": "p", "flow_feedback": "none" });
        assert!(flow::valid(old.cell("Q1").unwrap().schema.as_ref().unwrap(), &pick));
        assert!(!flow::valid(new.cell("Q1").unwrap().schema.as_ref().unwrap(), &pick));
        let interjected = json!({ "actions": [{ "kind": "interject", "work": 7, "text": "Stop." }] });
        let held = json!({ "item_changes": [{ "kind": "edit_item", "item": "1", "part": "title", "text": "T" }] });
        let cases = [
            (
                answered(2, interjected),
                Some("acted at once on a work or on new starts"),
            ),
            (answered(2, json!({})), Some("replied to the engineer")),
            (answered(2, held), None),
        ];
        for (h3, done) in cases {
            let rounds = [ran(1, "Q1", "picked", pick.clone()), h3];
            let got = plan(&old, &new, &at("Q0"), &rounds);
            match (done, got) {
                (Some(what), Plan::Stuck { why, back_to }) => {
                    assert!(
                        why.ends_with(&format!("would do round R1-2 again, which {what}")),
                        "{why}"
                    );
                    assert_eq!(back_to.as_deref(), Some("Q1"));
                }
                (None, Plan::GoBack { to, .. }) => assert_eq!(to, "Q1"),
                (done, got) => panic!("{done:?}: {got:?}"),
            }
        }
    }

    // A cell done again from its start redoes the rounds of its latest stretch, its detours between them included, and
    // none before it; a cell the work never ran redoes what came after its last cell of the main way.
    #[test]
    fn the_rounds_done_again_are_the_cell_s_latest_stretch() {
        let g = engineer();
        let rounds = [
            ran(1, "R1", "start", sample("R1", "start")),
            ran(2, "R0", "done", sample("R0", "done")),
            ran(3, "I1", "tools", sample("I1", "tools")),
            ran(4, "X1", "ran", sample("X1", "ran")),
            took_none(5, "I1"),
            ran(6, "I1", "tools", sample("I1", "tools")),
        ];
        assert_eq!(redone_from(&g, &rounds, "I1"), 2);
        assert_eq!(redone_from(&g, &rounds, "R1"), 0);
        assert_eq!(redone_from(&g, &rounds[..4], "M1"), 3);
        assert_eq!(redone_from(&g, &rounds[..2], "M1"), 2);
    }
}
