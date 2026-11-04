//! The engine's checks of a return: `engine.checks` in the flow, each named by the cells that use it. A return that
//! fails one is sent back with the reason, before it takes a row, unless only checks the work says it changes found its
//! problems, which are then set aside ([`crate::set_aside`]). The reason says every problem of every check that
//! applies, and a check says every problem it can find in the return ([`Problems`]), since each send-back costs a round.
//! The one check that does not is `cards_match_records`: no cell of the flow calls it, so it sends nothing back and
//! `crate::cards::check`, which stops at its first problem, is not converted; both go with `cards.rs` in #203.
//!
//! A check whose `at` lies in one of the forms every agent has (ask, answers, redirect) runs on returns of that form;
//! any other runs on the cell's own answer, whether or not the place is there, since some checks are about a place
//! left out.
//!
//! In this version some checks look at less than the flow says, because what they would compare is not recorded
//! yet: cited files are checked to exist, not to be in the diff; a table of findings is checked for one answer per id;
//! a red check said not to be caused by the change is taken as written when main's checks cannot be read; a ruling
//! said to be the advisor's, or the architect's or the owner's in its place, is taken as written once the advisor was
//! asked.

use std::collections::BTreeSet;
use std::path::Path;

use flow::{CompiledCell, CompiledGraph};
use serde_json::Value;

use crate::inputs::{answered, at_path, last_return};
use crate::work::{self, Item, Round, RoundId, Work};
use crate::{Engine, form_of, git, results, set_aside, test_name, test_tables, values};
use sessions::worktree::Unfit;

pub use db::DESIGN_CARD_RULING;

pub struct Ctx<'a> {
    pub e: &'a Engine,
    pub w: &'a Work,
    pub g: &'a CompiledGraph,
    pub cell: &'a CompiledCell,
    /// The item the work is on, and its project; none for a work of the architect.
    pub item: Option<&'a Item>,
    pub project: Option<&'a config::Project>,
    pub output: &'a Value,
    pub unwrapped: Value,
    /// The worktree's HEAD when the round started.
    pub head_start: Option<String>,
    pub columns: &'a [(String, String)],
    /// The rows of the cell's numbered table as this round was given them, each with its number.
    pub table: &'a [Value],
    pub interjections: &'a [Value],
    /// The commit of the project's main the round was pinned to, which its inputs that read the project's files were
    /// read at; none for a round of a cell with none.
    pub main: Option<&'a str>,
}

/// What a row a fix answered by its number in the table it was given is known by outside the table: a finding's
/// number or a test's name; the number itself for a round given no numbered table, as before the fix table numbered
/// its rows.
/// Where a row of the fix's table comes from when the re-check insisted on it, as the table's rows say it.
const INSISTED: &str = "T4·rows[insist]";

fn known_as(table: &[Value], id: &str) -> String {
    table
        .iter()
        .find(|r| r["row"] == id)
        .and_then(crate::inputs::row_key)
        .unwrap_or(id)
        .to_string()
}

/// The numbered table a round of the fix cell was given, as its record keeps it.
fn table_of(r: &Round) -> Vec<Value> {
    items(r.input.get("engine").and_then(|x| x.get("table")))
}

/// What a check found wrong with a return: one text for each problem, in the order found.
#[derive(Debug, Default, PartialEq)]
pub struct Problems(Vec<String>);

impl Problems {
    /// Adds a problem; one already found, word for word, is the same problem and is not added again.
    pub fn push(&mut self, why: impl Into<String>) {
        let why = why.into();
        if !self.0.contains(&why) {
            self.0.push(why);
        }
    }

    /// The check's result once it has looked at everything: ok when it found nothing.
    pub fn into_result(self) -> Result<(), Problems> {
        if self.is_empty() { Ok(()) } else { Err(self) }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds one problem about a group of ids, all of them in it: `single` says it of one id, `several` of the ids joined
    /// with `, `; nothing for no id.
    pub fn about(&mut self, ids: &[String], single: impl Fn(&str) -> String, several: impl Fn(&str) -> String) {
        match ids {
            [] => {}
            [x] => self.push(single(x)),
            xs => self.push(several(&xs.join(", "))),
        }
    }

    /// Adds the problems of another look at the same return, after those found so far.
    pub fn extend(&mut self, other: Problems) {
        for why in other.0 {
            self.push(why);
        }
    }

    pub fn texts(&self) -> &[String] {
        &self.0
    }
}

// A reason built as one text (a failed read, a message of a helper) is one problem: it keeps every `?` on a
// `Result<_, String>` inside a check as it is.
impl From<String> for Problems {
    fn from(why: String) -> Self {
        Problems(vec![why])
    }
}

impl From<&str> for Problems {
    fn from(why: &str) -> Self {
        Problems(vec![why.to_string()])
    }
}

impl From<Vec<String>> for Problems {
    fn from(all: Vec<String>) -> Self {
        Problems(all)
    }
}

/// One problem of a return that is sent back, with the check that found it and the field it is about: what
/// `rounds.rejected` keeps under `problems`, one entry each, so that returns sent back can be counted by check and by
/// field.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    /// The check's name; `format` for a return that does not fit the schema of its cell, which no check is named.
    pub check: String,
    /// The check's `at` in the flow (`columns[pass].evidence`), or for `format` the path in the schema with a list's
    /// items written `[]` (`columns[].evidence`); none for a check about the return as a whole, and for a refusal
    /// about no field.
    pub field: Option<String>,
    /// What is wrong, and how to put it right.
    pub text: String,
}

impl Problem {
    /// A place of a return that does not fit its cell's schema.
    pub fn format(place: &flow::Misfit) -> Problem {
        Problem {
            check: "format".to_string(),
            field: place.field.clone(),
            text: place.text.clone(),
        }
    }

    /// The line this problem makes in the text a return is sent back with ([`db::problem_line`]).
    fn line(&self) -> String {
        db::problem_line(&self.check, self.field.as_deref(), &self.text)
    }
}

/// The text a return that failed checks is sent back with: a line for each problem, in the order found.
pub fn reason(problems: &[Problem]) -> String {
    problems.iter().map(Problem::line).collect::<Vec<_>>().join("\n")
}

/// What `rounds.rejected` keeps for a return sent back: under `kind` (`check` or `format`) the text it was sent back
/// with, and then under `problems` each problem, `{check, field, text}`. The text comes first; what reads it reads it
/// by `kind` ([`db::sent_back_text`]).
pub fn record(kind: &str, text: String, problems: &[Problem]) -> Value {
    let mut sent = serde_json::Map::new();
    sent.insert(kind.to_string(), Value::String(text));
    sent.insert(
        "problems".to_string(),
        Value::Array(
            problems
                .iter()
                .map(|p| serde_json::json!({ "check": p.check, "field": p.field, "text": p.text }))
                .collect(),
        ),
    );
    Value::Object(sent)
}

/// Runs every check of the cell that applies to the return; gives every problem of every check, each once, in the
/// order found: a return is sent back once with all that can be found, since each send-back costs a round.
/// [`reason`] makes the text it is sent back with. A problem only a check the work says it changes found is not sent
/// back but set aside ([`crate::set_aside`]): the return is taken with those, and sent back with only the others when
/// there are others. A declaration the return makes that its own check refuses holds nothing.
pub fn run(c: &Ctx<'_>) -> Result<Vec<Problem>, Vec<Problem>> {
    let found = every_problem(c);
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let refused = found.iter().any(|p| p.check == set_aside::KNOWN);
    let own = (!refused).then(|| at_path(&c.unwrapped, set_aside::DECLARED)).flatten();
    let declared = match set_aside::declared(c.e, c.w, c.g, c.item, &c.cell.id, own.as_ref()) {
        Ok(d) => d,
        Err(why) => {
            let mut back = found;
            back.push(Problem {
                check: set_aside::KNOWN.to_string(),
                field: Some(set_aside::DECLARED.to_string()),
                text: format!("the checks this work says it changes could not be read: {why}"),
            });
            return Err(back);
        }
    };
    let (back, aside) = set_aside::part(found, &declared);
    if back.is_empty() { Ok(aside) } else { Err(back) }
}

/// Every problem of every check of the cell that applies to the return, each once, in the order found.
fn every_problem(c: &Ctx<'_>) -> Vec<Problem> {
    let form = form_of(c.g, c.cell, c.output);
    let mut found: Vec<Problem> = Vec::new();
    for ch in &c.cell.checks {
        let top = ch.at.as_deref().map(|a| a.split(['.', '[']).next().unwrap_or_default());
        let form_field = top.filter(|t| c.cell.forms.iter().any(|f| f.field.as_deref() == Some(*t)));
        let applies = match form_field {
            Some(f) => form.as_deref() == Some(f),
            None => form.is_none(),
        };
        if !applies {
            continue;
        }
        let value = ch.at.as_deref().and_then(|a| at_path(&c.unwrapped, a));
        if let Err(problems) = one(c, &ch.check, ch.at.as_deref(), value.as_ref()) {
            add(&mut found, &ch.check, ch.at.as_deref(), &problems);
        }
    }
    found
}

/// Adds the problems a check found to those found so far, each as a [`Problem`] of that check and place. A problem
/// whose line is there already, from this check or another, is the same problem and is not added again.
fn add(found: &mut Vec<Problem>, check: &str, at: Option<&str>, problems: &Problems) {
    for text in problems.texts() {
        let p = Problem {
            check: check.to_string(),
            field: at.map(str::to_string),
            text: text.clone(),
        };
        if !found.iter().any(|q| q.line() == p.line()) {
            found.push(p);
        }
    }
}

/// For a miss that the tests after it presuppose to be absent: the check stops here, and says this one. Any other
/// miss is pushed to a [`Problems`] and the check goes on, so that one send-back says all of them: a return sent back
/// costs a round, and a check that stops at the first of several misses costs one for each.
pub(crate) fn fail<T>(why: impl Into<String>) -> Result<T, Problems> {
    Err(Problems(vec![why.into()]))
}

/// How the ids a return gave differ from the ids the engine listed.
#[derive(Debug, PartialEq)]
pub(crate) struct IdMisses {
    /// Given, and not listed; each once, in the order given.
    pub(crate) unknown: Vec<String>,
    /// Given more than once, listed or not; each once, in the order of its first appearance.
    pub(crate) twice: Vec<String>,
    /// Listed, and not given; in the order listed.
    pub(crate) missing: Vec<String>,
}

pub(crate) fn id_misses<S: AsRef<str>>(want: &[S], given: &[String]) -> IdMisses {
    let mut unknown: Vec<String> = Vec::new();
    let mut twice: Vec<String> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for id in given {
        if !want.iter().any(|w| w.as_ref() == id) && !unknown.contains(id) {
            unknown.push(id.clone());
        }
        if !seen.insert(id) && !twice.contains(id) {
            twice.push(id.clone());
        }
    }
    // The order of the first appearance: a text given twice is placed where it was first given.
    twice.sort_by_key(|id| given.iter().position(|g| g == id));
    let mut missing: Vec<String> = Vec::new();
    for w in want.iter().map(AsRef::as_ref) {
        if !seen.contains(w) && !missing.iter().any(|m| m == w) {
            missing.push(w.to_string());
        }
    }
    IdMisses {
        unknown,
        twice,
        missing,
    }
}

/// What is wrong with the columns a review's return answers (`given`) against the columns the engine listed (`want`):
/// the columns that are none of them, those answered twice, and those not answered, each kind said once with all its
/// columns.
fn columns_problems(want: &[&str], given: &[String]) -> Problems {
    let misses = id_misses(want, given);
    let mut problems = Problems::default();
    problems.about(
        &misses.unknown,
        |x| format!("there is no column {x}"),
        |xs| format!("there are no columns {xs}"),
    );
    problems.about(
        &misses.twice,
        |x| format!("column {x} is answered twice"),
        |xs| format!("columns {xs} are answered twice"),
    );
    problems.about(
        &misses.missing,
        |x| format!("column {x} is not answered"),
        |xs| format!("columns {xs} are not answered"),
    );
    problems
}

/// The findings of the last review (`due`) that the table of the fix holds no row for (`given`), all of them in one
/// problem.
fn previous_problems(due: &[String], given: &[String]) -> Problems {
    let mut problems = Problems::default();
    problems.about(
        &id_misses(due, given).missing,
        |x| format!("no row for {x}, a finding of the last review"),
        |xs| format!("no row for {xs}, findings of the last review"),
    );
    problems
}

/// What is wrong with the checks a declaration names (`named`) against the flow's checks (`known`): a name that is no
/// check, one named twice, and one no declaration sets aside, each kind said once with all its names.
fn declared_problems(known: &[&str], named: &[String]) -> Problems {
    let misses = id_misses(known, named);
    let mut problems = Problems::default();
    problems.about(
        &misses.unknown,
        |x| format!("there is no check {x} among the flow's engine.checks"),
        |xs| format!("there are no checks {xs} among the flow's engine.checks"),
    );
    problems.about(
        &misses.twice,
        |x| format!("check {x} is named twice"),
        |xs| format!("checks {xs} are named twice"),
    );
    let never: Vec<String> = set_aside::NEVER
        .iter()
        .filter(|c| named.iter().any(|n| n == *c))
        .map(|c| c.to_string())
        .collect();
    problems.about(
        &never,
        |x| format!("{x} is what makes a declaration no exemption, so no declaration names it: leave it out"),
        |xs| format!("{xs} are what makes a declaration no exemption, so no declaration names them: leave them out"),
    );
    problems
}

/// What is wrong with a review's answers to the findings set aside it was given (`due`, their numbers): a number that
/// is none of them, one answered twice, one not answered, and one answered real that names no finding of the return
/// (`findings`, their numbers), each kind said once with all its numbers.
fn set_aside_problems(due: &[String], answers: &[Value], findings: &[String]) -> Problems {
    let given: Vec<String> = answers
        .iter()
        .map(|a| a["id"].as_str().unwrap_or_default().to_string())
        .collect();
    let misses = id_misses(due, &given);
    let mut problems = Problems::default();
    problems.about(
        &misses.unknown,
        |x| format!("{x} is not a finding set aside you were given"),
        |xs| format!("{xs} are not findings set aside you were given"),
    );
    problems.about(
        &misses.twice,
        |x| format!("finding set aside {x} is answered twice"),
        |xs| format!("findings set aside {xs} are answered twice"),
    );
    problems.about(
        &misses.missing,
        |x| format!("finding set aside {x} is not answered: answer it old_rule or real"),
        |xs| format!("findings set aside {xs} are not answered: answer each old_rule or real"),
    );
    let unfound: Vec<String> = answers
        .iter()
        .filter(|a| a["answer"] == "real")
        .filter(|a| !a["finding"].as_str().is_some_and(|f| findings.iter().any(|x| x == f)))
        .map(|a| a["id"].as_str().unwrap_or_default().to_string())
        .collect();
    problems.about(
        &unfound,
        |x| {
            format!("{x} is answered real, but the finding it names is not one of this return's: write it as a finding")
        },
        |xs| {
            format!("{xs} are answered real, but the findings they name are not this return's: write each as a finding")
        },
    );
    problems
}

/// What is wrong with the answers of a table of rows (`given`, the id of each answer) against the rows of the table the
/// round was given (`rows`, when it `numbered` them): an id answered twice, whether or not it is a row; and, for a
/// numbered table, the ids that are no row of it and the rows with no answer. A round given no numbered table is
/// checked for answers given twice alone.
fn table_problems(given: &[String], rows: &[&str], numbered: bool) -> Problems {
    let misses = id_misses(rows, given);
    let mut problems = Problems::default();
    problems.about(
        &misses.twice,
        |x| format!("{x} is answered twice"),
        |xs| format!("{xs} are answered twice"),
    );
    if numbered {
        problems.about(
            &misses.unknown,
            |x| format!("{x} is not a row of the table: answer each row by its number"),
            |xs| format!("{xs} are not rows of the table: answer each row by its number"),
        );
        problems.about(
            &misses.missing,
            |x| format!("no answer for {x}"),
            |xs| format!("no answer for {xs}"),
        );
    }
    problems
}

/// The texts at a place, a list flattened.
fn strs(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(xs)) => xs.iter().flat_map(|x| strs(Some(x))).collect(),
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other.to_string()],
    }
}

fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(xs)) => xs.clone(),
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other.clone()],
    }
}

/// Whether the owner ruled on a decision card of the item, disagreeing with a decision or choosing another option,
/// since its acceptance was locked (or at all, while none is locked). The locked acceptance changes in two ways only:
/// the advisor's `new_acceptance`, and the design that follows such a ruling, which hands in the acceptance it needs
/// and which the engine locks after design. An owner's answer that only confirms, or edits to the item's text, are not
/// such a ruling.
fn ruled_since_locked(conn: &db::rusqlite::Connection, item: i64) -> Result<bool, String> {
    let locked_at: Option<i64> = conn
        .query_row(
            "SELECT r.ended_at FROM items i JOIN rounds r ON r.work = i.acceptance_work AND r.n = i.acceptance_round
             WHERE i.id = ?1",
            [item],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let cards = work::query_json(
        conn,
        "SELECT c.rulings FROM design_cards c JOIN works w ON w.id = c.work WHERE w.item = ?1",
        &[&item],
        100,
    )?;
    Ok(cards.iter().flat_map(|x| items(Some(&x["rulings"]))).any(|r| {
        r["by"] == "owner"
            && db::ruling_disagrees(&r)
            && locked_at.is_none_or(|l| r["at"].as_i64().is_some_and(|at| at > l))
    }))
}

impl Ctx<'_> {
    /// Whether the owner ruled on a decision card of the item, disagreeing with a decision, since its acceptance was
    /// locked: the design that follows such a ruling writes the acceptance it needs, which the engine locks after
    /// design.
    fn ruled_since_locked(&self) -> Result<bool, String> {
        ruled_since_locked(self.e.db.conn(), self.item()?.id)
    }

    /// The item the work is on, for a check about it.
    fn item(&self) -> Result<&Item, String> {
        self.item
            .ok_or_else(|| format!("work {} is on no item, and the check is about one", self.w.id))
    }

    /// The project the work's item is in, for a check about it.
    fn project(&self) -> Result<&config::Project, String> {
        self.project
            .ok_or_else(|| format!("work {} is in no project the configuration has", self.w.id))
    }

    fn wt(&self) -> Result<&Path, String> {
        self.w
            .worktree
            .as_deref()
            .ok_or_else(|| "the work has no worktree".to_string())
    }

    fn fork(&self) -> Result<String, String> {
        git::fork_point(self.wt()?, &self.project()?.main)
    }

    /// The files the branch changed against main, committed.
    fn branch_changes(&self) -> Result<Vec<String>, String> {
        git::changed(self.wt()?, &self.fork()?, "HEAD")
    }

    /// The files the work itself changed from one commit to another, apart from what merging main brought in.
    fn own(&self, from: &str, to: &str) -> Result<Vec<String>, String> {
        git::own_changes(self.wt()?, &self.project()?.main, from, to)
    }

    fn design(&self, files: &[String]) -> Vec<String> {
        let paths = self.project.map(config::Project::design_paths).unwrap_or_default();
        files.iter().filter(|f| git::under(f, &paths)).cloned().collect()
    }

    /// Where the attempts of this cell that came just before this round, each sent back, cut off or failed, began: the
    /// head at the start of the earliest of them; none when the round before at this cell was not one.
    fn attempts_start(&self) -> Result<Option<String>, String> {
        let rounds = work::rounds_at(self.e.db.conn(), self.w.id, &self.cell.id)?;
        Ok(rounds
            .iter()
            .filter(|r| r.ended_at.is_some() && !crate::failed::was_step(r))
            .take_while(|r| r.rejected.is_some())
            .last()
            .and_then(|r| r.input["engine"]["head"].as_str().map(str::to_string)))
    }

    /// The rounds of a cell of this work that handed in its own answer, newest first.
    fn accepted(&self, cell: &str) -> Result<Vec<Round>, String> {
        answered(self.e, self.w, self.g, cell)
    }

    fn last(&self, cell: &str, path: &str) -> Result<Option<Value>, String> {
        Ok(last_return(self.e, self.w, self.g, cell, path)?.and_then(|r| at_path(&r, path)))
    }

    /// The plan of the work a review of it tries: the cell that wrote it, its return as the rows read it, and the
    /// decisions of the item its round was given, numbered as that round had them; the newest design. None before any.
    fn plan_reviewed(&self) -> Result<Option<(String, Value, Vec<Value>)>, String> {
        let Some(r) = self.accepted("D2")?.into_iter().next() else {
            return Ok(None);
        };
        let plan = r.output.as_ref().map(|o| self.g.unwrap(o)).unwrap_or(Value::Null);
        Ok(Some(("D2".to_string(), plan, crate::inputs::decisions_of(&r))))
    }

    /// The acceptance conditions in force: those recorded on the item, or else those the latest design handed in, or
    /// the research's plan.
    fn acceptance(&self) -> Result<Vec<Value>, String> {
        let locked: Option<String> = self
            .e
            .db
            .conn()
            .query_row("SELECT acceptance FROM items WHERE id = ?1", [self.item()?.id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if let Some(Value::Array(xs)) = work::json_col(locked) {
            return Ok(xs);
        }
        let mut written: Vec<(RoundId, Value)> = Vec::new();
        for (cell, path) in [("D2", "design.acceptance"), ("R1", "can_start.acceptance")] {
            let returns = crate::inputs::returns_holding(self.e, self.w, self.g, cell, path)?;
            if let Some((round, conditions)) = returns.iter().find_map(|(r, x)| Some((*r, at_path(x, path)?))) {
                written.push((round, conditions));
            }
        }
        let newest = written.into_iter().max_by_key(|(r, _)| *r).map(|(_, v)| v);
        Ok(items(newest.as_ref()))
    }

    /// The results of the ci and ci_before runs of this work, newest first: the tool, the head it ran on, and its
    /// checks.
    fn ci_runs(&self, tool: &str) -> Result<Vec<Value>, String> {
        let rows = work::query_json(
            self.e.db.conn(),
            "SELECT result FROM tool_runs WHERE work = ?1 AND tool = ?2 AND result IS NOT NULL ORDER BY id DESC",
            &[&self.w.id, &tool],
            1000,
        )?;
        Ok(rows.into_iter().map(|r| r["result"].clone()).collect())
    }

    /// The runs of ci_before of this work that ran, newest first, each with the files and the commit its request put on
    /// the code before the change. The files and the commit are in the request the tool was asked with; a run whose
    /// request does not name files has none.
    fn before_runs(&self) -> Result<Vec<BeforeRun>, String> {
        before_runs_of(self.e, self.w)
    }

    /// The files of a run of ci_before that differ between its commit and the head handed in: the test files it put on
    /// the code before the change, as they are now. A run whose commit cannot be compared with the head, because the
    /// worktree no longer has it, counts all its files as changed: it is run again. A run with no files has none.
    fn changed_since(&self, run: &BeforeRun) -> Vec<String> {
        if run.files.is_empty() {
            return Vec::new();
        }
        let compared = match (&run.head, self.wt()) {
            (Some(head), Ok(wt)) => git::changed_among(wt, head, "HEAD", &run.files),
            _ => Err("the commit of the run is not in the worktree".to_string()),
        };
        compared.unwrap_or_else(|_| run.files.clone())
    }

    /// Whether a gh command or a query, as an agent writes it, is one this work handed in.
    fn handed_in(&self, command: &str) -> Result<bool, String> {
        let plain = |s: &str| s.trim().trim_start_matches("gh ").trim().to_string();
        let want = plain(command);
        if want.is_empty() {
            return Ok(false);
        }
        let rows = work::query_json(
            self.e.db.conn(),
            "SELECT coalesce(json_extract(args, '$.command'), json_extract(args, '$.sql')) AS given
             FROM tool_runs WHERE work = ?1 AND tool IN ('gh', 'query')",
            &[&self.w.id],
            10_000,
        )?;
        Ok(rows
            .iter()
            .any(|r| r["given"].as_str().is_some_and(|g| plain(g) == want)))
    }

    fn pushed_head(&self) -> Result<Option<String>, String> {
        self.e
            .db
            .conn()
            .query_row("SELECT pushed_head FROM works WHERE id = ?1", [self.w.id], |r| r.get(0))
            .map_err(|e| e.to_string())
    }

    /// What each run of ci on the head pushed last reported (its `push`), newest first; none when nothing was pushed.
    fn head_pushes(&self) -> Result<Vec<Value>, String> {
        head_pushes_of(self.e, self.w)
    }

    /// The separators of the parts of a test's name, as the project's configuration names them; none when it names
    /// none, or the work is in no project.
    fn separators(&self) -> Vec<String> {
        self.project
            .and_then(|p| p.ci.results.as_ref())
            .map(|r| r.separators.clone())
            .unwrap_or_default()
    }

    /// Whether the project's configuration names the results of its fast CI (`ci.results`), so that a run of ci or
    /// ci_before can report a test's result; a work in no project is not judged by it.
    fn results_named(&self) -> bool {
        self.project.is_none_or(|p| p.ci.results.is_some())
    }

    /// The names of the checks of the CI runs recorded for this work, newest run first: those of the fast CI (a `ci`
    /// run keeps them under `push`) and of the full CI, which the merge runs (an attempt of it keeps its list as it is),
    /// as did the full CI on the draft of an earlier flow (an attempt keeps its first run's under `first`, a run again
    /// its list as it is). A run that did not get as far as its checks has none.
    fn ci_jobs(&self) -> Result<Vec<String>, String> {
        let mut runs = self.ci_runs("ci")?;
        runs.extend(self.ci_runs("merge")?);
        runs.extend(self.ci_runs("full_ci")?);
        Ok(check_names(&runs))
    }
}

/// The names of the checks in the recorded results of CI runs, each once, in the order the runs give them.
fn check_names(runs: &[Value]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for run in runs {
        let checks = run
            .pointer("/push/checks")
            .or_else(|| run.get("first"))
            .or(run.is_array().then_some(run));
        for check in items(checks) {
            if let Some(name) = check["name"].as_str()
                && !names.iter().any(|n| n == name)
            {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Whether some file of the branch at HEAD holds `text` as it is written: how a command a condition points to is
/// looked for, as a test's name is.
fn text_on_branch(worktree: &Path, text: &str) -> Result<(), String> {
    git::git(worktree, &["grep", "-q", "-F", "-e", text, "HEAD"])
        .map(|_| ())
        .map_err(|_| format!("the step {text} is not in any file of the branch"))
}

fn one(c: &Ctx<'_>, check: &str, at: Option<&str>, v: Option<&Value>) -> Result<(), Problems> {
    match check {
        "research_only" => {
            let clean = git::is_clean(c.wt()?)?;
            let outside: Vec<String> = match &c.head_start {
                Some(start) => c.own(start, "HEAD")?,
                None => Vec::new(),
            };
            research_problems(clean, &outside).into_result()
        }
        "design_paths_unchanged" => {
            let wt = c.wt()?;
            let Some(start) = &c.head_start else { return Ok(()) };
            // The change is counted from where the attempts of this cell began: commits an earlier attempt made and was
            // sent back for are still on the branch, and a revert in this round that undoes them leaves no change.
            let base = c.attempts_start()?.unwrap_or_else(|| start.clone());
            let net = c.design(&c.own(&base, "HEAD")?);
            let loose = c.design(&git::changed_since(wt, "HEAD")?);
            if net.is_empty() && loose.is_empty() {
                return Ok(());
            }
            let own = c.design(&c.own(start, "HEAD")?);
            let left = if base == *start {
                Vec::new()
            } else {
                c.design(&c.own(&base, start)?)
            };
            let earlier = if left.is_empty() {
                Vec::new()
            } else {
                // The work's own commits that change them; main's, which a merge of main brought in, are left out.
                let range = format!("{base}..{start}");
                let not_main = format!("^origin/{}", c.project()?.main);
                let mut args = vec!["log", "--format=%h %s", range.as_str(), not_main.as_str(), "--"];
                args.extend(left.iter().map(String::as_str));
                git::git(wt, &args)?.lines().map(str::to_string).collect::<Vec<_>>()
            };
            let mut why = Vec::new();
            if !own.is_empty() {
                why.push(format!("this round changed the design paths: {}", own.join(", ")));
            }
            if !earlier.is_empty() {
                why.push(format!(
                    "commits that an earlier attempt of this cell made, and was sent back for, are still on the branch \
                     and change the design paths: {}; this round did not make them: revert them (git revert <commit>) \
                     before handing in",
                    earlier.join("; ")
                ));
            }
            if !loose.is_empty() {
                why.push(format!(
                    "the worktree holds changes to the design paths that are not committed, made in this round or \
                     left by an earlier attempt of this cell: {}; undo them before handing in",
                    loose.join(", ")
                ));
            }
            if why.is_empty() {
                why.push(format!("the design paths changed: {}", net.join(", ")));
            }
            fail(why.join(". "))
        }
        "measure_matches_runs" => {
            let verdict = v.and_then(|m| m["verdict"].as_str()).unwrap_or_default();
            let ran = c.ci_runs("measure")?.iter().any(|r| r["push"]["verdict"] == "ran");
            measure_problems(verdict, ran).into_result()
        }
        // The records are the design's: after it, the work changes none of its own, in any round, a round that ran
        // tools included; what main brought by a merge is main's.
        "records_unchanged_since_design" => {
            let designed = c
                .accepted("D2")?
                .first()
                .and_then(|r| r.input["engine"]["head_after"].as_str().map(str::to_string));
            let base = match designed {
                Some(h) => h,
                None => c.fork()?,
            };
            let changed = c.design(&c.own(&base, "HEAD")?);
            let loose = c.design(&git::changed_since(c.wt()?, "HEAD")?);
            let all: BTreeSet<&String> = changed.iter().chain(&loose).collect();
            if all.is_empty() {
                Ok(())
            } else {
                fail(format!(
                    "the records changed since the design ({}): a change to them goes back to derive again; undo it \
                     and hand in records_must_change",
                    all.into_iter().cloned().collect::<Vec<_>>().join(", ")
                ))
            }
        }
        "cards_match_records" => {
            let Some(tree) = v else { return Ok(()) };
            let verdict = tree["verdict"].as_str().unwrap_or_default();
            let wt = c.wt()?;
            let fork = c.fork()?;
            let changed = git::changed(wt, &fork, "HEAD")?;
            let in_records = c.design(&changed);
            if verdict != "unchanged" {
                if c.item()?.deliver == "report" {
                    return fail("a report can only answer unchanged");
                }
                if in_records.is_empty() {
                    return fail("the records did not change on the branch");
                }
                let dirty = c.design(&git::changed_since(wt, "HEAD")?);
                if !dirty.is_empty() {
                    return fail(format!(
                        "changes to the records are not committed: {}",
                        dirty.join(", ")
                    ));
                }
                if verdict == "design_paths_only" {
                    let outside: Vec<&String> = changed.iter().filter(|f| !in_records.contains(f)).collect();
                    if !outside.is_empty() {
                        return fail(format!(
                            "design_paths_only, but files outside the records changed: {outside:?}"
                        ));
                    }
                }
            } else if !in_records.is_empty() {
                return fail(format!(
                    "unchanged, but the branch changed the records: {}",
                    in_records.join(", ")
                ));
            }
            let revised = match &c.project()?.records {
                Some(r) => crate::reasoning::revised(wt, &fork, "HEAD", &r.root)?,
                None => Default::default(),
            };
            let cards = items(tree.get("cards"));
            let edits = items(tree.get("edits"));
            let withdrawn = items(at_path(&c.unwrapped, "withdrawn").as_ref());
            let solution = at_path(&c.unwrapped, "design.solution")
                .and_then(|s| s.as_str().map(str::to_string))
                .unwrap_or_default();
            let current = crate::cards::current(c.e.db.conn(), c.w.id)?;
            let earlier = crate::cards::chosen_elsewhere(c.e.db.conn(), c.item()?.id, c.w.id)?;
            let handed = crate::cards::Handed {
                cards: &cards,
                edits: &edits,
                withdrawn: &withdrawn,
                solution: &solution,
            };
            crate::cards::check(&handed, &revised, &current, &earlier).or_else(fail)
        }
        "acceptance_matches_locked" => {
            // A plan that turns the work to the normal route hands in no conditions.
            if v.is_none() {
                return Ok(());
            }
            let given = items(v);
            let report_answer = c.item()?.deliver == "change" && given.iter().any(|a| a["by"] == "report_answer");
            let locked: Option<String> =
                c.e.db
                    .conn()
                    .query_row("SELECT acceptance FROM items WHERE id = ?1", [c.item()?.id], |r| {
                        r.get(0)
                    })
                    .map_err(|e| e.to_string())?;
            let differs = work::json_col(locked).is_some_and(|l| l != Value::Array(given));
            // Whether the owner ruled since is looked up only when the conditions differ.
            let changed = differs && !c.ruled_since_locked()?;
            acceptance_problems(report_answer, changed).into_result()
        }
        "head_pushed_and_green" => {
            let wt = c.wt()?;
            let head = v.and_then(Value::as_str).unwrap_or_default();
            // The session manager says whether the head handed in is the clean worktree's.
            let sha = match sessions::worktree::handed_in(wt, head) {
                Ok(sha) => sha,
                Err(Unfit::NoCommit) => return fail(format!("there is no commit {head}")),
                Err(Unfit::NotHead(_)) => return fail("the head is not the worktree's HEAD"),
                Err(Unfit::Uncommitted) => return fail("the worktree has uncommitted changes"),
                Err(Unfit::Git(e)) => return Err(e.into()),
            };
            if c.pushed_head()?.as_deref() != Some(sha.as_str()) {
                return fail("the head was not pushed with ci");
            }
            let runs = c.ci_runs("ci")?;
            let Some(red) = red_checks_of_head(&runs, &sha) else {
                return fail("no fast CI run on the head");
            };
            if red.is_empty() {
                Ok(())
            } else {
                fail(format!(
                    "the latest fast CI on the head is not green: {}",
                    red.join(", ")
                ))
            }
        }
        "branch_changes_fit_work" => {
            let changed = c.branch_changes()?;
            let kind = values::value(c.e, c.w, c.g, c.item, "work")?;
            match kind.as_str() {
                "report" => {
                    let outside: Vec<&String> = changed.iter().filter(|f| !f.starts_with("research/")).collect();
                    if outside.is_empty() {
                        Ok(())
                    } else {
                        fail(format!("a report changes nothing outside research/: {outside:?}"))
                    }
                }
                "design_paths_only" => {
                    let paths = c.project()?.design_paths();
                    let outside: Vec<&String> = changed.iter().filter(|f| !git::under(f, &paths)).collect();
                    if outside.is_empty() {
                        Ok(())
                    } else {
                        fail(format!(
                            "a change to the design paths only changes nothing else: {outside:?}"
                        ))
                    }
                }
                _ if changed.is_empty() => fail("the branch has no change against main; if another PR fixed it, ask"),
                _ => Ok(()),
            }
        }
        // A declaration of the engine's checks a change changes names checks the flow has, each once, and only in the
        // project the engine is built from.
        "checks_changed_known" => {
            let named = set_aside::names(v);
            if named.is_empty() {
                return Ok(());
            }
            let project = &c.item()?.project;
            if !set_aside::engine_s_own(c.e, c.item) {
                return fail(format!(
                    "this project's changes cannot change the engine's checks: the engine is not built from project \
                     {project}, so what a check named here finds stays a problem; leave checks_changed out"
                ));
            }
            let flow = c.e.flow(c.w.flow_version)?;
            let known: Vec<&str> = flow.engine.checks.keys().map(String::as_str).collect();
            declared_problems(&known, &named).into_result()
        }
        "conditions_cover_acceptance" => {
            let Some(rows) = v else { return Ok(()) };
            let rows = items(Some(rows));
            let wanted: Vec<String> = c
                .acceptance()?
                .iter()
                .filter_map(|a| a["id"].as_str().map(str::to_string))
                .collect();
            let recheck = c.cell.id == "T1" && !c.accepted("T1")?.is_empty();
            let wt = c.wt()?;
            // The CI runs recorded are looked up only for a row that names one of their jobs or tests.
            let jobs = if rows.iter().any(|r| r["how"] == "ci_job") {
                c.ci_jobs()?
            } else {
                Vec::new()
            };
            let on_head = if rows.iter().any(|r| r["how"] == "tested") {
                c.head_pushes()?
            } else {
                Vec::new()
            };
            let seps = c.separators();
            let named = c.results_named();
            conditions_problems(
                &wanted,
                &rows,
                recheck,
                |t| on_head.iter().any(|run| reported(run, t, &seps).is_some()),
                |names| {
                    if named {
                        not_reported_on_head(names, on_head.first())
                    } else {
                        no_results(names)
                    }
                },
                &jobs,
                |s| text_on_branch(wt, s),
            )
            .into_result()
        }
        "new_test_results" | "guard_test_results" => {
            let new = check == "new_test_results";
            // The tests this return adds or extends, each written once in the test changes, with the two results the
            // engine reads from its runs.
            let tables = test_tables::of_return(&c.unwrapped);
            let rows = tables.table(if new { "new_tests" } else { "guard_tests" });
            if rows.is_empty() {
                return Ok(());
            }
            if !c.results_named() {
                return fail(no_results(&test_names(rows)));
            }
            let on_head = c.head_pushes()?;
            let before = c.before_runs()?;
            let seps = c.separators();
            let filled = with_results(rows, &on_head, &before, &seps);
            let mut problems = Problems::default();
            let unreported: Vec<String> = filled
                .iter()
                .filter(|t| t.get("on_head").is_none())
                .filter_map(|t| t["name"].as_str().map(str::to_string))
                .collect();
            if !unreported.is_empty() {
                problems.push(not_reported_on_head(&unreported, on_head.first()));
            }
            for t in &filled {
                let name = t["name"].as_str().unwrap_or_default();
                let excused = t["not_before"].as_str().is_some_and(|x| !x.trim().is_empty());
                let changed = run_for(&before, name, &seps)
                    .map(|run| c.changed_since(run))
                    .unwrap_or_default();
                if !changed.is_empty() {
                    problems.push(format!(
                        "{name}: its result before the change comes from a run of ci_before, and {} changed after \
                         it; {RUN_AGAIN}",
                        changed.join(", ")
                    ));
                } else if t.get("on_base").is_none() && !excused {
                    problems.push(format!(
                        "{name} has no result before the change: run it with ci_before, or say in not_before why it \
                         cannot run on the code before the change"
                    ));
                }
            }
            problems.extend(result_problems(&filled, new));
            problems.into_result()
        }
        "adversarial_test_results" => {
            let mut problems = test_results_match_runs(c, &items(v))?;
            if let Some(h) = c.unwrapped.pointer("/wrote/head").and_then(Value::as_str)
                && c.unwrapped["wrote"]["verdict"] == "wrote"
                && c.pushed_head()?.as_deref() != git::resolve(c.wt()?, h).as_deref()
            {
                problems.push("tests were written, and the head was not pushed with ci");
            }
            problems.into_result()
        }
        "test_kinds_match" => {
            let table = if at.is_some_and(|a| a.starts_with("new_tests")) {
                "new_tests"
            } else {
                "guard_tests"
            };
            let rows = items(c.unwrapped.get(table));
            // The kind each test has in the test changes of all the engineer's rounds, oldest round first, so that a
            // test written again has the kind of its later row, as the tables the engine derives have it.
            let mut returns = Vec::new();
            for cell in test_tables::CELLS {
                returns.extend(crate::inputs::returns_holding(c.e, c.w, c.g, cell, "test_changes")?);
                // A return of a flow version before the answer was flat holds the test changes under end.
                returns.extend(crate::inputs::returns_holding(c.e, c.w, c.g, cell, "end.test_changes")?);
            }
            returns.sort_by_key(|(round, _)| *round);
            let kinds = test_tables::accumulated(returns.iter().map(|(_, ret)| ret)).kinds();
            kind_problems(&rows, &kinds, table == "new_tests").into_result()
        }
        "tests_only" => {
            let Some(from) = c.w.pushed_head.clone() else {
                return Ok(());
            };
            let head = v.and_then(Value::as_str).unwrap_or("HEAD");
            let tests = &c.project()?.test_paths;
            let outside: Vec<String> = c
                .own(&from, head)?
                .into_iter()
                .filter(|f| !git::under(f, tests))
                .collect();
            if outside.is_empty() {
                Ok(())
            } else {
                fail(format!(
                    "adversarial testing touches only the test paths: {}",
                    outside.join(", ")
                ))
            }
        }
        "disputed_tests_answered" => {
            let Some(last6) = c.accepted("T1")?.first().map(|r| r.id) else {
                return Ok(());
            };
            let tests = strs(v);
            // The last review of the tests is the same for every row of every fix.
            let review = c.accepted("T1")?.into_iter().next();
            let mut disputes: Vec<(String, String)> = Vec::new();
            for fix in c.accepted("T6")?.into_iter().filter(|r| r.id > last6) {
                let table = table_of(&fix);
                for row in items(fix.output.as_ref().and_then(|o| o[&c.g.within].get("rows"))) {
                    let id = known_as(&table, row["id"].as_str().unwrap_or_default());
                    if matches!(row["answer"].as_str(), Some("test_wrong" | "advisor_ruled"))
                        && review.as_ref().is_some_and(|r| mentions(r, &id))
                    {
                        disputes.push((id, row["answer"].to_string()));
                    }
                }
            }
            disputed_problems(&disputes, &tests).into_result()
        }
        "no_insisting_after_ruling" => {
            let ruled = ruled_ids(c)?;
            let previous: Vec<Value> = if c.cell.id == "T4" {
                items(c.unwrapped.get("previous"))
            } else {
                Vec::new()
            };
            insisting_problems(&ruled, &items(v), &previous).into_result()
        }
        "fact_basis_ran" => {
            let bases: Vec<Value> = items(v)
                .iter()
                .map(|x| x.get("basis").cloned().unwrap_or_else(|| x.clone()))
                .collect();
            let recorded = |run: &str, check: &str| -> Result<bool, String> {
                let linked: i64 =
                    c.e.db
                        .conn()
                        .query_row(
                            "SELECT count(*) FROM tool_runs WHERE work = ?1 AND ?2 <> '' AND instr(result, ?2) > 0",
                            db::rusqlite::params![c.w.id, run],
                            |r| r.get(0),
                        )
                        .map_err(|e| e.to_string())?;
                // A gh command or a query has no link: it is cited by what was handed in.
                Ok(linked > 0 || c.handed_in(check)?)
            };
            unrecorded_facts(&bases, recorded)?.into_result()
        }
        "conclusions_cover_questions" => {
            let questions = c.acceptance()?.iter().filter(|a| a["by"] == "report_answer").count();
            let given = items(v).len();
            if c.item()?.deliver == "report" {
                if given < questions {
                    return fail(format!("{questions} report_answer conditions, and {given} conclusions"));
                }
            } else if given > 0 {
                return fail("only a report hands in conclusions");
            }
            Ok(())
        }
        "columns_answered" => {
            let ids = strs(v);
            let want: Vec<&str> = c.columns.iter().map(|(id, _)| id.as_str()).collect();
            columns_problems(&want, &ids).into_result()
        }
        "evidence_holds" => cited_evidence_exists(c.e.db.conn(), c.wt()?, v),
        // The re-check answers every row the latest fix answered, by its number, and no other.
        "previous_findings_answered" => {
            let Some(fix) = c.accepted("T6")?.into_iter().next() else {
                return Ok(());
            };
            let ids = |rows: Vec<Value>| -> Vec<String> {
                rows.iter()
                    .filter_map(|r| r["id"].as_str().map(str::to_string))
                    .collect()
            };
            let due = ids(items(fix.output.as_ref().and_then(|o| o[&c.g.within].get("rows"))));
            previous_problems(&due, &ids(items(v))).into_result()
        }
        // A review answers each finding set aside it was given, by its number, and no other; one it finds real is a
        // finding of its own return too.
        "set_aside_answered" => {
            let due = set_aside::due_ids(c.e, c.w, c.g, c.cell)?;
            let answers = items(v);
            let findings: Vec<String> = items(c.unwrapped.get("findings"))
                .into_iter()
                .chain(items(c.unwrapped.pointer("/brought/new")))
                .filter_map(|f| f["fid"].as_str().map(str::to_string))
                .collect();
            set_aside_problems(&due, &answers, &findings).into_result()
        }
        // An approved design has the option the owner chose for each of its decision points.
        "points_chosen" => {
            if v.and_then(|x| x["verdict"].as_str()) != Some("approved") {
                return Ok(());
            }
            let points = items(c.last("D2", "design.points")?.as_ref());
            let choices = items(v.and_then(|x| x.get("choices")));
            points_problems(&points, &choices).into_result()
        }
        // The answer whether anything must be fixed is yes exactly when one of the two passes found something.
        "must_fix_fits_findings" => {
            let first = items(c.last("T2", "findings")?.as_ref()).len();
            let found = first + items(c.unwrapped.get("findings")).len();
            let yes = v.and_then(|x| x["verdict"].as_str()) == Some("yes");
            match (yes, found) {
                (true, 0) => fail("nothing was found in either pass: nothing must be fixed"),
                (false, n) if n > 0 => fail(format!(
                    "the two passes found {n} problems: each must be fixed before the change can merge"
                )),
                _ => Ok(()),
            }
        }
        // Each option the owner chose is written in a file of decisions/ the branch adds or changes.
        "decisions_written" => {
            let chosen: Vec<String> = c
                .last("D4", "verdict")?
                .filter(|x| x["verdict"] == "approved")
                .map(|x| {
                    items(x.get("choices"))
                        .iter()
                        .filter_map(|ch| ch["point"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let rows = items(v);
            let given: Vec<String> = rows
                .iter()
                .filter_map(|r| r["point"].as_str().map(str::to_string))
                .collect();
            let changed = if rows.is_empty() {
                Vec::new()
            } else {
                c.branch_changes()?
            };
            let files: Vec<String> = rows
                .iter()
                .filter_map(|r| r["file"].as_str().map(str::to_string))
                .collect();
            let dir = c
                .project()?
                .decisions
                .to_string_lossy()
                .trim_end_matches('/')
                .to_string();
            written_problems(&chosen, &given, &files, &changed, &dir).into_result()
        }
        "table_answered" => {
            let ids: Vec<String> = items(v)
                .iter()
                .map(|r| r["id"].as_str().unwrap_or_default().to_string())
                .collect();
            // A round given a numbered table answers each of its rows by its number, and nothing else.
            let rows: Vec<&str> = c.table.iter().filter_map(|r| r["row"].as_str()).collect();
            table_problems(&ids, &rows, !c.table.is_empty()).into_result()
        }
        "no_disagreeing_after_insist" => {
            // The rows of the table the re-check insisted on: those that came from its rows it gave the verdict insist.
            let insisted: BTreeSet<String> = c
                .table
                .iter()
                .filter(|r| r["from"].as_str().is_some_and(|f| f.starts_with(INSISTED)))
                .filter_map(|r| r["row"].as_str().map(str::to_string))
                .collect();
            let rows = items(c.unwrapped.get("rows"));
            let disagreed: Vec<String> = rows
                .iter()
                .filter(|r| matches!(r["answer"].as_str(), Some("disagree" | "test_wrong")))
                .filter_map(|r| r["id"].as_str().map(str::to_string))
                .collect();
            disagreeing_problems(&insisted, &disagreed).into_result()
        }
        "not_caused_by_change" => {
            let Some(wt) = c.w.worktree.as_deref() else {
                return Ok(());
            };
            let project = c.project()?;
            let mut not_red = Vec::new();
            for r in items(c.unwrapped.get("rows"))
                .iter()
                .filter(|r| r["answer"] == "not_caused_by_change")
            {
                let id = r["id"].as_str().unwrap_or_default();
                let check = known_as(c.table, id);
                // Main's run of the check that cannot be read leaves the answer as written.
                if crate::tools::red_beside_the_change(c.e, &c.item()?.project, project, wt, &check)? == Some(false) {
                    not_red.push(format!("{id} ({check})"));
                }
            }
            if not_red.is_empty() {
                Ok(())
            } else {
                fail(format!(
                    "{} is green on main and not a known red check: this change turned it red. Fix it, or answer \
                     unrelated_flake if it is flaky",
                    not_red.join(", ")
                ))
            }
        }
        "ruling_matches_advisor" => {
            if v.is_none() || asked_the_advisor(c.e.db.conn(), c.w.id)? {
                Ok(())
            } else {
                fail("this work never asked the advisor")
            }
        }
        // A flow version stored before an engineer asked the architect what only the owner decides names this check;
        // no kind is asked of the owner alone now.
        "owner_kinds_to_owner" => Ok(()),
        // A flow version stored before this check was named so calls it `card_disagreement_redirects`.
        "design_card_disagreement_redirects" | "card_disagreement_redirects" => {
            let disagreed = c.interjections.iter().any(|i| {
                let t = i["text"].as_str().unwrap_or_default();
                t.starts_with(db::CARD_RULING) || (t.starts_with(DESIGN_CARD_RULING) && t.contains("disagree"))
            });
            if disagreed && v.is_some() {
                fail(
                    "this round's interjections choose another option on a decision card or disagree with one: hand in redirect, not answers",
                )
            } else {
                Ok(())
            }
        }
        "answers_match_speakers" => {
            let speakers: BTreeSet<String> = c
                .interjections
                .iter()
                .filter_map(|i| i["speaker"].as_str().map(str::to_string))
                .collect();
            let speakers: Vec<String> = speakers.into_iter().collect();
            speaker_problems(&speakers, &strs(v)).into_result()
        }
        "constitution_covered" => {
            let project = match c.item {
                Some(i) => i.project.clone(),
                None => crate::architect::project_of(c.w)?.to_string(),
            };
            let articles = articles(c.e, &project, c.main)?;
            constitution_problems(&articles, &items(v)).into_result()
        }
        // A plan of the work answers each decision of its item, a passage it quotes found in its own sections.
        "decisions_answered" => {
            let listed = crate::decisions::listed(c.item()?.approach.as_deref().unwrap_or_default());
            let sections = plan_sections(&c.cell.id, &c.unwrapped);
            decisions_problems(&listed, &items(v), &sections).into_result()
        }
        // A review of a plan tries each claim the plan makes against the passage of the plan's sections most likely to
        // refute it.
        "claims_answered" => {
            let Some((cell, plan, given)) = c.plan_reviewed()? else {
                return fail("there is no plan of the work to review");
            };
            let project = c.item()?.project.clone();
            let articles = articles(c.e, &project, c.main)?;
            let claims = Claims {
                sections: plan_sections(&cell, &plan),
                claimed: plan.get("articles").and_then(Value::as_array).cloned(),
                answered: plan_answers(&plan, &given),
                listed: crate::decisions::listed(c.item()?.approach.as_deref().unwrap_or_default()),
            };
            let tried = v.cloned().unwrap_or(Value::Null);
            claims
                .problems(&articles, &items(tried.get("articles")), &items(tried.get("decisions")))
                .into_result()
        }
        "claims_fit_verdict" => {
            let tried = v.cloned().unwrap_or(Value::Null);
            let verdict = c.unwrapped.pointer(REVIEW_VERDICT).and_then(Value::as_str);
            verdict_problems(&refuted(&tried), verdict).into_result()
        }
        other => crate::architect::check(c, other, at, v),
    }
}

/// What is wrong with the options chosen for the decision points of a design (`points`, each with its number and
/// options) that the review approved: a choice of no such point, a point chosen twice or left out, and an option that
/// is none of its point's, each kind said once with all its points.
fn points_problems(points: &[Value], choices: &[Value]) -> Problems {
    let want: Vec<String> = points
        .iter()
        .filter_map(|p| p["point"].as_str().map(str::to_string))
        .collect();
    let given: Vec<String> = choices
        .iter()
        .filter_map(|ch| ch["point"].as_str().map(str::to_string))
        .collect();
    let misses = id_misses(&want, &given);
    let mut problems = Problems::default();
    problems.about(
        &misses.unknown,
        |x| format!("the design has no decision point {x}"),
        |xs| format!("the design has no decision points {xs}"),
    );
    problems.about(
        &misses.twice,
        |x| format!("decision point {x} is chosen twice"),
        |xs| format!("decision points {xs} are chosen twice"),
    );
    problems.about(
        &misses.missing,
        |x| format!("no option is chosen for decision point {x}"),
        |xs| format!("no option is chosen for decision points {xs}"),
    );
    for ch in choices {
        let (Some(id), Some(option)) = (ch["point"].as_str(), ch["chosen"].as_str()) else {
            continue;
        };
        let Some(point) = points.iter().find(|p| p["point"] == id) else {
            continue;
        };
        let options = items(point.get("options"));
        let names: Vec<&str> = options.iter().filter_map(|o| o["option"].as_str()).collect();
        if !names.iter().any(|n| n.trim() == option.trim()) {
            problems.push(format!(
                "{option} is no option of decision point {id}, whose options are: {}",
                names.join("; ")
            ));
        }
    }
    problems
}

/// What is wrong with the decisions the implementation says it wrote (`given`, by point, in `files`) against the
/// points the owner chose for (`chosen`) and the files the branch changes (`changed`): a point not chosen, one written
/// twice or left out, and a file that is no file of the project's decisions (`dir`) the branch adds or changes.
fn written_problems(chosen: &[String], given: &[String], files: &[String], changed: &[String], dir: &str) -> Problems {
    let misses = id_misses(chosen, given);
    let mut problems = Problems::default();
    problems.about(
        &misses.unknown,
        |x| format!("the owner chose no option for decision point {x}"),
        |xs| format!("the owner chose no option for decision points {xs}"),
    );
    problems.about(
        &misses.twice,
        |x| format!("decision point {x} is written twice"),
        |xs| format!("decision points {xs} are written twice"),
    );
    problems.about(
        &misses.missing,
        |x| format!("the option the owner chose for decision point {x} is written in no file of decisions/"),
        |xs| format!("the options the owner chose for decision points {xs} are written in no file of decisions/"),
    );
    for f in files {
        let decision = f
            .strip_prefix(dir)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|rest| !rest.is_empty() && rest != "README.md");
        if !decision {
            problems.push(format!("{f} is no file of the project's decisions ({dir}/)"));
        } else if !changed.iter().any(|c| c == f) {
            problems.push(format!("the branch does not add or change {f}"));
        }
    }
    problems
}

/// Where a review of a design says whether a better design is found, in its return.
const REVIEW_VERDICT: &str = "/better/verdict";

/// The articles of a project's constitution, in its order: its level-two headings that start with "Article". They are
/// counted in the constitution at the commit of main the round was pinned to (`main`), the one its agent was given, so
/// that the count is of the articles the agent read.
pub(crate) fn articles(e: &Engine, project: &str, main: Option<&str>) -> Result<Vec<String>, String> {
    let Some(text) = crate::architect::constitution(e, project, main)? else {
        return Ok(Vec::new());
    };
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .filter(|h| h.trim_start().to_lowercase().starts_with("article"))
        .map(|h| h.trim().to_string())
        .collect())
}

/// Whether a row names an article: by its number, as "Article 2", or by its title.
pub(crate) fn names_article(principle: &str, article: &str) -> bool {
    let p = principle.to_lowercase();
    let a = article.to_lowercase();
    let (number, title) = a.split_once('.').unwrap_or((a.as_str(), ""));
    let number = number.trim();
    let by_number = p
        .match_indices(number)
        .any(|(i, _)| !p[i + number.len()..].chars().next().is_some_and(|c| c.is_ascii_digit()));
    let title = title.trim();
    by_number || (!title.is_empty() && p.contains(title))
}

/// What is wrong with the articles of a project's constitution a table covers (`rows`, one for each article of
/// `articles`, in its order): a table with the wrong number of rows, and nothing else, since which row is for which
/// article is told by position; otherwise every row that names another article than its own.
pub(crate) fn constitution_problems(articles: &[String], rows: &[Value]) -> Problems {
    let mut problems = Problems::default();
    if rows.len() != articles.len() {
        problems.push(format!(
            "there is one row for each article of the project's constitution, {} of them, and there are {}",
            articles.len(),
            rows.len()
        ));
        return problems;
    }
    for (i, (row, article)) in rows.iter().zip(articles).enumerate() {
        let named = row["article"].as_str().unwrap_or_default();
        if !names_article(named, article) {
            problems.push(format!(
                "row {} is for \"{article}\", in the constitution's order, and names \"{named}\"",
                i + 1
            ));
        }
    }
    problems
}

/// A text on one line, each run of white space one space: how a passage quoted from a plan is compared with it.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The sections of a plan of the work that a passage of a claim about it is quoted from, by the cell that writes the
/// plan (`cell`) and its return (`plan`), each on one line: for a design (`D2`), its solution, where it changes, what it
/// leaves out, its pitfalls and each of its acceptance conditions. Not what the plan claims of itself (why, its claims
/// of the constitution and of the item's decisions): a claim is tried against the plan, never against a claim.
fn plan_sections(cell: &str, plan: &Value) -> Vec<String> {
    let places: &[&str] = match cell {
        "D2" => &[
            "/design/solution",
            "/design/where",
            "/design/excluded",
            "/design/pitfalls",
        ],
        _ => &[],
    };
    let mut out: Vec<String> = places
        .iter()
        .filter_map(|p| plan.pointer(p).and_then(Value::as_str))
        .map(flat)
        .collect();
    out.extend(
        items(plan.get("acceptance"))
            .iter()
            .filter_map(|a| a["condition"].as_str())
            .map(flat),
    );
    out
}

/// What a plan's sections are called, in a reason.
const SECTIONS: &str = "the plan's own sections (a design's solution, where it changes, what it leaves out, its \
                        pitfalls and its acceptance conditions; not why, nor its claims)";

/// Whether a passage is found in a plan's sections as it stands, each run of white space one space.
fn quoted_in(passage: &str, sections: &[String]) -> bool {
    let p = flat(passage);
    !p.is_empty() && sections.iter().any(|s| s.contains(&p))
}

/// What a plan answered of each decision of the item, by the line of the decision as its round was given it: the
/// verdict of its row (`follows`, `departs` or `not_concerned`). A line is the key, since the numbers are the round's own
/// and a decision recorded since moves those after it.
fn plan_answers(plan: &Value, given: &[Value]) -> Vec<(String, String)> {
    items(plan.get("item_decisions"))
        .iter()
        .filter_map(|row| {
            let id = row["decision"].as_str()?.trim();
            let line = given.iter().find(|g| g["decision"] == id)?["line"].as_str()?;
            Some((line.to_string(), row["verdict"].as_str()?.to_string()))
        })
        .collect()
}

/// What is wrong with how a plan answers the decisions of its item (`rows`) against those the round was given
/// (`listed`): a number that is none of them, one answered twice, one not answered, and a passage it quotes, to follow
/// one or to depart from it, that is not found as it stands in the plan's sections (`sections`).
fn decisions_problems(listed: &[Value], rows: &[Value], sections: &[String]) -> Problems {
    let want: Vec<String> = listed
        .iter()
        .filter_map(|l| l["decision"].as_str().map(str::to_string))
        .collect();
    let given: Vec<String> = rows
        .iter()
        .map(|r| r["decision"].as_str().unwrap_or_default().trim().to_string())
        .collect();
    let misses = id_misses(&want, &given);
    let mut problems = Problems::default();
    problems.about(
        &misses.unknown,
        |x| format!("\"{x}\" is no decision of the item you were given: answer each by its number"),
        |xs| format!("{xs} are no decisions of the item you were given: answer each by its number"),
    );
    problems.about(
        &misses.twice,
        |x| format!("decision {x} is answered twice"),
        |xs| format!("decisions {xs} are answered twice"),
    );
    problems.about(
        &misses.missing,
        |x| format!("decision {x} of the item is not answered"),
        |xs| format!("decisions {xs} of the item are not answered"),
    );
    for (row, id) in rows.iter().zip(&given) {
        if !matches!(row["verdict"].as_str(), Some("follows" | "departs")) {
            continue;
        }
        let passage = row["passage"].as_str().unwrap_or_default();
        if !quoted_in(passage, sections) {
            problems.push(format!(
                "decision {id}: the passage it quotes, \"{passage}\", is not found as it stands in {SECTIONS}: quote it \
                 letter for letter"
            ));
        }
    }
    problems
}

/// What a review of a plan tries its claims against: the plan's sections, its rows of the constitution (none for a plan
/// that has no list of them), what it answered of each decision of the item by the decision's line, and the decisions
/// the review's round was given.
struct Claims {
    sections: Vec<String>,
    claimed: Option<Vec<Value>>,
    answered: Vec<(String, String)>,
    listed: Vec<Value>,
}

impl Claims {
    /// What is wrong with how a review tries the plan's claims: its rows of the articles (`articles` of the
    /// constitution, `rows` one each in its order) and of the decisions of the item (`decisions`, one each by its
    /// number), each with the passage of the plan's sections most likely to refute the claim, found there as it stands;
    /// none near only where the plan says it does not touch the article, or that the decision does not concern it.
    fn problems(&self, articles: &[String], rows: &[Value], decisions: &[Value]) -> Problems {
        let mut problems = constitution_problems(articles, rows);
        // Which article a row is for is told by its place, which a table of the wrong length does not tell.
        let placed = rows.len() == articles.len();
        for (i, row) in rows.iter().enumerate() {
            let n = i + 1;
            let named = row["article"].as_str().unwrap_or_default();
            match row["verdict"].as_str() {
                Some("none_near") => {
                    let claim = self
                        .claimed
                        .as_ref()
                        .and_then(|c| c.get(i))
                        .and_then(|c| c["verdict"].as_str());
                    if placed && self.claimed.is_some() && claim != Some("untouched") {
                        problems.push(format!(
                            "article row {n} ({named}): the plan claims it keeps it, so a passage of its sections bears \
                             on the claim: quote the one most likely to refute it, not none_near"
                        ));
                    }
                }
                _ => self.passage(&mut problems, &format!("article row {n} ({named})"), row),
            }
        }
        let want: Vec<String> = self
            .listed
            .iter()
            .filter_map(|l| l["decision"].as_str().map(str::to_string))
            .collect();
        let given: Vec<String> = decisions
            .iter()
            .map(|r| r["decision"].as_str().unwrap_or_default().trim().to_string())
            .collect();
        let misses = id_misses(&want, &given);
        problems.about(
            &misses.unknown,
            |x| format!("\"{x}\" is no decision of the item you were given: answer each by its number"),
            |xs| format!("{xs} are no decisions of the item you were given: answer each by its number"),
        );
        problems.about(
            &misses.twice,
            |x| format!("decision {x} is tried twice"),
            |xs| format!("decisions {xs} are tried twice"),
        );
        problems.about(
            &misses.missing,
            |x| format!("decision {x} of the item is not tried"),
            |xs| format!("decisions {xs} of the item are not tried"),
        );
        for (row, id) in decisions.iter().zip(&given) {
            if row["verdict"] != "none_near" {
                self.passage(&mut problems, &format!("decision {id}"), row);
                continue;
            }
            let line = self
                .listed
                .iter()
                .find(|l| l["decision"] == id.as_str())
                .and_then(|l| l["line"].as_str());
            let answer = line.and_then(|l| self.answered.iter().find(|(x, _)| x == l).map(|(_, v)| v.as_str()));
            if let Some(a) = answer.filter(|a| *a != "not_concerned") {
                problems.push(format!(
                    "decision {id}: the plan says it {a} it, so a passage of its sections bears on the claim: quote the \
                     one most likely to refute it, not none_near"
                ));
            }
        }
        problems
    }

    /// The passage a row of a review quotes, which must be found in the plan's sections as it stands.
    fn passage(&self, problems: &mut Problems, what: &str, row: &Value) {
        let passage = row["passage"].as_str().unwrap_or_default();
        if !quoted_in(passage, &self.sections) {
            problems.push(format!(
                "{what}: the passage it quotes, \"{passage}\", is not found as it stands in {SECTIONS}: quote the plan, \
                 letter for letter, not a claim about it"
            ));
        }
    }
}

/// The claims a review found refuted, each as its row says it: an article the plan contradicts, a decision the plan's
/// answer to is wrong.
fn refuted(tried: &Value) -> Vec<String> {
    let mut out: Vec<String> = items(tried.get("articles"))
        .iter()
        .filter(|r| r["verdict"] == "contradicted")
        .map(|r| format!("the claim of {}", r["article"].as_str().unwrap_or("an article")))
        .collect();
    out.extend(
        items(tried.get("decisions"))
            .iter()
            .filter(|r| r["verdict"] == "wrong")
            .map(|r| format!("the answer to decision {}", r["decision"].as_str().unwrap_or("?"))),
    );
    out
}

/// What is wrong with a review's verdict (`verdict`, the better design's) beside the claims it found refuted: a claim
/// refuted is where the plan must change, so the verdict says a better one is found.
fn verdict_problems(refuted: &[String], verdict: Option<&str>) -> Problems {
    let mut problems = Problems::default();
    if !refuted.is_empty() && verdict != Some("found") {
        problems.push(format!(
            "{} is refuted, which is where the design must change: the verdict is found, with what must change",
            refuted.join(", ")
        ));
    }
    problems
}

/// What is wrong with the kinds of the tests in a table of new tests (`new`) or of guard tests: each test the engineer
/// added under the other kind (`kinds`, each added test's name and kind), and each adversarial test whose result before
/// the change does not fit the table, every row once. A guard that cannot be built before the change fits: the review
/// judges it, as it judges one not run.
fn kind_problems(rows: &[Value], kinds: &[(String, String)], new: bool) -> Problems {
    let want = if new { "new_behaviour" } else { "old_behaviour" };
    let mut problems = Problems::default();
    for r in rows {
        let name = r["name"].as_str().unwrap_or_default();
        if r["by"] == "engineer"
            && let Some((_, k)) = kinds.iter().find(|(n, _)| n == name)
            && k != want
        {
            problems.push(format!(
                "{name} was added by the engineer as {k}; it belongs in the other table"
            ));
        }
        if r["by"] == "adversarial" {
            let base = r["on_base"].as_str().unwrap_or_default();
            let wrong = if new {
                base == "green"
            } else {
                !matches!(base, "green" | "not_run" | "cannot_run")
            };
            if wrong {
                problems.push(format!(
                    "{name}: its result before the change does not fit its kind; fix it in this round and run it again"
                ));
            }
        }
    }
    problems
}

/// What is wrong with the results a table of new or guard tests gives on its own: each row that is red on the head, and
/// each that does not fit its kind before the change (a new test that passes, a guard that does not pass), the problems
/// independent of each other and of the other rows. A guard that cannot be built before the change (`cannot_run`) fits
/// its kind when its row says why in `not_before`, which the review judges; with no reason it is said once. A result a
/// row lacks is judged by nothing here: the check of the engineer's tests says it.
fn result_problems(rows: &[Value], new: bool) -> Problems {
    let mut problems = Problems::default();
    for t in rows {
        let name = t["name"].as_str().unwrap_or_default();
        let (head, base) = (t["on_head"].as_str(), t["on_base"].as_str());
        if head == Some("red") {
            problems.push(format!("{name} is red on the head: fix it in this round"));
        }
        if new && base == Some("green") {
            problems.push(format!(
                "{name} passes before the change: it does not test new behaviour; fix it in this round"
            ));
        }
        if !new && base == Some("cannot_run") {
            if !t["not_before"].as_str().is_some_and(|x| !x.trim().is_empty()) {
                problems.push(format!(
                    "{name} cannot be built before the change: say why in not_before, and the review judges it"
                ));
            }
        } else if !new && base.is_some_and(|b| !matches!(b, "green" | "not_run")) {
            problems.push(format!(
                "{name} does not pass before the change: it does not guard old behaviour; fix it in this round"
            ));
        }
    }
    problems
}

/// The answers a fix gave that disagree (`disagreed`, each row's name) with a finding or a test the review insisted on
/// (`insisted`), each one.
fn disagreeing_problems(insisted: &BTreeSet<String>, disagreed: &[String]) -> Problems {
    let mut problems = Problems::default();
    for id in disagreed.iter().filter(|id| insisted.contains(*id)) {
        problems.push(format!("{id} was insisted on; when neither side gives way, ask"));
    }
    problems
}

/// What is wrong with the answers a return gives to those who interjected in the round (`given`, who each answers)
/// against the speakers there were: answers to someone who did not interject, and two answers to the same one, each
/// kind said once with all its speakers.
fn speaker_problems(speakers: &[String], given: &[String]) -> Problems {
    let misses = id_misses(speakers, given);
    let mut problems = Problems::default();
    problems.about(
        &misses.unknown,
        |x| format!("the {x} did not interject in this round"),
        |xs| format!("the {xs} did not interject in this round"),
    );
    problems.about(
        &misses.twice,
        |x| format!("two answers to the {x}"),
        |xs| format!("two answers to each of: {xs}"),
    );
    problems
}

/// The rows a fix answered `test_wrong` or `advisor_ruled` that the table of tests holds no row for (`disputes`, each
/// row's name and its answer as written), each one.
fn disputed_problems(disputes: &[(String, String)], tests: &[String]) -> Problems {
    let mut problems = Problems::default();
    for (id, answer) in disputes.iter().filter(|(id, _)| !tests.contains(id)) {
        problems.push(format!("no row for {id}, which the engineer answered {answer}"));
    }
    problems
}

/// What a review insists on or marks not fixed that the engineer answered `advisor_ruled` (`ruled`): each row of the
/// review's table (`insisted`) that names a ruled finding or test, and, at the table review, each finding it marks
/// `not_fixed` among the `previous` verdicts that was ruled on.
fn insisting_problems(ruled: &BTreeSet<String>, insisted: &[Value], previous: &[Value]) -> Problems {
    let mut problems = Problems::default();
    for r in insisted {
        let id = r["fid"].as_str().or(r["test"].as_str()).unwrap_or_default();
        if ruled.contains(id) {
            problems.push(format!("{id} was ruled on; it is not insisted on"));
        }
    }
    for r in previous.iter().filter(|r| r["verdict"] == "not_fixed") {
        let id = r["fid"].as_str().unwrap_or_default();
        if ruled.contains(id) {
            problems.push(format!("{id} was ruled on; it is not marked not_fixed"));
        }
    }
    problems
}

/// The facts in the bases of a review's findings (`bases`) whose run the engine did not record for the work, each one;
/// `recorded` says for a run and the check as written whether it did, or why it cannot be read.
fn unrecorded_facts(
    bases: &[Value],
    recorded: impl Fn(&str, &str) -> Result<bool, String>,
) -> Result<Problems, String> {
    let mut problems = Problems::default();
    for b in bases.iter().filter(|b| b["kind"] == "fact") {
        let run = b["run"].as_str().unwrap_or_default();
        let check = b["check"].as_str().unwrap_or_default();
        if !recorded(run, check)? {
            problems.push(format!(
                "the run {run:?} is not one the engine recorded for this work: give the link from the tool's result, or, for a gh command or a query, write it in check as you handed it in"
            ));
        }
    }
    Ok(problems)
}

/// What is wrong with the rows of a table of conditions against the acceptance conditions in force (`wanted`): when
/// there is none and rows name some, that alone, since every other miss presupposes one; otherwise the ids that are
/// no condition, and, unless the table is a recheck of a cell that ran before (`recheck`), the conditions with no row.
/// An id given twice is not reported.
fn cover_problems(wanted: &[String], ids: &[String], recheck: bool) -> Problems {
    let mut problems = Problems::default();
    if wanted.is_empty() && !ids.is_empty() {
        problems.push(NO_ACCEPTANCE);
        return problems;
    }
    let misses = id_misses(wanted, ids);
    problems.about(
        &misses.unknown,
        |x| format!("{x} is not an acceptance condition"),
        |xs| format!("{xs} are not acceptance conditions"),
    );
    if !recheck {
        problems.about(
            &misses.missing,
            |x| format!("no row for acceptance condition {x}"),
            |xs| format!("no row for acceptance conditions {xs}"),
        );
    }
    problems
}

/// What is wrong with a table of condition rows (`rows`) against the acceptance conditions in force (`wanted`): what
/// [`cover_problems`] says about their ids; the tests rows of kind tested name that no run of ci on the pushed head
/// reported (`reported` says whether one did), all in one problem (`unreported` says it); and, for a row of kind ci_job, each job that is not
/// a check of a CI run recorded for the work (`ci`; when none is recorded, that once, since any name would pass) and
/// each step that is not text in a file of the branch (`step_on_branch` says). A row of kind read_diff names nothing
/// the engine can look up: the table review reads it. With no condition in force the rows are no rows of any condition,
/// so what they name is not looked for.
fn conditions_problems(
    wanted: &[String],
    rows: &[Value],
    recheck: bool,
    reported: impl Fn(&str) -> bool,
    unreported: impl Fn(&[String]) -> String,
    ci: &[String],
    step_on_branch: impl Fn(&str) -> Result<(), String>,
) -> Problems {
    let ids: Vec<String> = rows
        .iter()
        .filter_map(|r| r["id"].as_str().map(str::to_string))
        .collect();
    let mut problems = cover_problems(wanted, &ids, recheck);
    if wanted.is_empty() && !ids.is_empty() {
        return problems;
    }
    let mut missing: Vec<String> = Vec::new();
    for r in rows.iter().filter(|r| r["how"] == "tested") {
        for t in strs(r.get("tests")) {
            if !reported(&t) && !missing.contains(&t) {
                missing.push(t);
            }
        }
    }
    if !missing.is_empty() {
        problems.push(unreported(&missing));
    }
    // A job or step several rows name is said once, as `push` keeps a problem word for word the same once.
    for r in rows.iter().filter(|r| r["how"] == "ci_job") {
        if ci.is_empty() {
            problems.push("no CI run of this work has recorded its checks, so a ci_job row cannot name a job");
        } else {
            for job in strs(r.get("jobs")) {
                if !ci.contains(&job) {
                    problems.push(format!(
                        "the job {job} is not a check of any CI run of this work; the checks recorded are: {}",
                        ci.join(", ")
                    ));
                }
            }
        }
        for step in strs(r.get("steps")) {
            if step.trim().is_empty() || step.contains('\n') {
                problems.push("a step names one command, written as one line of text");
            } else if let Err(why) = step_on_branch(&step) {
                problems.push(why);
            }
        }
    }
    problems
}

/// What a return is sent back with when rows name acceptance conditions and the item has none in force. Only the forms
/// the agent has are named: `ask` to the advisor, whose `what` says what is wanted.
const NO_ACCEPTANCE: &str = "the item has no acceptance conditions in force; use ask to the advisor for them, saying in \
                             what that the acceptance is to be set";

/// What a return is sent back with when it hands in acceptance conditions that differ from the locked ones with no
/// ruling of the owner since. The advisor's reply is the way to change them, and its `new_acceptance` replaces the whole.
const LOCKED_ACCEPTANCE: &str = "the acceptance conditions are locked and must be handed in word for word; to change \
                                 them, use ask to the advisor first, saying in what that the acceptance is to change; \
                                 its reply with new_acceptance replaces all of them";

/// What is wrong with the answer a design gives to whether its choice rests on a measurement (`verdict`) against the
/// measure runs this work has (`ran`): `measured` has runs, and any other answer, `not_measured` or `from_records`
/// (the choice rests on reading the engine's records, which needs no run), has none.
fn measure_problems(verdict: &str, ran: bool) -> Problems {
    let mut problems = Problems::default();
    match (verdict == "measured", ran) {
        (true, false) => problems.push("measured, but this work has no measure run"),
        (false, true) => problems.push(format!(
            "{verdict}, but this work has measure runs; answer measured when the choice rests on them"
        )),
        _ => {}
    }
    problems
}

/// The checks that are not green in the runs of the tool `ci` on `sha` that decide whether the head is green, from the
/// results of the tool, the newest first: the latest run on the head and, beside it, the latest that ran tests. Where
/// the CI runs on demand a run that only ran checks named, as the lint (`push.run` is `checks`, or `lint` in the
/// records of an engine before the checks were named), is made after the run that ran the tests and must not hide it,
/// as a red test run would be on the same head; and a push that ran nothing (`none`, as when the call only hands in the
/// PR's text) is no run. A run that does not say what it ran is one of the whole of the fast CI. None when no run is on
/// the head.
fn red_checks_of_head(runs: &[Value], sha: &str) -> Option<Vec<String>> {
    let mut on_head = runs
        .iter()
        .filter(|r| r["push"]["head"] == sha && r["push"]["run"] != "none");
    let latest = on_head.clone().next()?;
    let tests = on_head.find(|r| !matches!(r["push"]["run"].as_str(), Some("checks" | "lint")));
    let mut red: Vec<String> = Vec::new();
    for run in [Some(latest), tests].into_iter().flatten() {
        for check in items(run["push"].get("checks")) {
            let name = check["name"].as_str().unwrap_or_default().to_string();
            if check["result"] != "green" && !red.contains(&name) {
                red.push(name);
            }
        }
    }
    Some(red)
}

/// What is wrong with the acceptance conditions a return gives back: a work that delivers a change hands in a
/// `report_answer` condition (`report_answer`), and the conditions are locked and differ from the locked ones with no
/// ruling of the owner since (`unlocked`); the two are independent.
fn acceptance_problems(report_answer: bool, unlocked: bool) -> Problems {
    let mut problems = Problems::default();
    if report_answer {
        problems.push("a work that delivers a change has no report_answer condition");
    }
    if unlocked {
        problems.push(LOCKED_ACCEPTANCE);
    }
    problems
}

/// What is wrong with a research round's changes: the worktree holds changes that are not committed, and the round
/// changed files outside `research/` (`changed`, the files it changed); the two are independent.
fn research_problems(clean: bool, changed: &[String]) -> Problems {
    let mut problems = Problems::default();
    if !clean {
        problems.push("the worktree has uncommitted changes");
    }
    let outside: Vec<&str> = changed
        .iter()
        .map(String::as_str)
        .filter(|f| !f.starts_with("research/"))
        .collect();
    if !outside.is_empty() {
        problems.push(format!(
            "this round changed files outside research/: {}",
            outside.join(", ")
        ));
    }
    problems
}

/// What an entry of a review's evidence cites, read by the characters it begins with.
enum Cited<'a> {
    /// A round of any work, by its name `R<work>-<n>`.
    Round(RoundId),
    /// A file, by its path from the repository's root.
    File(&'a str),
    /// Anything else a review reads (a pull request's description, what an answer says): nothing to look up.
    Text,
}

/// What an entry of the evidence cites, a round or a file or text: the same for `files` and for `nodes`, since a
/// review puts what it read in either.
fn cited(entry: &str) -> Cited<'_> {
    let p = path_in(entry).trim_end_matches('/');
    // The engine's one reading of a round's name, in any project.
    if let Some(round) = RoundId::parse(p) {
        Cited::Round(round)
    } else if (p.contains('/') || p.contains('.')) && !p.starts_with('/') {
        Cited::File(p)
    } else {
        Cited::Text
    }
}

/// Whether what the evidence cites is there: a round in the database, which may be of another work, as a review
/// reads the rounds that came before; a file named by a path from the repository's root, in the worktree. A citation
/// that is neither (a pull request, a note on an answer) names nothing to look up and is not looked up. Every round
/// and file cited that is not there is a problem of its own.
fn cited_evidence_exists(conn: &db::rusqlite::Connection, wt: &Path, evidence: Option<&Value>) -> Result<(), Problems> {
    let mut problems = Problems::default();
    for ev in items(evidence) {
        for entry in strs(ev.get("files")).into_iter().chain(strs(ev.get("nodes"))) {
            match cited(&entry) {
                Cited::Round(id) => {
                    let found: i64 = conn
                        .query_row(
                            "SELECT count(*) FROM rounds WHERE work = ?1 AND n = ?2",
                            db::rusqlite::params![id.work, id.n],
                            |r| r.get(0),
                        )
                        .map_err(|e| e.to_string())?;
                    if found == 0 {
                        problems.push(format!("the round {id} does not exist"));
                    }
                }
                Cited::File(p) if !wt.join(p).exists() => {
                    problems.push(format!(
                        "{p} is not in the worktree: name a file by its path from the repository's root, or a round \
                         as R<work>-<n>"
                    ));
                }
                Cited::File(_) | Cited::Text => {}
            }
        }
    }
    problems.into_result()
}

/// The path a cited file or round begins with, before the lines or a note that may follow it, as in
/// `crates/x.rs:12-30` or `flow/flow.json (the node's description)`.
fn path_in(cited: &str) -> &str {
    cited
        .trim()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')))
        .next()
        .unwrap_or_default()
        .trim_end_matches('.')
}

/// Whether a round's return names an id.
fn mentions(r: &Round, id: &str) -> bool {
    r.output
        .as_ref()
        .is_some_and(|o| o.to_string().contains(&format!("\"{id}\"")))
}

/// Whether the work asked its advisor: a round of the advisor, whether it ruled or handed the question on, the
/// architect or the owner then ruling in its place.
fn asked_the_advisor(conn: &db::rusqlite::Connection, work: i64) -> Result<bool, String> {
    Ok(!work::rounds_at(conn, work, "A1")?.is_empty())
}

/// The ids the engineer answered advisor_ruled in the fixes since the last review.
fn ruled_ids(c: &Ctx<'_>) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    for fix in c.accepted("T6")? {
        let table = table_of(&fix);
        for row in items(fix.output.as_ref().and_then(|o| o[&c.g.within].get("rows"))) {
            if row["answer"] == "advisor_ruled" {
                out.extend(row["id"].as_str().map(|id| known_as(&table, id)));
            }
        }
    }
    Ok(out)
}

/// Whether every check of a run is green.
fn all_green(checks: Option<&Value>) -> bool {
    items(checks).iter().all(|x| x["result"] == "green")
}

/// What a recorded run of ci or ci_before (its `push` or its `ran`) reports of the test a table names (`name`): the
/// result of the tests the name names among those the run's rows list, by [`test_name::names`] with the project's
/// separators (`seps`); none when the rows list none the name names, as those of a run of an engine before the results
/// were read from JUnit list none.
fn reported(run: &Value, name: &str, seps: &[String]) -> Option<&'static str> {
    let rows = items(run.get("tests"));
    let outcomes = rows
        .iter()
        .flat_map(results::matched_of)
        .filter(|(t, _)| test_name::names(name, t, seps))
        .map(|(_, o)| o);
    results::combined(outcomes)
}

/// What a run of ci_before reports of the test a table names: what [`reported`] gives, with no reason; else, for a row
/// that lists no test and whose request named the test as the table does, its result and why: not built, not run or
/// unread. None when it reports neither.
fn reported_before(ran: &Value, name: &str, seps: &[String]) -> Option<(String, Option<String>)> {
    if let Some(result) = reported(ran, name, seps) {
        return Some((result.to_string(), None));
    }
    items(ran.get("tests")).iter().find_map(|row| {
        let result = row["result"].as_str()?;
        let same = row["test"].as_str().is_some_and(|t| t.trim() == name.trim());
        (same && matches!(result, "not_built" | "not_run" | "unread") && results::matched_of(row).is_empty())
            .then(|| (result.to_string(), row["why"].as_str().map(str::to_string)))
    })
}

/// What a return is sent back with for the tests it names that no run of ci on the pushed head reported (`names`), all
/// of them in one problem, by why none did: the latest run there (`latest`) could not read its results, which naming
/// the tests again does not mend ([`unread_on_head`]); else they were not named in a run that read them
/// ([`unreported`]).
fn not_reported_on_head(names: &[String], latest: Option<&Value>) -> String {
    match latest.and_then(unread_why) {
        Some(why) => unread_on_head(names, why),
        None => unreported(names),
    }
}

/// Why a recorded run of ci or ci_before (its `push` or its `ran`) could not read its results, as its first unread row
/// says; none when it read them, or named no test.
fn unread_why(run: &Value) -> Option<&str> {
    run["tests"]
        .as_array()?
        .iter()
        .find(|row| row["result"] == "unread")
        .map(|row| row["why"].as_str().unwrap_or("its rows say no reason"))
}

/// What a return is sent back with for the tests it names (`names`) when the latest run of ci on the pushed head could
/// not read its results (`why`).
fn unread_on_head(names: &[String], why: &str) -> String {
    format!(
        "the latest run of ci on the pushed head reports none of these, since it could not read its results ({why}): \
         {}. Run ci again naming them; if its results still cannot be read, ask the architect, since the project's \
         fast CI then does not report them where the project's configuration says",
        names.join(", ")
    )
}

/// What a return is sent back with for the tests it names (`names`) when the project's configuration names no results
/// of its fast CI, so that no run of ci or ci_before can report any of them.
fn no_results(names: &[String]) -> String {
    format!(
        "the project's configuration names no results of its fast CI (`ci.results`), so no run of ci or ci_before \
         can report a result for these: {}. Running them again does not help: ask the architect",
        names.join(", ")
    )
}

/// What a return is sent back with for the tests it names that no run of ci on the pushed head reported (`names`), all
/// of them in one problem.
fn unreported(names: &[String]) -> String {
    format!(
        "no run of ci on the pushed head reported a test that these name: {}. Name them in the tests of the next ci, \
         together with the tests of the test table; {}",
        names.join(", "),
        test_name::FORMS
    )
}

/// The tests of a table whose results do not match the runs of ci on the pushed head and the runs of ci_before.
///
/// Every test that does not match is a problem, with everything that is wrong with it.
fn test_results_match_runs(c: &Ctx<'_>, rows: &[Value]) -> Result<Problems, String> {
    if !c.results_named() {
        let mut problems = Problems::default();
        let names = test_names(rows);
        if !names.is_empty() {
            problems.push(no_results(&names));
        }
        return Ok(problems);
    }
    let on_head = c.head_pushes()?;
    let before = c.before_runs()?;
    Ok(results_vs_runs(rows, &on_head, &before, &c.separators(), |run| {
        c.changed_since(run)
    }))
}

/// What each run of ci on the work's pushed head reported (its `push`), newest first; none when nothing was pushed.
fn head_pushes_of(e: &Engine, w: &Work) -> Result<Vec<Value>, String> {
    let pushed: Option<String> =
        e.db.conn()
            .query_row("SELECT pushed_head FROM works WHERE id = ?1", [w.id], |r| r.get(0))
            .map_err(|x| x.to_string())?;
    let Some(pushed) = pushed else {
        return Ok(Vec::new());
    };
    let rows = work::query_json(
        e.db.conn(),
        "SELECT result FROM tool_runs WHERE work = ?1 AND tool = 'ci' AND result IS NOT NULL ORDER BY id DESC",
        &[&w.id],
        1000,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| r["result"].clone())
        .filter(|r| r["push"]["head"].as_str() == Some(pushed.as_str()))
        .map(|r| r["push"].clone())
        .collect())
}

/// The runs of ci_before of the work that ran, newest first, each with the files and the commit its request put on the
/// code before the change. The files and the commit are in the request the tool was asked with; a run whose request does
/// not name files has none.
fn before_runs_of(e: &Engine, w: &Work) -> Result<Vec<BeforeRun>, String> {
    let rows = work::query_json(
        e.db.conn(),
        "SELECT args, result FROM tool_runs
         WHERE work = ?1 AND tool = 'ci_before' AND result IS NOT NULL ORDER BY id DESC",
        &[&w.id],
        1000,
    )?;
    let wt = w.worktree.as_deref();
    Ok(rows
        .into_iter()
        .filter(|r| r["result"]["ran"]["verdict"] == "ran")
        .map(|r| BeforeRun {
            ran: r["result"]["ran"].clone(),
            head: r["args"]["head"]
                .as_str()
                .and_then(|h| wt.and_then(|wt| git::resolve(wt, h))),
            files: strs(r["args"].get("files")),
        })
        .collect())
}

/// The rows of a table of the engineer's tests (each with its name, read from the test changes) with the two results
/// the engine reads from the runs it recorded, in place of any a return of a flow version before wrote: on the head, the
/// latest run of ci on the pushed head that reported the test, passed green and failed red; before the change, the
/// latest run of ci_before that reported it, passed green, failed red, not built cannot_run and not run not_run. A
/// result no run gave is left out.
fn with_results(rows: &[Value], on_head: &[Value], before: &[BeforeRun], seps: &[String]) -> Vec<Value> {
    rows.iter()
        .map(|t| {
            let mut t = t.clone();
            let name = t["name"].as_str().unwrap_or_default().to_string();
            let head = on_head.iter().find_map(|run| reported(run, &name, seps));
            let base = run_for(before, &name, seps).and_then(|r| reported_before(&r.ran, &name, seps));
            let said = |r: &str| match r {
                "passed" => "green",
                "failed" => "red",
                "not_built" => "cannot_run",
                _ => "not_run",
            };
            if let Some(o) = t.as_object_mut() {
                o.remove("on_head");
                o.remove("on_base");
                if let Some(h) = head {
                    o.insert("on_head".into(), Value::String(said(h).into()));
                }
                if let Some((b, _)) = base.as_ref().filter(|(b, _)| b != "unread") {
                    o.insert("on_base".into(), Value::String(said(b).into()));
                }
            }
            t
        })
        .collect()
}

/// The engine's tables of the engineer's tests with their two results read from the runs ([`with_results`]): what the
/// input of the engineer's tests gives the review.
pub(crate) fn engineer_results(e: &Engine, w: &Work, rows: &[Value]) -> Result<Vec<Value>, String> {
    let seps = e
        .project_of(w)
        .ok()
        .and_then(|p| p.ci.results.as_ref().map(|r| r.separators.clone()))
        .unwrap_or_default();
    Ok(with_results(
        rows,
        &head_pushes_of(e, w)?,
        &before_runs_of(e, w)?,
        &seps,
    ))
}

/// What a test is told to do when any of the files the latest run of ci_before that reported it put on the code before
/// the change changed after it (the engine cannot tell which file a test is in, so a run lists only the files of the
/// tests it is for). The forms of implementing, fixing and adversarial testing say it in the same words.
const RUN_AGAIN: &str = "run ci_before again";

/// A run of ci_before that ran, with what its request put on the code before the change.
#[derive(Debug, Clone, PartialEq)]
struct BeforeRun {
    /// What the run reported: its checks, and its result for each test it was asked to run.
    ran: Value,
    /// The commit the test files were taken from, when the worktree has the commit the request names.
    head: Option<String>,
    /// The test files the request put on the code before the change; none for a run whose request did not say.
    files: Vec<String>,
}

/// The run of ci_before whose result for a test counts: the latest that reported the test ([`reported_before`]). A test
/// none of them reported (a run of an engine before the results were read from JUnit, or a name the request left out)
/// is judged by the latest run's checks alone, as it was when there was one run to look at.
fn run_for<'a>(runs: &'a [BeforeRun], name: &str, seps: &[String]) -> Option<&'a BeforeRun> {
    runs.iter()
        .find(|r| reported_before(&r.ran, name, seps).is_some())
        .or_else(|| runs.first())
}

/// The tests of a table (`rows`) whose results do not match the runs of ci on the pushed head (`on_head`, the push of
/// each, the latest first) and the run of ci_before whose result for the test counts (`before`, the runs that ran, the
/// latest first; [`run_for`]), every test and every result once; `seps` are the project's separators. A result on the
/// head is that of the latest run there that reported the test; one that none reported is to be named in the next ci,
/// and the tests no run there reported are said in one problem. A result before the change counts only while the files
/// that run put on the code before the change are as they were (`changed_since` gives those of a run that changed after
/// its commit): a test whose result comes from a run they changed after is to be run again, and is not compared with
/// that run.
fn results_vs_runs(
    rows: &[Value],
    on_head: &[Value],
    before: &[BeforeRun],
    seps: &[String],
    changed_since: impl Fn(&BeforeRun) -> Vec<String>,
) -> Problems {
    let mut problems = Problems::default();
    let mut unreported_names: Vec<String> = Vec::new();
    for t in rows {
        let name = t["name"].as_str().unwrap_or_default();
        match (
            t["on_head"].as_str(),
            on_head.iter().find_map(|run| reported(run, name, seps)),
        ) {
            (_, None) => {
                if !unreported_names.iter().any(|n| n == name) {
                    unreported_names.push(name.to_string());
                }
            }
            (Some("green"), Some(result @ ("failed" | "not_run"))) => problems.push(format!(
                "{name} is said to be green on the head, and the latest run of ci there that reported it reports it \
                 {}",
                if result == "failed" {
                    "failed"
                } else {
                    "skipped, so that nothing measured it"
                }
            )),
            (Some("red"), Some(result @ ("passed" | "not_run"))) => problems.push(format!(
                "{name} is said to be red on the head, and the latest run of ci there that reported it reports it {}",
                if result == "passed" {
                    "passed"
                } else {
                    "skipped, so that nothing measured it"
                }
            )),
            _ => {}
        }
        if let Some(run) = run_for(before, name, seps) {
            let changed = changed_since(run);
            if changed.is_empty() {
                problems.extend(base_matches_run(name, t["on_base"].as_str(), &run.ran, seps));
            } else {
                let at = run
                    .head
                    .as_deref()
                    .map_or("a commit the worktree lacks", |h| &h[..h.len().min(10)]);
                problems.push(format!(
                    "{name}: its result before the change comes from the latest run of ci_before that reported it, at \
                     {at}, and {} changed after that; {RUN_AGAIN}",
                    changed.join(", ")
                ));
            }
        }
    }
    if !unreported_names.is_empty() {
        problems.push(not_reported_on_head(&unreported_names, on_head.first()));
    }
    problems
}

/// The names of a table's tests, each once, in the table's order.
fn test_names(rows: &[Value]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in rows.iter().filter_map(|t| t["name"].as_str()) {
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names
}

/// Where what a test is said to give before the change (`said`) does not match what the run of ci_before (`ran`)
/// reports of it ([`reported_before`]): passed is green, failed red, not built cannot_run, and not run not_run or
/// cannot_run; a run whose results could not be read is to be run again. A test the run reports nothing of (a run of an
/// engine before the results were read from JUnit, or a name the request left out) is judged by the run's checks alone:
/// said red while every check there is green. A result not said is not judged here.
fn base_matches_run(name: &str, said: Option<&str>, ran: &Value, seps: &[String]) -> Problems {
    let mut problems = Problems::default();
    let Some(said) = said else {
        return problems;
    };
    let Some((result, why)) = reported_before(ran, name, seps) else {
        if said == "red" && all_green(ran.get("checks")) {
            problems.push(format!(
                "{name} is said to be red before the change, and every check there is green"
            ));
        }
        return problems;
    };
    let why = why.map(|w| format!(" ({w})")).unwrap_or_default();
    let (fits, say) = match result.as_str() {
        "passed" => (said == "green", "say green"),
        "failed" => (said == "red", "say red"),
        "not_built" => (
            said == "cannot_run",
            "say cannot_run, since what it tests cannot be built there",
        ),
        "not_run" => (
            matches!(said, "not_run" | "cannot_run"),
            "say not_run, or cannot_run when what it tests does not exist before the change",
        ),
        _ => {
            problems.push(format!(
                "{name}: the latest run of ci_before that named it could not read its results{why}; {RUN_AGAIN}, and \
                 if its results still cannot be read, ask the architect, since the project's fast CI then does not \
                 report them where the project's configuration says"
            ));
            return problems;
        }
    };
    if !fits {
        let reports = match result.as_str() {
            "not_run" => "not run, so that nothing measured it".to_string(),
            "not_built" => format!("not built{why}"),
            other => other.to_string(),
        };
        problems.push(format!(
            "{name} is said to be {said} before the change, and ci_before reports it {reports}: {say}"
        ));
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rows(v: Value) -> Vec<Value> {
        v.as_array().cloned().unwrap_or_default()
    }

    // A design answers each decision of its item by its number, once each, and a passage it quotes to follow one or to
    // depart from one is from the design's own sections, found as it stands but for runs of white space; every problem
    // is said at once. A passage of why, which is a claim about the design and none of its sections, is not found.
    #[test]
    fn a_design_answers_each_decision_of_its_item_from_its_own_sections() {
        let listed = crate::decisions::listed(
            "Decisions:\n- Decision (2026-11-03, owner): One path.\nDecision (2026-11-03, owner): Two.\n\
             Decision (2026-11-03, architect): Three.",
        );
        let plan = json!({
            "why": "It keeps one path for every work.",
            "design": { "solution": "Keep one path\nfor every work.", "where": "flow.json", "excluded": "None.", "pitfalls": "None." },
            "acceptance": [{ "by": "test", "id": "A1", "condition": "The light way is its own path." }],
        });
        let sections = plan_sections("D2", &plan);
        let fine = rows(json!([
            { "decision": "L1", "verdict": "follows", "passage": "Keep one path  for every work." },
            { "decision": "L2", "verdict": "departs", "passage": "The light way is its own path.", "instead": "Better." },
            { "decision": "L3", "verdict": "not_concerned", "why": "It is about the page." },
        ]));
        assert!(decisions_problems(&listed, &fine, &sections).is_empty());
        let wrong = rows(json!([
            { "decision": "L1", "verdict": "follows", "passage": "It keeps one path for every work." },
            { "decision": "L1", "verdict": "not_concerned", "why": "x" },
            { "decision": "L4", "verdict": "not_concerned", "why": "x" },
            { "decision": "L2", "verdict": "departs", "passage": "", "instead": "Better." },
        ]));
        let found = decisions_problems(&listed, &wrong, &sections);
        let t = found.texts();
        assert_eq!(t.len(), 5, "{t:?}");
        assert!(t[0].starts_with("\"L4\" is no decision of the item"), "{t:?}");
        assert!(t[1].starts_with("decision L1 is answered twice"), "{t:?}");
        assert!(t[2].starts_with("decision L3 of the item is not answered"), "{t:?}");
        assert!(
            t[3].starts_with("decision L1: the passage it quotes, \"It keeps one path"),
            "{t:?}"
        );
        assert!(t[4].starts_with("decision L2: the passage it quotes, \"\""), "{t:?}");
        // An item with no decision is answered by an empty list, and by nothing else.
        assert!(decisions_problems(&[], &[], &sections).is_empty());
        let t = decisions_problems(&[], &fine[..1], &sections);
        assert_eq!(t.texts().len(), 1, "{:?}", t.texts());
    }

    // The review of a design tries each claim against the passage of the design's own sections most likely to refute
    // it. W268's design claimed that a project not configured goes on as before; its solution said so of `ci`, and also
    // that `ci_before` refuses at once when no tests_input is configured. A review that leaves an article out, quotes the
    // claim itself instead of the design, answers none near for an article the design says it keeps, or finds the claim
    // contradicted and still says the present design is best, is sent back; one that quotes the passage on ci_before,
    // says the claim is contradicted and finds a better design, is taken.
    #[test]
    fn a_review_tries_each_claim_of_a_design_against_its_own_sections() {
        let articles = vec![
            "Article 1. Independent".to_string(),
            "Article 2. Information".to_string(),
        ];
        let ci = "`ci` stays as it was for a project that names no on_demand.";
        let ci_before = "`ci_before`: with no tests_input configured it refuses at once and says why (this has \
                         nothing to do with on_demand, and takes effect at once).";
        let plan = json!({
            "design": { "solution": format!("{ci}\n\n{ci_before}"), "where": "tools.rs", "excluded": "None.", "pitfalls": "None." },
            "acceptance": [{ "by": "test", "id": "A2", "condition": "ci_before refuses with no tests_input." }],
            "articles": [
                { "article": "Article 1", "verdict": "keeps", "claim": "A project not configured goes on as before." },
                { "article": "Article 2", "verdict": "untouched", "why": "It changes no input." },
            ],
            "item_decisions": [],
        });
        let claims = Claims {
            sections: plan_sections("D2", &plan),
            claimed: plan["articles"].as_array().cloned(),
            answered: plan_answers(&plan, &[]),
            listed: Vec::new(),
        };
        let left_out = rows(json!([{ "article": "Article 1", "verdict": "holds", "passage": ci, "why": "x" }]));
        let p = claims.problems(&articles, &left_out, &[]);
        assert_eq!(p.texts().len(), 1, "{:?}", p.texts());
        assert!(p.texts()[0].contains("one row for each article"), "{:?}", p.texts());
        let claim_quoted = rows(json!([
            { "article": "Article 1", "verdict": "holds", "passage": "A project not configured goes on as before.", "why": "It says so." },
            { "article": "Article 2", "verdict": "none_near", "why": "None." },
        ]));
        let p = claims.problems(&articles, &claim_quoted, &[]);
        assert_eq!(p.texts().len(), 1, "{:?}", p.texts());
        assert!(
            p.texts()[0].starts_with("article row 1 (Article 1): the passage it quotes"),
            "{:?}",
            p.texts()
        );
        let near = rows(json!([
            { "article": "Article 1", "verdict": "none_near", "why": "None." },
            { "article": "Article 2", "verdict": "none_near", "why": "None." },
        ]));
        let p = claims.problems(&articles, &near, &[]);
        assert_eq!(p.texts().len(), 1, "{:?}", p.texts());
        assert!(
            p.texts()[0].starts_with("article row 1 (Article 1): the plan claims it keeps it"),
            "{:?}",
            p.texts()
        );
        let contradicted = json!({
            "articles": [
                { "article": "Article 1", "verdict": "contradicted", "passage": ci_before, "why": "Unconfigured projects are refused." },
                { "article": "Article 2", "verdict": "none_near", "why": "None." },
            ],
            "decisions": [],
        });
        assert!(
            claims
                .problems(&articles, &rows(contradicted["articles"].clone()), &[])
                .is_empty()
        );
        let p = verdict_problems(&refuted(&contradicted), Some("present_best"));
        assert_eq!(p.texts().len(), 1, "{:?}", p.texts());
        assert!(
            p.texts()[0].starts_with("the claim of Article 1 is refuted"),
            "{:?}",
            p.texts()
        );
        assert!(verdict_problems(&refuted(&contradicted), Some("found")).is_empty());
        assert!(verdict_problems(&[], Some("present_best")).is_empty());
    }

    // A review tries the design's answer to each decision of the item by the decision's line, not its number: a
    // decision recorded between the design and its review moves the numbers, and the design's answer stays with its
    // line. None near is for a decision the design said does not concern it, or one it never answered; every decision
    // the review's round was given is tried, once.
    #[test]
    fn a_review_reads_the_design_s_answers_by_the_line_of_each_decision() {
        let (a, b) = ("Decision (2026-11-01, owner): A.", "Decision (2026-11-02, owner): B.");
        let plan = json!({
            "design": { "solution": "Do A.", "where": "x", "excluded": "x", "pitfalls": "x" },
            "acceptance": [],
            "item_decisions": [{ "decision": "L1", "verdict": "follows", "passage": "Do A." }],
        });
        let given = crate::decisions::listed(&format!("Decisions:\n{a}"));
        let claims = Claims {
            sections: plan_sections("D2", &plan),
            claimed: Some(Vec::new()),
            answered: plan_answers(&plan, &given),
            listed: crate::decisions::listed(&format!("Decisions:\n{b}\n{a}")),
        };
        let tried = |l1: &str, l2: &str| {
            rows(json!([
                { "decision": "L1", "verdict": l1, "passage": "Do A.", "why": "x" },
                { "decision": "L2", "verdict": l2, "passage": "Do A.", "why": "x" },
            ]))
        };
        let p = claims.problems(&[], &[], &tried("none_near", "none_near"));
        assert_eq!(
            p.texts(),
            [
                "decision L2: the plan says it follows it, so a passage of its sections bears on the claim: quote the \
              one most likely to refute it, not none_near"
            ]
        );
        assert!(claims.problems(&[], &[], &tried("none_near", "holds")).is_empty());
        let p = claims.problems(&[], &[], &tried("holds", "holds")[1..]);
        assert_eq!(p.texts(), ["decision L1 of the item is not tried"]);
    }

    // A design handed in before the design answered in rows (item #247), which a work under way carries, wrote its
    // articles in a text and answered no decision: its review finds no passage near an article or a decision without
    // being sent back, since the design claims nothing of either that a passage bears on; a passage it quotes is still
    // one of the design's own sections, and every article and decision is still tried.
    #[test]
    fn a_review_of_a_design_from_before_its_rows_may_find_no_passage_near() {
        let articles = vec!["Article 1. Independent".to_string()];
        let plan = json!({
            "design": { "solution": "Do A.", "where": "x", "excluded": "x", "pitfalls": "x" },
            "acceptance": [],
            "articles": "It keeps Article 1: it names no project.",
        });
        let claims = Claims {
            sections: plan_sections("D2", &plan),
            claimed: plan["articles"].as_array().cloned(),
            answered: plan_answers(&plan, &[]),
            listed: crate::decisions::listed("Decisions:\nDecision (2026-11-01, owner): A."),
        };
        assert_eq!((claims.claimed.is_none(), claims.answered.is_empty()), (true, true));
        let near = rows(json!([{ "article": "Article 1", "verdict": "none_near", "why": "None." }]));
        let decided = rows(json!([{ "decision": "L1", "verdict": "none_near", "why": "None." }]));
        assert!(claims.problems(&articles, &near, &decided).is_empty());
        let quoted = rows(json!([{ "decision": "L1", "verdict": "holds", "passage": "Do B.", "why": "x" }]));
        let p = claims.problems(&articles, &near, &quoted);
        assert_eq!(p.texts().len(), 1, "{:?}", p.texts());
        assert!(
            p.texts()[0].starts_with("decision L1: the passage it quotes"),
            "{:?}",
            p.texts()
        );
        let p = claims.problems(&articles, &[], &[]);
        assert_eq!(p.texts().len(), 2, "{:?}", p.texts());
    }

    fn problem(check: &str, field: Option<&str>, text: &str) -> Problem {
        Problem {
            check: check.to_string(),
            field: field.map(str::to_string),
            text: text.to_string(),
        }
    }

    // The text a return is sent back with is a line for each problem, `check (field): text`, the return standing for a
    // problem of no field, joined by a new line in the order given; the record keeps that text under its key and then
    // each problem as `{check, field, text}`, a problem of no field with null, and the same text and problems are in
    // the record whatever the key. The lines are written here by the form the owner and the agent have always read.
    #[test]
    fn the_text_a_return_is_sent_back_with_and_the_record_of_its_problems_say_the_same() {
        let problems = [
            problem(
                "evidence_holds",
                Some("columns[pass].evidence"),
                "a.rs is not in the worktree",
            ),
            problem("columns_answered", Some("columns.id"), "column 2 is not answered"),
            problem("no_field", None, "it is wrong"),
        ];
        let text = "evidence_holds (columns[pass].evidence): a.rs is not in the worktree\n\
                    columns_answered (columns.id): column 2 is not answered\n\
                    no_field (the return): it is wrong";
        assert_eq!(reason(&problems), text);
        assert_eq!(reason(&problems[..1]), text.lines().next().unwrap());
        assert_eq!(reason(&[]), "");
        for key in ["check", "format"] {
            let record = record(key, text.to_string(), &problems);
            // The text first, then the problems: text readers and the pause quote the column in that order.
            assert_eq!(
                record.as_object().unwrap().keys().collect::<Vec<_>>(),
                [key, "problems"]
            );
            assert_eq!(record[key], text);
            assert_eq!(
                record["problems"],
                json!([
                    { "check": "evidence_holds", "field": "columns[pass].evidence", "text": "a.rs is not in the worktree" },
                    { "check": "columns_answered", "field": "columns.id", "text": "column 2 is not answered" },
                    { "check": "no_field", "field": null, "text": "it is wrong" },
                ])
            );
            assert_eq!(db::sent_back_text(&record), Some(text));
        }
    }

    // A place of a return that does not fit its cell's schema is a problem of the check `format`, with the path the
    // place is about, or none for the return as a whole and for a refusal about no field, as the record keeps it.
    #[test]
    fn a_place_that_does_not_fit_is_a_problem_of_the_check_format() {
        let at = flow::Misfit {
            field: Some("columns[].evidence".to_string()),
            text: "columns[3] has no evidence".to_string(),
        };
        let whole = flow::Misfit {
            field: None,
            text: "no row of D2 takes the return".to_string(),
        };
        assert_eq!(
            Problem::format(&at),
            problem("format", Some("columns[].evidence"), "columns[3] has no evidence")
        );
        assert_eq!(
            Problem::format(&whole),
            problem("format", None, "no row of D2 takes the return")
        );
        let record = record("format", whole.text.clone(), &[Problem::format(&whole)]);
        assert_eq!(
            record,
            json!({
                "format": "no row of D2 takes the return",
                "problems": [{ "check": "format", "field": null, "text": "no row of D2 takes the return" }]
            })
        );
    }

    /// A result of the tool `ci`: the head it pushed, what it ran and its checks.
    fn pushed(head: &str, run: Option<&str>, checks: &[(&str, &str)]) -> Value {
        let checks: Vec<Value> = checks.iter().map(|(n, r)| json!({ "name": n, "result": r })).collect();
        let mut push = json!({ "verdict": "pushed", "head": head, "checks": checks });
        if let Some(run) = run {
            push["run"] = json!(run);
        }
        json!({ "push": push })
    }

    // The head is green only if the latest run that ran tests is: a run of a check named alone, as the lint, made later,
    // does not stand for them, nor does a push that ran nothing, made to hand in the PR's text, which is no run at all.
    // Runs of another head, and a run of a check that is itself red, are as ever; the records of an engine that ran the
    // lint beside the tests (`named_and_lint`, `lint`) are read alike.
    #[test]
    fn a_run_of_a_check_alone_or_of_nothing_does_not_hide_a_red_test_run_on_the_same_head() {
        for (tests, alone) in [("named", "checks"), ("named_and_lint", "lint")] {
            let red_tests = pushed("h", Some(tests), &[("test", "red"), ("fast", "red")]);
            let lint_green = pushed("h", Some(alone), &[("lint", "green"), ("fast", "green")]);
            let nothing = pushed("h", Some("none"), &[]);
            // The newest run is first.
            let runs = [nothing.clone(), lint_green.clone(), red_tests.clone()];
            assert_eq!(
                red_checks_of_head(&runs, "h"),
                Some(vec!["test".to_string(), "fast".to_string()]),
                "{tests}"
            );
            // A later run that ran the tests and passed covers the red one.
            let green_tests = pushed("h", Some(tests), &[("test", "green"), ("fast", "green")]);
            let runs = [lint_green.clone(), green_tests.clone(), red_tests.clone()];
            assert_eq!(red_checks_of_head(&runs, "h"), Some(vec![]), "{tests}");
            // A head that only a check named has run is green on it; red on it, it is not.
            assert_eq!(red_checks_of_head(std::slice::from_ref(&lint_green), "h"), Some(vec![]));
            let lint_red = pushed("h", Some(alone), &[("lint", "red")]);
            let runs = [nothing.clone(), lint_red, green_tests.clone()];
            assert_eq!(
                red_checks_of_head(&runs, "h"),
                Some(vec!["lint".to_string()]),
                "{tests}"
            );
            // A head that only pushes of nothing have run has no run.
            assert_eq!(red_checks_of_head(&[nothing.clone(), nothing], "h"), None);
            // A run that does not say what it ran is the whole of the fast CI; the red run of another head is not the
            // head's.
            let old = pushed("h", None, &[("test 1 of 3", "red")]);
            assert_eq!(
                red_checks_of_head(&[lint_green.clone(), old], "h"),
                Some(vec!["test 1 of 3".to_string()])
            );
            let elsewhere = pushed("g", Some(tests), &[("fast", "red")]);
            assert_eq!(red_checks_of_head(&[lint_green, elsewhere], "h"), Some(vec![]));
        }
        assert_eq!(red_checks_of_head(&[], "h"), None);
    }

    /// The separators of a project whose tests are named by paths in the code and chains of titles.
    fn seps() -> Vec<String> {
        vec!["::".to_string(), " > ".to_string()]
    }

    /// A result row of ci or ci_before for the test named `test`, which names the tests `matched`, each its unit, its
    /// name and what became of it.
    fn row(test: &str, result: &str, matched: &[(&str, &str, &str)]) -> Value {
        let matched: Vec<Value> = matched
            .iter()
            .map(|(unit, name, result)| json!({ "unit": unit, "name": name, "result": result }))
            .collect();
        json!({ "test": test, "result": result, "matched": matched })
    }

    // A test ci_before did not reach, as when its job stopped at an earlier red test, is said not run, never red or
    // green, and one it ran is not said not run: in the sixth trial a work had two such tests and no true value for
    // them. A test the run reports nothing of is checked by the run's checks alone.
    #[test]
    fn a_result_before_the_change_matches_what_ci_before_reports_of_the_test() {
        let ran = json!({
            "checks": [{ "name": "test", "result": "red" }],
            "tests": [
                row("first", "failed", &[("e", "first", "failed")]),
                row("later", "not_run", &[]),
                row("skipped", "not_run", &[("e", "m::skipped", "skipped")]),
                row("both", "passed", &[("e", "m::both", "passed")]),
            ]
        });
        let said = |name: &str, said: &str| base_matches_run(name, Some(said), &ran, &seps()).into_result();
        assert!(said("first", "red").is_ok());
        assert!(said("e first", "red").is_ok());
        assert!(said("first", "not_run").unwrap_err().texts()[0].contains("reports it failed: say red"));
        assert!(said("first", "green").unwrap_err().texts()[0].contains("reports it failed: say red"));
        assert!(said("both", "green").is_ok());
        assert!(said("m::both", "red").unwrap_err().texts()[0].contains("reports it passed: say green"));
        for later in ["later", "skipped", "m::skipped"] {
            assert!(said(later, "not_run").is_ok(), "{later}");
            assert!(said(later, "cannot_run").is_ok(), "{later}");
            for wrong in ["red", "green"] {
                let e = said(later, wrong).unwrap_err();
                assert!(e.texts()[0].contains("reports it not run"), "{later}: {e:?}");
            }
        }
        // A name the run reports nothing of is judged by the checks alone, which are red: nothing is wrong.
        assert!(said("unnamed", "not_run").is_ok());
        assert!(said("unnamed", "red").is_ok());
        let green = json!({ "checks": [{ "name": "test", "result": "green" }] });
        assert!(
            base_matches_run("old", Some("red"), &green, &seps())
                .into_result()
                .is_err()
        );
        assert!(
            base_matches_run("old", Some("not_run"), &green, &seps())
                .into_result()
                .is_ok()
        );
        // A row of an engine before the results were read from JUnit lists no test: it reports nothing.
        let old = json!({ "checks": [{ "name": "test", "result": "green" }], "tests": [{ "test": "a", "result": "failed" }] });
        assert!(
            base_matches_run("a", Some("green"), &old, &seps())
                .into_result()
                .is_ok()
        );
    }

    // Item #11: a test whose unit was not built before the change cannot run there, and is said so; green or red is
    // sent back, saying which unit was not built and why. Results that could not be read are to be read again, by
    // running ci_before again, whatever the test is said to be.
    #[test]
    fn a_test_whose_unit_was_not_built_before_the_change_is_said_unable_to_run() {
        let why = "no test the run reported is named so, and these units were not built: db::rounds: error[E0599]";
        let ran = json!({
            "checks": [{ "name": "test", "result": "red" }],
            "tests": [
                row("built", "passed", &[("engine::e2e", "built", "passed")]),
                { "test": "new_api", "result": "not_built", "matched": [], "why": why },
                { "test": "unread", "result": "unread", "why": "the run uploaded no artifact test-results" },
            ]
        });
        let said = |name: &str, said: &str| base_matches_run(name, Some(said), &ran, &seps()).into_result();
        assert!(said("built", "green").is_ok());
        assert!(said("new_api", "cannot_run").is_ok());
        for wrong in ["green", "red", "not_run"] {
            let e = said("new_api", wrong).unwrap_err();
            assert_eq!(
                e.texts(),
                [format!(
                    "new_api is said to be {wrong} before the change, and ci_before reports it not built ({why}): say \
                     cannot_run, since what it tests cannot be built there"
                )]
            );
        }
        for any in ["green", "red", "not_run", "cannot_run"] {
            let e = said("unread", any).unwrap_err();
            assert!(e.texts()[0].contains("could not read its results"), "{e:?}");
            assert!(e.texts()[0].contains(RUN_AGAIN), "{e:?}");
            assert!(e.texts()[0].contains("ask the architect"), "{e:?}");
        }
        // The not-built row is tied to the table by the very name it was asked under; another name is not it.
        assert!(said("db::rounds new_api", "red").is_ok());
    }

    // A ruling counts once the work asked its advisor, also when the advisor handed the question on and the architect
    // ruled in its place; a work that never asked the advisor has none.
    #[test]
    fn a_ruling_counts_once_the_advisor_was_asked_whoever_ruled() {
        let mut base = db::Db::in_memory().unwrap();
        let work = base
            .write(db::Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', 'l')",
                    [],
                )?;
                let item = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'T6')",
                    [item],
                )?;
                Ok(tx.last_insert_rowid())
            })
            .unwrap();
        assert!(!asked_the_advisor(base.conn(), work).unwrap());
        let handed_on = json!({ "ask": { "to": "architect", "points": [] } }).to_string();
        base.write(db::Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, 1, 'A1', '{}', ?2, 'to_architect', 'c', 1, 2)",
                db::rusqlite::params![work, handed_on],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(asked_the_advisor(base.conn(), work).unwrap());
    }

    // A cited file or round is its path or name, whatever follows it: lines, or a note, as in the sixth trial a review
    // cited a file with `(read through the index entry …)` after it, which the check took whole and refused.
    #[test]
    fn a_cited_path_is_read_before_what_follows_it() {
        assert_eq!(path_in("crates/x.rs:12-30"), "crates/x.rs");
        assert_eq!(
            path_in("crates/engine/src/checks.rs (read through the index entry …)"),
            "crates/engine/src/checks.rs"
        );
        assert_eq!(path_in(" flow/flow.json."), "flow/flow.json");
        assert_eq!(path_in("R234-12 measure: not_measured"), "R234-12");
        assert_eq!(path_in("R227-10·D2·measure"), "R227-10");
        assert_eq!(path_in("(nothing)"), "");
    }

    // An entry of the evidence is a round when it begins with a round's name, a file when it begins with a path, and
    // otherwise text: reviews wrote `R234-12 measure: not_measured` and `PR 17 description (read through gh)` in
    // `nodes`, which the check took for files and refused.
    #[test]
    fn an_entry_of_the_evidence_is_a_round_a_file_or_text() {
        let round = |e: &str| match cited(e) {
            Cited::Round(id) => Some(id.to_string()),
            _ => None,
        };
        let file = |e: &str| match cited(e) {
            Cited::File(p) => Some(p.to_string()),
            _ => None,
        };
        for (entry, named) in [
            ("R234-12", "R234-12"),
            ("R234-12 measure: not_measured", "R234-12"),
            ("R227-10·D2·measure", "R227-10"),
            ("R5-9.", "R5-9"),
            (" R5-9 (the round)", "R5-9"),
        ] {
            assert_eq!(round(entry).as_deref(), Some(named), "{entry}");
        }
        for (entry, path) in [
            ("crates/x.rs:12-30", "crates/x.rs"),
            ("crates/engine/", "crates/engine"),
            ("flow/flow.json (the node's description)", "flow/flow.json"),
            ("README.md", "README.md"),
            ("R5-9/notes.md", "R5-9/notes.md"),
        ] {
            assert_eq!(file(entry).as_deref(), Some(path), "{entry}");
        }
        for entry in [
            "PR 17 description (read through gh)",
            "the answer to D5 says not_measured",
            "R5",
            "R5-",
            "R-9",
            "Rx-9",
            "/etc/passwd",
            "",
        ] {
            assert!(matches!(cited(entry), Cited::Text), "{entry:?}");
        }
    }

    /// The evidence check on what a review put in `files` and in `nodes`, with a database whose work 1 has rounds 1
    /// and 12, and a worktree with `crates/x.rs` in it.
    fn evidence_checked(files: &[&str], nodes: &[&str]) -> Result<(), Vec<String>> {
        let mut base = db::Db::in_memory().unwrap();
        let work = base
            .write(db::Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', 'l')",
                    [],
                )?;
                let item = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'T4')",
                    [item],
                )?;
                let work = tx.last_insert_rowid();
                for n in [1, 12] {
                    tx.execute(
                        "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, 'T4', '{}', 'c', 1)",
                        db::rusqlite::params![work, n],
                    )?;
                }
                Ok(work)
            })
            .unwrap();
        assert_eq!(work, 1, "the first work of a new database");
        // Each call has a directory of its own: the tests run side by side.
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let wt = std::env::temp_dir().join(format!("wf-agents-evidence-{}-{call}", std::process::id()));
        std::fs::create_dir_all(wt.join("crates")).unwrap();
        std::fs::write(wt.join("crates/x.rs"), "fn x() {}\n").unwrap();
        let checked = cited_evidence_exists(base.conn(), &wt, Some(&json!([{ "files": files, "nodes": nodes }])));
        let _ = std::fs::remove_dir_all(&wt);
        checked.map_err(|found| found.texts().to_vec())
    }

    // A round cited in `files` or in `nodes`, of this work or another, must be in the database; the refusal names it.
    #[test]
    fn a_round_cited_in_the_evidence_must_exist() {
        assert!(evidence_checked(&[], &["R1-12 measure: not_measured", "R1-1·D2·measure"]).is_ok());
        assert!(evidence_checked(&["R1-12"], &[]).is_ok());
        // A number of the work that has no round, in each field, and a round of a work the database does not have.
        assert_eq!(
            evidence_checked(&[], &["R1-2 measure: not_measured"]).unwrap_err(),
            ["the round R1-2 does not exist"]
        );
        assert_eq!(
            evidence_checked(&["R1-3"], &[]).unwrap_err(),
            ["the round R1-3 does not exist"]
        );
        assert_eq!(
            evidence_checked(&[], &["R9999-1"]).unwrap_err(),
            ["the round R9999-1 does not exist"]
        );
    }

    // Every citation that is not there is said at once, in `files` and in `nodes`, rounds and files together, each
    // once: a review that cites three missing things is not sent back three times (item #216).
    #[test]
    fn every_citation_in_the_evidence_that_is_not_there_is_said_at_once() {
        let found = evidence_checked(
            &["R1-3", "crates/nope.rs:4", "crates/x.rs"],
            &["R1-2", "R9999-1", "crates/nope.rs (a note)", "R1-12"],
        )
        .unwrap_err();
        for round in ["R1-3", "R1-2", "R9999-1"] {
            assert!(
                found.contains(&format!("the round {round} does not exist")),
                "{found:?}"
            );
        }
        // The same file cited twice is one problem, and what is there is none.
        let files = found
            .iter()
            .filter(|f| f.starts_with("crates/nope.rs is not in the worktree"))
            .count();
        assert_eq!(files, 1, "{found:?}");
        assert_eq!(found.len(), 4, "{found:?}");
    }

    // A path-shaped entry in `files` or in `nodes` must be a file in the worktree, and the refusal says how to name
    // one, without the directory the design tree had; text is not looked up.
    #[test]
    fn a_file_cited_in_the_evidence_must_exist_and_text_is_not_looked_up() {
        assert!(evidence_checked(&["crates/x.rs:1-3", "crates/x.rs (a note)"], &["crates/x.rs"]).is_ok());
        assert!(
            evidence_checked(
                &[],
                &["PR 17 description (read through gh)", "the answer says not_measured"]
            )
            .is_ok()
        );
        for (files, nodes) in [
            (&["crates/nope.rs"][..], &[][..]),
            (&[][..], &["crates/nope.rs (a note)"][..]),
        ] {
            let why = evidence_checked(files, nodes).unwrap_err().join("\n");
            assert!(
                why.starts_with("crates/nope.rs is not in the worktree: name a file by its path"),
                "{why}"
            );
            assert!(why.contains("R<work>-<n>") && !why.contains("design/"), "{why}");
        }
    }

    // An entry that does not begin with a round's name is a file exactly when the rule before this change said so
    // (a path with a `/` or a `.` in it that does not start at the root), whatever follows it; one that begins with a
    // round's name is that round, with a mark or a field or a note after it.
    #[test]
    fn an_entry_is_a_file_as_before_unless_it_begins_with_a_rounds_name() {
        let before = |entry: &str| {
            let p = path_in(entry).trim_end_matches('/');
            (p.contains('/') || p.contains('.')) && !p.starts_with('/')
        };
        let (mut files, mut texts) = (0, 0);
        for stem in [
            "crates/x.rs",
            "README.md",
            "4.357s",
            "D5/D6",
            "PR",
            "design",
            "a b",
            "/abs/x.rs",
            "../up.rs",
            "",
            "R5",
            "R5-9x",
            "R5-9.md",
            "R5-9-2",
            "Rx-9",
        ] {
            for after in [
                "",
                ":12-30",
                " (note)",
                "\u{b7}D2\u{b7}measure",
                ".",
                "/",
                ", more",
                ")",
            ] {
                let entry = format!("{stem}{after}");
                match cited(&entry) {
                    Cited::File(_) => {
                        assert!(before(&entry), "{entry:?} was not a file before");
                        files += 1;
                    }
                    Cited::Text => {
                        assert!(!before(&entry), "{entry:?} was a file before");
                        texts += 1;
                    }
                    Cited::Round(id) => panic!("{entry:?} read as the round {id}"),
                }
            }
        }
        assert!(files > 10 && texts > 10, "{files} files, {texts} texts");
        for (entry, round) in [
            ("R5-9,", "R5-9"),
            ("R5-9)", "R5-9"),
            ("R5-9;", "R5-9"),
            ("R5-9:", "R5-9"),
            ("R5-9/", "R5-9"),
            ("R5-9\u{b7}I1\u{b7}test_changes[added]", "R5-9"),
            ("R246-18\u{b7}D11\u{b7}adversarial[needed].why", "R246-18"),
            ("R5-9 and R5-10", "R5-9"),
        ] {
            assert!(
                matches!(cited(entry), Cited::Round(id) if id.to_string() == round),
                "{entry:?}"
            );
        }
    }

    fn ids(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    /// The problems of a check as plain texts.
    fn said(p: Problems) -> Vec<String> {
        p.texts().to_vec()
    }

    // Item #217, R231-21: a design that decided by reading the engine's records had no answer of its own, and `measured`
    // is refused without a run of measure. `from_records` is taken with or without runs of query and never with a run
    // of measure; `measured` keeps needing one; the answer a refusal names is the one given.
    #[test]
    fn a_design_that_read_the_records_is_not_refused_for_having_no_measure_run() {
        assert!(measure_problems("from_records", false).is_empty());
        assert!(measure_problems("not_measured", false).is_empty());
        assert!(measure_problems("measured", true).is_empty());
        assert_eq!(
            said(measure_problems("measured", false)),
            ["measured, but this work has no measure run"]
        );
        for other in ["not_measured", "from_records"] {
            let texts = said(measure_problems(other, true));
            assert_eq!(texts.len(), 1);
            assert!(
                texts[0].starts_with(&format!("{other}, but this work has measure runs")),
                "{texts:?}"
            );
        }
    }

    // Item #217, R232-45: the design may hand in other acceptance than the locked one only after the owner, once it was
    // locked, disagreed with a decision of the design or chose another option. A confirmation, a ruling before the lock
    // and a ruling by someone else are not such a ruling.
    #[test]
    fn only_the_owner_disagreeing_after_the_lock_lets_the_design_change_the_acceptance() {
        let mut d = db::Db::in_memory().unwrap();
        d.write(db::Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 't', 'x', 'y', 'change', 'l')",
                [],
            )?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                 VALUES (1, 1, 'D2', '{}', 'c', 0, 100)",
                [],
            )?;
            tx.execute(
                "UPDATE items SET acceptance = '[]', acceptance_work = 1, acceptance_round = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        // The owner confirmed, and disagreed before the lock, and the architect disagreed after it.
        rule(&mut d, "owner", "confirm", 200);
        rule(&mut d, "owner", "disagree", 50);
        rule(&mut d, "architect", "disagree", 300);
        assert!(!ruled_since_locked(d.conn(), 1).unwrap());
        // The owner chose another option after the lock.
        rule(&mut d, "owner", "choose", 200);
        assert!(ruled_since_locked(d.conn(), 1).unwrap());
    }

    /// Adds a decision card of the item's work with one ruling.
    fn rule(d: &mut db::Db, by: &str, verdict: &str, at: i64) {
        let rulings = json!([{ "by": by, "verdict": verdict, "at": at }]).to_string();
        d.write(db::Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO design_cards (work, round, key, body, status, rulings)
                 VALUES (1, 1, ?1, '{}', 'ruled', ?2)",
                db::rusqlite::params![format!("{by}-{verdict}-{at}"), rulings],
            )?;
            Ok(())
        })
        .unwrap();
    }

    // Item #217, R232-45: the retry texts asked for a kind, `acceptance_change`, that no form of any cell has. They name
    // only what the agent can write: `ask` to the advisor, and what the advisor's reply does.
    #[test]
    fn the_retry_texts_about_the_acceptance_name_only_forms_the_agent_has() {
        for text in [NO_ACCEPTANCE, LOCKED_ACCEPTANCE] {
            assert!(!text.contains("acceptance_change"), "{text}");
            assert!(text.contains("use ask to the advisor"), "{text}");
        }
        assert!(LOCKED_ACCEPTANCE.contains("new_acceptance replaces all of them"));
    }

    // Item #216: a return sent back said only the first of its misses, so that an agent fixed one and met the next in
    // another round (R246-46 said column 4, R246-47 column 5). The three kinds of miss of a list of ids are found
    // together, and none hides another.
    #[test]
    fn the_ids_a_return_gave_are_compared_with_the_listed_ones_in_all_three_ways_at_once() {
        let m = id_misses(&["1", "2", "3", "4", "5"], &ids(&["9", "2", "2", "8", "9", "3"]));
        assert_eq!(m.unknown, ids(&["9", "8"]));
        assert_eq!(m.twice, ids(&["9", "2"]));
        assert_eq!(m.missing, ids(&["1", "4", "5"]));
        let none = id_misses(&["1", "2"], &ids(&["2", "1"]));
        assert_eq!((none.unknown, none.twice, none.missing), (vec![], vec![], vec![]));
        // Nothing given: everything listed is missing; nothing listed: everything given is unknown.
        assert_eq!(id_misses(&["1", "2"], &[]).missing, ids(&["1", "2"]));
        assert_eq!(id_misses::<&str>(&[], &ids(&["a", "a"])).unknown, ids(&["a"]));
        // A listed id that is listed twice is missing once.
        assert_eq!(id_misses(&["1", "1"], &[]).missing, ids(&["1"]));
    }

    #[test]
    fn the_columns_a_review_left_out_are_all_said_in_one_problem() {
        let want = ["1", "2", "3", "4", "5", "6"];
        assert_eq!(
            said(columns_problems(&want, &ids(&["x", "2", "2", "3", "y"]))),
            [
                "there are no columns x, y",
                "column 2 is answered twice",
                "columns 1, 4, 5, 6 are not answered"
            ]
        );
        // What R246-46 and R246-47 were sent back for, in one send-back.
        assert_eq!(
            said(columns_problems(&want, &ids(&["1", "2", "3", "6"]))),
            ["columns 4, 5 are not answered"]
        );
        // One miss of a kind is said as it always was.
        assert_eq!(
            said(columns_problems(&want, &ids(&["1", "2", "3", "4", "5"]))),
            ["column 6 is not answered"]
        );
        assert_eq!(
            said(columns_problems(&want, &ids(&["1", "2", "3", "4", "5", "6", "7"]))),
            ["there is no column 7"]
        );
        assert!(columns_problems(&want, &ids(&["6", "5", "4", "3", "2", "1"])).is_empty());
        assert!(columns_problems(&[], &[]).is_empty());
    }

    // The reason a return is sent back with is one line for each problem, `check (place): problem`; a problem whose
    // line is there already is not added again, and a check with one problem gives the line it always gave. Each
    // problem keeps its check and its place.
    #[test]
    fn a_check_with_several_problems_gives_a_line_for_each_and_one_with_a_single_problem_the_line_it_gave() {
        let mut found = Vec::new();
        add(
            &mut found,
            "columns_answered",
            Some("columns.id"),
            &Problems::from("column 4 is not answered"),
        );
        let line = "columns_answered (columns.id): column 4 is not answered";
        assert_eq!(reason(&found), line);
        add(
            &mut found,
            "table_answered",
            None,
            &Problems::from(vec!["a is answered twice".to_string(), "no answer for b".to_string()]),
        );
        // The same check twice, and a problem twice in one check, add nothing.
        add(
            &mut found,
            "columns_answered",
            Some("columns.id"),
            &Problems::from(vec![
                "column 4 is not answered".to_string(),
                "column 4 is not answered".to_string(),
            ]),
        );
        assert_eq!(
            found,
            [
                problem("columns_answered", Some("columns.id"), "column 4 is not answered"),
                problem("table_answered", None, "a is answered twice"),
                problem("table_answered", None, "no answer for b"),
            ]
        );
        let lines = [
            line,
            "table_answered (the return): a is answered twice",
            "table_answered (the return): no answer for b",
        ];
        assert_eq!(reason(&found), lines.join("\n"));
        let mut p = Problems::default();
        assert!(p.is_empty());
        p.push("a");
        p.push("a");
        p.push("b");
        assert_eq!(said(p), ["a", "b"]);
        assert!(Problems::default().into_result().is_ok());
        assert!(Problems::from("a").into_result().is_err());
    }

    #[test]
    fn a_table_of_answers_is_checked_for_twice_unknown_and_missing_rows_together() {
        let rows = ["r1", "r2", "r3", "r4"];
        assert_eq!(
            said(table_problems(&ids(&["r1", "r1", "zz", "yy", "r2"]), &rows, true)),
            [
                "r1 is answered twice",
                "zz, yy are not rows of the table: answer each row by its number",
                "no answer for r3, r4"
            ]
        );
        // One of a kind: the text it always had.
        assert_eq!(
            said(table_problems(&ids(&["r1", "r2", "r3", "r4", "zz"]), &rows, true)),
            ["zz is not a row of the table: answer each row by its number"]
        );
        // A round given no numbered table is checked for an answer given twice alone.
        assert_eq!(
            said(table_problems(&ids(&["a", "b", "a"]), &[], false)),
            ["a is answered twice"]
        );
        assert!(table_problems(&ids(&["a", "b"]), &[], false).is_empty());
        assert!(table_problems(&ids(&["r4", "r3", "r2", "r1"]), &rows, true).is_empty());
    }

    #[test]
    fn the_answers_to_those_who_interjected_are_checked_for_strangers_and_twice_together() {
        let speakers = ids(&["owner", "architect"]);
        assert_eq!(
            said(speaker_problems(
                &speakers,
                &ids(&["owner", "owner", "advisor", "engineer"])
            )),
            [
                "the advisor, engineer did not interject in this round",
                "two answers to the owner"
            ]
        );
        assert_eq!(
            said(speaker_problems(
                &speakers,
                &ids(&["owner", "owner", "architect", "architect"])
            )),
            ["two answers to each of: owner, architect"]
        );
        assert_eq!(
            said(speaker_problems(&speakers, &ids(&["architect", "owner", "advisor"]))),
            ["the advisor did not interject in this round"]
        );
        assert!(speaker_problems(&speakers, &ids(&["architect"])).is_empty());
        // Nobody interjected and nothing is answered.
        assert!(speaker_problems(&[], &[]).is_empty());
    }

    #[test]
    fn a_table_of_conditions_is_checked_for_unknown_and_missing_ones_together_and_not_for_twice() {
        let wanted = ids(&["A1", "A2", "A3", "A4"]);
        assert_eq!(
            said(cover_problems(&wanted, &ids(&["A1", "X9", "Y8"]), false)),
            [
                "X9, Y8 are not acceptance conditions",
                "no row for acceptance conditions A2, A3, A4"
            ]
        );
        assert_eq!(
            said(cover_problems(&wanted, &ids(&["A1", "A2", "A3", "A4", "X9"]), false)),
            ["X9 is not an acceptance condition"]
        );
        assert_eq!(
            said(cover_problems(&wanted, &ids(&["A1", "A2", "A3"]), false)),
            ["no row for acceptance condition A4"]
        );
        // A recheck answers only the rows that changed; an id given twice was never a problem here.
        assert_eq!(
            said(cover_problems(&wanted, &ids(&["A1", "A1", "X9"]), true)),
            ["X9 is not an acceptance condition"]
        );
        assert!(cover_problems(&wanted, &ids(&["A1", "A1", "A2", "A3", "A4"]), false).is_empty());
        assert!(cover_problems(&[], &[], false).is_empty());
    }

    // The tests a table of conditions names that no run of ci on the pushed head reported are said together, each once,
    // with the ids that are no condition and the conditions with no row, in one call; a row that is not of kind tested
    // names no test to look for.
    #[test]
    fn every_test_a_table_of_conditions_names_that_ci_did_not_report_is_said_with_its_ids() {
        let wanted = ids(&["A1", "A2", "A3"]);
        let rows = [
            json!({ "id": "A1", "how": "tested", "tests": ["reported", "gone_one", "gone_two"] }),
            json!({ "id": "X9", "how": "tested", "tests": ["reported", "gone_one"] }),
            json!({ "id": "A2", "how": "report_answer", "where": "gone_three" }),
        ];
        let reported = |t: &str| !t.starts_with("gone");
        assert_eq!(
            said(conditions_problems(
                &wanted,
                &rows,
                false,
                reported,
                unreported,
                &[],
                found
            )),
            [
                "X9 is not an acceptance condition".to_string(),
                "no row for acceptance condition A3".to_string(),
                unreported(&ids(&["gone_one", "gone_two"])),
            ]
        );
        let text = unreported(&ids(&["gone_one", "gone_two"]));
        assert!(text.contains("these name: gone_one, gone_two."), "{text}");
        assert!(text.contains("together with the tests of the test table"), "{text}");
        assert!(text.contains("ci.results.separators"), "{text}");
        // With no condition in force, only that is said: their tests are not looked for.
        assert_eq!(
            said(conditions_problems(&[], &rows, false, reported, unreported, &[], found)),
            [NO_ACCEPTANCE]
        );
    }

    // Item #11: the check that a test a condition names was reported on the head reads the runs of ci there with the
    // same name rule as the results: `a_row` is reported by a run that ran `a_row`, never by one that ran only
    // `a_row_into_the_asking_part`, and a run of an engine before the results were read from JUnit reports nothing.
    #[test]
    fn a_test_a_condition_names_is_reported_on_the_head_by_the_name_rule_of_the_results() {
        let wanted = ids(&["A1"]);
        let rows = [json!({ "id": "A1", "how": "tested", "tests": ["a_row"] })];
        let check = |run: Value| {
            let on_head = [run];
            said(conditions_problems(
                &wanted,
                &rows,
                false,
                |t| on_head.iter().any(|r| reported(r, t, &seps()).is_some()),
                |names| not_reported_on_head(names, on_head.first()),
                &[],
                found,
            ))
        };
        let longer = json!({ "tests": [row("a_row_into_the_asking_part", "failed",
            &[("engine::e2e", "a_row_into_the_asking_part", "failed")])] });
        assert_eq!(check(longer), [unreported(&ids(&["a_row"]))]);
        let old = json!({ "tests": [{ "test": "a_row", "result": "passed" }] });
        assert_eq!(check(old), [unreported(&ids(&["a_row"]))]);
        let ran = json!({ "tests": [row("engine::e2e a_row", "passed", &[("engine::e2e", "a_row", "passed")])] });
        assert!(check(ran).is_empty());
    }

    /// A step as the branch's files have it, or not.
    fn found(step: &str) -> Result<(), String> {
        if step.starts_with("gone") {
            Err(format!("the step {step} is not in any file of the branch"))
        } else {
            Ok(())
        }
    }

    // A row of kind ci_job is accepted when each job is a check of a CI run recorded for the work and each step is text
    // in a file of the branch; what is not is said, all of it in one call and with the other problems of the table,
    // each job or step once, however many rows name it. With no CI run recorded nothing can be named, said once. A row
    // of kind read_diff names nothing the engine looks up.
    #[test]
    fn a_ci_job_row_names_jobs_recorded_and_steps_on_the_branch_and_what_is_not_is_said_together() {
        let wanted = ids(&["A1", "A2", "A3", "A4"]);
        let ci = ids(&["lint", "test 1 of 3", "check"]);
        let ok = |_: &str| true;
        let good = [
            json!({ "id": "A1", "how": "ci_job", "jobs": ["lint"], "steps": ["npm run check:ui"] }),
            json!({ "id": "A2", "how": "ci_job", "jobs": ["check", "test 1 of 3"], "steps": [] }),
            json!({ "id": "A3", "how": "read_diff", "where": "gone_file", "what": "gone_text" }),
            json!({ "id": "A4", "how": "ci_job", "jobs": ["check"] }),
        ];
        assert!(conditions_problems(&wanted, &good, false, ok, unreported, &ci, found).is_empty());
        // Jobs that were never run, steps that are in no file, and a condition left without a row, together.
        let bad = [
            json!({ "id": "A1", "how": "ci_job", "jobs": ["lint", "build"], "steps": ["gone one", "npm run check:ui"] }),
            json!({ "id": "A2", "how": "ci_job", "jobs": ["build"], "steps": ["gone one", " ", "a\nb"] }),
            json!({ "id": "X9", "how": "tested", "tests": ["gone_test"] }),
        ];
        let reported = |t: &str| !t.starts_with("gone");
        assert_eq!(
            said(conditions_problems(&wanted, &bad, false, reported, unreported, &ci, found)),
            [
                "X9 is not an acceptance condition".to_string(),
                "no row for acceptance conditions A3, A4".to_string(),
                unreported(&ids(&["gone_test"])),
                "the job build is not a check of any CI run of this work; the checks recorded are: lint, test 1 of 3, check"
                    .to_string(),
                "the step gone one is not in any file of the branch".to_string(),
                "a step names one command, written as one line of text".to_string(),
            ]
        );
        // No CI run recorded: any job name would pass, so it is one problem and not one for each job.
        assert_eq!(
            said(conditions_problems(&wanted, &good, false, ok, unreported, &[], found)),
            ["no CI run of this work has recorded its checks, so a ci_job row cannot name a job"]
        );
        // A recheck of a row that changed is judged by what it names alone.
        let one = [json!({ "id": "A2", "how": "ci_job", "jobs": ["nothing"], "steps": [] })];
        assert_eq!(
            said(conditions_problems(&wanted, &one, true, ok, unreported, &ci, found)),
            [
                "the job nothing is not a check of any CI run of this work; the checks recorded are: lint, test 1 of 3, check"
            ]
        );
    }

    // A ci_job row's jobs are read from what the tools recorded: the fast CI keeps its checks under `push`, the merge
    // its run of the full CI as a list, as a run again does, and the full CI on the draft its first run's under `first`;
    // a run that failed or was refused has none, nor does the merge's record of a merge commit or of none yet.
    #[test]
    fn the_names_of_the_checks_of_every_recorded_ci_run_are_read_from_each_tools_record() {
        let check = |n: &str| json!({ "name": n, "result": "green", "run": "u" });
        let fast =
            json!({ "push": { "verdict": "pushed", "head": "h", "checks": [check("lint"), check("test 1 of 3")] } });
        let merge = json!([check("check"), check("lint")]);
        let first = json!({ "first": [check("lint"), check("full only")] });
        let again = json!([check("again only")]);
        let refused = json!({ "push": { "verdict": "refused", "why": "w" } });
        let failed = json!({ "failed": { "kind": "other", "what": "w" } });
        assert_eq!(
            check_names(&[
                fast,
                merge,
                json!("0123abc"),
                Value::Null,
                first,
                again,
                refused,
                failed
            ]),
            ["lint", "test 1 of 3", "check", "full only", "again only"]
        );
    }

    // Rows that name conditions where none is in force are no rows of any condition, so that nothing else is said of
    // them: not that each is none, and not what they cite.
    #[test]
    fn with_no_condition_in_force_only_that_is_said() {
        assert_eq!(
            said(cover_problems(&[], &ids(&["A1", "A2", "A2"]), false)),
            [NO_ACCEPTANCE]
        );
        assert_eq!(
            cover_problems(&[], &ids(&["A1"]), true),
            cover_problems(&[], &ids(&["A1"]), false)
        );
    }

    #[test]
    fn the_findings_of_the_last_review_with_no_row_are_all_said_in_one_problem() {
        let due = ids(&["F1", "F2", "F3", "F2"]);
        assert_eq!(
            said(previous_problems(&due, &ids(&["F2"]))),
            ["no row for F1, F3, findings of the last review"]
        );
        assert_eq!(
            said(previous_problems(&due, &ids(&["F1", "F2", "X"]))),
            ["no row for F3, a finding of the last review"]
        );
        // Rows nobody asked for, and rows twice, are no concern of this check.
        assert!(previous_problems(&due, &ids(&["F1", "F2", "F3", "F3", "X"])).is_empty());
        assert!(previous_problems(&[], &[]).is_empty());
    }

    #[test]
    fn every_row_with_a_wrong_result_is_said_not_the_first() {
        let rows = vec![
            json!({ "name": "a", "on_head": "red", "on_base": "green" }),
            json!({ "name": "b", "on_head": "green", "on_base": "green" }),
            json!({ "name": "c", "on_head": "red", "on_base": "red" }),
        ];
        assert_eq!(
            said(result_problems(&rows, true)),
            [
                "a is red on the head: fix it in this round",
                "a passes before the change: it does not test new behaviour; fix it in this round",
                "b passes before the change: it does not test new behaviour; fix it in this round",
                "c is red on the head: fix it in this round",
            ]
        );
        assert_eq!(
            said(result_problems(&rows, false)),
            [
                "a is red on the head: fix it in this round",
                "c is red on the head: fix it in this round",
                "c does not pass before the change: it does not guard old behaviour; fix it in this round",
            ]
        );
        let fine = vec![json!({ "name": "d", "on_head": "green", "on_base": "red" })];
        assert!(result_problems(&fine, true).is_empty());
        let fine = vec![json!({ "name": "d", "on_head": "green", "on_base": "not_run" })];
        assert!(result_problems(&fine, false).is_empty());
    }

    // Item #270: a guard test that cannot be built before the change is reported cannot_run by ci_before; with its
    // reason in not_before the review judges it, as it judges one not run, and it is not sent back. With no reason it is
    // sent back to give one, and a guard that fails before the change is still sent back with or without a reason.
    #[test]
    fn a_guard_test_that_cannot_be_built_before_the_change_is_judged_by_its_reason() {
        let said_row = |base: &str, why: Option<&str>| {
            let mut row = json!({ "name": "g", "by": "engineer", "on_head": "green", "on_base": base });
            if let Some(why) = why {
                row["not_before"] = json!(why);
            }
            said(result_problems(&[row], false))
        };
        assert!(said_row("cannot_run", Some("the function is extracted by this change")).is_empty());
        let ask = ["g cannot be built before the change: say why in not_before, and the review judges it"];
        assert_eq!(said_row("cannot_run", None), ask);
        assert_eq!(said_row("cannot_run", Some("  ")), ask);
        for why in [None, Some("a reason")] {
            assert_eq!(
                said_row("red", why),
                ["g does not pass before the change: it does not guard old behaviour; fix it in this round"]
            );
        }
        // The table of new tests is as it was: cannot_run is right there, with or without a reason.
        let new = json!({ "name": "n", "by": "engineer", "on_head": "green", "on_base": "cannot_run" });
        assert!(result_problems(&[new], true).is_empty());
    }

    // Item #270: an adversarial guard test that cannot be built before the change fits the table of guard tests, as
    // one not run does; one that fails before the change is still wrong.
    #[test]
    fn an_adversarial_guard_test_that_cannot_be_built_fits_its_table() {
        let rows = vec![
            json!({ "name": "built", "by": "adversarial", "on_base": "green" }),
            json!({ "name": "not_built", "by": "adversarial", "on_base": "cannot_run" }),
            json!({ "name": "fails", "by": "adversarial", "on_base": "red" }),
        ];
        assert_eq!(
            said(kind_problems(&rows, &[], false)),
            ["fails: its result before the change does not fit its kind; fix it in this round and run it again"]
        );
    }

    // Item #217: a result a row lacks is said once, by the check of the engineer's tests, and not also judged as a
    // wrong one (a guard test with no result before the change "does not pass").
    #[test]
    fn a_test_row_with_no_result_before_the_change_is_not_told_it_does_not_pass() {
        let rows = vec![
            json!({ "name": "g", "by": "engineer", "on_head": "green" }),
            json!({ "name": "n", "by": "engineer", "on_head": "green", "on_base": null }),
        ];
        assert!(result_problems(&rows, false).is_empty());
        assert!(result_problems(&rows, true).is_empty());
    }

    // Item #217, R241-41: the forms say which run of ci_before counts and what to do when a file of the run changed after
    // it in the words the engine's retry text uses, so that the two do not part.
    #[test]
    fn the_forms_and_the_retry_text_say_the_same_of_which_run_counts() {
        let flow: Value = serde_json::from_str(include_str!("../../../flow/flow.json")).unwrap();
        let mut texts = Vec::new();
        for id in ["I1", "T1"] {
            let cell = flow["flows"][0]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == id)
                .unwrap();
            texts.push((id.to_string(), cell["prompt"].to_string()));
        }
        for (what, text) in texts {
            assert!(text.contains("latest run of ci_before that reported"), "{what}");
            assert!(text.contains(RUN_AGAIN), "{what}");
            // The engine compares all the files the run put there, not the one file of the test, which it cannot tell.
            assert!(text.contains("any of the files"), "{what}");
        }
    }

    // Item #217, R241-28: an extended test, written once in the test changes with its kind and its results, is in the
    // table of its kind with nothing missing and nothing wrong: the return as the item's report of it would be handed in
    // now is not sent back for a table it has no longer, or for the kind it gave.
    #[test]
    fn an_extended_test_written_once_has_no_problem_of_a_table() {
        let ret = json!({ "test_changes": [{
            "change": "extended", "test": "every_sample_takes_its_row", "catches": "c", "kind": "new_behaviour",
            "on_head": "green", "on_base": "red", "time": "1s", "worth": "w"
        }] });
        let tables = test_tables::of_return(&ret);
        assert!(result_problems(&tables.new, true).is_empty());
        assert!(tables.guard.is_empty());
        // Adversarial testing lists the engineer's test in the table of the kind the engineer gave, and is told when it
        // lists it in the other.
        let kinds = vec![("every_sample_takes_its_row".to_string(), "new_behaviour".to_string())];
        let listed = vec![json!({ "name": "every_sample_takes_its_row", "by": "engineer" })];
        assert!(kind_problems(&listed, &kinds, true).is_empty());
        assert_eq!(kind_problems(&listed, &kinds, false).texts().len(), 1);
    }

    /// A run of ci_before that ran, from `head`, with the test files `files` and the results `ran` reports.
    fn before_run(ran: Value, head: &str, files: &[&str]) -> BeforeRun {
        BeforeRun {
            ran,
            head: Some(head.to_string()),
            files: ids(files),
        }
    }

    // Item #217, R241-41: the result of a test before the change is the latest run of ci_before that reported it, and
    // counts while the files that run put on the code before the change are as they were. Three cases: a file changed
    // after the run (the test is to be run again, and is not compared with that run), no file changed (the earlier
    // run's result stands), and two runs that ran different tests (each test is judged by its own run).
    #[test]
    fn the_result_before_the_change_comes_from_the_latest_run_that_reported_the_test() {
        let green = json!({
            "checks": [{ "name": "test", "result": "green" }],
            "tests": [row("one", "passed", &[("t", "one", "passed")])],
        });
        let red = json!({
            "checks": [{ "name": "test", "result": "red" }],
            "tests": [row("two", "failed", &[("t", "two", "failed")])],
        });
        // The head reports both tests, so that only what is said before the change is judged here.
        let head = [json!({ "tests": [row("one two", "passed", &[("t", "one", "passed"), ("t", "two", "passed")])] })];
        let rows = vec![json!({ "name": "one", "on_base": "red" })];
        let none_changed = |_: &BeforeRun| Vec::new();
        // Said red, and the run reports it passed: said, when no file changed after it.
        let runs = vec![before_run(green.clone(), "aaaaaaaaaaaa", &["t/one.rs"])];
        let passed = "one is said to be red before the change, and ci_before reports it passed: say green";
        assert_eq!(
            said(results_vs_runs(&rows, &head, &runs, &seps(), none_changed)),
            [passed]
        );
        // A file changed after the run: the test is to be run again, and what the run says of it is not said.
        let changed = |run: &BeforeRun| run.files.clone();
        assert_eq!(
            said(results_vs_runs(&rows, &head, &runs, &seps(), changed)),
            [
                "one: its result before the change comes from the latest run of ci_before that reported it, at \
                 aaaaaaaaaa, and t/one.rs changed after that; run ci_before again"
            ]
        );
        // Two runs, the later one of other tests: `one` keeps the earlier run it was reported in (not the latest run,
        // which does not name it), and `two` is judged by the later one.
        let runs = vec![
            before_run(red, "bbbbbbbbbbbb", &["t/two.rs"]),
            before_run(green, "aaaaaaaaaaaa", &["t/one.rs"]),
        ];
        let rows = vec![
            json!({ "name": "one", "on_base": "red" }),
            json!({ "name": "two", "on_base": "green" }),
        ];
        assert_eq!(
            said(results_vs_runs(&rows, &head, &runs, &seps(), |_| Vec::new())),
            [
                passed,
                "two is said to be green before the change, and ci_before reports it failed: say red",
            ]
        );
        // Only the file of the run a test is reported in decides: `two.rs` changed after the later run and `one.rs`
        // did not, so `one` is compared with its run and `two` is to be run again.
        let after_later = |run: &BeforeRun| -> Vec<String> {
            run.files.iter().filter(|f| f.as_str() == "t/two.rs").cloned().collect()
        };
        let said_both = said(results_vs_runs(&rows, &head, &runs, &seps(), after_later));
        assert_eq!(said_both.len(), 2, "{said_both:?}");
        assert_eq!(said_both[0], passed, "{said_both:?}");
        assert!(
            said_both[1].starts_with("two: its result before the change comes from the latest run of ci_before"),
            "{said_both:?}"
        );
        // A run that cannot be compared with the head (no commit in the worktree) counts its files changed.
        let lost = BeforeRun {
            head: None,
            ..runs[1].clone()
        };
        assert!(
            said(results_vs_runs(&rows[..1], &head, &[lost], &seps(), |r| r
                .files
                .clone()))[0]
                .contains("a commit the worktree lacks")
        );
        // With no run of ci_before, nothing is said of before the change.
        assert!(results_vs_runs(&rows, &head, &[], &seps(), |_| Vec::new()).is_empty());
    }

    // Item #11: each test's result on the head is that of the latest run of ci on the pushed head that reported it, by
    // the name rule of the results, and every test that does not match is said with everything wrong with it; the
    // tests no run there reported are said once, together, to be named in the next ci.
    #[test]
    fn every_test_whose_result_does_not_match_the_runs_is_said_with_everything_wrong_with_it() {
        let rows = vec![
            json!({ "name": "one", "on_head": "red", "on_base": "red" }),
            json!({ "name": "two", "on_head": "green", "on_base": "green" }),
            json!({ "name": "three", "on_head": "green", "on_base": "cannot_run" }),
            json!({ "name": "a_row", "on_head": "green", "on_base": "green" }),
            json!({ "name": "skipped", "on_head": "green", "on_base": "not_run" }),
            json!({ "name": "four", "on_head": "green", "on_base": "green" }),
        ];
        // The latest run on the head reports `two` passed, and an earlier one reported it failed: the latest counts.
        let head = [
            json!({ "tests": [row("two", "passed", &[("e", "two", "passed")])] }),
            json!({ "tests": [
                row("one", "passed", &[("e", "one", "passed")]),
                row("two", "failed", &[("e", "two", "failed")]),
                row("three", "passed", &[("e", "m::three", "passed")]),
                row("a_row", "passed", &[("e", "a_row", "passed")]),
                row("a_row_into_the_asking_part", "failed", &[("e", "a_row_into_the_asking_part", "failed")]),
                row("skipped", "not_run", &[("e", "skipped", "skipped")]),
            ] }),
        ];
        let before = vec![before_run(
            json!({
                "checks": [{ "name": "test", "result": "red" }],
                "tests": [
                    row("one", "passed", &[("e", "one", "passed")]),
                    row("two", "failed", &[("e", "two", "failed")]),
                    { "test": "three", "result": "not_built", "matched": [], "why": "u: no such method" },
                    row("a_row", "passed", &[("e", "a_row", "passed")]),
                    row("a_row_into_the_asking_part", "failed", &[("e", "a_row_into_the_asking_part", "failed")]),
                ],
            }),
            "aaaaaaaaaaaa",
            &["t/x.rs"],
        )];
        assert_eq!(
            said(results_vs_runs(&rows, &head, &before, &seps(), |_| Vec::new())),
            [
                "one is said to be red on the head, and the latest run of ci there that reported it reports it passed"
                    .to_string(),
                "one is said to be red before the change, and ci_before reports it passed: say green".to_string(),
                "two is said to be green before the change, and ci_before reports it failed: say red".to_string(),
                "skipped is said to be green on the head, and the latest run of ci there that reported it reports it \
                 skipped, so that nothing measured it"
                    .to_string(),
                unreported(&ids(&["four"])),
            ]
        );
        // With no run on the head, every test is to be named in the next ci, said once.
        assert_eq!(
            said(results_vs_runs(&rows[..2], &[], &[], &seps(), |_| Vec::new())),
            [unreported(&ids(&["one", "two"]))]
        );
    }

    // Item #11: when the latest run of ci on the pushed head could not read its results, the tests no run there
    // reported are said with why, and to ask the architect if running again does not mend it, never only to be named
    // again; a test an earlier run on the head reported is judged by that run. A project whose configuration names no
    // results is told that running again does not help.
    #[test]
    fn tests_a_run_on_the_head_could_not_read_are_said_with_why_and_not_only_to_be_named_again() {
        let rows = vec![
            json!({ "name": "one", "on_head": "green" }),
            json!({ "name": "two", "on_head": "green" }),
        ];
        let why = "the run's results could not be read: the run uploaded no artifact test-results";
        let unread = json!({ "tests": [
            { "test": "one", "result": "unread", "why": why },
            { "test": "two", "result": "unread", "why": why },
        ] });
        let earlier = json!({ "tests": [row("one", "passed", &[("e", "one", "passed")])] });
        let problems = said(results_vs_runs(&rows, &[unread.clone(), earlier], &[], &seps(), |_| {
            Vec::new()
        }));
        assert_eq!(problems, [unread_on_head(&ids(&["two"]), why)]);
        assert!(
            problems[0].contains(why) && problems[0].contains("ask the architect"),
            "{problems:?}"
        );
        assert!(
            !problems[0].contains("together with the tests of the test table"),
            "{problems:?}"
        );
        // An unread run before a later one that read its results but left the test out: the test is to be named.
        let read = json!({ "tests": [row("one", "passed", &[("e", "one", "passed")])] });
        assert_eq!(
            said(results_vs_runs(&rows, &[read, unread], &[], &seps(), |_| Vec::new())),
            [unreported(&ids(&["two"]))]
        );
        let text = no_results(&ids(&["one", "two"]));
        assert!(text.contains("`ci.results`") && text.contains("one, two"), "{text}");
        assert!(text.contains("does not help: ask the architect"), "{text}");
    }

    // Item #11: a table's name is tied to the tests a run reported by `names`, not to the row a request wrote: ci named
    // `a_row`, which names a passing test of one unit and a failing one of another, and the row says failed. The table
    // that names the passing test alone (`engine::e2e a_row`) said green rightly; one that names both (`a_row`) or the
    // failing one did not; and ci_before is read the same way.
    #[test]
    fn a_table_s_name_is_judged_by_the_tests_it_names_not_by_the_row_it_was_run_under() {
        let both = row(
            "a_row",
            "failed",
            &[
                ("engine::e2e", "a_row", "passed"),
                ("engine", "tools::tests::a_row", "failed"),
            ],
        );
        let head = [json!({ "tests": [both.clone()] })];
        let before = vec![before_run(
            json!({ "checks": [{ "name": "test", "result": "red" }], "tests": [both] }),
            "aaaaaaaaaaaa",
            &["t/x.rs"],
        )];
        let rows = vec![
            json!({ "name": "engine::e2e a_row", "on_head": "green", "on_base": "green" }),
            json!({ "name": "e2e::a_row", "on_head": "green", "on_base": "green" }),
            json!({ "name": "a_row", "on_head": "green", "on_base": "green" }),
            json!({ "name": "tests::a_row", "on_head": "red", "on_base": "red" }),
            json!({ "name": "engine tools::tests::a_row", "on_head": "green", "on_base": "red" }),
        ];
        assert_eq!(
            said(results_vs_runs(&rows, &head, &before, &seps(), |_| Vec::new())),
            [
                "a_row is said to be green on the head, and the latest run of ci there that reported it reports it \
                 failed",
                "a_row is said to be green before the change, and ci_before reports it failed: say red",
                "engine tools::tests::a_row is said to be green on the head, and the latest run of ci there that \
                 reported it reports it failed",
            ]
        );
    }

    #[test]
    fn every_test_in_a_table_of_the_wrong_kind_is_said() {
        let rows = vec![
            json!({ "name": "a", "by": "engineer" }),
            json!({ "name": "b", "by": "engineer" }),
            json!({ "name": "fine", "by": "engineer" }),
            json!({ "name": "c", "by": "adversarial", "on_base": "green" }),
            json!({ "name": "d", "by": "adversarial", "on_base": "red" }),
        ];
        let kinds = vec![
            ("a".to_string(), "old_behaviour".to_string()),
            ("b".to_string(), "old_behaviour".to_string()),
            ("fine".to_string(), "new_behaviour".to_string()),
        ];
        assert_eq!(
            said(kind_problems(&rows, &kinds, true)),
            [
                "a was added by the engineer as old_behaviour; it belongs in the other table",
                "b was added by the engineer as old_behaviour; it belongs in the other table",
                "c: its result before the change does not fit its kind; fix it in this round and run it again",
            ]
        );
        // In the table of guard tests: `fine` was added as new, and an adversarial test that does not pass is wrong.
        let kinds = vec![("fine".to_string(), "new_behaviour".to_string())];
        assert_eq!(
            said(kind_problems(&rows, &kinds, false)),
            [
                "fine was added by the engineer as new_behaviour; it belongs in the other table",
                "d: its result before the change does not fit its kind; fix it in this round and run it again",
            ]
        );
    }

    #[test]
    fn every_dispute_with_no_row_is_said() {
        let disputes = vec![
            ("t1".to_string(), "\"test_wrong\"".to_string()),
            ("t2".to_string(), "\"advisor_ruled\"".to_string()),
            ("t3".to_string(), "\"test_wrong\"".to_string()),
        ];
        assert_eq!(
            said(disputed_problems(&disputes, &ids(&["t2"]))),
            [
                "no row for t1, which the engineer answered \"test_wrong\"",
                "no row for t3, which the engineer answered \"test_wrong\""
            ]
        );
        assert!(disputed_problems(&disputes, &ids(&["t1", "t2", "t3"])).is_empty());
        assert!(disputed_problems(&[], &[]).is_empty());
    }

    #[test]
    fn every_finding_insisted_on_after_a_ruling_is_said() {
        let ruled: BTreeSet<String> = ids(&["f1", "f2", "t9"]).into_iter().collect();
        let insisted = vec![json!({ "fid": "f1" }), json!({ "test": "t9" }), json!({ "fid": "f3" })];
        let previous = vec![
            json!({ "fid": "f2", "verdict": "not_fixed" }),
            json!({ "fid": "f1", "verdict": "pass" }),
            json!({ "fid": "f9", "verdict": "not_fixed" }),
        ];
        assert_eq!(
            said(insisting_problems(&ruled, &insisted, &previous)),
            [
                "f1 was ruled on; it is not insisted on",
                "t9 was ruled on; it is not insisted on",
                "f2 was ruled on; it is not marked not_fixed"
            ]
        );
        assert!(insisting_problems(&ruled, &[json!({ "fid": "f3" })], &[]).is_empty());
    }

    #[test]
    fn every_disagreement_with_something_insisted_on_is_said() {
        let insisted: BTreeSet<String> = ids(&["f1", "t2"]).into_iter().collect();
        assert_eq!(
            said(disagreeing_problems(&insisted, &ids(&["f1", "other", "t2"]))),
            [
                "f1 was insisted on; when neither side gives way, ask",
                "t2 was insisted on; when neither side gives way, ask"
            ]
        );
        assert!(disagreeing_problems(&insisted, &ids(&["other"])).is_empty());
    }

    #[test]
    fn every_fact_whose_run_the_engine_did_not_record_is_said_and_a_read_that_fails_is_not_a_problem() {
        let bases = vec![
            json!({ "kind": "fact", "run": "r1", "check": "c1" }),
            json!({ "kind": "fact", "run": "r2", "check": "c2" }),
            json!({ "kind": "fact", "run": "r3", "check": "c3" }),
            json!({ "kind": "inferred", "run": "r4" }),
        ];
        let found = unrecorded_facts(&bases, |run, _| Ok(run == "r2")).unwrap();
        assert_eq!(found.texts().len(), 2, "{found:?}");
        assert!(
            found.texts()[0].contains("\"r1\"") && found.texts()[1].contains("\"r3\""),
            "{found:?}"
        );
        assert!(unrecorded_facts(&bases, |_, _| Ok(true)).unwrap().is_empty());
        assert_eq!(
            unrecorded_facts(&bases, |_, _| Err("no database".into())).unwrap_err(),
            "no database"
        );
        // A basis the finding keeps under `basis` is read by the caller; one that is no fact is not looked up.
        assert!(
            unrecorded_facts(&[json!({ "kind": "inferred" })], |_, _| Ok(false))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_research_round_that_left_changes_uncommitted_and_changed_outside_research_is_told_both() {
        assert_eq!(
            said(research_problems(false, &ids(&["research/a.md", "src/x.rs", "y.rs"]))),
            [
                "the worktree has uncommitted changes",
                "this round changed files outside research/: src/x.rs, y.rs"
            ]
        );
        assert_eq!(
            said(research_problems(false, &ids(&["research/a.md"]))),
            ["the worktree has uncommitted changes"]
        );
        assert_eq!(
            said(research_problems(true, &ids(&["x.rs"]))),
            ["this round changed files outside research/: x.rs"]
        );
        assert!(research_problems(true, &ids(&["research/a.md", "research/b/c.md"])).is_empty());
        assert!(research_problems(true, &[]).is_empty());
    }

    #[test]
    fn acceptance_conditions_with_a_report_answer_in_a_change_and_not_handed_in_word_for_word_are_both_said() {
        assert_eq!(
            said(acceptance_problems(true, true)),
            [
                "a work that delivers a change has no report_answer condition",
                LOCKED_ACCEPTANCE
            ]
        );
        assert_eq!(
            said(acceptance_problems(true, false)),
            ["a work that delivers a change has no report_answer condition"]
        );
        assert_eq!(acceptance_problems(false, true).texts().len(), 1);
        assert!(acceptance_problems(false, false).is_empty());
    }
}
