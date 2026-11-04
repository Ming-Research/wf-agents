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

/// The engine's value `in_effect`: whether what carries the change holds the merge commit for every condition. Main
/// holds it as the work merges. For the installed program, a commit the checkout lacks may only not be fetched yet, so
/// main is fetched once before it says it cannot tell, unless the build is one no fetch can bring.
pub fn in_effect(e: &Engine, w: &Work, item: &Item) -> Result<&'static str, String> {
    let installed = conditions(e.db.conn(), item.id)?
        .iter()
        .any(|c| c["when"] == INSTALLED);
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
    if let Err(why) = config::fetch_main(&project) {
        e.say(&format!("work {}: {why}", w.id));
        return Ok(found);
    }
    Ok(carries(&project.checkout, &merge, &e.built_from))
}

/// The engine's value `install_wait`: whether the time since the merge is past the project's limit of the wait for the
/// change to take effect.
pub fn install_wait(e: &Engine, w: &Work, now: i64) -> Result<&'static str, String> {
    let hours = e.project_of(w)?.install_wait_hours.unwrap_or(config::INSTALL_WAIT_HOURS);
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
    let minutes = last.output.as_ref().map(|o| g.unwrap(o)).and_then(|o| o["again_after_minutes"].as_i64());
    let due = match (minutes, last.ended_at) {
        (Some(m), Some(ended)) => now >= ended + m * MINUTE_MS,
        _ => true,
    };
    Ok(if due { "yes" } else { "no" })
}

/// The engine's value `looks`: whether the cell that checks has sent the work back to wait as many times as the
/// project's limit.
pub fn looks(e: &Engine, w: &Work) -> Result<&'static str, String> {
    let limit = e.project_of(w)?.verification_looks.unwrap_or(config::VERIFICATION_LOOKS);
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
        "in_effect": in_effect(e, w, item)?,
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
    /// Whether the project lists a measurement workflow.
    pub measures: bool,
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
        if c["tool"] == "measure" && !p.measures {
            problems.push(format!(
                "{id} is checked with measure, and project {} lists no measurement workflow",
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
/// `skipped` only once the look was given words said to the work (`words`).
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
        problems.push("skipped is only for a condition someone said in words given to this look not to check; none were");
    }
    problems
}

/// What is wrong with how long a look says to wait: it says so exactly when a condition is not_yet, within the bound.
pub fn again_problems(verdicts: &[Value], minutes: Option<i64>) -> Problems {
    let mut problems = Problems::default();
    let not_yet = verdicts.iter().any(|v| v["verdict"] == "not_yet");
    match minutes {
        None if not_yet => {
            problems.push("a condition is not_yet: say in again_after_minutes how long the missing evidence needs");
        }
        Some(_) if !not_yet => {
            problems.push("again_after_minutes is only for a condition that is not_yet, and none is");
        }
        Some(m) if !(1..=AGAIN_AFTER_MOST).contains(&m) => {
            problems.push(format!("again_after_minutes is from 1 to {AGAIN_AFTER_MOST}, and it says {m}"));
        }
        _ => {}
    }
    problems
}

/// One run of a tool a look may rest on, as the engine recorded it: the tool, what was handed in for it, and its result.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub tool: String,
    pub given: String,
    pub result: Value,
}

/// The runs of the tools that check, of this work, since the time given.
pub fn runs_since(conn: &Connection, work: i64, since: i64) -> Result<Vec<Run>, String> {
    let rows = work::query_json(
        conn,
        "SELECT tool, coalesce(json_extract(args, '$.sql'), json_extract(args, '$.command'), '') AS given, result
         FROM tool_runs
         WHERE work = ?1 AND tool IN ('query', 'gh', 'measure') AND started_at >= ?2 AND result IS NOT NULL
         ORDER BY id",
        &[&work, &since],
        10_000,
    )?;
    Ok(rows
        .iter()
        .map(|r| Run {
            tool: r["tool"].as_str().unwrap_or_default().to_string(),
            given: r["given"].as_str().unwrap_or_default().to_string(),
            result: r["result"].clone(),
        })
        .collect())
}

/// What a run was handed in as, in the words a look names it by: without the spaces around it, or the name of `gh`.
fn plain(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix("gh ").map_or(text, str::trim)
}

/// Whether a text is, letter for letter, in a value of a run's result or in the result as written.
fn shown_in(result: &Value, seen: &str) -> bool {
    fn in_a_value(v: &Value, seen: &str) -> bool {
        match v {
            Value::String(s) => s.contains(seen),
            Value::Array(xs) => xs.iter().any(|x| in_a_value(x, seen)),
            Value::Object(o) => o.values().any(|x| in_a_value(x, seen)),
            other => other.to_string().contains(seen),
        }
    }
    in_a_value(result, seen) || result.to_string().contains(seen)
}

/// What is wrong with what a look's verdicts rest on: a verdict of holds or fails names a run among `runs`, those the
/// engine recorded for this work since the change took effect, by what was handed in for it or, for a measurement, a
/// link in its result; and what it says it saw is in that run's result. A verdict that rests on no run names none.
pub fn basis_problems(verdicts: &[Value], runs: &[Run]) -> Problems {
    let mut problems = Problems::default();
    for v in verdicts.iter().filter(|v| v["verdict"] == "holds" || v["verdict"] == "fails") {
        let id = v["id"].as_str().unwrap_or_default();
        let run = plain(v["run"].as_str().unwrap_or_default());
        let seen = v["seen"].as_str().unwrap_or_default().trim();
        let matching: Vec<&Run> = runs
            .iter()
            .filter(|r| {
                !run.is_empty()
                    && (plain(&r.given) == run || (r.tool == "measure" && r.result.to_string().contains(run)))
            })
            .collect();
        if matching.is_empty() {
            problems.push(format!(
                "{id}: the run {run:?} is not one the engine recorded for this work since the change took effect: \
                 name the SQL or the gh arguments as you handed them in, or a measurement's link, and run it if you \
                 have not"
            ));
        } else if seen.is_empty() || !matching.iter().any(|r| shown_in(&r.result, seen)) {
            problems.push(format!(
                "{id}: {seen:?} is not in the result of that run: say what it showed, copied letter for letter from \
                 its result"
            ));
        }
    }
    problems
}

/// Whether the look under way was given words said to the work: since the wait let the work on, in this round or a
/// round before it.
pub fn words_given(conn: &Connection, work: i64) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            "SELECT count(*) FROM interjections
             WHERE work = ?1 AND delivered_round > coalesce(
               (SELECT max(n) FROM rounds WHERE work = ?1 AND cell = ?2), 0)",
            params![work, WAIT],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(n > 0)
}

/// `tell_wait`: tells the owner once, as the work first takes the row, what only the owner can do about a wait the
/// engine cannot judge.
pub fn tell_wait(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let merge = ctx
        .last(tx, MERGE, "result.merge_commit")?
        .and_then(|(_, c)| c.as_str().map(str::to_string))
        .unwrap_or_default();
    let text = format!(
        "{} waits for its change to take effect, and the engine cannot tell whether the installed program holds merge \
         {merge}: its build's commit is unknown or not in the project's checkout. Install a build of main, or say in the \
         work's group that it is in effect.",
        ctx.about()
    );
    crate::actions::notify(tx, Some(ctx.work.id), &text)
}

/// What a report says of a look or a limit, in a sentence: a verdict with what it rests on.
fn verdict_line(v: &Value) -> String {
    let id = v["id"].as_str().unwrap_or_default();
    let verdict = v["verdict"].as_str().unwrap_or_default();
    match ["why", "missing", "seen", "words"]
        .iter()
        .find_map(|k| v[*k].as_str())
    {
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
    let what = match ctx.row.name.as_str() {
        NEVER_IN_EFFECT => format!(
            "The change of {} never took effect within the limit of the wait: {} not checked, and stand unverified",
            ctx.about(),
            all.join(", ")
        ),
        PAST_LOOKS => format!(
            "The evidence for {} did not come within the limit of looks, so these stand unverified: {}",
            ctx.about(),
            lines.join("; ")
        ),
        _ => format!("Checking {} once its change took effect: {}", ctx.about(), lines.join("; ")),
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
            ["skipped is only for a condition someone said in words given to this look not to check; none were"]
        );
    }

    // How long to wait goes with a condition not_yet and none other, from a minute to two days.
    #[test]
    fn how_long_to_wait_is_said_exactly_for_a_condition_not_yet_and_within_two_days() {
        let waiting = [not_yet("A1")];
        let settled = [holds("A1", "q", "s")];
        assert!(again_problems(&waiting, Some(1)).is_empty());
        assert!(again_problems(&waiting, Some(AGAIN_AFTER_MOST)).is_empty());
        assert!(again_problems(&settled, None).is_empty());
        assert_eq!(
            said(again_problems(&waiting, None)),
            ["a condition is not_yet: say in again_after_minutes how long the missing evidence needs"]
        );
        assert_eq!(
            said(again_problems(&settled, Some(5))),
            ["again_after_minutes is only for a condition that is not_yet, and none is"]
        );
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
        }
    }

    // A verdict that rests on a run names one the engine recorded, as handed in, or by a measurement's link; what it saw
    // is in that run's result. A run no record has, or a text that run did not show, as `fetched` where every run was
    // refused, is said; a verdict that rests on no run names none.
    #[test]
    fn a_verdict_rests_on_a_run_the_engine_recorded_and_on_what_that_run_showed() {
        let query = ran(
            "query",
            "SELECT verdict FROM t",
            json!({ "ran": { "verdict": "ran", "rows": [{ "verdict": "refused", "ms": 218_000 }] } }),
        );
        let gh = ran(
            "gh",
            "pr view 12 --json state",
            json!({ "ran": { "output": "{\"state\":\"MERGED\"}" } }),
        );
        let measure = ran("measure", "", json!({ "push": { "run": "https://github.com/o/p/runs/9" } }));
        let runs = [query, gh, measure];

        for good in [
            holds("A1", " SELECT verdict FROM t ", "refused"),
            holds("A1", "SELECT verdict FROM t", "218000"),
            holds("A1", "gh pr view 12 --json state", "\"state\":\"MERGED\""),
            holds("A1", "https://github.com/o/p/runs/9", "runs/9"),
        ] {
            assert!(basis_problems(std::slice::from_ref(&good), &runs).is_empty(), "{good}");
        }
        let unrecorded = said(basis_problems(&[holds("A1", "SELECT 1", "1")], &runs));
        assert_eq!(unrecorded.len(), 1);
        assert!(unrecorded[0].starts_with("A1: the run \"SELECT 1\" is not one the engine recorded"), "{unrecorded:?}");
        let not_shown = json!({
            "verdict": "fails", "id": "A2", "run": "SELECT verdict FROM t", "seen": "fetched", "why": "x", "harm_now": false,
        });
        assert_eq!(
            said(basis_problems(&[not_shown], &runs)),
            ["A2: \"fetched\" is not in the result of that run: say what it showed, copied letter for letter from its result"]
        );
        // With no run on record since the change took effect, none can be named.
        assert_eq!(
            basis_problems(&[holds("A1", "SELECT verdict FROM t", "refused")], &[]).texts().len(),
            1
        );
        assert!(basis_problems(&[not_yet("A1")], &[]).is_empty());
    }

    fn known(report: bool, engine_s_own: bool, measures: bool) -> Project<'static> {
        Project {
            name: "p",
            report,
            engine_s_own,
            measures,
        }
    }

    // In the project the engine is built from a condition may name the installed program; in any other it may not, and
    // nor may it name measure where the project lists no workflow, nor a report write any such condition.
    #[test]
    fn a_condition_in_effect_is_one_this_project_can_check() {
        let acceptance = json!([
            { "by": "test", "id": "A1", "condition": "x" },
            { "by": "in_effect", "id": "A2", "when": "installed", "tool": "query" },
            { "by": "in_effect", "id": "A3", "when": "merged", "tool": "measure" },
        ]);
        assert!(known_problems(&acceptance, &known(false, true, true)).is_empty());
        let other = said(known_problems(&acceptance, &known(false, false, true)));
        assert_eq!(other.len(), 1);
        assert!(other[0].contains("this project's changes cannot change the installed program"), "{other:?}");
        assert_eq!(
            said(known_problems(&acceptance, &known(false, true, false))),
            ["A3 is checked with measure, and project p lists no measurement workflow"]
        );
        assert_eq!(
            said(known_problems(&acceptance, &known(true, true, true))),
            ["a work that delivers a report writes no condition in_effect: nothing of it takes effect"]
        );
        assert!(known_problems(&json!([{ "by": "test", "id": "A1" }]), &known(true, false, false)).is_empty());
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
