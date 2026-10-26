//! What the engine does at each kind of cell: an agent's round, its own steps, the tool cells, and asking the owner.

use db::Actor;
use db::rusqlite::params;
use flow::{CompiledCell, CompiledGraph, CompiledRow};
use serde_json::{Map, Value, json};

use crate::agent::{self, Request};
use crate::inputs::{self, Gathered, at_path};
use crate::work::{self, Ended, Item, Round, Work};
use crate::{Advance, Engine, MAX_REJECTED, actions, checks, git, prompt, tools, values};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A round the engine stopped in the middle of is closed as not done; its cell runs again.
fn close_interrupted(e: &mut Engine, w: &Work) -> Result<Option<Round>, String> {
    let Some(r) = work::open_round(e.db.conn(), w.id)? else {
        return Ok(None);
    };
    let keep = r.cell.as_deref() == Some(w.pos.at.as_str()) && g_step_waits(e, w, &r).unwrap_or(false);
    if keep {
        return Ok(Some(r));
    }
    let ended = Ended {
        rejected: Some(json!({ "error": "the engine stopped before this round ended; it runs again" })),
        ..Ended::default()
    };
    e.db.write(Actor::Engine, |tx| {
        work::finish_round(tx, r.id, &ended).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    Ok(None)
}

/// Whether an open round is one that waits on purpose: a question to the owner or the architect.
fn g_step_waits(e: &mut Engine, w: &Work, r: &Round) -> Result<bool, String> {
    let g = e.graph_of(w)?;
    let cell = r.cell.as_deref().and_then(|c| g.cell(c));
    Ok(cell.is_some_and(|c| matches!(c.step.as_deref(), Some("ask_owner" | "ask_architect"))))
}

fn need_met(project: &config::Project, need: &str) -> bool {
    project.has(need).unwrap_or(false)
}

/// The inputs as recorded in a round: what each was, and its value.
fn recorded_inputs(gathered: &[Gathered]) -> Value {
    Value::Array(
        gathered
            .iter()
            .filter(|x| !x.itself())
            .map(|x| json!({ "what": x.input.what, "value": x.value, "held": x.held() }))
            .collect(),
    )
}

pub fn agent(e: &mut Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Advance, String> {
    close_interrupted(e, w)?;
    let item = e.item_of(w)?;
    let project = e.project_of(w)?;
    if let Some(need) = &cell.needs
        && !need_met(&project, need)
    {
        return not_applicable(e, w, g, cell, need);
    }
    let at_cell = work::rounds_at(e.db.conn(), w.id, &cell.id)?;
    let rejected: Vec<&Round> = at_cell.iter().take_while(|r| r.rejected.is_some()).collect();
    let reason = rejected
        .first()
        .map(|r| r.rejected.clone().unwrap_or_default().to_string());
    if rejected.len() >= MAX_REJECTED {
        let why = format!(
            "{} returns of cell {} in a row were sent back; the last: {}",
            rejected.len(),
            cell.id,
            reason.unwrap_or_default()
        );
        e.pause(w.id, &why)?;
        (e.say)(&format!("work {} paused: {why}", w.id));
        return Ok(Advance::Stopped);
    }
    let line = cell.line.clone().unwrap_or_else(|| "engineer".to_string());
    let (level, cli) = e
        .config
        .config
        .level(&item.level)
        .map(|(l, c)| (l.clone(), c.clone()))
        .ok_or_else(|| format!("item {}: no such level in the configuration: {}", item.id, item.level))?;
    let cli_name = level.cli.clone();
    let session: Option<(i64, String)> =
        e.db.conn()
            .query_row(
                "SELECT id, cli_session FROM sessions WHERE work = ?1 AND line = ?2",
                params![w.id, line],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
    // A session the CLI has started is resumed; one it never started is started under the id kept for it, and is
    // given everything its line holds.
    let line_rounds: Vec<Round> = match &session {
        Some((sid, _)) => work::rounds(e.db.conn(), w.id)?
            .into_iter()
            .filter(|r| r.session == Some(*sid))
            .collect(),
        None => Vec::new(),
    };
    let used = line_rounds
        .iter()
        .any(|r| r.usage.as_ref().is_some_and(|u| !u.is_null()));
    let compacted = line_rounds
        .iter()
        .rev()
        .find(|r| r.ended_at.is_some())
        .is_some_and(|r| r.compactions > 0);
    let give_all = !used || compacted;
    let gathered = inputs::gather(e, w, g, cell, &item)?;
    let given = values::given(e, w, g, cell, &item, &gathered)?;
    let interjections: Vec<Value> = inputs::db_rows(e, w, &item, "waiting_interjections", Some("work"))?
        .into_iter()
        .filter(|r| r["line"] == line.as_str())
        .collect();
    let columns = prompt::columns(e, w, g, cell, &item);
    let text = prompt::agent(
        e,
        w,
        g,
        cell,
        &item,
        &gathered,
        &interjections,
        reason.as_deref(),
        &given,
        give_all,
        &columns,
    );
    let head = w.worktree.as_deref().and_then(|wt| git::head(wt).ok());
    let change = w
        .worktree
        .as_deref()
        .and_then(|wt| values::change_hash(wt, &project.main));
    let ids: Vec<i64> = interjections
        .iter()
        .filter_map(|i| i["interjection"].as_i64())
        .collect();
    let input = json!({
        "prompt": text,
        "inputs": recorded_inputs(&gathered),
        "engine": { "head": head, "change": change, "columns": columns, "interjections": ids },
    });
    let commit = e.config.commit.clone();
    let fresh = agent::new_session_id();
    let (sid, resume, new_session) = match session {
        Some((id, s)) if used => (id, Some(s.clone()), s),
        Some((id, s)) => (id, None, s),
        None => {
            let id =
                e.db.write(Actor::Engine, |tx| {
                    tx.execute(
                        "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, ?2, ?3, ?4)",
                        params![w.id, line, cli_name, fresh],
                    )?;
                    Ok(tx.last_insert_rowid())
                })
                .map_err(sql)?;
            (id, None, fresh)
        }
    };
    let round =
        e.db.write(Actor::Engine, |tx| {
            work::start_round(tx, w.id, &cell.id, Some(sid), &input, &commit, Some(&level.model))
                .map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    let wt = w.worktree.clone().unwrap_or_else(|| project.checkout.clone());
    let req = Request {
        work: w.id,
        cell: cell.id.clone(),
        line: line.clone(),
        cli,
        model: level.model.clone(),
        effort: level.effort.clone(),
        worktree: wt,
        resume,
        new_session: new_session.clone(),
        prompt: text,
        schema: cell.schema.clone().unwrap_or_else(|| json!({ "type": "object" })),
    };
    (e.say)(&format!("work {} cell {}: round {round} of the {line}", w.id, cell.id));
    let reply = e.agents.run(&req);
    if reply.session != new_session && !reply.session.is_empty() {
        let s = reply.session.clone();
        e.db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE sessions SET cli_session = ?2 WHERE id = ?1", params![sid, s])?;
            Ok(())
        })
        .map_err(sql)?;
    }
    let ended = Ended {
        usage: Some(reply.usage.clone()).filter(|u| !u.is_null()),
        model: reply.model.clone(),
        compactions: reply.compactions,
        ..Ended::default()
    };
    let Some(output) = reply.output.clone() else {
        let why = reply
            .error
            .clone()
            .unwrap_or_else(|| "the round handed in nothing".to_string());
        return reject(e, round, ended, json!({ "error": why }));
    };
    let row = match g.take(&cell.id, &output, &given) {
        Ok(r) => r.clone(),
        Err(why) => {
            return reject(
                e,
                round,
                Ended {
                    output: Some(output),
                    ..ended
                },
                json!({ "format": why }),
            );
        }
    };
    let head_after = w.worktree.as_deref().and_then(|wt| git::head(wt).ok());
    let ctx = checks::Ctx {
        e,
        w,
        g,
        cell,
        item: &item,
        project: &project,
        output: &output,
        unwrapped: g.unwrap(&output),
        head_start: head.clone(),
        columns: &columns,
        interjections: &interjections,
    };
    if let Err(why) = checks::run(&ctx) {
        return reject(
            e,
            round,
            Ended {
                output: Some(output),
                ..ended
            },
            json!({ "check": why }),
        );
    }
    finish(e, w, g, cell, round, output, &row, ended, &ids, head_after)
}

fn reject(e: &mut Engine, round: i64, ended: Ended, why: Value) -> Result<Advance, String> {
    (e.say)(&format!("round {round} sent back: {why}"));
    let ended = Ended {
        rejected: Some(why),
        transition: None,
        ..ended
    };
    e.db.write(Actor::Engine, |tx| {
        work::finish_round(tx, round, &ended).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    Ok(Advance::Moved)
}

/// A gate whose need the project's configuration does not answer is not asked: the engine takes its not-applicable
/// row, and says why.
fn not_applicable(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    need: &str,
) -> Result<Advance, String> {
    let desc = e
        .flow(w.flow_version)?
        .engine
        .needs
        .get(need)
        .cloned()
        .unwrap_or_default();
    let why = format!("Not asked: this question needs what this project does not have. {desc}");
    let mut output = Map::new();
    output.insert(
        g.within.clone(),
        json!({ "answer": { "verdict": "not_applicable", "why": why } }),
    );
    output.insert(
        "progress".into(),
        json!(format!("The engine answered not applicable: {need}.")),
    );
    output.insert("side_reports".into(), json!([]));
    let output = Value::Object(output);
    let item = e.item_of(w)?;
    let gathered = inputs::gather(e, w, g, cell, &item)?;
    let given = values::given(e, w, g, cell, &item, &gathered)?;
    let row = g.take(&cell.id, &output, &given)?.clone();
    let commit = e.config.commit.clone();
    let input = json!({ "engine": { "not_applicable": need } });
    let round =
        e.db.write(Actor::Engine, |tx| {
            work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None)
}

/// Ends a round that took a row: the round, what the row and the cell's own actions do, and where the work goes,
/// in one transaction.
#[allow(clippy::too_many_arguments)]
pub fn finish(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    round: i64,
    output: Value,
    row: &CompiledRow,
    ended: Ended,
    delivered: &[i64],
    head_after: Option<String>,
) -> Result<Advance, String> {
    let next = g.step(&w.pos, row)?;
    let mut moved = w.clone();
    moved.pos = next;
    let ctx = actions::Ctx {
        work: w.clone(),
        item: e.item_of(w).ok(),
        project: e.project_of(w).ok(),
        cli: e
            .item_of(w)
            .ok()
            .and_then(|i| e.config.config.level(&i.level).map(|(l, _)| l.cli.clone())),
        round,
        cell: cell.clone(),
        output: output.clone(),
        unwrapped: if cell.kind == "agent" {
            g.unwrap(&output)
        } else {
            output.clone()
        },
        within: g.within.clone(),
        data: e.data.clone(),
    };
    let ended = Ended {
        output: Some(output),
        transition: Some(row.name.clone()),
        ..ended
    };
    let mut done: Vec<String> = Vec::new();
    e.db.write(Actor::Engine, |tx| {
        work::finish_round(tx, round, &ended).map_err(db::Error::Abort)?;
        if let Some(h) = &head_after {
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.head_after', ?2) WHERE id = ?1",
                params![round, h],
            )?;
        }
        for id in delivered {
            tx.execute(
                "UPDATE interjections SET delivered_round = ?2 WHERE id = ?1",
                params![id, round],
            )?;
        }
        for a in &cell.always {
            if let Some(name) = &a.action
                && at_path(&ctx.unwrapped, &a.at).is_some()
            {
                actions::apply(&ctx, tx, name, Some(a.at.as_str())).map_err(db::Error::Abort)?;
                done.push(name.clone());
            }
        }
        for d in &row.actions {
            actions::apply(&ctx, tx, d.name(), d.at()).map_err(db::Error::Abort)?;
            done.push(d.name().to_string());
        }
        moved.save_position(tx).map_err(db::Error::Abort)?;
        Ok(())
    })
    .map_err(sql)?;
    let did = if done.is_empty() {
        String::new()
    } else {
        format!(" ({})", done.join(", "))
    };
    (e.say)(&format!(
        "work {} cell {} took \"{}\"{did}: now at {}",
        w.id, cell.id, row.name, moved.pos.at
    ));
    Ok(Advance::Moved)
}

/// Ends the round at the graph's end, which has no rows: the work is done.
fn finish_end(e: &mut Engine, w: &Work, round: i64, output: Value) -> Result<Advance, String> {
    let ended = Ended {
        output: Some(output),
        ..Ended::default()
    };
    e.db.write(Actor::Engine, |tx| {
        work::finish_round(tx, round, &ended).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    (e.say)(&format!("work {} done", w.id));
    Ok(Advance::Moved)
}

pub fn engine(e: &mut Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Advance, String> {
    let item = e.item_of(w)?;
    let open = close_interrupted(e, w)?;
    let step = cell.step.clone();
    if matches!(step.as_deref(), Some("ask_owner" | "ask_architect")) {
        return ask(e, w, g, cell, &item, open);
    }
    if step.as_deref() == Some("merge") {
        let ahead: Option<i64> =
            e.db.conn()
                .query_row(
                    "SELECT work FROM merge_queue WHERE project = ?1 LIMIT 1",
                    [&item.project],
                    |r| r.get(0),
                )
                .ok();
        if ahead.is_some_and(|a| a != w.id) {
            return Ok(Advance::Waits(format!(
                "work {} is ahead in the merge queue",
                ahead.unwrap_or_default()
            )));
        }
    }
    let gathered = inputs::gather(e, w, g, cell, &item)?;
    let given = values::given(e, w, g, cell, &item, &gathered)?;
    let commit = e.config.commit.clone();
    let input = json!({ "inputs": recorded_inputs(&gathered), "given": given });
    let quick = matches!(step.as_deref(), None | Some("choose_checks") | Some("check_plan"));
    if quick {
        let output = match step.as_deref() {
            Some("choose_checks") => choose_checks(e, w, g)?,
            Some("check_plan") => json!({ "plan": { "verdict": "fits" } }),
            _ => json!({}),
        };
        let row = g.take(&cell.id, &output, &given)?.clone();
        if row.waits {
            // A row that waits, taken again with nothing new, is not recorded again.
            let last = work::rounds(e.db.conn(), w.id)?.pop();
            if last.is_some_and(|r| {
                r.cell.as_deref() == Some(cell.id.as_str()) && r.transition.as_deref() == Some(row.name.as_str())
            }) {
                return Ok(Advance::Waits(format!("cell {} \"{}\"", cell.id, row.name)));
            }
        }
        let round =
            e.db.write(Actor::Engine, |tx| {
                work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
            })
            .map_err(sql)?;
        return finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None);
    }
    // A step that acts outside the database starts its round first, so what it does is recorded under it.
    let round =
        e.db.write(Actor::Engine, |tx| {
            work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    (e.say)(&format!(
        "work {} cell {}: {}",
        w.id,
        cell.id,
        step.clone().unwrap_or_default()
    ));
    let output = match step.as_deref() {
        Some("start_work") => start_work(e, w, &item)?,
        Some("run_tools") => tools::run_tools(e, w, &item, round, &gathered)?,
        Some("full_ci_on_draft") => tools::full_ci(e, w, &item, round)?,
        Some("merge") => tools::merge(e, w, &item, round)?,
        Some("watch_main") => tools::watch_main(e, w, g, &item, round)?,
        Some("end_work") => json!({}),
        Some(other) => return Err(format!("no such engine step: {other}")),
        None => json!({}),
    };
    if cell.rows.is_empty() {
        let output = if step.as_deref() == Some("end_work") {
            end_work(e, w)?
        } else {
            output
        };
        return finish_end(e, w, round, output);
    }
    let row = g.take(&cell.id, &output, &given)?.clone();
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None)
}

/// Cell 0: a worktree on a new branch from main, and the id the engineer's session will have.
fn start_work(e: &mut Engine, w: &Work, item: &Item) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let hosts = &e.config.config.install.machines.hosts;
    let (machine, host) = hosts.iter().next().ok_or("the configuration names no machine")?;
    let dir = host.worktrees.join(format!("{}-{}", item.project, w.id));
    let branch = format!("wf-agents/{}-{}", item.id, w.id);
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let head = git::create_worktree(&project.checkout, &dir, &branch, &project.main)?;
    Ok(json!({
        "place": {
            "machine": machine,
            "worktree": dir.display().to_string(),
            "branch": branch,
            "head": head,
            "session": agent::new_session_id(),
        }
    }))
}

/// Cell 20: the worktree is removed once the round is written; the branch stays.
fn end_work(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let project = e.project_of(w)?;
    if let Some(wt) = &w.worktree {
        let args = json!({ "checkout": project.checkout, "dir": wt });
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, 'remove_worktree', ?2, ?3)",
                params![w.id, args.to_string(), db::now()],
            )?;
            Ok(())
        })
        .map_err(sql)?;
    }
    Ok(json!({}))
}

/// Cells 15 and 16: the question becomes a card for the owner, and the work waits for the answer. In this version
/// the owner answers for the architect too, in the architect's form.
fn ask(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    open: Option<Round>,
) -> Result<Advance, String> {
    let to = if cell.step.as_deref() == Some("ask_owner") {
        "owner"
    } else {
        "architect"
    };
    if let Some(r) = open.filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
        let card: Option<(i64, String, String)> =
            e.db.conn()
                .query_row("SELECT id, status, rulings FROM cards WHERE round = ?1", [r.id], |x| {
                    Ok((x.get(0)?, x.get(1)?, x.get(2)?))
                })
                .ok();
        let Some((id, status, rulings)) = card else {
            return Err(format!("round {} asks, but has no card", r.id));
        };
        if status != "ruled" {
            return Ok(Advance::Waits(format!("card {id} waits for the owner")));
        }
        let rulings: Value = serde_json::from_str(&rulings).unwrap_or(Value::Null);
        let last = rulings
            .as_array()
            .and_then(|rs| rs.last())
            .cloned()
            .unwrap_or(Value::Null);
        let text = last["text"].as_str().unwrap_or_default().to_string();
        let output = if to == "owner" {
            json!({ "reply": { "text": text } })
        } else {
            match last.get("reply").filter(|x| x.is_object()) {
                Some(reply) => json!({ "reply": reply }),
                None => {
                    json!({ "reply": { "text": text, "why": last["why"].as_str().unwrap_or("The owner answered for the architect.") } })
                }
            }
        };
        let gathered = inputs::gather(e, w, g, cell, item)?;
        let given = values::given(e, w, g, cell, item, &gathered)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None);
    }
    let gathered = inputs::gather(e, w, g, cell, item)?;
    let asked: Vec<Value> = gathered
        .iter()
        .filter(|x| x.value.is_some())
        .map(|x| json!({ "what": x.input.what, "value": x.value }))
        .collect();
    let body = json!({ "to": to, "work": w.id, "item": item.id, "title": item.title, "asked": asked });
    let input = json!({ "inputs": recorded_inputs(&gathered) });
    let commit = e.config.commit.clone();
    let text = format!("Work {} on item {} ({}) asks the {to}: card", w.id, item.id, item.title);
    let card =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            tx.execute(
                "INSERT INTO cards (round, kind, body, status) VALUES (?1, 'ask', ?2, 'open')",
                params![round, body.to_string()],
            )?;
            let card = tx.last_insert_rowid();
            let note = json!({ "text": format!("{text} {card}") });
            tx.execute(
                "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, 'notify', ?2, ?3)",
                params![w.id, note.to_string(), db::now()],
            )?;
            Ok(card)
        })
        .map_err(sql)?;
    Ok(Advance::Waits(format!("card {card} waits for the owner")))
}

/// Cell 18: what runs next, from what the design chose, what the review accepted, whether the change changed since
/// each side last looked, and what the fix answered.
fn choose_checks(e: &mut Engine, w: &Work, g: &CompiledGraph) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let conn = e.db.conn();
    let accepted = |c: &str| -> Result<Vec<Round>, String> {
        Ok(work::rounds_at(conn, w.id, c)?
            .into_iter()
            .filter(|r| r.transition.is_some())
            .collect())
    };
    let reviews = accepted("5a")?;
    let adversarial = accepted("6")?;
    let fixes = accepted("8")?;
    let chose = inputs::last_return(e, w, g, "3f", "adversarial")?
        .and_then(|r| at_path(&r, "adversarial.verdict"))
        .is_some_and(|v| v == "yes");
    let not_accepted = inputs::last_return(e, w, g, "5a", "no_adversarial")?
        .and_then(|r| at_path(&r, "no_adversarial.verdict"))
        .is_some_and(|v| v == "does_not_stand");
    let now = w
        .worktree
        .as_deref()
        .and_then(|wt| values::change_hash(wt, &project.main));
    let changed_since = |rs: &[Round]| {
        rs.first()
            .is_some_and(|r| r.input["engine"]["change"].as_str() != now.as_deref())
    };
    let answers_after = |rs: &[Round], kinds: &[&str]| -> bool {
        let since = rs.first().map_or(0, |r| r.id);
        fixes.iter().filter(|f| f.id > since).any(|f| {
            f.output
                .as_ref()
                .and_then(|o| o[&g.within]["rows"].as_array().cloned())
                .unwrap_or_default()
                .iter()
                .any(|row| row["answer"].as_str().is_some_and(|a| kinds.contains(&a)))
        })
    };
    let adv = if (chose || not_accepted) && adversarial.is_empty() {
        "all"
    } else if !adversarial.is_empty() && changed_since(&adversarial) {
        "changed"
    } else if !adversarial.is_empty() && answers_after(&adversarial, &["test_wrong"]) {
        "disputes"
    } else {
        "skipped"
    };
    let mut review = if reviews.is_empty() {
        "all"
    } else if changed_since(&reviews) {
        "changed"
    } else if answers_after(&reviews, &["disagree", "cannot_verify"]) {
        "disputes"
    } else {
        "none"
    };
    if adv != "skipped" && matches!(review, "none" | "disputes") {
        review = "changed";
    }
    Ok(json!({ "plan": { "review": review, "adversarial": adv } }))
}
