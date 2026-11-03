//! The engine's checks of a return: `engine.checks` in the flow, each named by the cells that use it. A return that
//! fails one is sent back with the reason, before it takes a row. The reason says every problem of every check that
//! applies, and a check says every problem it can find in the return ([`Problems`]), since each send-back costs a round.
//! The one check that does not is `cards_match_records`: no cell of the flow calls it, so it sends nothing back and
//! `crate::cards::check`, which stops at its first problem, is not converted; both go with `cards.rs` in #203.
//!
//! A check whose `at` lies in one of the forms every agent has (ask, answers, redirect) runs on returns of that form;
//! any other runs on the cell's own answer, whether or not the place is there, since some checks are about a place
//! left out.
//!
//! In this version some checks look at less than the flow says, because what they would compare is not recorded
//! yet: CI gives each job's result, not each test's, so test results are checked only against what the jobs show;
//! cited files are checked to exist, not to be in the diff; a table of findings is checked for one answer per id;
//! a red check said not to be caused by the change is taken as written when main's checks cannot be read; a ruling
//! said to be the advisor's, or the architect's or the owner's in its place, is taken as written once the advisor was
//! asked.

use std::collections::BTreeSet;
use std::path::Path;

use flow::{CompiledCell, CompiledGraph};
use serde_json::Value;

use crate::inputs::{answered, at_path, last_return};
use crate::work::{self, Item, Round, RoundId, Work};
use crate::{Engine, form_of, git, test_name, test_tables, values};
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

    /// The line this problem makes in the text a return is sent back with, `check (place): problem`.
    fn line(&self) -> String {
        format!(
            "{} ({}): {}",
            self.check,
            self.field.as_deref().unwrap_or("the return"),
            self.text
        )
    }
}

/// The text a return that failed checks is sent back with: a line for each problem, in the order found. The only
/// place this line is made.
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
/// [`reason`] makes the text it is sent back with.
pub fn run(c: &Ctx<'_>) -> Result<(), Vec<Problem>> {
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
    if found.is_empty() { Ok(()) } else { Err(found) }
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

/// What is wrong with how a review carries the blind review's findings (`blind`) into its columns (`carried`) and its
/// withdrawals (`withdrawn`): findings withdrawn that the blind review did not hand in, and findings of the blind
/// review neither carried nor withdrawn, each kind said once with all its findings.
fn blind_problems(blind: &[String], carried: &[String], withdrawn: &[String]) -> Problems {
    let mut problems = Problems::default();
    problems.about(
        &once(withdrawn.iter().filter(|w| !blind.contains(w))),
        |x| format!("{x} is withdrawn, and the blind review handed in no such finding"),
        |xs| format!("{xs} are withdrawn, and the blind review handed in no such findings"),
    );
    problems.about(
        &once(blind.iter().filter(|b| !carried.contains(b) && !withdrawn.contains(b))),
        |x| format!("the blind review's finding {x} is neither carried into a column nor withdrawn"),
        |xs| format!("the blind review's findings {xs} are neither carried into a column nor withdrawn"),
    );
    problems
}

/// The texts of an iterator, each once, in the order they come.
fn once<'a>(texts: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in texts {
        if !out.contains(t) {
            out.push(t.clone());
        }
    }
    out
}

/// The columns of a review that the engineer could not verify either (`both`) and that the review does not decide this
/// time, each column once.
fn unverified_problems(both: &BTreeSet<String>, columns: &[Value]) -> Problems {
    let mut problems = Problems::default();
    for id in columns
        .iter()
        .filter_map(|col| col["id"].as_str())
        .filter(|id| both.contains(*id))
    {
        problems.push(format!(
            "column {id}: the engineer could not verify it either; decide it now, a pass on what the work \
             shows or findings that say what evidence is needed and how to get it, not cannot_verify again"
        ));
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

    /// The acceptance conditions in force: those locked on the item, or else those the design handed in.
    fn acceptance(&self) -> Result<Vec<Value>, String> {
        let locked: Option<String> = self
            .e
            .db
            .conn()
            .query_row("SELECT acceptance FROM items WHERE id = ?1", [self.item()?.id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        match work::json_col(locked) {
            Some(Value::Array(xs)) => Ok(xs),
            _ => Ok(items(self.last("D2", "acceptance")?.as_ref())),
        }
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
        let rows = work::query_json(
            self.e.db.conn(),
            "SELECT args, result FROM tool_runs
             WHERE work = ?1 AND tool = 'ci_before' AND result IS NOT NULL ORDER BY id DESC",
            &[&self.w.id],
            1000,
        )?;
        let wt = self.w.worktree.as_deref();
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

    /// The names of the checks of the CI runs recorded for this work, newest run first: those of the fast CI (a `ci`
    /// run keeps them under `push`) and of the full CI (an attempt keeps its first run's under `first`, a run again
    /// its list as it is). A run that did not get as far as its checks has none.
    fn ci_jobs(&self) -> Result<Vec<String>, String> {
        let mut runs = self.ci_runs("ci")?;
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
            // The CI runs recorded are looked up only for a row that names a job.
            let jobs = if rows.iter().any(|r| r["how"] == "ci_job") {
                c.ci_jobs()?
            } else {
                Vec::new()
            };
            conditions_problems(
                &wanted,
                &rows,
                recheck,
                |t| test_name::on_branch(wt, t),
                &jobs,
                |s| text_on_branch(wt, s),
            )
            .into_result()
        }
        "new_test_results" | "guard_test_results" => {
            if c.last("D11", "adversarial")?.is_some_and(|a| a["verdict"] == "needed") {
                return Ok(());
            }
            let new = check == "new_test_results";
            // The tests this return adds or extends, each written once in the test changes, with their two results.
            let tables = test_tables::of_return(&c.unwrapped);
            let rows = tables.table(if new { "new_tests" } else { "guard_tests" });
            let mut problems = missing_results(rows);
            problems.extend(result_problems(rows, new));
            problems.extend(test_results_match_runs(c, rows)?);
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
        "previous_findings_answered" => {
            let Some(last) = c.accepted("T4")?.into_iter().next() else {
                return Ok(());
            };
            let out = last.output.map(|o| c.g.unwrap(&o)).unwrap_or(Value::Null);
            let mut due: Vec<String> = strs(at_path(&out, "columns.findings.fid").as_ref());
            due.extend(
                items(out.get("previous"))
                    .iter()
                    .filter(|p| matches!(p["verdict"].as_str(), Some("not_fixed" | "insist")))
                    .filter_map(|p| p["fid"].as_str().map(str::to_string)),
            );
            previous_problems(&due, &strs(v)).into_result()
        }
        "unverified_decided" => {
            // The columns the engineer could not verify either: rows of the fix since the last review that came from
            // a column that review could not verify, answered cannot_verify.
            let since = c.accepted("T4")?.first().map(|r| r.id);
            let mut both = BTreeSet::new();
            for fix in c.accepted("T6")?.into_iter().filter(|r| since.is_none_or(|s| r.id > s)) {
                let table = table_of(&fix);
                for row in items(fix.output.as_ref().and_then(|o| o[&c.g.within].get("rows"))) {
                    let given = table.iter().find(|t| t["row"] == row["id"]);
                    if row["answer"] == "cannot_verify"
                        && let Some(t) = given.filter(|t| t["from"] == "T4·columns[cannot_verify]")
                    {
                        both.extend(t["column"].as_str().map(str::to_string));
                    }
                }
            }
            unverified_problems(&both, &items(v)).into_result()
        }
        "no_adversarial_when_due" => {
            let first = c.accepted("T4")?.is_empty();
            let chose = c.last("D11", "T1")?.is_some_and(|a| a["verdict"] == "needed");
            let ran = !c.accepted("T1")?.is_empty();
            let kind = values::value(c.e, c.w, c.g, c.item, "work")?;
            let due = c.item()?.deliver == "change" && kind == "implementation" && !chose && !ran;
            let verdict = v.and_then(|x| x["verdict"].as_str());
            match (due, first, verdict) {
                (true, true, None) => fail(
                    "the first review of a change whose design chose no adversarial testing hands in no_adversarial",
                ),
                (true, false, Some("stands")) => {
                    fail("at a re-review, no_adversarial is handed in only as does_not_stand")
                }
                (false, _, Some(_)) => fail("no_adversarial is not due here"),
                _ => Ok(()),
            }
        }
        "blind_findings_carried" => {
            let Some(blind) = c.accepted("T3")?.into_iter().next() else {
                return Ok(());
            };
            if c.accepted("T4")?.first().is_some_and(|r| r.id > blind.id) {
                return Ok(());
            }
            let out = blind.output.map(|o| c.g.unwrap(&o)).unwrap_or(Value::Null);
            let blind_ids = strs(at_path(&out, "findings.fid").as_ref());
            let carried = strs(at_path(&c.unwrapped, "columns.findings.fid").as_ref());
            blind_problems(&blind_ids, &carried, &strs(v)).into_result()
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
            let mut insisted: BTreeSet<String> = BTreeSet::new();
            for (cell, path) in [("T4", "previous"), ("T1", "previous")] {
                for p in items(c.last(cell, path)?.as_ref())
                    .iter()
                    .filter(|p| p["verdict"] == "insist")
                {
                    insisted.extend(p["fid"].as_str().or(p["test"].as_str()).map(str::to_string));
                }
            }
            let rows = items(c.unwrapped.get("rows"));
            let disagreed: Vec<String> = rows
                .iter()
                .filter(|r| matches!(r["answer"].as_str(), Some("disagree" | "test_wrong")))
                .filter_map(|r| r["id"].as_str().map(|id| known_as(c.table, id)))
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
        other => crate::architect::check(c, other, at, v),
    }
}

/// What is wrong with the kinds of the tests in a table of new tests (`new`) or of guard tests: each test the engineer
/// added under the other kind (`kinds`, each added test's name and kind), and each adversarial test whose result before
/// the change does not fit the table, every row once.
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
                !matches!(base, "green" | "not_run")
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
/// each that does not fit its kind before the change (a new test that passes, a guard that does not pass), the three
/// kinds of problem independent of each other and of the other rows. A result a row lacks is judged by nothing here:
/// [`missing_results`] says it.
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
        if !new && base.is_some_and(|b| !matches!(b, "green" | "not_run")) {
            problems.push(format!(
                "{name} does not pass before the change: it does not guard old behaviour; fix it in this round"
            ));
        }
    }
    problems
}

/// The results a test of the engineer's tables (`rows`, read from the test changes) lacks, each one: a design that chose
/// no adversarial testing has the engineer run every test it adds or extends, and the row of the test changes holds the
/// two results.
fn missing_results(rows: &[Value]) -> Problems {
    let mut problems = Problems::default();
    for t in rows {
        let name = t["name"].as_str().unwrap_or_default();
        for (field, when) in [("on_head", "with the change"), ("on_base", "before the change")] {
            if t.get(field).is_none_or(Value::is_null) {
                problems.push(format!(
                    "{name} has no result {when}: write it as {field} in its row of the test changes"
                ));
            }
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
/// [`cover_problems`] says about their ids; each test a row of kind tested names that is not on the branch
/// (`on_branch` says, with the reason when not); and, for a row of kind ci_job, each job that is not a check of a CI
/// run recorded for the work (`ci`; when none is recorded, that once, since any name would pass) and each step that is
/// not text in a file of the branch (`step_on_branch` says). A row of kind read_diff names nothing the engine can look
/// up: the table review reads it. With no condition in force the rows are no rows of any condition, so what they name
/// is not looked for.
fn conditions_problems(
    wanted: &[String],
    rows: &[Value],
    recheck: bool,
    on_branch: impl Fn(&str) -> Result<(), String>,
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
    for r in rows.iter().filter(|r| r["how"] == "tested") {
        for t in strs(r.get("tests")) {
            if let Err(why) = on_branch(&t) {
                problems.push(why);
            }
        }
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
/// the CI runs on demand a run that only ran the lint (`push.run` is `lint`, as when the call only hands in the PR's
/// text) is made after the run that ran the tests and must not hide it, as a red test run would be on the same head.
/// A run that does not say what it ran is one of the whole of the fast CI. None when no run is on the head.
fn red_checks_of_head(runs: &[Value], sha: &str) -> Option<Vec<String>> {
    let mut on_head = runs.iter().filter(|r| r["push"]["head"] == sha);
    let latest = on_head.clone().next()?;
    let tests = on_head.find(|r| r["push"]["run"] != "lint");
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

/// Whether a log says a test failed: a line names it with a word of failure besides its name. A red run's log lists
/// the tests that passed too.
fn names_failing(log: &str, name: &str) -> bool {
    const FAILED: [&str; 6] = ["FAIL", "fail", "panicked", "----", "\u{2715}", "\u{2717}"];
    !name.is_empty()
        && log.lines().filter(|l| l.contains(name)).any(|l| {
            let rest = l.replace(name, "");
            FAILED.iter().any(|w| rest.contains(w))
        })
}

/// Whether every check of a run is green.
fn all_green(checks: Option<&Value>) -> bool {
    items(checks).iter().all(|x| x["result"] == "green")
}

/// Whether the log of a red check of a run names a test failing.
fn names_red(checks: Option<&Value>, name: &str) -> bool {
    items(checks)
        .iter()
        .any(|x| x["result"] == "red" && x["log_tail"].as_str().is_some_and(|l| names_failing(l, name)))
}

/// A test said to be green on the head while every run on the head was red names it, or said to be red while every
/// check on the head was green, does not match the runs; the same for the code before the change.
///
/// Every test that does not match is a problem, with everything that is wrong with it.
fn test_results_match_runs(c: &Ctx<'_>, rows: &[Value]) -> Result<Problems, String> {
    let pushed = c.pushed_head()?;
    let ci = c.ci_runs("ci")?;
    let on_head = ci.iter().find(|r| r["push"]["head"].as_str() == pushed.as_deref());
    let before = c.before_runs()?;
    Ok(results_vs_runs(rows, on_head.map(|run| &run["push"]), &before, |run| {
        c.changed_since(run)
    }))
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

/// The run of ci_before whose result for a test counts: the latest that reported the test. A test none of them
/// reported (a run of an engine that did not report each test, or a name the request left out) is judged by the latest
/// run's checks alone, as it was when there was one run to look at.
fn run_for<'a>(runs: &'a [BeforeRun], name: &str) -> Option<&'a BeforeRun> {
    runs.iter()
        .find(|r| {
            items(r.ran.get("tests"))
                .iter()
                .any(|t| t["test"].as_str() == Some(name))
        })
        .or_else(|| runs.first())
}

/// The tests of a table (`rows`) whose results do not match the run of ci on the head (`on_head`, the run's push, when
/// there is one) and the run of ci_before whose result for the test counts (`before`, the runs that ran, the latest
/// first; [`run_for`]), every test and every result once. A result before the change counts only while the files that
/// run put on the code before the change are as they were (`changed_since` gives those of a run that changed after
/// its commit): a test whose result comes from a run they changed after is to be run again, and is not compared with
/// that run.
fn results_vs_runs(
    rows: &[Value],
    on_head: Option<&Value>,
    before: &[BeforeRun],
    changed_since: impl Fn(&BeforeRun) -> Vec<String>,
) -> Problems {
    let mut problems = Problems::default();
    for t in rows {
        let name = t["name"].as_str().unwrap_or_default();
        if let Some(run) = on_head {
            let checks = run.get("checks");
            match t["on_head"].as_str() {
                Some("red") if all_green(checks) => problems.push(format!(
                    "{name} is said to be red on the head, and every check there is green"
                )),
                Some("green") if names_red(checks, name) => problems.push(format!(
                    "{name} is said to be green on the head, and a red check's log names it"
                )),
                _ => {}
            }
        }
        if let Some(run) = run_for(before, name) {
            let changed = changed_since(run);
            if changed.is_empty() {
                problems.extend(base_matches_run(name, t["on_base"].as_str(), &run.ran));
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
    problems
}

/// Where what a test is said to give before the change does not match the run of ci_before that ran: said red while
/// every check there is green, or green while a red check's log names it; said red or green while ci_before reports
/// it not run, which nothing measured; said not run while it reports it passed or failed. Each that applies is a
/// problem of its own. A test the run was named under no such name, or a run of an engine that did not report each
/// test, is checked by the run's checks alone.
fn base_matches_run(name: &str, said: Option<&str>, ran: &Value) -> Problems {
    let checks = ran.get("checks");
    let reported = items(ran.get("tests"))
        .iter()
        .find(|t| t["test"].as_str() == Some(name))
        .and_then(|t| t["result"].as_str().map(str::to_string));
    let mut problems = Problems::default();
    if said == Some("red") && all_green(checks) {
        problems.push(format!(
            "{name} is said to be red before the change, and every check there is green"
        ));
    }
    if said == Some("green") && names_red(checks, name) {
        problems.push(format!(
            "{name} is said to be green before the change, and a red check's log names it"
        ));
    }
    if let (Some(said @ ("red" | "green")), Some("not_run")) = (said, reported.as_deref()) {
        problems.push(format!(
            "{name} is said to be {said} before the change, and ci_before reports it not run, so that nothing \
             measured it: say not_run, or cannot_run when what it tests does not exist before the change"
        ));
    }
    if let (Some("not_run"), Some(result @ ("passed" | "failed"))) = (said, reported.as_deref()) {
        problems.push(format!(
            "{name} is said not to have run before the change, and ci_before reports it {result}"
        ));
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    // The head is green only if the latest run that ran tests is: a run of the lint alone, made later to hand in the
    // PR's text, does not stand for them. Runs of another head, and a lint-only run that is itself red, are as ever.
    #[test]
    fn a_lint_only_run_does_not_hide_a_red_test_run_on_the_same_head() {
        let red_tests = pushed("h", Some("named_and_lint"), &[("lint", "green"), ("fast", "red")]);
        let lint_green = pushed("h", Some("lint"), &[("lint", "green"), ("fast", "green")]);
        // The newest run is first.
        let runs = [lint_green.clone(), red_tests.clone()];
        assert_eq!(red_checks_of_head(&runs, "h"), Some(vec!["fast".to_string()]));
        // A later run that ran the tests and passed covers the red one.
        let green_tests = pushed("h", Some("named_and_lint"), &[("lint", "green"), ("fast", "green")]);
        let runs = [lint_green.clone(), green_tests.clone(), red_tests.clone()];
        assert_eq!(red_checks_of_head(&runs, "h"), Some(vec![]));
        // A head that only the lint has run is green on it; red on it, it is not.
        assert_eq!(red_checks_of_head(std::slice::from_ref(&lint_green), "h"), Some(vec![]));
        let lint_red = pushed("h", Some("lint"), &[("lint", "red")]);
        let runs = [lint_red, green_tests.clone()];
        assert_eq!(red_checks_of_head(&runs, "h"), Some(vec!["lint".to_string()]));
        // A run that does not say what it ran is the whole of the fast CI; the red run of another head is not the head's.
        let old = pushed("h", None, &[("test 1 of 3", "red")]);
        assert_eq!(
            red_checks_of_head(&[lint_green.clone(), old], "h"),
            Some(vec!["test 1 of 3".to_string()])
        );
        let elsewhere = pushed("g", Some("named_and_lint"), &[("fast", "red")]);
        assert_eq!(red_checks_of_head(&[lint_green, elsewhere], "h"), Some(vec![]));
        assert_eq!(red_checks_of_head(&[], "h"), None);
    }

    #[test]
    fn a_log_names_a_test_failing_only_on_a_line_that_says_so() {
        let log = "test a::passes ... ok\ntest a::breaks ... FAILED\n---- a::panics stdout ----\ntest a::fails_cleanly ... ok";
        assert!(!names_failing(log, "passes"));
        assert!(!names_failing(log, "fails_cleanly"));
        assert!(names_failing(log, "breaks"));
        assert!(names_failing(log, "panics"));
        assert!(!names_failing(log, ""));
    }

    // A test ci_before did not reach, as when its job stopped at an earlier red test, is said not run, never red or
    // green, and one it ran is not said not run: in the sixth trial a work had two such tests and no true value for
    // them. A test the run reports nothing of is checked by the run's checks alone.
    #[test]
    fn a_result_before_the_change_matches_what_ci_before_reports_of_the_test() {
        let ran = json!({
            "checks": [{ "name": "test", "result": "red", "log_tail": "FAIL [ 0.1s] e::t first\n" }],
            "tests": [
                { "test": "first", "result": "failed", "ended": "FAIL" },
                { "test": "later", "result": "not_run" }
            ]
        });
        let said = |name: &str, said: &str| base_matches_run(name, Some(said), &ran).into_result();
        assert!(said("first", "red").is_ok());
        assert!(said("first", "not_run").unwrap_err().texts()[0].contains("reports it failed"));
        assert!(said("later", "not_run").is_ok());
        assert!(said("later", "cannot_run").is_ok());
        for wrong in ["red", "green"] {
            let e = said("later", wrong).unwrap_err();
            assert!(e.texts()[0].contains("reports it not run"), "{e:?}");
        }
        assert!(said("unnamed", "not_run").is_ok());
        assert!(said("unnamed", "red").is_ok());
        let green = json!({ "checks": [{ "name": "test", "result": "green" }] });
        assert!(base_matches_run("old", Some("red"), &green).into_result().is_err());
        assert!(base_matches_run("old", Some("not_run"), &green).into_result().is_ok());
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

    // The tests a table names that are not on the branch are each said, with the ids that are no condition and the
    // conditions with no row, in one call; a row that is not of kind tested names no test to look for.
    #[test]
    fn every_test_a_table_of_conditions_names_that_is_not_on_the_branch_is_said_with_its_ids() {
        let wanted = ids(&["A1", "A2", "A3"]);
        let rows = [
            json!({ "id": "A1", "how": "tested", "tests": ["on_branch", "gone_one", "gone_two"] }),
            json!({ "id": "X9", "how": "tested", "tests": ["on_branch"] }),
            json!({ "id": "A2", "how": "report_answer", "where": "gone_three" }),
        ];
        let on_branch = |t: &str| {
            if t.starts_with("gone") {
                Err(format!("the test {t} is not on the branch"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            said(conditions_problems(&wanted, &rows, false, on_branch, &[], found)),
            [
                "X9 is not an acceptance condition",
                "no row for acceptance condition A3",
                "the test gone_one is not on the branch",
                "the test gone_two is not on the branch"
            ]
        );
        // With no condition in force, only that is said: their tests are not looked for.
        assert_eq!(
            said(conditions_problems(&[], &rows, false, on_branch, &[], found)),
            [NO_ACCEPTANCE]
        );
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
        let ok = |_: &str| -> Result<(), String> { Ok(()) };
        let good = [
            json!({ "id": "A1", "how": "ci_job", "jobs": ["lint"], "steps": ["npm run check:ui"] }),
            json!({ "id": "A2", "how": "ci_job", "jobs": ["check", "test 1 of 3"], "steps": [] }),
            json!({ "id": "A3", "how": "read_diff", "where": "gone_file", "what": "gone_text" }),
            json!({ "id": "A4", "how": "ci_job", "jobs": ["check"] }),
        ];
        assert!(conditions_problems(&wanted, &good, false, ok, &ci, found).is_empty());
        // Jobs that were never run, steps that are in no file, and a condition left without a row, together.
        let bad = [
            json!({ "id": "A1", "how": "ci_job", "jobs": ["lint", "build"], "steps": ["gone one", "npm run check:ui"] }),
            json!({ "id": "A2", "how": "ci_job", "jobs": ["build"], "steps": ["gone one", " ", "a\nb"] }),
            json!({ "id": "X9", "how": "tested", "tests": ["gone_test"] }),
        ];
        let on_branch = |t: &str| {
            if t.starts_with("gone") {
                Err(format!("the test {t} is not on the branch"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            said(conditions_problems(&wanted, &bad, false, on_branch, &ci, found)),
            [
                "X9 is not an acceptance condition",
                "no row for acceptance conditions A3, A4",
                "the test gone_test is not on the branch",
                "the job build is not a check of any CI run of this work; the checks recorded are: lint, test 1 of 3, check",
                "the step gone one is not in any file of the branch",
                "a step names one command, written as one line of text"
            ]
        );
        // No CI run recorded: any job name would pass, so it is one problem and not one for each job.
        assert_eq!(
            said(conditions_problems(&wanted, &good, false, ok, &[], found)),
            ["no CI run of this work has recorded its checks, so a ci_job row cannot name a job"]
        );
        // A recheck of a row that changed is judged by what it names alone.
        let one = [json!({ "id": "A2", "how": "ci_job", "jobs": ["nothing"], "steps": [] })];
        assert_eq!(
            said(conditions_problems(&wanted, &one, true, ok, &ci, found)),
            [
                "the job nothing is not a check of any CI run of this work; the checks recorded are: lint, test 1 of 3, check"
            ]
        );
    }

    // A ci_job row's jobs are read from what the tools recorded: the fast CI keeps its checks under `push`, the full CI
    // its first run's under `first` and a run again its list as it is; a run that failed or was refused has none.
    #[test]
    fn the_names_of_the_checks_of_every_recorded_ci_run_are_read_from_each_tools_record() {
        let check = |n: &str| json!({ "name": n, "result": "green", "run": "u" });
        let fast =
            json!({ "push": { "verdict": "pushed", "head": "h", "checks": [check("lint"), check("test 1 of 3")] } });
        let first = json!({ "first": [check("lint"), check("full only")] });
        let again = json!([check("again only")]);
        let refused = json!({ "push": { "verdict": "refused", "why": "w" } });
        let failed = json!({ "failed": { "kind": "other", "what": "w" } });
        assert_eq!(
            check_names(&[fast, first, again, refused, failed]),
            ["lint", "test 1 of 3", "full only", "again only"]
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

    // Item #217: a result a row lacks is said once, by missing_results, and not also judged as a wrong one (a guard
    // test with no result before the change "does not pass").
    #[test]
    fn a_test_row_with_no_result_before_the_change_is_not_told_it_does_not_pass() {
        let rows = vec![
            json!({ "name": "g", "by": "engineer", "on_head": "green" }),
            json!({ "name": "n", "by": "engineer", "on_head": "green", "on_base": null }),
        ];
        assert!(result_problems(&rows, false).is_empty());
        assert!(result_problems(&rows, true).is_empty());
        assert_eq!(said(missing_results(&rows)).len(), 2);
    }

    // Item #217, R241-28: a test is written once, in the test changes, and the results its row lacks are said, each: a
    // design with no adversarial testing has the engineer run what it adds or extends.
    #[test]
    fn every_result_a_test_row_lacks_is_said() {
        let rows = vec![
            json!({ "name": "both", "by": "engineer" }),
            json!({ "name": "base_only", "by": "engineer", "on_head": "green" }),
            json!({ "name": "head_only", "by": "engineer", "on_base": "red" }),
            json!({ "name": "none", "by": "engineer", "on_head": null, "on_base": null }),
            json!({ "name": "fine", "by": "engineer", "on_head": "green", "on_base": "red" }),
        ];
        assert_eq!(
            said(missing_results(&rows)),
            [
                "both has no result with the change: write it as on_head in its row of the test changes",
                "both has no result before the change: write it as on_base in its row of the test changes",
                "base_only has no result before the change: write it as on_base in its row of the test changes",
                "head_only has no result with the change: write it as on_head in its row of the test changes",
                "none has no result with the change: write it as on_head in its row of the test changes",
                "none has no result before the change: write it as on_base in its row of the test changes",
            ]
        );
        assert!(missing_results(&[]).is_empty());
    }

    // Item #217, R241-41: the forms say which run of ci_before counts and what to do when a file of the run changed after
    // it in the words the engine's retry text uses, so that the two do not part.
    #[test]
    fn the_forms_and_the_retry_text_say_the_same_of_which_run_counts() {
        let flow: Value = serde_json::from_str(include_str!("../../../flow/flow.json")).unwrap();
        let mut texts = vec![(
            "the test changes".to_string(),
            flow["shapes"]["test_changes"].to_string(),
        )];
        for id in ["I1", "T6", "T1"] {
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
        assert!(missing_results(&tables.new).is_empty());
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
        let green = json!({ "checks": [{ "name": "test", "result": "green" }], "tests": [{ "test": "one", "result": "passed" }] });
        let red = json!({
            "checks": [{ "name": "test", "result": "red", "log_tail": "test a::two ... FAILED\n" }],
            "tests": [{ "test": "two", "result": "failed" }],
        });
        let rows = vec![json!({ "name": "one", "on_base": "red" })];
        let none_changed = |_: &BeforeRun| Vec::new();
        // Said red, and the run where every check is green: said, when no file changed after it.
        let runs = vec![before_run(green.clone(), "aaaaaaaaaaaa", &["t/one.rs"])];
        assert_eq!(
            said(results_vs_runs(&rows, None, &runs, none_changed)),
            ["one is said to be red before the change, and every check there is green"]
        );
        // A file changed after the run: the test is to be run again, and what the run says of it is not said.
        let changed = |run: &BeforeRun| run.files.clone();
        assert_eq!(
            said(results_vs_runs(&rows, None, &runs, changed)),
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
            said(results_vs_runs(&rows, None, &runs, |_| Vec::new())),
            [
                "one is said to be red before the change, and every check there is green",
                "two is said to be green before the change, and a red check's log names it",
            ]
        );
        // Only the file of the run a test is reported in decides: `two.rs` changed after the later run and `one.rs`
        // did not, so `one` is compared with its run and `two` is to be run again.
        let after_later = |run: &BeforeRun| -> Vec<String> {
            run.files.iter().filter(|f| f.as_str() == "t/two.rs").cloned().collect()
        };
        let said_both = said(results_vs_runs(&rows, None, &runs, after_later));
        assert_eq!(said_both.len(), 2, "{said_both:?}");
        assert!(
            said_both[0].starts_with("one is said to be red before the change"),
            "{said_both:?}"
        );
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
            said(results_vs_runs(&rows[..1], None, &[lost], |r| r.files.clone()))[0]
                .contains("a commit the worktree lacks")
        );
        // With no run, nothing is said.
        assert!(results_vs_runs(&rows, None, &[], |_| Vec::new()).is_empty());
    }

    #[test]
    fn every_test_whose_result_does_not_match_the_runs_is_said_with_everything_wrong_with_it() {
        let green = json!({ "checks": [{ "name": "test", "result": "green" }] });
        let red = json!({
            "checks": [{ "name": "test", "result": "red", "log_tail": "test a::two ... FAILED\ntest a::one ... ok\n" }],
        });
        let rows = vec![
            json!({ "name": "one", "on_head": "red", "on_base": "red" }),
            json!({ "name": "two", "on_head": "green", "on_base": "green" }),
            json!({ "name": "three", "on_head": "green", "on_base": "cannot_run" }),
        ];
        // On a head where every check is green: `one` is not red there, and before the change nothing is red either.
        let before = |ran: &Value| vec![before_run(ran.clone(), "aaaaaaaaaaaa", &["t/x.rs"])];
        assert_eq!(
            said(results_vs_runs(&rows, Some(&green), &before(&green), |_| Vec::new())),
            [
                "one is said to be red on the head, and every check there is green",
                "one is said to be red before the change, and every check there is green",
            ]
        );
        // On a head where a red check's log names `two`, and before the change the same.
        assert_eq!(
            said(results_vs_runs(&rows, Some(&red), &before(&red), |_| Vec::new())),
            [
                "two is said to be green on the head, and a red check's log names it",
                "two is said to be green before the change, and a red check's log names it",
            ]
        );
        // With no run to look at, nothing is said.
        assert!(results_vs_runs(&rows, None, &[], |_| Vec::new()).is_empty());
    }

    // A test said red while every check is green, and also said so while ci_before reports it not run: two things
    // wrong, said together.
    #[test]
    fn a_result_before_the_change_with_two_things_wrong_is_said_with_both() {
        let ran = json!({
            "checks": [{ "name": "test", "result": "green" }],
            "tests": [{ "test": "later", "result": "not_run" }]
        });
        let both = said(base_matches_run("later", Some("red"), &ran));
        assert_eq!(both.len(), 2, "{both:?}");
        assert!(both[0].contains("every check there is green"), "{both:?}");
        assert!(both[1].contains("reports it not run"), "{both:?}");
        assert!(base_matches_run("later", Some("cannot_run"), &ran).is_empty());
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
    fn the_blind_reviews_findings_dropped_and_the_withdrawals_it_did_not_make_are_each_said_once_with_all() {
        let blind = ids(&["b1", "b2", "b3", "b4", "b2"]);
        assert_eq!(
            said(blind_problems(&blind, &ids(&["b1"]), &ids(&["b2", "w9", "w8", "w9"]))),
            [
                "w9, w8 are withdrawn, and the blind review handed in no such findings",
                "the blind review's findings b3, b4 are neither carried into a column nor withdrawn"
            ]
        );
        assert_eq!(
            said(blind_problems(&blind, &ids(&["b1", "b2", "b3"]), &ids(&["b4", "w9"]))),
            ["w9 is withdrawn, and the blind review handed in no such finding"]
        );
        assert_eq!(
            said(blind_problems(&blind, &ids(&["b1", "b2", "b3"]), &[])),
            ["the blind review's finding b4 is neither carried into a column nor withdrawn"]
        );
        assert!(blind_problems(&blind, &ids(&["b1", "b2", "b3"]), &ids(&["b4"])).is_empty());
    }

    #[test]
    fn every_column_the_engineer_could_not_verify_either_and_the_review_left_open_is_said() {
        let both: BTreeSet<String> = ids(&["c2", "c4"]).into_iter().collect();
        let columns = vec![json!({ "id": "c1" }), json!({ "id": "c2" }), json!({ "id": "c4" })];
        let found = said(unverified_problems(&both, &columns));
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found[0].starts_with("column c2: the engineer could not verify it either; decide it now"),
            "{found:?}"
        );
        assert!(found[1].starts_with("column c4: "), "{found:?}");
        assert!(unverified_problems(&both, &[json!({ "id": "c1" })]).is_empty());
        assert!(unverified_problems(&BTreeSet::new(), &columns).is_empty());
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
