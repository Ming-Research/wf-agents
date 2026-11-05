//! Checking what only real runs can show, once a work's change took effect: an acceptance condition written `in_effect`
//! says what carries the change (`when`), the tool that checks it, what to run, what result means it holds and what
//! evidence it waits for. A merged work with such a condition does not end after what it learned: it waits, holding no
//! slot (cell `W1`), until every carrier holds its merge commit, then a round in a new session (cell `W2`) runs each
//! condition's command. Where the evidence is not there yet the round says how long to wait and the work waits again,
//! as many times as the project's limit; the wait for the change to take effect has a limit too. What does not hold,
//! cannot be judged or stands unverified at a limit goes to the project's architect as a report.
//!
//! The definition of in effect is one for every project: what the condition names as its carrier holds the merge
//! commit, main as the work merges, the installed program when the commit it was built from has the merge commit among
//! its ancestors. Only a merged work comes here: a change pushed straight to main goes through no cell of the flow, so
//! nothing here checks it (item #275 takes that part).

use std::collections::BTreeMap;
use std::path::Path;

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Matter, queue_matter};
use flow::CompiledGraph;
use serde_json::{Value, json};

use crate::actions::Ctx;
use crate::checks::{Problems, id_misses};
use crate::inputs::answered;
use crate::work::{self, Item, Work};
use crate::{ENGINEER, Engine, MERGE, git};

/// The engine's input that says how the change took effect.
pub const INPUT: &str = "effect";

/// The kind of an acceptance condition this module is about, and of a row of the conditions met.
const KIND: &str = "in_effect";

/// The carrier of a condition that the installed program holds its change, which only the project the engine is built
/// from can name.
const INSTALLED: &str = "installed";

/// The cell that waits for the change to take effect, and the one that checks each condition with a real run.
const WAIT: &str = "W1";
const CHECK: &str = "W2";

/// The row of the cell that checks by which the work goes back to wait, which the limit of looks counts.
const AGAIN: &str = "again";

/// The rows that report a condition unverified at a limit: the wait for the change to take effect, and the looks.
const NEVER_IN_EFFECT: &str = "never_in_effect";
const PAST_LOOKS: &str = "past_looks";

/// The most minutes a look may say to wait before the next: two days, so that with the project's limit of looks a work
/// waits a bounded time, and a work that needs a work after it run to its end has that well within the limit.
const AGAIN_AFTER_MOST: i64 = 2880;

/// The commit a build not made in a git checkout says it was built from.
const NOT_KNOWN: &str = "unknown";

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;

/// How long after a fetch of main that failed another may be tried for the same merge and build.
const FETCH_AGAIN_MS: i64 = 10 * MINUTE_MS;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Whether the work stands waiting for its change to take effect, which keeps no session warm: the wait may last days,
/// and its next round starts a new session.
pub fn waits_for_effect(w: &Work) -> bool {
    w.graph == ENGINEER && w.pos.at == WAIT
}

/// The acceptance conditions of the item written in_effect, as they stand on it.
pub fn conditions(conn: &Connection, item: i64) -> Result<Vec<Value>, String> {
    let locked: Option<String> = conn
        .query_row("SELECT acceptance FROM items WHERE id = ?1", [item], |r| r.get(0))
        .optional()
        .map_err(sql)?
        .flatten();
    Ok(written(&work::json_col(locked).unwrap_or(Value::Null)))
}

/// Those of a list of acceptance conditions written in_effect.
fn written(acceptance: &Value) -> Vec<Value> {
    acceptance
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["by"] == KIND)
        .cloned()
        .collect()
}

/// The number of each condition of a list.
fn ids(conditions: &[Value]) -> Vec<String> {
    conditions
        .iter()
        .filter_map(|c| c["id"].as_str().map(str::to_string))
        .collect()
}

/// The commit the work merged as and when, by its latest merge.
fn merged(conn: &Connection, work: i64) -> Result<Option<(String, i64)>, String> {
    conn.query_row(
        &format!(
            "SELECT json_extract(output, '$.result.merge_commit'), ended_at FROM rounds
             WHERE work = ?1 AND cell IN {} AND transition = 'merged' ORDER BY n DESC LIMIT 1",
            db::cells_sql(ENGINEER, &[MERGE])
        ),
        [work],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(sql)
}

/// Whether the repository at `dir` has a commit of that name.
fn has_commit(dir: &Path, commit: &str) -> bool {
    git::git(dir, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_ok()
}

/// Whether a build holds a merge, by the project's checkout: `yes` when the commit it was built from has the merge
/// commit among its ancestors, `no` when it has not, and `cannot_tell` when either commit is not in the checkout, as a
/// build made outside a git checkout, whose commit is `unknown`, never is. It looks at nothing but these three.
pub fn carries(checkout: &Path, merge: &str, build: &str) -> &'static str {
    if !has_commit(checkout, merge) || !has_commit(checkout, build) {
        return "cannot_tell";
    }
    if git::is_ancestor(checkout, merge, build) {
        "yes"
    } else {
        "no"
    }
}

/// What a fetch of main for a build the checkout lacks is for: the project, the merge and the build.
pub type FetchKey = (String, String, String);

/// The fetches of main made for a build the checkout lacks, for the engine's process. The value that wants one is worked
/// out at every step of a wait, and a build that is not on main never comes by fetching: one fetch that worked is enough
/// for a merge and a build, and one that failed is tried again only after [`FETCH_AGAIN_MS`].
#[derive(Default)]
pub struct Fetches {
    /// When the last attempt began, for each; none once one worked.
    tried: BTreeMap<FetchKey, Option<i64>>,
}

impl Fetches {
    /// Whether to fetch now, which marks the attempt.
    pub fn due(&mut self, key: &FetchKey, now: i64) -> bool {
        let due = match self.tried.get(key) {
            Some(None) => false,
            Some(Some(at)) => now - at >= FETCH_AGAIN_MS,
            None => true,
        };
        if due {
            self.tried.insert(key.clone(), Some(now));
        }
        due
    }

    /// The fetch worked: none is needed again for this merge and build.
    pub fn done(&mut self, key: &FetchKey) {
        self.tried.insert(key.clone(), None);
    }
}

/// The engine's value `in_effect`: whether what carries the change holds the merge commit for every condition. Main
/// holds it as the work merges. For the installed program, a commit the checkout lacks may only not be fetched yet, so
/// main is fetched, as [`Fetches`] allows, before it says it cannot tell, unless the build is one no fetch can bring.
pub fn carriers_hold(e: &Engine, w: &Work, item: &Item) -> Result<&'static str, String> {
    let installed = conditions(e.db.conn(), item.id)?.iter().any(|c| c["when"] == INSTALLED);
    if !installed {
        return Ok("yes");
    }
    let Some((merge, _)) = merged(e.db.conn(), w.id)? else {
        return Ok("cannot_tell");
    };
    let project = e.project_of(w)?;
    let found = carries(&project.checkout, &merge, &e.built_from);
    if found != "cannot_tell" || e.built_from == NOT_KNOWN {
        return Ok(found);
    }
    let key = (item.project.clone(), merge.clone(), e.built_from.clone());
    if !e.build_fetch_due(&key, db::now()) {
        return Ok(found);
    }
    if let Err(why) = config::fetch_main(&project) {
        e.say(&format!("work {}: {why}", w.id));
        return Ok(found);
    }
    e.build_fetched(&key);
    Ok(carries(&project.checkout, &merge, &e.built_from))
}

/// The engine's value `install_wait`: whether the time since the merge is past the project's limit of the wait for the
/// change to take effect.
pub fn install_wait(e: &Engine, w: &Work, now: i64) -> Result<&'static str, String> {
    let hours = e
        .project_of(w)?
        .install_wait_hours
        .unwrap_or(config::INSTALL_WAIT_HOURS);
    let since = merged(e.db.conn(), w.id)?.map_or(now, |(_, at)| at);
    Ok(if past_wait(since, hours, now) { "past" } else { "within" })
}

fn past_wait(since: i64, hours: u64, now: i64) -> bool {
    now - since > i64::try_from(hours).unwrap_or(i64::MAX / HOUR_MS) * HOUR_MS
}

/// The engine's value `look_due`: the cell that checks has not looked yet, its last look asked for no wait, or the time
/// it said to wait has passed.
pub fn look_due(e: &Engine, w: &Work, g: &CompiledGraph, now: i64) -> Result<&'static str, String> {
    let Some(last) = answered(e, w, g, CHECK)?.into_iter().next() else {
        return Ok("yes");
    };
    let minutes = last
        .output
        .as_ref()
        .map(|o| g.unwrap(o))
        .and_then(|o| o["again_after_minutes"].as_i64());
    let due = match (minutes, last.ended_at) {
        (Some(m), Some(ended)) => now >= ended + m * MINUTE_MS,
        _ => true,
    };
    Ok(if due { "yes" } else { "no" })
}

/// The engine's value `looks`: whether the cell that checks has sent the work back to wait as many times as the
/// project's limit.
pub fn looks(e: &Engine, w: &Work) -> Result<&'static str, String> {
    let limit = e
        .project_of(w)?
        .verification_looks
        .unwrap_or(config::VERIFICATION_LOOKS);
    let again = work::rounds_at(e.db.conn(), w.id, CHECK)?
        .iter()
        .filter(|r| r.transition.as_deref() == Some(AGAIN))
        .count();
    Ok(if again as u64 >= limit { "past" } else { "within" })
}

/// The time the change took effect, which a command takes as `:in_effect_at` and a run must not be older than: the
/// merge's, while no condition names the installed program or the wait found none holding it; else when the wait first
/// found them all holding the merge, as its round records what it was given.
pub fn took_effect_at(conn: &Connection, work: i64, item: i64) -> Result<Option<i64>, String> {
    let merge = merged(conn, work)?.map(|(_, at)| at);
    if !conditions(conn, item)?.iter().any(|c| c["when"] == INSTALLED) {
        return Ok(merge);
    }
    let found: Option<i64> = conn
        .query_row(
            "SELECT min(started_at) FROM rounds
             WHERE work = ?1 AND cell = ?2 AND json_extract(input, '$.given.in_effect') = 'yes'",
            params![work, WAIT],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(found.or(merge))
}

/// The engine's input `effect`: how the work's change took effect, for the cell that checks to put in its commands.
pub fn effect(e: &Engine, w: &Work, item: &Item) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some((merge, merged_at)) = merged(conn, w.id)? else {
        return Ok(None);
    };
    let carriers: Vec<Value> = conditions(conn, item.id)?
        .iter()
        .map(|c| {
            let installed = c["when"] == INSTALLED;
            json!({
                "id": c["id"],
                "when": c["when"],
                "carrier": if installed { "the installed program" } else { "main" },
                "commit": if installed { e.built_from.as_str() } else { merge.as_str() },
            })
        })
        .collect();
    Ok(Some(json!({
        "merge_commit": merge,
        "merged_at": merged_at,
        "pr": w.pr,
        "carriers": carriers,
        "in_effect": carriers_hold(e, w, item)?,
        "at": took_effect_at(conn, w.id, item.id)?,
    })))
}

/// What a check of a return is told of the project, for `in_effect_known`.
pub struct Project<'a> {
    pub name: &'a str,
    /// Whether the item delivers a report, of which nothing takes effect.
    pub report: bool,
    /// Whether the project is the one the engine is built from.
    pub engine_s_own: bool,
}

/// What is wrong with the conditions written in_effect of a list of acceptance conditions a return hands in.
pub fn known_problems(acceptance: &Value, p: &Project<'_>) -> Problems {
    let mut problems = Problems::default();
    let conditions = written(acceptance);
    if p.report && !conditions.is_empty() {
        problems.push("a work that delivers a report writes no condition in_effect: nothing of it takes effect");
    }
    for c in &conditions {
        let id = c["id"].as_str().unwrap_or_default();
        if c["when"] == INSTALLED && !p.engine_s_own {
            problems.push(format!(
                "{id} says the installed program carries the change, and this project's changes cannot change the \
                 installed program: the engine is not built from project {}. Write `when` as merged, or the condition \
                 another way",
                p.name
            ));
        }
    }
    problems
}

/// What is wrong with the rows of a table of conditions met against the acceptance conditions in force, as to in_effect:
/// a condition written so is met by a row of that kind and no other, as nothing on the branch meets it, and no other
/// condition has such a row.
pub fn rows_problems(acceptance: &[Value], rows: &[Value]) -> Problems {
    let mut problems = Problems::default();
    for r in rows {
        let id = r["id"].as_str().unwrap_or_default();
        let Some(c) = acceptance.iter().find(|c| c["id"].as_str() == Some(id)) else {
            continue;
        };
        if c["by"] == KIND && r["how"] != KIND {
            problems.push(format!(
                "{id} is written in_effect: it is checked after the merge, and nothing on the branch meets it, so its \
                 row says how in_effect"
            ));
        }
        if c["by"] != KIND && r["how"] == KIND {
            problems.push(format!(
                "{id} is not written in_effect, so its row cannot say how in_effect: it is met on the branch"
            ));
        }
    }
    problems
}

/// What is wrong with a look's verdicts against the conditions written in_effect (`wanted`): one for each and no other;
/// `skipped` only once words were said to the work since its merge (`words`).
pub fn cover_problems(wanted: &[String], verdicts: &[Value], words: bool) -> Problems {
    let mut problems = Problems::default();
    let given: Vec<String> = ids(verdicts);
    let misses = id_misses(wanted, &given);
    problems.about(
        &misses.unknown,
        |x| format!("{x} is not an acceptance condition written in_effect"),
        |xs| format!("{xs} are not acceptance conditions written in_effect"),
    );
    problems.about(
        &misses.twice,
        |x| format!("{x} has two verdicts"),
        |xs| format!("{xs} have two verdicts each"),
    );
    problems.about(
        &misses.missing,
        |x| format!("no verdict for acceptance condition {x}"),
        |xs| format!("no verdict for acceptance conditions {xs}"),
    );
    if !words && verdicts.iter().any(|v| v["verdict"] == "skipped") {
        problems
            .push("skipped is only for a condition someone said not to check, since the merge; no one said anything");
    }
    problems
}

/// What is wrong with how long a look says to wait: it says so exactly when the look waits, a condition being not_yet and
/// none failing, within the bound.
pub fn again_problems(verdicts: &[Value], minutes: Option<i64>) -> Problems {
    let mut problems = Problems::default();
    let waits = verdicts.iter().any(|v| v["verdict"] == "not_yet") && !verdicts.iter().any(|v| v["verdict"] == "fails");
    match minutes {
        None if waits => {
            problems.push("a condition is not_yet: say in again_after_minutes how long the missing evidence needs");
        }
        Some(_) if !waits => {
            problems.push(
                "again_after_minutes is only for a look that waits, with a condition not_yet and none that fails",
            );
        }
        Some(m) if !(1..=AGAIN_AFTER_MOST).contains(&m) => {
            problems.push(format!(
                "again_after_minutes is from 1 to {AGAIN_AFTER_MOST}, and it says {m}"
            ));
        }
        _ => {}
    }
    problems
}

/// One run of a tool a look may rest on, as the engine recorded it: the tool, what was handed in for it, its result, and
/// the text of the file its result was saved to when it was too long to give.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub tool: String,
    pub given: String,
    pub result: Value,
    pub saved: Option<String>,
}

/// The runs of the tools that check, of this work, since the time given.
pub fn runs_since(conn: &Connection, work: i64, since: i64) -> Result<Vec<Run>, String> {
    let rows = work::query_json(
        conn,
        "SELECT tool, coalesce(json_extract(args, '$.sql'), json_extract(args, '$.command'), '') AS given, result
         FROM tool_runs
         WHERE work = ?1 AND tool IN ('query', 'gh') AND started_at >= ?2 AND result IS NOT NULL
         ORDER BY id",
        &[&work, &since],
        10_000,
    )?;
    Ok(rows
        .iter()
        .map(|r| {
            let saved = r["result"]["ran"]["saved"].as_str().and_then(crate::tools::saved_to);
            Run {
                tool: r["tool"].as_str().unwrap_or_default().to_string(),
                given: r["given"].as_str().unwrap_or_default().to_string(),
                result: r["result"].clone(),
                saved: saved.and_then(|path| std::fs::read_to_string(path).ok()),
            }
        })
        .collect())
}

/// What a run was handed in as, in the words a look names it by: without the spaces around it, or the name of `gh`.
fn plain(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix("gh ").map_or(text, str::trim)
}

/// A condition's command as it is run: with the time the change took effect put in, the one thing put in.
fn placed(command: &str, at: i64) -> String {
    command.replace(":in_effect_at", &at.to_string())
}

/// Whether a run ran to its end and well: a query that gave rows, or a gh command that exited 0, whose record is its exit
/// code and its whole output. One that did not is cannot_judge, and nothing it said is evidence.
fn ran_well(run: &Run) -> bool {
    if run.tool == "gh" {
        return run.result[0] == 0;
    }
    run.result["ran"]["verdict"] == "ran"
}

/// Whether `seen` is in `text` as a whole: not the `7` of `217`, nor the `fetch` of `fetched`.
fn whole_in(text: &str, seen: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(seen).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + seen.len()..].chars().next();
        let cut_before = seen.starts_with(word) && before.is_some_and(word);
        let cut_after = seen.ends_with(word) && after.is_some_and(word);
        !cut_before && !cut_after
    })
}

/// A value of a row as it reads: a text as it is, anything else as JSON writes it.
fn cell_text(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_string)
}

/// The values `v` holds, however deep: a query gives a column of JSON text as an object or an array, and what is in it is
/// as much a value of the row as a column of its own. An object or an array with nothing in it is a value itself.
fn leaves(v: &Value) -> Vec<&Value> {
    match v {
        Value::Object(o) if !o.is_empty() => o.values().flat_map(leaves).collect(),
        Value::Array(a) if !a.is_empty() => a.iter().flat_map(leaves).collect(),
        leaf => vec![leaf],
    }
}

/// Whether `seen` is a value of a row, or part of the row that holds a value: not a field's name alone.
fn in_row(row: &Value, seen: &str) -> bool {
    let values = leaves(row);
    let in_a_value = values.iter().any(|v| match v {
        Value::String(s) => whole_in(s, seen),
        other => cell_text(other) == seen,
    });
    in_a_value
        || (whole_in(&row.to_string(), seen)
            && values
                .iter()
                .copied()
                .map(cell_text)
                .any(|v| !v.is_empty() && whole_in(seen, &v)))
}

/// Whether a run that ran well showed `seen`: in a gh command's output, or among a query's rows, or in the file where those
/// were saved. What the tool itself says of the run, its fields' names and the path of that file, shows nothing.
fn shown_in(run: &Run, seen: &str) -> bool {
    if run.tool == "gh" {
        return run.result[1].as_str().is_some_and(|text| whole_in(text, seen));
    }
    let given = run.result["ran"]["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|row| in_row(row, seen));
    let saved = run
        .saved
        .iter()
        .flat_map(|text| text.lines())
        .any(|line| serde_json::from_str::<Value>(line).is_ok_and(|row| in_row(&row, seen)));
    given || saved
}

/// What is wrong with what a look's verdicts rest on: a verdict of holds or fails names its condition's own command with
/// the time the change took effect (`at`) put in, among `runs`, those the engine recorded for this work since then; that
/// run ran well; and what the verdict says it saw is what that run showed. A verdict that rests on no run names none.
pub fn basis_problems(verdicts: &[Value], conditions: &[Value], at: i64, runs: &[Run]) -> Problems {
    let mut problems = Problems::default();
    for v in verdicts
        .iter()
        .filter(|v| v["verdict"] == "holds" || v["verdict"] == "fails")
    {
        let id = v["id"].as_str().unwrap_or_default();
        // A verdict for no such condition is said by the check of what the verdicts cover.
        let Some(condition) = conditions.iter().find(|c| c["id"].as_str() == Some(id)) else {
            continue;
        };
        let own = placed(condition["command"].as_str().unwrap_or_default(), at);
        let run = plain(v["run"].as_str().unwrap_or_default());
        let seen = v["seen"].as_str().unwrap_or_default().trim();
        if run != plain(&own) {
            problems.push(format!(
                "{id}: the run {run:?} is not the condition's own command, which with the time put in is {:?}: run it as \
                 it is written and name it so",
                plain(&own)
            ));
            continue;
        }
        let matching: Vec<&Run> = runs
            .iter()
            .filter(|r| r.tool == condition["tool"].as_str().unwrap_or_default() && plain(&r.given) == run)
            .collect();
        if matching.is_empty() {
            problems.push(format!(
                "{id}: the run {run:?} is not one the engine recorded for this work since the change took effect: name \
                 the SQL or the gh arguments as you handed them in, and run it if you have not"
            ));
            continue;
        }
        let well: Vec<&&Run> = matching.iter().filter(|r| ran_well(r)).collect();
        if well.is_empty() {
            problems.push(format!(
                "{id}: that run did not run to its end: a command that cannot run is cannot_judge, saying why"
            ));
        } else if seen.is_empty() || !well.iter().any(|r| shown_in(r, seen)) {
            problems.push(format!(
                "{id}: {seen:?} is not in the result of that run: say what it showed, copied letter for letter from \
                 its result"
            ));
        }
    }
    problems
}

/// Whether words were said to the work since its merge, given to this look or to one before it: they hold for each look
/// after, though each look is a session of its own.
pub fn words_given(conn: &Connection, work: i64) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            &format!(
                "SELECT count(*) FROM interjections
                 WHERE work = ?1 AND delivered_round > coalesce(
                   (SELECT max(n) FROM rounds WHERE work = ?1 AND cell IN {} AND transition = 'merged'), 0)",
                db::cells_sql(ENGINEER, &[MERGE])
            ),
            params![work],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(n > 0)
}

/// `tell_wait`: tells the owner, the first time in the work it takes the row, what settles a wait the engine cannot judge
/// and what words said to the work do.
pub fn tell_wait(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let told: i64 = tx
        .query_row(
            "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = ?2 AND transition = ?3 AND n < ?4",
            params![ctx.work.id, WAIT, ctx.row.name, ctx.round.n],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if told > 0 {
        return Ok(());
    }
    let merge = ctx
        .last(tx, MERGE, "result.merge_commit")?
        .and_then(|(_, c)| c.as_str().map(str::to_string))
        .unwrap_or_default();
    let text = format!(
        "{} waits for its change to take effect, and the engine cannot tell whether the installed program holds merge \
         {merge}: its build's commit is unknown or not in the project's checkout. Only installing a build of main settles \
         it. Saying something in the work's group buys one look at its conditions, and if the evidence is not there the \
         work waits again.",
        ctx.about()
    );
    crate::actions::notify(tx, Some(ctx.work.id), &text)
}

/// What a report says of a look or a limit, in a sentence: a verdict with what it rests on.
fn verdict_line(v: &Value) -> String {
    let id = v["id"].as_str().unwrap_or_default();
    let verdict = v["verdict"].as_str().unwrap_or_default();
    match ["why", "missing", "seen", "words"].iter().find_map(|k| v[*k].as_str()) {
        Some(said) => format!("{id} {verdict}: {}", db::one_sentence(said)),
        None => format!("{id} {verdict}"),
    }
}

/// `report_verification`: queues what became of the work's checks for the project's architect, as a report of its own
/// detail: what each condition came to with the run that shows it, or that it stands unverified at a limit. A condition
/// that fails and does harm now makes it urgent.
pub fn report(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let Some(item) = &ctx.item else { return Ok(()) };
    let merge = ctx.last(tx, MERGE, "result.merge_commit")?.map(|(_, c)| c);
    let verdicts: Vec<Value> = ctx.unwrapped["verdicts"].as_array().cloned().unwrap_or_default();
    let urgent = verdicts.iter().any(|v| v["harm_now"] == true);
    let all = ids(&conditions(tx, item.id)?);
    let lines: Vec<String> = verdicts.iter().map(verdict_line).collect();
    let (waiting, settled): (Vec<&Value>, Vec<&Value>) = verdicts.iter().partition(|v| v["verdict"] == "not_yet");
    let joined = |vs: &[&Value]| vs.iter().map(|v| verdict_line(v)).collect::<Vec<_>>().join("; ");
    let what = match ctx.row.name.as_str() {
        NEVER_IN_EFFECT if ctx.given.get("in_effect").is_some_and(|v| v == "cannot_tell") => format!(
            "The wait for the change of {} reached its limit, and the engine cannot tell whether the change took effect, \
             the installed program's build being unknown or not in the project's checkout: {} not checked, and stand \
             unverified",
            ctx.about(),
            all.join(", ")
        ),
        NEVER_IN_EFFECT => format!(
            "The change of {} never took effect within the limit of the wait: {} not checked, and stand unverified",
            ctx.about(),
            all.join(", ")
        ),
        PAST_LOOKS if settled.is_empty() => format!(
            "The evidence for {} did not come within the limit of looks, so these stand unverified: {}",
            ctx.about(),
            joined(&waiting)
        ),
        PAST_LOOKS => format!(
            "The evidence for {} did not come within the limit of looks, so these stand unverified: {}. The others: {}",
            ctx.about(),
            joined(&waiting),
            joined(&settled)
        ),
        _ if waiting.is_empty() => format!(
            "Checking {} once its change took effect: {}",
            ctx.about(),
            lines.join("; ")
        ),
        _ => format!(
            "Checking {} once its change took effect: {}. Those not_yet were not checked, and stand unverified, since the \
             work ended on what does not hold",
            ctx.about(),
            lines.join("; ")
        ),
    };
    let m = Matter {
        project: item.project.clone(),
        kind: "report".into(),
        detail: Some(crate::architect::VERIFICATION.into()),
        what: what.clone(),
        body: json!({
            "report": { "kind": crate::architect::VERIFICATION, "what": what, "urgent": urgent },
            "cell": ctx.cell.id,
            "line": ENGINEER,
            "urgent": urgent,
            "merge_commit": merge,
            "pr": ctx.work.pr,
            "conditions": all,
            "verdicts": verdicts,
        }),
        item: Some(item.id),
        raised: Some((ctx.round.work, ctx.round.n)),
    };
    queue_matter(tx, &m).map_err(sql)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn said(p: Problems) -> Vec<String> {
        p.texts().to_vec()
    }

    fn run(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn commit(dir: &Path, message: &str) -> String {
        run(dir, &["commit", "-q", "--allow-empty", "-m", message]);
        run(dir, &["rev-parse", "HEAD"])
    }

    // A build holds a merge when the merge is among its ancestors: one built after it does, one built before it or on a
    // side branch does not; a build whose commit is unknown, or that the checkout does not have, cannot be told, nor can
    // a merge the checkout does not have.
    #[test]
    fn a_build_holds_a_merge_by_its_ancestors_and_one_the_checkout_lacks_cannot_be_told() {
        let dir = std::env::temp_dir().join(format!("wf-agents-carries-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q", "-b", "main"]);
        let before = commit(&dir, "before");
        run(&dir, &["checkout", "-q", "-b", "side"]);
        let side = commit(&dir, "side");
        run(&dir, &["checkout", "-q", "main"]);
        let merge = commit(&dir, "the merge");
        let after = commit(&dir, "after");

        assert_eq!(carries(&dir, &merge, &after), "yes");
        assert_eq!(carries(&dir, &merge, &merge), "yes");
        assert_eq!(carries(&dir, &merge, &before), "no");
        assert_eq!(carries(&dir, &merge, &side), "no");
        assert_eq!(carries(&dir, &merge, NOT_KNOWN), "cannot_tell");
        let gone = "0123456789012345678901234567890123456789";
        assert_eq!(carries(&dir, &merge, gone), "cannot_tell");
        assert_eq!(carries(&dir, gone, &after), "cannot_tell");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn holds(id: &str, run: &str, seen: &str) -> Value {
        json!({ "verdict": "holds", "id": id, "run": run, "seen": seen })
    }

    fn not_yet(id: &str) -> Value {
        json!({ "verdict": "not_yet", "id": id, "missing": "No run yet." })
    }

    // Each condition has one verdict and none has two: one missing, one that is no condition written in_effect and one
    // given twice are each said, and skipped stands only once the look was given words.
    #[test]
    fn a_look_has_one_verdict_for_each_condition_and_skips_only_on_words() {
        let wanted = ["A1".to_string(), "A2".to_string()];
        let two = [holds("A1", "q", "s"), not_yet("A2")];
        assert!(cover_problems(&wanted, &two, false).is_empty());
        assert_eq!(
            said(cover_problems(&wanted, &two[..1], false)),
            ["no verdict for acceptance condition A2"]
        );
        let more = [holds("A1", "q", "s"), not_yet("A2"), holds("A3", "q", "s")];
        assert_eq!(
            said(cover_problems(&wanted, &more, false)),
            ["A3 is not an acceptance condition written in_effect"]
        );
        let twice = [holds("A1", "q", "s"), not_yet("A2"), not_yet("A2")];
        assert_eq!(said(cover_problems(&wanted, &twice, false)), ["A2 has two verdicts"]);
        let skipped = [
            holds("A1", "q", "s"),
            json!({ "verdict": "skipped", "id": "A2", "words": "The architect: it was reverted." }),
        ];
        assert!(cover_problems(&wanted, &skipped, true).is_empty());
        assert_eq!(
            said(cover_problems(&wanted, &skipped, false)),
            ["skipped is only for a condition someone said not to check, since the merge; no one said anything"]
        );
    }

    // How long to wait goes with a look that waits and none other, from a minute to two days: a condition that fails ends
    // the work, whatever else is not_yet.
    #[test]
    fn how_long_to_wait_is_said_exactly_for_a_condition_not_yet_and_within_two_days() {
        let waiting = [not_yet("A1")];
        let settled = [holds("A1", "q", "s")];
        let ending = [not_yet("A1"), json!({ "verdict": "fails", "id": "A2" })];
        assert!(again_problems(&waiting, Some(1)).is_empty());
        assert!(again_problems(&waiting, Some(AGAIN_AFTER_MOST)).is_empty());
        assert!(again_problems(&settled, None).is_empty());
        assert!(again_problems(&ending, None).is_empty());
        assert_eq!(
            said(again_problems(&waiting, None)),
            ["a condition is not_yet: say in again_after_minutes how long the missing evidence needs"]
        );
        for no_wait in [&settled[..], &ending[..]] {
            assert_eq!(
                said(again_problems(no_wait, Some(5))),
                ["again_after_minutes is only for a look that waits, with a condition not_yet and none that fails"]
            );
        }
        assert_eq!(
            said(again_problems(&waiting, Some(AGAIN_AFTER_MOST + 1))),
            ["again_after_minutes is from 1 to 2880, and it says 2881"]
        );
        assert_eq!(
            said(again_problems(&waiting, Some(0))),
            ["again_after_minutes is from 1 to 2880, and it says 0"]
        );
    }

    fn ran(tool: &str, given: &str, result: Value) -> Run {
        Run {
            tool: tool.into(),
            given: given.into(),
            result,
            saved: None,
        }
    }

    fn checked_by(id: &str, tool: &str, command: &str) -> Value {
        json!({ "by": "in_effect", "id": id, "tool": tool, "command": command })
    }

    // A verdict that rests on a run names the condition's own command with the time put in, as the engine recorded it;
    // what it saw is what that run showed. Another command, a run no record has, a run that did not end well, or a text the
    // run did not show, as `fetched` where every run was refused, is said; a verdict that rests on no run names none.
    #[test]
    fn a_verdict_rests_on_a_run_the_engine_recorded_and_on_what_that_run_showed() {
        const QUERY: &str = "SELECT verdict FROM t WHERE at > :in_effect_at";
        const PLACED: &str = "SELECT verdict FROM t WHERE at > 50";
        const VIEW: &str = "pr view 12 --json state";
        let query = ran(
            "query",
            PLACED,
            json!({ "ran": { "verdict": "ran", "rows": [{ "verdict": "refused", "ms": 218_000 }] } }),
        );
        let gh = ran("gh", VIEW, json!([0, "{\"state\":\"MERGED\"}"]));
        let runs = [query, gh];
        let conditions = [checked_by("A1", "query", QUERY), checked_by("A2", "gh", VIEW)];
        let problems = |v: Value, runs: &[Run]| said(basis_problems(&[v], &conditions, 50, runs));

        for good in [
            holds("A1", &format!(" {PLACED} "), "refused"),
            holds("A1", PLACED, "218000"),
            holds("A1", PLACED, "\"ms\":218000"),
            holds("A2", "gh pr view 12 --json state", "\"state\":\"MERGED\""),
            holds("A2", VIEW, "MERGED"),
        ] {
            let found = problems(good.clone(), &runs);
            assert!(found.is_empty(), "{good}: {found:?}");
        }
        let another = problems(holds("A1", "SELECT 1", "1"), &runs);
        assert_eq!(another.len(), 1);
        assert!(
            another[0].starts_with("A1: the run \"SELECT 1\" is not the condition's own command"),
            "{another:?}"
        );
        // The time not put in is another command than the one that was run.
        assert_eq!(problems(holds("A1", QUERY, "refused"), &runs).len(), 1);
        for seen in ["fetched", "verdict", "218", "ran"] {
            let not_shown = json!({
                "verdict": "fails", "id": "A1", "run": PLACED, "seen": seen, "why": "x", "harm_now": false,
            });
            let found = problems(not_shown, &runs);
            assert_eq!(found.len(), 1, "{seen}: {found:?}");
            assert!(found[0].contains("is not in the result of that run"), "{found:?}");
        }
        // With no run on record since the change took effect, none can be named.
        let none = problems(holds("A1", PLACED, "refused"), &[]);
        assert_eq!(none.len(), 1);
        assert!(none[0].contains("is not one the engine recorded"), "{none:?}");
        assert!(problems(not_yet("A1"), &[]).is_empty());

        // A run that did not end well is evidence of nothing, whatever it said.
        let broken = ran(
            "query",
            PLACED,
            json!({ "ran": { "verdict": "error", "error": "no such table: t" } }),
        );
        let exited = ran("gh", VIEW, json!([1, "MERGED"]));
        let cut_off = ran("gh", VIEW, json!({ "failed": { "kind": "other", "what": "MERGED" } }));
        let cannot = "that run did not run to its end";
        assert!(problems(holds("A1", PLACED, "no such table"), &[broken])[0].contains(cannot));
        assert!(problems(holds("A2", VIEW, "MERGED"), &[exited])[0].contains(cannot));
        assert!(problems(holds("A2", VIEW, "MERGED"), &[cut_off])[0].contains(cannot));
    }

    // Rows too long to give are looked for in the file they were saved to, and the words that say where it is show nothing.
    // A gh command's record holds its whole output, however long.
    #[test]
    fn a_long_result_is_looked_for_in_the_file_it_was_saved_to() {
        let conditions = [checked_by("A1", "query", "Q"), checked_by("A2", "gh", "pr list")];
        let query = Run {
            saved: Some("{\"ms\":218000}\n{\"ms\":7}".into()),
            ..ran(
                "query",
                "Q",
                json!({ "ran": { "verdict": "ran", "rows": [], "saved": "Too long to give here (9 bytes); saved to /x/y" } }),
            )
        };
        let long = format!("{} the state is MERGED", "x".repeat(30_000));
        let runs = [query, ran("gh", "pr list", json!([0, long]))];
        let problems = |v: Value| basis_problems(&[v], &conditions, 0, &runs);
        assert!(problems(holds("A1", "Q", "218000")).is_empty());
        assert!(problems(holds("A1", "Q", "7")).is_empty());
        assert!(problems(holds("A2", "pr list", "MERGED")).is_empty());
        for (id, run, seen) in [("A1", "Q", "8"), ("A1", "Q", "saved to"), ("A2", "pr list", "CLOSED")] {
            assert_eq!(problems(holds(id, run, seen)).texts().len(), 1, "{seen}");
        }
    }

    // A command that selects a column of JSON text is given it as the query gives it, parsed: a value inside it, or part
    // of the row that holds one, is what the run showed, and a field's name alone, at any depth, is not.
    #[test]
    fn a_value_inside_a_column_of_json_is_shown_and_a_field_s_name_is_not() {
        let column = |text: &str| crate::work::cell_json(db::rusqlite::types::ValueRef::Text(text.as_bytes()));
        let fetched = column(r#"{"sources":{"verdict":"fetched","took":[217,{"ok":true}],"none":[]}}"#);
        let refused = column(r#"{"sources":{"fetched":"refused"}}"#);
        let run = |result: Value| {
            ran(
                "query",
                "Q",
                json!({ "ran": { "verdict": "ran", "rows": [{ "result": result }] } }),
            )
        };
        let conditions = [checked_by("A1", "query", "Q")];
        let problems = |runs: &[Run], seen: &str| basis_problems(&[holds("A1", "Q", seen)], &conditions, 0, runs);

        let holding = [run(fetched)];
        for good in ["fetched", r#""verdict":"fetched""#, "217", "true", "[]"] {
            assert!(problems(&holding, good).is_empty(), "{good}");
        }
        for bad in ["verdict", "sources", "took", "ok", "7", "fetch"] {
            assert_eq!(problems(&holding, bad).texts().len(), 1, "{bad}");
        }

        let not_holding = [run(refused)];
        assert_eq!(problems(&not_holding, "fetched").texts().len(), 1);
        assert!(problems(&not_holding, "refused").is_empty());
    }

    // A fetch for a build the checkout lacks is made at most once when it works, however many steps the wait takes, and
    // again only after a while when it fails. Another merge or build has its own. The count is of what would reach the
    // network, stepping as the wait does.
    #[test]
    fn a_wait_fetches_main_for_a_build_once_and_again_only_after_a_failure_and_a_while() {
        let key = ("p".to_string(), "merge".to_string(), "build".to_string());
        let mut fetches = Fetches::default();
        let mut made = 0;
        for step in 0..1000 {
            if fetches.due(&key, step * 10_000) {
                made += 1;
                fetches.done(&key);
            }
        }
        assert_eq!(made, 1);

        let failing = ("p".to_string(), "merge".to_string(), "other".to_string());
        let mut made = 0;
        for step in 0..=360 {
            // An hour of steps ten seconds apart, the fetch failing each time it is made.
            if fetches.due(&failing, step * 10_000) {
                made += 1;
            }
        }
        assert_eq!(made, 7, "one each ten minutes, the first at once");
    }

    // A text is shown whole: not the digit of a larger number, nor the start of a longer word, and a text that does not
    // begin or end with a letter or digit is not held to it there.
    #[test]
    fn a_text_is_found_only_whole() {
        assert!(whole_in("took 217 ms", "217"));
        assert!(!whole_in("took 217 ms", "7"));
        assert!(!whole_in("fetched", "fetch"));
        assert!(whole_in("{\"a\":1}", "\"a\":"));
        assert!(whole_in("x fetch x fetched", "fetch"));
        // A letter of several bytes before the text, made from its number so that this file holds none.
        let letter = char::from_u32(0x4E2D).unwrap();
        assert!(whole_in(&format!("{letter} ok"), "ok"));
    }

    fn known(report: bool, engine_s_own: bool) -> Project<'static> {
        Project {
            name: "p",
            report,
            engine_s_own,
        }
    }

    // In the project the engine is built from a condition may name the installed program; in any other it may not, nor may
    // a report write any such condition.
    #[test]
    fn a_condition_in_effect_is_one_this_project_can_check() {
        let acceptance = json!([
            { "by": "test", "id": "A1", "condition": "x" },
            { "by": "in_effect", "id": "A2", "when": "installed", "tool": "query" },
            { "by": "in_effect", "id": "A3", "when": "merged", "tool": "gh" },
        ]);
        assert!(known_problems(&acceptance, &known(false, true)).is_empty());
        let other = said(known_problems(&acceptance, &known(false, false)));
        assert_eq!(other.len(), 1);
        assert!(
            other[0].contains("this project's changes cannot change the installed program"),
            "{other:?}"
        );
        assert_eq!(
            said(known_problems(&acceptance, &known(true, true))),
            ["a work that delivers a report writes no condition in_effect: nothing of it takes effect"]
        );
        assert!(known_problems(&json!([{ "by": "test", "id": "A1" }]), &known(true, false)).is_empty());
    }

    // A condition written in_effect is met by a row of that kind and no other, and only such a condition has one.
    #[test]
    fn a_row_says_in_effect_exactly_for_a_condition_written_so() {
        let acceptance = [
            json!({ "by": "test", "id": "A1" }),
            json!({ "by": "in_effect", "id": "A2" }),
        ];
        let rows = [
            json!({ "how": "tested", "id": "A1", "tests": ["t"] }),
            json!({ "how": "in_effect", "id": "A2" }),
        ];
        assert!(rows_problems(&acceptance, &rows).is_empty());
        let swapped = [
            json!({ "how": "in_effect", "id": "A1" }),
            json!({ "how": "read_diff", "id": "A2", "where": "x", "what": "y" }),
        ];
        let told = said(rows_problems(&acceptance, &swapped));
        assert_eq!(told.len(), 2);
        assert!(told[0].starts_with("A1 is not written in_effect"), "{told:?}");
        assert!(told[1].starts_with("A2 is written in_effect"), "{told:?}");
    }

    // The wait is past its limit only once more than the limit's hours passed since the merge.
    #[test]
    fn the_wait_is_past_its_limit_only_beyond_its_hours() {
        assert!(!past_wait(0, 72, 72 * HOUR_MS));
        assert!(past_wait(0, 72, 72 * HOUR_MS + 1));
        assert!(past_wait(0, 0, 1));
    }
}
