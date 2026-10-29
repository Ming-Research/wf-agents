//! How a work ends, whichever way it ends: at its graph's end, stopped before it (its item cancelled, or merged into
//! another), or stopped for a new work that takes it over. [`end`] is the only place a work's end is written, so that
//! no way of ending leaves behind what the work held: its sessions, its worktree, its pull request, its questions to
//! the architect and its asks of the owner. Cancelling an item ([`cancel_item`]) also lets go of the items that
//! depended on it, telling each.

use std::collections::BTreeMap;

use db::rusqlite::{Transaction, params};
use serde_json::{Value, json};

use crate::work;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// How a work ends.
#[derive(Debug, Clone, Copy)]
pub enum End<'a> {
    /// At its graph's end: its pull request merged, or the sub-items that took its work over closed it.
    Done,
    /// Before its end, saying why, such as its item cancelled.
    Stopped(&'a str),
    /// Before its end, saying why, for a new work that takes over its worktree, branch and pull request.
    TakenOver(&'a str),
}

/// The cell of the engineer's graph a work is done at, as the views read it.
const DONE_AT: &str = "E1";

/// The kinds of matter a work raises that wait on it: its questions to the architect, its failures, and its design
/// cards for the architect to look at. A side report it handed in is about something beside it, and stays.
const WAITING_ON_IT: &str = "('engineer_question', 'failure', 'design_card')";

/// Ends a work: the only place a work's end is written. Its matters that wait on it leave the architect's queue,
/// withdrawn, saying why. A work stopped before its end records why; a round of it still open, a question that waits
/// or a round the engine was cut off in, is cut off saying why, and a round that was running when the work ended hands
/// in nothing (`steps::agent`); its design cards are replaced, since the owner can no longer rule on them, and its asks
/// of the owner still open need no answer. A work of the engineer's graph then lets go of what it held: its sessions
/// end; unless a new work takes them over, its worktree is removed and, when it has a pull request that did not merge,
/// the pull request is closed with a comment saying why its work ended, while a work done at its end had its pull
/// request merged, or closed by the sub-items that took it over. Its branch stays. A wake of the architect holds no
/// worktree or pull request, and its line's session goes on in the project's next wake. A work done at its end that
/// stops later, as when its item is cancelled, let go of what it held at its end, and a work already ended is left as
/// it is. `projects` are the configuration's, which say where each project's checkout and repository are.
pub fn end(
    tx: &Transaction<'_>,
    work: i64,
    how: End<'_>,
    projects: &BTreeMap<String, config::Project>,
) -> Result<(), String> {
    type Row = (
        String,
        String,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<String>,
    );
    let (graph, at, worktree, pr, ended, item, project): Row = tx
        .query_row(
            "SELECT w.graph, w.at, w.worktree, w.pr, w.ended, w.item, i.project
             FROM works w LEFT JOIN items i ON i.id = w.item WHERE w.id = ?1",
            [work],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .map_err(|e| format!("work {work}: {e}"))?;
    if ended.is_some() {
        return Ok(());
    }
    let why = match how {
        End::Done => None,
        End::Stopped(why) | End::TakenOver(why) => Some(why),
    };
    let engineer = graph == crate::ENGINEER;
    if engineer {
        withdraw_matters(tx, work, why)?;
    }
    // A work of the engineer's graph at its end let go of what it held there.
    let done_before = engineer && db::is_cell(crate::ENGINEER, DONE_AT, &at) && why.is_some();
    if let Some(why) = why {
        tx.execute("UPDATE works SET ended = ?2 WHERE id = ?1", params![work, why])
            .map_err(sql)?;
    }
    if let Some(why) = why.filter(|_| !done_before) {
        if let Some(r) = work::open_round(tx, work)? {
            let cut = work::Ended {
                rejected: Some(json!({ "interrupted": why })),
                ..work::Ended::default()
            };
            work::finish_round(tx, r.id, &cut)?;
        }
        replace_design_cards(tx, work)?;
        close_asks(tx, work, why)?;
    }
    if !engineer || done_before {
        return Ok(());
    }
    end_sessions(tx, work)?;
    if matches!(how, End::TakenOver(_)) {
        return Ok(());
    }
    let place = project.as_ref().and_then(|p| projects.get(p));
    if let (Some(dir), Some(p)) = (&worktree, place) {
        remove_worktree(tx, work, p, dir)?;
    }
    if let (Some(why), Some(n), Some(p)) = (why, pr, place) {
        close_pr(tx, work, item, n, p, why)?;
    }
    Ok(())
}

// The shared ending, by the names the flow gives it on the cells a work ends at (`E1`, and `C1` for a work cancelled
// or taken over): each is one part of [`end`], which says when it is done.

/// `withdraw_matters`: what the work raised that waits on it leaves the architect's queue, saying why.
fn withdraw_matters(tx: &Transaction<'_>, work: i64, why: Option<&str>) -> Result<(), String> {
    let withdrawn = match why {
        Some(why) => format!("the work that raised it ended ({why})"),
        None => "the work that raised it is done".to_string(),
    };
    tx.execute(
        &format!(
            "UPDATE architect_matters SET withdrawn = ?2
             WHERE work = ?1 AND kind IN {WAITING_ON_IT} AND handled_round IS NULL AND withdrawn IS NULL"
        ),
        params![work, withdrawn],
    )
    .map_err(sql)?;
    Ok(())
}

/// `replace_design_cards`: the owner can no longer rule on the work's design cards.
fn replace_design_cards(tx: &Transaction<'_>, work: i64) -> Result<(), String> {
    tx.execute(
        "UPDATE design_cards SET status = 'replaced' WHERE status <> 'replaced' AND work = ?1",
        [work],
    )
    .map_err(sql)?;
    Ok(())
}

/// `close_asks`: the work's open asks of the owner need no answer.
fn close_asks(tx: &Transaction<'_>, work: i64, why: &str) -> Result<(), String> {
    let answer = json!({
        "by": "engine",
        "at": db::now(),
        "text": format!("No answer is needed: the work ended ({why})."),
    });
    tx.execute(
        "UPDATE owner_asks SET answer = ?2 WHERE answer IS NULL AND work = ?1",
        params![work, answer.to_string()],
    )
    .map_err(sql)?;
    Ok(())
}

/// `end_sessions`: the work's sessions end.
fn end_sessions(tx: &Transaction<'_>, work: i64) -> Result<(), String> {
    sessions::choose::end_work(tx, work)
}

/// `remove_worktree`: the work's worktree is removed once the transaction is written; its branch stays.
fn remove_worktree(tx: &Transaction<'_>, work: i64, p: &config::Project, dir: &str) -> Result<(), String> {
    outbox(
        tx,
        work,
        "remove_worktree",
        &json!({ "checkout": p.checkout, "dir": dir }),
    )
}

/// `close_pr`: the work's pull request, unless it merged, here or in an earlier work on the item that had it, is closed
/// with a comment saying why the work ended.
fn close_pr(
    tx: &Transaction<'_>,
    work: i64,
    item: Option<i64>,
    n: i64,
    p: &config::Project,
    why: &str,
) -> Result<(), String> {
    // A pull request that merged, here or in an earlier work on the item that had it, is not closed.
    let merged: bool = tx
        .query_row(
            &format!(
                "SELECT EXISTS (SELECT 1 FROM rounds r JOIN works x ON x.id = r.work
                                    WHERE x.item = ?1 AND x.pr = ?2 AND r.cell IN {} AND r.transition = 'merged')",
                db::cells_sql(crate::ENGINEER, &["M2"])
            ),
            params![item, n],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if !merged {
        let comment = format!("Its work ended without merging: {why}.");
        outbox(
            tx,
            work,
            "close_pr",
            &json!({ "repo": p.repo, "pr": n, "comment": comment }),
        )?;
    }
    Ok(())
}

/// Cancels an item, as `by`, the owner or its project's architect, does: it is not to be done, it leaves the owner's
/// queue, every work on it that has not ended ends, saying why, and it no longer depends on any item or holds any back.
/// Each item that depended on it is told, by where it stands: a work running on it, by an interjection from `by`, which
/// a work standing where it waits, for the items it depends on (`A6`), to enter the merge queue (`M1`) or for the
/// architect's reply (`A2`), takes at once to choose the way on (`A4`) by a row of its own; an item with no work running, by a
/// matter for its project's architect, to set straight.
pub fn cancel_item(
    tx: &Transaction<'_>,
    item: i64,
    why: &str,
    by: &str,
    projects: &BTreeMap<String, config::Project>,
) -> Result<(), String> {
    tx.execute("UPDATE items SET cancelled = ?2 WHERE id = ?1", params![item, why])
        .map_err(sql)?;
    tx.execute("DELETE FROM dispatch_queue WHERE item = ?1", [item])
        .map_err(sql)?;
    let works = ids(
        tx,
        "SELECT id FROM works WHERE item = ?1 AND ended IS NULL ORDER BY id",
        item,
    )?;
    let ended = format!("item cancelled: {why}");
    for w in works {
        end(tx, w, End::Stopped(&ended), projects)?;
    }
    let title: String = tx
        .query_row("SELECT title FROM items WHERE id = ?1", [item], |r| r.get(0))
        .map_err(sql)?;
    let dependents = work::query_json(
        tx,
        &format!(
            "SELECT l.item, l.needs, i.project, i.title,
                    (SELECT w.id FROM works w WHERE w.item = l.item AND w.graph = 'engineer' AND w.ended IS NULL
                       AND w.at NOT IN {} ORDER BY w.id DESC LIMIT 1) AS work
             FROM item_links l JOIN items i ON i.id = l.item
             WHERE l.depends_on = ?1 AND i.cancelled IS NULL ORDER BY l.item",
            db::cells_sql(crate::ENGINEER, &[DONE_AT])
        ),
        &[&item],
        100_000,
    )?;
    tx.execute("DELETE FROM item_links WHERE item = ?1 OR depends_on = ?1", [item])
        .map_err(sql)?;
    for d in dependents {
        let dependent = d["item"].as_i64().unwrap_or_default();
        let needs = d["needs"].as_str().unwrap_or_default();
        match d["work"].as_i64() {
            Some(w) => {
                let text = format!(
                    "Item {item} ({title}), which this item depended on for: {needs}, was cancelled and will not \
                     deliver it: {why}. This item no longer depends on it."
                );
                // The notice names the item cancelled: the cells where the work waits take it to choose the way on
                // by a row of their own (the engine's value `cancelled`).
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text, cancelled_item) VALUES (?1, 'engineer', ?2, ?3, ?4)",
                    params![w, by, text, item],
                )
                .map_err(sql)?;
            }
            None => {
                let what = format!(
                    "Item {item} ({title}), which item {dependent} ({}) depended on for: {needs}, was cancelled: {why}",
                    d["title"].as_str().unwrap_or_default()
                );
                let m = db::Matter {
                    project: d["project"].as_str().unwrap_or_default().to_string(),
                    kind: "set_straight".into(),
                    detail: Some("dependencies".into()),
                    what,
                    body: json!({ "cancelled": { "item": item, "title": title, "why": why, "by": by },
                                  "dependent": dependent, "needs": needs }),
                    item: Some(dependent),
                    raised: None,
                };
                db::queue_matter(tx, &m).map_err(sql)?;
            }
        }
    }
    Ok(())
}

/// The numbers a query of one number gives.
fn ids(tx: &Transaction<'_>, q: &str, arg: i64) -> Result<Vec<i64>, String> {
    let mut st = tx.prepare(q).map_err(sql)?;
    st.query_map([arg], |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// Queues an action to carry out once the transaction is in.
fn outbox(tx: &Transaction<'_>, work: i64, kind: &str, args: &Value) -> Result<(), String> {
    tx.execute(
        "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, ?2, ?3, ?4)",
        params![work, kind, args.to_string(), db::now()],
    )
    .map_err(sql)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use db::{Actor, Db};

    use super::*;

    /// Project p as the configuration has it: where its checkout and repository are.
    fn projects() -> BTreeMap<String, config::Project> {
        let p: config::Project = serde_json::from_value(json!({
            "repo": "o/p", "checkout": "/checkout", "design_goal": "simple", "design_paths": ["design/"],
            "test_paths": ["tests/"], "ci": { "fast": "ci", "full": ["ci"], "merge_checks": ["check"] }
        }))
        .unwrap();
        BTreeMap::from([("p".to_string(), p)])
    }

    /// An item of project p and a work on it at a cell, with its worktree and pull request, a round at that cell still
    /// open, a session going on, an ask of the owner, a design card, a question to the architect and a side report;
    /// gives the item and the work.
    fn a_work(tx: &Transaction<'_>, title: &str, at: &str, pr: i64) -> Result<(i64, i64), db::Error> {
        tx.execute(
            "INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
            [],
        )?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
            [title],
        )?;
        let item = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, worktree, branch, pr)
             VALUES (?1, 1, 'engineer', ?2, ?3, 'b', ?4)",
            params![item, at, format!("/worktrees/{item}"), pr],
        )?;
        let work = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, 1, ?2, '{}', 'c', 1)",
            params![work, at],
        )?;
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, 'engineer', 'claude', ?2)",
            params![work, format!("s{work}")],
        )?;
        tx.execute(
            "INSERT INTO owner_asks (work, round, body) VALUES (?1, 1, '{}')",
            [work],
        )?;
        tx.execute(
            "INSERT INTO design_cards (work, round, body, status) VALUES (?1, 1, '[]', 'open')",
            [work],
        )?;
        for kind in ["engineer_question", "report"] {
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: kind.into(),
                    what: format!("Work {work} asks"),
                    body: json!({}),
                    item: Some(item),
                    raised: Some((work, 1)),
                    ..db::Matter::default()
                },
            )?;
        }
        Ok((item, work))
    }

    fn one<T: db::rusqlite::types::FromSql>(db: &Db, q: &str) -> T {
        db.conn().query_row(q, [], |r| r.get(0)).unwrap()
    }

    /// The outbox's actions, each with the work it is for, its kind and its arguments.
    fn outbox(db: &Db) -> Vec<(i64, String, Value)> {
        work::query_json(db.conn(), "SELECT work, kind, args FROM outbox ORDER BY id", &[], 100)
            .unwrap()
            .into_iter()
            .map(|r| {
                (
                    r["work"].as_i64().unwrap_or_default(),
                    r["kind"].as_str().unwrap_or_default().to_string(),
                    r["args"].clone(),
                )
            })
            .collect()
    }

    // A cancelled item's work lets go of all it held: its round open is cut off, its sessions end, its ask of the owner
    // needs no answer, its design card is replaced, its question to the architect leaves the queue while its side
    // report stays, and its worktree's removal and its pull request's closing, saying why, are queued; its branch stays.
    #[test]
    fn a_cancelled_item_s_work_lets_go_of_all_it_held() {
        let mut db = Db::in_memory().unwrap();
        let (item, work) = db.write(Actor::Owner, |tx| a_work(tx, "Lex", "I1", 7)).unwrap();
        db.write(Actor::Owner, |tx| {
            cancel_item(tx, item, "a duplicate", "owner", &projects()).map_err(db::Error::Abort)
        })
        .unwrap();
        let ended: String = one(&db, "SELECT ended FROM works");
        assert_eq!(ended, "item cancelled: a duplicate");
        let open: i64 = one(&db, "SELECT count(*) FROM sessions WHERE ended IS NULL");
        assert_eq!(open, 0);
        let cut: String = one(&db, "SELECT json_extract(rejected, '$.interrupted') FROM rounds");
        assert_eq!(cut, ended);
        let answer: String = one(&db, "SELECT json_extract(answer, '$.text') FROM owner_asks");
        assert_eq!(
            answer,
            "No answer is needed: the work ended (item cancelled: a duplicate)."
        );
        let card: String = one(&db, "SELECT status FROM design_cards");
        assert_eq!(card, "replaced");
        let queued: Vec<String> = work::query_json(db.conn(), "SELECT kind FROM architect_queue", &[], 10)
            .unwrap()
            .iter()
            .map(|r| r["kind"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(queued, ["report"]);
        let withdrawn: String = one(
            &db,
            "SELECT withdrawn FROM architect_matters WHERE kind = 'engineer_question'",
        );
        assert_eq!(withdrawn, "the work that raised it ended (item cancelled: a duplicate)");
        let comment = "Its work ended without merging: item cancelled: a duplicate.";
        assert_eq!(
            outbox(&db),
            [
                (
                    work,
                    "remove_worktree".to_string(),
                    json!({ "checkout": "/checkout", "dir": format!("/worktrees/{item}") })
                ),
                (
                    work,
                    "close_pr".to_string(),
                    json!({ "repo": "o/p", "pr": 7, "comment": comment })
                ),
            ]
        );
        // A work already ended is left as it is.
        db.write(Actor::Owner, |tx| {
            end(tx, work, End::Stopped("again"), &projects()).map_err(db::Error::Abort)
        })
        .unwrap();
        assert_eq!(outbox(&db).len(), 2);
    }

    // A work done at its end lets go of its sessions and its worktree there, and leaves the round of its end open for
    // the end to close; its pull request merged. One taken over by a new work only ends its sessions, the new work
    // taking its worktree and pull request. A pull request that merged is not closed, and a work done at its end that
    // stops later only records why.
    #[test]
    fn each_way_of_ending_lets_go_of_what_is_its_own_to_let_go() {
        let mut db = Db::in_memory().unwrap();
        let (_, done) = db.write(Actor::Engine, |tx| a_work(tx, "Done", "E1", 1)).unwrap();
        let (_, taken) = db.write(Actor::Engine, |tx| a_work(tx, "Taken", "T3", 2)).unwrap();
        let (_, merged) = db.write(Actor::Engine, |tx| a_work(tx, "Merged", "L1", 3)).unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, 2, 'M2', '{}', '{}', 'merged', 'c', 2, 2)",
                [merged],
            )?;
            end(tx, done, End::Done, &projects()).map_err(db::Error::Abort)?;
            end(tx, taken, End::TakenOver("stopped: a new flow"), &projects()).map_err(db::Error::Abort)?;
            end(tx, merged, End::Stopped("item cancelled: late"), &projects()).map_err(db::Error::Abort)
        })
        .unwrap();
        let kinds: Vec<(i64, String)> = outbox(&db).into_iter().map(|x| (x.0, x.1)).collect();
        assert_eq!(
            kinds,
            [
                (done, "remove_worktree".to_string()),
                (merged, "remove_worktree".to_string())
            ]
        );
        let open: i64 = one(&db, "SELECT count(*) FROM sessions WHERE ended IS NULL");
        assert_eq!(open, 0);
        let ended: Option<String> = one(&db, &format!("SELECT ended FROM works WHERE id = {done}"));
        assert_eq!(ended, None);
        let still: i64 = one(
            &db,
            &format!("SELECT count(*) FROM rounds WHERE work = {done} AND ended_at IS NULL"),
        );
        assert_eq!(still, 1);
        db.write(Actor::Owner, |tx| {
            end(tx, done, End::Stopped("item cancelled: after all"), &projects()).map_err(db::Error::Abort)
        })
        .unwrap();
        let card: String = one(&db, &format!("SELECT status FROM design_cards WHERE work = {done}"));
        assert_eq!(card, "open");
        assert_eq!(outbox(&db).len(), 2);
    }

    // A cancelled item depends on nothing and holds back nothing, and each item that depended on it is told: a work
    // running on it by an interjection from whoever cancelled it, an item with no work running by a matter for its
    // project's architect to set straight.
    #[test]
    fn the_items_that_depended_on_a_cancelled_item_are_told() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Architect, |tx| {
            let (lexer, _) = a_work(tx, "Lexer", "I1", 1)?;
            let (waiting, w) = a_work(tx, "Waiting", "M1", 2)?;
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Idle', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let idle = tx.last_insert_rowid();
            db::add_dependency(tx, waiting, lexer, "its tokens", "parallel")?;
            db::add_dependency(tx, idle, lexer, "its tokens too", "blocking")?;
            cancel_item(tx, lexer, "no longer needed", "architect", &projects()).map_err(db::Error::Abort)?;
            let links: i64 = tx.query_row("SELECT count(*) FROM item_links", [], |r| r.get(0))?;
            assert_eq!(links, 0);
            let (speaker, text): (String, String) = tx.query_row(
                "SELECT speaker, text FROM interjections WHERE work = ?1 AND line = 'engineer'",
                [w],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            assert_eq!(speaker, "architect");
            assert!(
                text.contains(&format!("Item {lexer} (Lexer)"))
                    && text.contains("its tokens")
                    && text.contains("no longer needed"),
                "{text}"
            );
            let (kind, detail, item, what): (String, String, i64, String) = tx.query_row(
                "SELECT kind, detail, item, what FROM architect_matters WHERE work IS NULL",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
            assert_eq!(
                (kind.as_str(), detail.as_str(), item),
                ("set_straight", "dependencies", idle)
            );
            assert!(
                what.contains("its tokens too") && what.contains("no longer needed"),
                "{what}"
            );
            Ok(())
        })
        .unwrap();
    }
}
