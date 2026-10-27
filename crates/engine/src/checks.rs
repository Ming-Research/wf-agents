//! The engine's checks of a return: `engine.checks` in the flow, each named by the cells that use it. A return that
//! fails one is sent back with the reason, before it takes a row.
//!
//! A check whose `at` lies in one of the forms every agent has (ask, answers, redirect) runs on returns of that form;
//! any other runs on the cell's own answer, whether or not the place is there, since some checks are about a place
//! left out.
//!
//! In this version some checks look at less than the flow says, because what they would compare is not recorded
//! yet: CI gives each job's result, not each test's, so test results are checked only against what the jobs show;
//! cited files are checked to exist, not to be in the diff; a table of findings is checked for one answer per id;
//! a red check said not to be caused by the change and a ruling said to be the advisor's are taken as written once
//! the advisor was asked; and a decision copied from a parent's design card is not compared with it.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use flow::{CompiledCell, CompiledGraph};
use serde_json::Value;

use crate::inputs::{answered, at_path, last_return};
use crate::work::{self, Item, Round, Work};
use crate::{Engine, form_of, git, values};

pub use db::DESIGN_CARD_RULING;

pub struct Ctx<'a> {
    pub e: &'a Engine,
    pub w: &'a Work,
    pub g: &'a CompiledGraph,
    pub cell: &'a CompiledCell,
    pub item: &'a Item,
    pub project: &'a config::Project,
    pub output: &'a Value,
    pub unwrapped: Value,
    /// The worktree's HEAD when the round started.
    pub head_start: Option<String>,
    pub columns: &'a [(String, String)],
    /// The rows of the cell's numbered table as this round was given them, each with its number.
    pub table: &'a [Value],
    pub interjections: &'a [Value],
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

/// Runs every check of the cell that applies to the return; gives every reason it fails.
pub fn run(c: &Ctx<'_>) -> Result<(), String> {
    let form = form_of(c.g, c.cell, c.output);
    let mut problems: Vec<String> = Vec::new();
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
        if let Err(why) = one(c, &ch.check, ch.at.as_deref(), value.as_ref()) {
            let p = format!("{} ({}): {why}", ch.check, ch.at.as_deref().unwrap_or("the return"));
            if !problems.contains(&p) {
                problems.push(p);
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn fail<T>(why: impl Into<String>) -> Result<T, String> {
    Err(why.into())
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

impl Ctx<'_> {
    fn wt(&self) -> Result<&Path, String> {
        self.w
            .worktree
            .as_deref()
            .ok_or_else(|| "the work has no worktree".to_string())
    }

    fn fork(&self) -> Result<String, String> {
        git::fork_point(self.wt()?, &self.project.main)
    }

    /// The files the branch changed against main, committed.
    fn branch_changes(&self) -> Result<Vec<String>, String> {
        git::changed(self.wt()?, &self.fork()?, "HEAD")
    }

    /// The files the work itself changed from one commit to another, apart from what merging main brought in.
    fn own(&self, from: &str, to: &str) -> Result<Vec<String>, String> {
        git::own_changes(self.wt()?, &self.project.main, from, to)
    }

    fn design(&self, files: &[String]) -> Vec<String> {
        files
            .iter()
            .filter(|f| git::under(f, &self.project.design_paths))
            .cloned()
            .collect()
    }

    /// Where the attempts of this cell that came just before this round, each sent back, cut off or failed, began: the
    /// head at the start of the earliest of them; none when the round before at this cell was not one.
    fn attempts_start(&self) -> Result<Option<String>, String> {
        let rounds = work::rounds_at(self.e.db.conn(), self.w.id, &self.cell.id)?;
        Ok(rounds
            .iter()
            .filter(|r| r.ended_at.is_some())
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
            .query_row("SELECT acceptance FROM items WHERE id = ?1", [self.item.id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        match work::json_col(locked) {
            Some(Value::Array(xs)) => Ok(xs),
            _ => Ok(items(self.last("2a", "acceptance")?.as_ref())),
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
}

/// The kinds of question only the owner may be asked, from the flow built into the engine.
fn owner_only() -> &'static BTreeSet<String> {
    static KINDS: OnceLock<BTreeSet<String>> = OnceLock::new();
    KINDS.get_or_init(|| {
        let f: Value = serde_json::from_str(flow::FLOW).unwrap_or(Value::Null);
        let mut out = BTreeSet::new();
        for g in f["flows"].as_array().cloned().unwrap_or_default() {
            for grp in g["askKinds"].as_array().cloned().unwrap_or_default() {
                if grp["only"] == "owner" {
                    out.extend(
                        grp["kinds"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .iter()
                            .filter_map(|k| k[0].as_str().map(str::to_string)),
                    );
                }
            }
        }
        out
    })
}

fn one(c: &Ctx<'_>, check: &str, at: Option<&str>, v: Option<&Value>) -> Result<(), String> {
    match check {
        "research_only" => {
            let wt = c.wt()?;
            if !git::is_clean(wt)? {
                return fail("the worktree has uncommitted changes");
            }
            let Some(start) = &c.head_start else { return Ok(()) };
            let outside: Vec<String> = c
                .own(start, "HEAD")?
                .into_iter()
                .filter(|f| !f.starts_with("research/"))
                .collect();
            if outside.is_empty() {
                Ok(())
            } else {
                fail(format!(
                    "this round changed files outside research/: {}",
                    outside.join(", ")
                ))
            }
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
                let not_main = format!("^origin/{}", c.project.main);
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
            let measured = v.and_then(|m| m["verdict"].as_str()) == Some("measured");
            let ran = c.ci_runs("measure")?.iter().any(|r| r["push"]["verdict"] == "ran");
            match (measured, ran) {
                (true, false) => fail("measured, but this work has no measure run"),
                (false, true) => fail("not_measured, but this work has measure runs"),
                _ => Ok(()),
            }
        }
        "design_card_matches_tree" => {
            let Some(card) = v else { return Ok(()) };
            let wt = c.wt()?;
            let changed = c.branch_changes()?;
            let tree = c.design(&changed);
            if c.item.deliver == "report" {
                return fail("a report can only answer unchanged");
            }
            if tree.is_empty() {
                return fail("the design paths did not change on the branch");
            }
            let dirty = c.design(&git::changed_since(wt, "HEAD")?);
            if !dirty.is_empty() {
                return fail(format!(
                    "changes to the design paths are not committed: {}",
                    dirty.join(", ")
                ));
            }
            let in_node = |node: &str, f: &str| {
                let n = node.trim_end_matches('/');
                f == n || f.starts_with(&format!("{n}/"))
            };
            let mut nodes = Vec::new();
            for d in items(Some(card)) {
                if d.get("carried_by").is_some() {
                    continue;
                }
                let node = d["node"].as_str().unwrap_or_default().to_string();
                if !tree.iter().any(|f| in_node(&node, f)) {
                    return fail(format!(
                        "the design card has a decision in {node}, which did not change on the branch"
                    ));
                }
                nodes.push(node);
            }
            if let Some(f) = tree.iter().find(|f| !nodes.iter().any(|n| in_node(n, f))) {
                return fail(format!(
                    "{f} changed on the branch, and no decision on the design card is in it"
                ));
            }
            if at.is_some_and(|a| a.contains("design_paths_only")) {
                let outside: Vec<&String> = changed.iter().filter(|f| !tree.contains(f)).collect();
                if !outside.is_empty() {
                    return fail(format!(
                        "design_paths_only, but files outside the design paths changed: {outside:?}"
                    ));
                }
            }
            Ok(())
        }
        "design_tree_check" => {
            if at.is_some_and(|a| a == "design_card") && v.is_none() {
                return Ok(());
            }
            let Some(cmd) = &c.project.design_check else {
                return Ok(());
            };
            let wt = c.wt()?;
            if c.design(&c.branch_changes()?).is_empty() {
                return Ok(());
            }
            let fork = c.fork()?;
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("{cmd} {fork}"))
                .current_dir(wt)
                .output()
                .map_err(|e| format!("{cmd}: {e}"))?;
            if out.status.success() {
                Ok(())
            } else {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                fail(format!(
                    "the design tree check `{cmd} {fork}` fails:\n{}",
                    crate::hub::tail(&text, 40)
                ))
            }
        }
        "acceptance_matches_locked" => {
            let given = items(v);
            if c.item.deliver == "change" && given.iter().any(|a| a["by"] == "report_answer") {
                return fail("a work that delivers a change has no report_answer condition");
            }
            let locked: Option<String> =
                c.e.db
                    .conn()
                    .query_row("SELECT acceptance FROM items WHERE id = ?1", [c.item.id], |r| r.get(0))
                    .map_err(|e| e.to_string())?;
            match work::json_col(locked) {
                Some(l) if l != Value::Array(given) => fail(
                    "the acceptance conditions are locked and must be handed in word for word; to change them, ask the advisor with the kind acceptance_change first",
                ),
                _ => Ok(()),
            }
        }
        "chosen_matches_ruling" => {
            let chosen = v.and_then(Value::as_str).unwrap_or_default();
            let cards = work::query_json(
                c.e.db.conn(),
                "SELECT c.body, c.rulings FROM design_cards c JOIN works w ON w.id = c.work WHERE w.item = ?1
                 ORDER BY c.id DESC",
                &[&c.item.id],
                100,
            )?;
            // The owner's last ruling, on the latest design card the owner ruled on.
            let Some((body, ruling)) = cards.iter().find_map(|x| {
                items(Some(&x["rulings"]))
                    .into_iter()
                    .rev()
                    .find(|r| r["by"] == "owner")
                    .map(|r| (items(Some(&x["body"])), r))
            }) else {
                return Ok(());
            };
            let verdicts = db::ruling_verdicts(&ruling);
            // A ruling on the card as a whole, made before rulings were per decision: its approach is the design's.
            if let [(0, _, Some(o))] = verdicts.as_slice() {
                return if o != chosen {
                    fail(format!(
                        "the owner chose \"{o}\" in a ruling on the design card; that is the approach chosen"
                    ))
                } else {
                    Ok(())
                };
            }
            // Each decision the owner chose another approach for chooses it on the new design card, in the same node.
            let card = items(at_path(&c.unwrapped, "design_tree.design_card").as_ref());
            for (k, _, o) in &verdicts {
                let Some(o) = o else { continue };
                let node = body
                    .get((*k - 1).max(0) as usize)
                    .and_then(|d| d["node"].as_str())
                    .unwrap_or_default();
                if !card.iter().any(|d| d["node"] == node && d["chosen"] == o.as_str()) {
                    return fail(format!(
                        "the owner chose \"{o}\" for decision {k} of the design card ({node}): the new design card's \
                         decision in that node chooses it"
                    ));
                }
            }
            Ok(())
        }
        "head_pushed_and_green" => {
            let wt = c.wt()?;
            let head = v.and_then(Value::as_str).unwrap_or_default();
            let Some(sha) = git::resolve(wt, head) else {
                return fail(format!("there is no commit {head}"));
            };
            if sha != git::head(wt)? {
                return fail("the head is not the worktree's HEAD");
            }
            if !git::is_clean(wt)? {
                return fail("the worktree has uncommitted changes");
            }
            if c.pushed_head()?.as_deref() != Some(sha.as_str()) {
                return fail("the head was not pushed with ci");
            }
            let runs = c.ci_runs("ci")?;
            let Some(run) = runs.iter().find(|r| r["push"]["head"] == sha.as_str()) else {
                return fail("no fast CI run on the head");
            };
            let red: Vec<String> = items(run["push"].get("checks"))
                .iter()
                .filter(|x| x["result"] != "green")
                .map(|x| x["name"].as_str().unwrap_or_default().to_string())
                .collect();
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
                    let outside: Vec<&String> = changed
                        .iter()
                        .filter(|f| !git::under(f, &c.project.design_paths))
                        .collect();
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
            let ids: Vec<String> = rows
                .iter()
                .filter_map(|r| r["id"].as_str().map(str::to_string))
                .collect();
            let wanted: Vec<String> = c
                .acceptance()?
                .iter()
                .filter_map(|a| a["id"].as_str().map(str::to_string))
                .collect();
            if wanted.is_empty() && !ids.is_empty() {
                return fail(
                    "the item has no acceptance conditions in force; ask the advisor for them with the kind acceptance_change",
                );
            }
            if let Some(x) = ids.iter().find(|i| !wanted.contains(i)) {
                return fail(format!("{x} is not an acceptance condition"));
            }
            let recheck = c.cell.id == "6" && !c.accepted("6")?.is_empty();
            if !recheck && let Some(x) = wanted.iter().find(|i| !ids.contains(i)) {
                return fail(format!("no row for acceptance condition {x}"));
            }
            let wt = c.wt()?;
            for r in rows.iter().filter(|r| r["how"] == "tested") {
                for t in strs(r.get("tests")) {
                    let word = t.rsplit([':', '/', '.']).next().unwrap_or(&t).trim().to_string();
                    if !word.is_empty() && git::git(wt, &["grep", "-q", "-F", "-w", &word, "HEAD"]).is_err() {
                        return fail(format!("the test {t} is not on the branch"));
                    }
                }
            }
            Ok(())
        }
        "new_test_results" | "guard_test_results" => {
            if c.last("3f", "adversarial")?.is_some_and(|a| a["verdict"] == "yes") {
                return Ok(());
            }
            let new = check == "new_test_results";
            for t in items(v) {
                let name = t["name"].as_str().unwrap_or_default();
                let (head, base) = (
                    t["on_head"].as_str().unwrap_or_default(),
                    t["on_base"].as_str().unwrap_or_default(),
                );
                if head == "red" {
                    return fail(format!("{name} is red on the head: fix it in this round"));
                }
                if new && base == "green" {
                    return fail(format!(
                        "{name} passes before the change: it does not test new behaviour; fix it in this round"
                    ));
                }
                if !new && base != "green" {
                    return fail(format!(
                        "{name} does not pass before the change: it does not guard old behaviour; fix it in this round"
                    ));
                }
            }
            test_results_match_runs(c, v)?;
            let changes = items(c.unwrapped.get("test_changes"));
            let listed: Vec<String> = strs(at_path(&Value::Array(items(v)), "name").as_ref());
            let kind = if new { "new_behaviour" } else { "old_behaviour" };
            for x in changes.iter().filter(|x| x["change"] == "added" && x["kind"] == kind) {
                let name = x["test"].as_str().unwrap_or_default();
                if !listed.iter().any(|l| l == name) {
                    return fail(format!("{name} was added as {kind}, and is not in its table"));
                }
            }
            Ok(())
        }
        "adversarial_test_results" => {
            test_results_match_runs(c, v)?;
            if let Some(h) = c.unwrapped.pointer("/wrote/head").and_then(Value::as_str)
                && c.unwrapped["wrote"]["verdict"] == "wrote"
                && c.pushed_head()?.as_deref() != git::resolve(c.wt()?, h).as_deref()
            {
                return fail("tests were written, and the head was not pushed with ci");
            }
            Ok(())
        }
        "test_kinds_match" => {
            let table = if at.is_some_and(|a| a.starts_with("new_tests")) {
                "new_tests"
            } else {
                "guard_tests"
            };
            let rows = items(c.unwrapped.get(table));
            let mut kinds: Vec<(String, String)> = Vec::new();
            for cell in ["4", "8"] {
                // A return of a flow version before the answer was flat holds the test changes under end.
                let changes = match c.last(cell, "test_changes")? {
                    Some(x) => Some(x),
                    None => c.last(cell, "end.test_changes")?,
                };
                for x in items(changes.as_ref()) {
                    if x["change"] == "added" {
                        kinds.push((
                            x["test"].as_str().unwrap_or_default().into(),
                            x["kind"].as_str().unwrap_or_default().into(),
                        ));
                    }
                }
            }
            let want = if table == "new_tests" {
                "new_behaviour"
            } else {
                "old_behaviour"
            };
            for r in rows {
                let name = r["name"].as_str().unwrap_or_default();
                if r["by"] == "engineer"
                    && let Some((_, k)) = kinds.iter().find(|(n, _)| n == name)
                    && k != want
                {
                    return fail(format!(
                        "{name} was added by the engineer as {k}; it belongs in the other table"
                    ));
                }
                if r["by"] == "adversarial" {
                    let base = r["on_base"].as_str().unwrap_or_default();
                    let wrong = if table == "new_tests" {
                        base == "green"
                    } else {
                        base != "green"
                    };
                    if wrong {
                        return fail(format!(
                            "{name}: its result before the change does not fit its kind; fix it in this round and run it again"
                        ));
                    }
                }
            }
            Ok(())
        }
        "design_card_when_tree_changed" => {
            let card = values::design_card(c.e, c.w)?;
            let base = match card.and_then(|k| k.head) {
                Some(h) => h,
                None => c.fork()?,
            };
            let tree = c.design(&c.own(&base, "HEAD")?);
            match (tree.is_empty(), v.is_some()) {
                (false, false) => fail(format!(
                    "the design paths changed since the last design card ({}): hand in the new design card",
                    tree.join(", ")
                )),
                (true, true) => fail("the design paths did not change since the last design card: hand in none"),
                _ => Ok(()),
            }
        }
        "design_card_changed_when_replacing" => {
            // The design card it says what changed of sits beside it, under the card's own name.
            let card_at = at.map(|a| a.trim_end_matches("_changed")).unwrap_or_default();
            let card = at_path(&c.unwrapped, card_at);
            let said = v.is_some_and(|x| x.as_str().is_none_or(|s| !s.trim().is_empty()));
            let before: i64 =
                c.e.db
                    .conn()
                    .query_row("SELECT count(*) FROM design_cards WHERE work = ?1", [c.w.id], |r| {
                        r.get(0)
                    })
                    .map_err(|e| e.to_string())?;
            match (card.is_some(), said, before > 0) {
                (false, true, _) => fail("what changed is said only with a design card"),
                (true, false, true) => fail(
                    "this design card replaces this work's earlier design card: say in design_card_changed what \
                     changed from it and why",
                ),
                (true, true, false) => {
                    fail("this is the work's first design card and replaces none: leave design_card_changed out")
                }
                _ => Ok(()),
            }
        }
        "tests_only" => {
            let Some(from) = c.w.pushed_head.clone() else {
                return Ok(());
            };
            let head = v.and_then(Value::as_str).unwrap_or("HEAD");
            let outside: Vec<String> = c
                .own(&from, head)?
                .into_iter()
                .filter(|f| !git::under(f, &c.project.test_paths))
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
            let Some(last6) = c.accepted("6")?.first().map(|r| r.id) else {
                return Ok(());
            };
            let tests = strs(v);
            for fix in c.accepted("8")?.into_iter().filter(|r| r.id > last6) {
                let table = table_of(&fix);
                for row in items(fix.output.as_ref().and_then(|o| o[&c.g.within].get("rows"))) {
                    let id = known_as(&table, row["id"].as_str().unwrap_or_default());
                    if matches!(row["answer"].as_str(), Some("test_wrong" | "advisor_ruled"))
                        && c.accepted("6")?.first().is_some_and(|r| mentions(r, &id))
                        && !tests.contains(&id)
                    {
                        return fail(format!(
                            "no row for {id}, which the engineer answered {}",
                            row["answer"]
                        ));
                    }
                }
            }
            Ok(())
        }
        "no_insisting_after_ruling" => {
            let ruled = ruled_ids(c)?;
            for r in items(v) {
                let id = r["fid"].as_str().or(r["test"].as_str()).unwrap_or_default();
                if ruled.contains(id) {
                    return fail(format!("{id} was ruled by the advisor; it is not insisted on"));
                }
            }
            if c.cell.id == "5a" {
                for r in items(c.unwrapped.get("previous"))
                    .iter()
                    .filter(|r| r["verdict"] == "not_fixed")
                {
                    let id = r["fid"].as_str().unwrap_or_default();
                    if ruled.contains(id) {
                        return fail(format!("{id} was ruled by the advisor; it is not marked not_fixed"));
                    }
                }
            }
            Ok(())
        }
        "fact_basis_ran" => {
            let bases: Vec<Value> = items(v)
                .iter()
                .map(|x| x.get("basis").cloned().unwrap_or_else(|| x.clone()))
                .collect();
            for b in bases.iter().filter(|b| b["kind"] == "fact") {
                let run = b["run"].as_str().unwrap_or_default();
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
                let check = b["check"].as_str().unwrap_or_default();
                if linked == 0 && !c.handed_in(check)? {
                    return fail(format!(
                        "the run {run:?} is not one the engine recorded for this work: give the link from the tool's result, or, for a gh command or a query, write it in check as you handed it in"
                    ));
                }
            }
            Ok(())
        }
        "conclusions_cover_questions" => {
            let questions = c.acceptance()?.iter().filter(|a| a["by"] == "report_answer").count();
            let given = items(v).len();
            if c.item.deliver == "report" {
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
            let want: Vec<&String> = c.columns.iter().map(|(id, _)| id).collect();
            let mut seen = BTreeSet::new();
            for id in &ids {
                if !want.contains(&id) {
                    return fail(format!("there is no column {id}"));
                }
                if !seen.insert(id) {
                    return fail(format!("column {id} is answered twice"));
                }
            }
            if let Some(m) = want.iter().find(|w| !seen.contains(**w)) {
                return fail(format!("column {m} is not answered"));
            }
            Ok(())
        }
        "evidence_holds" => {
            let wt = c.wt()?;
            for ev in items(v) {
                for f in strs(ev.get("files")) {
                    let p = f
                        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')))
                        .next()
                        .unwrap_or_default()
                        .trim_end_matches('.');
                    let looks = (p.contains('/') || p.contains('.')) && !p.starts_with('/');
                    if looks && !wt.join(p).exists() {
                        return fail(format!("{p} is not in the worktree"));
                    }
                }
                for n in strs(ev.get("nodes")) {
                    if !wt.join(n.trim_end_matches('/')).exists() {
                        return fail(format!("the node {n} does not exist"));
                    }
                }
            }
            Ok(())
        }
        "previous_findings_answered" => {
            let Some(last) = c.accepted("5a")?.into_iter().next() else {
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
            let given = strs(v);
            match due.iter().find(|d| !given.contains(d)) {
                Some(d) => fail(format!("no row for {d}, a finding of the last review")),
                None => Ok(()),
            }
        }
        "no_adversarial_when_due" => {
            let first = c.accepted("5a")?.is_empty();
            let chose = c.last("3f", "adversarial")?.is_some_and(|a| a["verdict"] == "yes");
            let ran = !c.accepted("6")?.is_empty();
            let kind = values::value(c.e, c.w, c.g, c.item, "work")?;
            let due = c.item.deliver == "change" && kind == "implementation" && !chose && !ran;
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
            let Some(blind) = c.accepted("5")?.into_iter().next() else {
                return Ok(());
            };
            if c.accepted("5a")?.first().is_some_and(|r| r.id > blind.id) {
                return Ok(());
            }
            let out = blind.output.map(|o| c.g.unwrap(&o)).unwrap_or(Value::Null);
            let blind_ids = strs(at_path(&out, "findings.fid").as_ref());
            let carried = strs(at_path(&c.unwrapped, "columns.findings.fid").as_ref());
            let withdrawn = strs(v);
            if let Some(w) = withdrawn.iter().find(|w| !blind_ids.contains(w)) {
                return fail(format!(
                    "{w} is withdrawn, and the blind review handed in no such finding"
                ));
            }
            match blind_ids
                .iter()
                .find(|b| !carried.contains(b) && !withdrawn.contains(b))
            {
                Some(b) => fail(format!(
                    "the blind review's finding {b} is neither carried into a column nor withdrawn"
                )),
                None => Ok(()),
            }
        }
        "table_answered" => {
            let mut seen = BTreeSet::new();
            for r in items(v) {
                let id = r["id"].as_str().unwrap_or_default().to_string();
                if !seen.insert(id.clone()) {
                    return fail(format!("{id} is answered twice"));
                }
            }
            // A round given a numbered table answers each of its rows by its number, and nothing else.
            if !c.table.is_empty() {
                let rows: Vec<&str> = c.table.iter().filter_map(|r| r["row"].as_str()).collect();
                if let Some(x) = seen.iter().find(|x| !rows.contains(&x.as_str())) {
                    return fail(format!("{x} is not a row of the table: answer each row by its number"));
                }
                let missing: Vec<&str> = rows.iter().copied().filter(|r| !seen.contains(*r)).collect();
                if !missing.is_empty() {
                    return fail(format!("no answer for {}", missing.join(", ")));
                }
            }
            Ok(())
        }
        "no_disagreeing_after_insist" => {
            let mut insisted: BTreeSet<String> = BTreeSet::new();
            for (cell, path) in [("5a", "previous"), ("6", "previous")] {
                for p in items(c.last(cell, path)?.as_ref())
                    .iter()
                    .filter(|p| p["verdict"] == "insist")
                {
                    insisted.extend(p["fid"].as_str().or(p["test"].as_str()).map(str::to_string));
                }
            }
            let rows = items(c.unwrapped.get("rows"));
            match rows
                .iter()
                .filter(|r| matches!(r["answer"].as_str(), Some("disagree" | "test_wrong")))
                .find_map(|r| {
                    r["id"]
                        .as_str()
                        .map(|id| known_as(c.table, id))
                        .filter(|k| insisted.contains(k))
                }) {
                Some(id) => fail(format!("{id} was insisted on; when neither side gives way, ask")),
                None => Ok(()),
            }
        }
        "not_caused_by_change" => Ok(()),
        "ruling_matches_advisor" => {
            if v.is_none() {
                return Ok(());
            }
            if c.accepted("12")?.is_empty() {
                fail("this work never asked the advisor")
            } else {
                Ok(())
            }
        }
        "owner_kinds_to_owner" => {
            let to = c.unwrapped["ask"]["to"].as_str().unwrap_or_default();
            let only = owner_only();
            match strs(v).iter().find(|k| only.contains(*k)) {
                Some(k) if to != "owner" => fail(format!("a point of the kind {k} is asked of the owner only")),
                _ => Ok(()),
            }
        }
        // A flow version stored before this check was named so calls it `card_disagreement_redirects`.
        "design_card_disagreement_redirects" | "card_disagreement_redirects" => {
            let disagreed = c.interjections.iter().any(|i| {
                let t = i["text"].as_str().unwrap_or_default();
                t.starts_with(DESIGN_CARD_RULING) && t.contains("disagree")
            });
            if disagreed && v.is_some() {
                fail("this round's interjections disagree with the design card: hand in redirect, not answers")
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
            let mut seen = BTreeSet::new();
            for to in strs(v) {
                if !speakers.contains(&to) {
                    return fail(format!("the {to} did not interject in this round"));
                }
                if !seen.insert(to.clone()) {
                    return fail(format!("two answers to the {to}"));
                }
            }
            Ok(())
        }
        other => fail(format!("no such engine check: {other}")),
    }
}

/// Whether a round's return names an id.
fn mentions(r: &Round, id: &str) -> bool {
    r.output
        .as_ref()
        .is_some_and(|o| o.to_string().contains(&format!("\"{id}\"")))
}

/// The ids the engineer answered advisor_ruled in the fixes since the last review.
fn ruled_ids(c: &Ctx<'_>) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    for fix in c.accepted("8")? {
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

/// A test said to be green on the head while every run on the head was red names it, or said to be red while every
/// check on the head was green, does not match the runs; the same for the code before the change.
fn test_results_match_runs(c: &Ctx<'_>, v: Option<&Value>) -> Result<(), String> {
    let pushed = c.pushed_head()?;
    let ci = c.ci_runs("ci")?;
    let on_head = ci.iter().find(|r| r["push"]["head"].as_str() == pushed.as_deref());
    let before = c
        .ci_runs("ci_before")?
        .into_iter()
        .find(|r| r["ran"]["verdict"] == "ran");
    let all_green = |checks: Option<&Value>| items(checks).iter().all(|x| x["result"] == "green");
    let names_red = |checks: Option<&Value>, name: &str| {
        items(checks)
            .iter()
            .any(|x| x["result"] == "red" && x["log_tail"].as_str().is_some_and(|l| names_failing(l, name)))
    };
    for t in items(v) {
        let name = t["name"].as_str().unwrap_or_default();
        if let Some(run) = on_head {
            let checks = run["push"].get("checks");
            match t["on_head"].as_str() {
                Some("red") if all_green(checks) => {
                    return fail(format!(
                        "{name} is said to be red on the head, and every check there is green"
                    ));
                }
                Some("green") if names_red(checks, name) => {
                    return fail(format!(
                        "{name} is said to be green on the head, and a red check's log names it"
                    ));
                }
                _ => {}
            }
        }
        if let Some(run) = &before {
            let checks = run["ran"].get("checks");
            match t["on_base"].as_str() {
                Some("red") if all_green(checks) => {
                    return fail(format!(
                        "{name} is said to be red before the change, and every check there is green"
                    ));
                }
                Some("green") if names_red(checks, name) => {
                    return fail(format!(
                        "{name} is said to be green before the change, and a red check's log names it"
                    ));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_names_a_test_failing_only_on_a_line_that_says_so() {
        let log = "test a::passes ... ok\ntest a::breaks ... FAILED\n---- a::panics stdout ----\ntest a::fails_cleanly ... ok";
        assert!(!names_failing(log, "passes"));
        assert!(!names_failing(log, "fails_cleanly"));
        assert!(names_failing(log, "breaks"));
        assert!(names_failing(log, "panics"));
        assert!(!names_failing(log, ""));
    }
}
