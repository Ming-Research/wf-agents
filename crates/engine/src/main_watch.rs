//! The watch of each project's main, drawn on the architect's graph: no work watches main after its own merge, since
//! the merge queue's full CI ran on the very commit that became main; main may still turn red by a flaky test, a change
//! of the environment or code pushed from elsewhere. The engine looks at main's full CI for every project (the action
//! `queue_main_red` of the cell the architect waits at to start): once the latest commit's run has finished red, it
//! queues a matter of main red, which stops the project's merge queue while it is open, so that no work merges onto a
//! red main. The architect's work takes it after the owner's words and before anything else: the engine first checks it
//! mechanically (cell `M1`), and only what that does not clear goes to the architect (`M2`), which judges which merge
//! made main red and whether to have it fixed or to revert that merge first.

use db::rusqlite::params;
use db::{Actor, Matter, queue_matter};
use serde_json::{Value, json};

use crate::github::CheckRun;
use crate::work::{RoundId, Work};
use crate::{ARCHITECT, Engine, git, tools};

/// How often, at most, the watch asks GitHub about one project's main.
pub const EVERY_MS: i64 = 60_000;

/// The kind of matter main's CI red is.
pub const MAIN_RED: &str = "main_red";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Whether a matter of main red is open for a project: while one is, the project's merge queue is stopped.
pub fn red(conn: &db::rusqlite::Connection, project: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT count(*) > 0 FROM architect_matters
         WHERE project = ?1 AND kind = ?2 AND handled_round IS NULL AND withdrawn IS NULL",
        params![project, MAIN_RED],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// Looks at every project's main, at most once a minute each ([`EVERY_MS`]): when the full CI of the commit main is at
/// has finished and a check that counts against a merge is red, known red checks aside, queues a matter of main red with
/// the commit and its red checks, unless one was queued for that commit already. A project whose full CI runs another
/// project's workflow, or that names none, is not watched; nothing is queued while GitHub cannot be asked.
pub fn look(e: &mut Engine) -> Result<(), String> {
    let now = db::now();
    let projects: Vec<(String, config::Project)> = e
        .config
        .config
        .projects
        .iter()
        .map(|(n, p)| (n.clone(), p.clone()))
        .collect();
    for (name, p) in projects {
        let (own, downstream) = tools::own_full(&p);
        if own.is_empty() || !downstream.is_empty() || !e.main_look_due(&name, now) {
            continue;
        }
        let repo = tools::repo(&p);
        let Ok(head) = e.github().branch_head(&repo, &p.main) else {
            continue;
        };
        let queued: i64 =
            e.db.conn()
                .query_row(
                    "SELECT count(*) FROM architect_matters
                     WHERE project = ?1 AND kind = ?2 AND json_extract(body, '$.commit') = ?3",
                    params![name, MAIN_RED, head],
                    |r| r.get(0),
                )
                .map_err(sql)?;
        if queued > 0 {
            continue;
        }
        let mut checks: Vec<CheckRun> = Vec::new();
        let mut finished = true;
        for wf in &own {
            match e.github().finished_checks(&repo, &head, wf) {
                Ok(Some(cs)) => checks.extend(cs),
                Ok(None) | Err(_) => {
                    finished = false;
                    break;
                }
            }
        }
        if !finished {
            continue;
        }
        let known = tools::known_red(e, &name)?;
        let red = tools::reds(&tools::counted(&checks, &p, &known));
        if red.is_empty() {
            continue;
        }
        let short: String = head.chars().take(10).collect();
        let m = Matter {
            project: name.clone(),
            kind: MAIN_RED.into(),
            detail: None,
            what: format!("Main's CI is red on {short}: {}", red.join(", ")),
            body: json!({ "commit": head, "checks": tools::checks_json(&checks), "red": red }),
            item: None,
            raised: None,
        };
        e.db.write(Actor::Engine, |tx| queue_matter(tx, &m)).map_err(sql)?;
        e.say(&format!("main of {name} is red on {short}: the merge queue stops"));
    }
    Ok(())
}

/// Cell `M1`: checks main's red CI mechanically, as the matter in hand gives it, the merge queue stopped meanwhile: runs
/// each red check again on the same commit, looks in the table of known red checks, and runs the red checks on main's
/// commit before it, in that order, and lists the merges up to it, each with the work and item it came of.
pub fn check(e: &mut Engine, w: &Work, round: RoundId) -> Result<Value, String> {
    let project = w
        .project
        .clone()
        .ok_or_else(|| format!("work {} names no project for its architect", w.id))?;
    let p = e
        .config
        .config
        .projects
        .get(&project)
        .cloned()
        .ok_or_else(|| format!("no project {project} in the configuration"))?;
    let matter = crate::architect::picked(e.db.conn(), w.id)?.ok_or("no matter of main red is in hand")?;
    let body: Value =
        e.db.conn()
            .query_row("SELECT body FROM architect_matters WHERE id = ?1", [matter], |r| {
                r.get::<_, String>(0)
            })
            .map(|b| serde_json::from_str(&b).unwrap_or(Value::Null))
            .map_err(sql)?;
    let commit = body["commit"].as_str().unwrap_or_default().to_string();
    let first: Vec<CheckRun> = serde_json::from_value(body["checks"].clone()).unwrap_or_default();
    let required = |cs: &[CheckRun]| tools::counted(cs, &p, &[]);
    if commit.is_empty() || tools::reds(&required(&first)).is_empty() {
        return Ok(json!({ "main": { "kind": "green" } }));
    }
    let args = json!({ "commit": commit });
    let again = match tools::attempts(e, round, "check_main", &args, |e| {
        tools::rerun_full(e, &p, &commit, &first).map(|c| tools::checks_json(&c))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(tools::not_run("main", x)),
    };
    let red = tools::reds(&required(&again));
    if red.is_empty() {
        return Ok(json!({ "main": { "kind": "flaky" } }));
    }
    let known = tools::known_red(e, &project)?;
    if red.iter().all(|c| known.contains(c)) {
        return Ok(json!({ "main": { "kind": "known_red" } }));
    }
    let _ = git::git(&p.checkout, &["fetch", "-q", "origin", &config::main_refspec(&p.main)]);
    let before = git::resolve(&p.checkout, &format!("{commit}^1")).ok_or("main's commit before is not known here")?;
    let earlier = match tools::attempts(e, round, "check_main", &json!({ "before": before }), |e| {
        tools::full_checks(e, &p, &before).map(|c| tools::checks_json(&c))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(tools::not_run("main", x)),
    };
    let red_before: Vec<&CheckRun> = earlier
        .iter()
        .filter(|c| red.contains(&c.name) && c.result != "green")
        .collect();
    if !red_before.is_empty() {
        let link = red_before.iter().map(|c| c.run.as_str()).collect::<Vec<_>>().join(" ");
        return Ok(json!({ "main": {
            "kind": "environment_changed",
            "checks": tools::checks_json(&again),
            "before": format!("{before} {link}"),
        } }));
    }
    let latest: Vec<CheckRun> = again.into_iter().filter(|c| red.contains(&c.name)).collect();
    Ok(json!({ "main": {
        "kind": "broken_by_merge",
        "checks": tools::checks_json(&latest),
        "merges": merges(e, &p, &commit)?,
    } }))
}

/// The merges up to main's commit `commit`, newest first, the five latest by main's first parents: each by its commit,
/// its first line, and the work and item that merged it, when a work of the engine's did.
fn merges(e: &Engine, p: &config::Project, commit: &str) -> Result<Vec<Value>, String> {
    let log = git::git(&p.checkout, &["log", "--first-parent", "--format=%H %s", "-5", commit]).unwrap_or_default();
    let mut out = Vec::new();
    for line in log.lines() {
        let (sha, subject) = line.split_once(' ').unwrap_or((line, ""));
        let by: Option<(i64, Option<i64>)> =
            e.db.conn()
                .query_row(
                    &format!(
                        "SELECT w.id, w.item FROM rounds r JOIN works w ON w.id = r.work
                         WHERE w.graph <> '{ARCHITECT}' AND json_extract(r.output, '$.result.merge_commit') = ?1
                         LIMIT 1"
                    ),
                    [sha],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
        let short: String = sha.chars().take(10).collect();
        let of = match by {
            Some((w, Some(i))) => format!(" (work {w} on #{i})"),
            Some((w, None)) => format!(" (work {w})"),
            None => String::new(),
        };
        out.push(json!(format!("{short} {subject}{of}")));
    }
    Ok(out)
}
