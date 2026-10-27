//! What the engine does at each kind of cell: an agent's round, its own steps, the tool cells, and asking the owner.

use db::Actor;
use db::rusqlite::params;
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph, CompiledRow};
use serde_json::{Map, Value, json};

use crate::agent::{self, Request};
use crate::failed::{self, Failed};
use crate::inputs::{self, Gathered, at_path};
use crate::message::{self, Held};
use crate::work::{self, Ended, Item, Round, Work};
use crate::{Advance, Engine, actions, checks, git, tools, values};

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
    // A round left open when the work was paused, and the work let go on since, ended with the pause; any other was
    // cut off by the engine stopping.
    let rejected = match work::last_resume(e.db.conn(), w.id)? {
        Some((at, why)) if at > r.started_at => json!({ "error": format!("the work was paused: {why}") }),
        _ => json!({ "interrupted": "the engine stopped before this round ended; it runs again" }),
    };
    let ended = Ended {
        rejected: Some(rejected),
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

/// The inputs of a step that is not an agent's round, as its round records them: the value of each, by its sources,
/// without what the flow says each is.
fn recorded_inputs(gathered: &[Gathered]) -> Value {
    Value::Array(
        gathered
            .iter()
            .filter(|x| !x.itself())
            .map(|x| json!({ "from": x.input.from, "value": x.value }))
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
    let section = failed::section(e, g)?;
    let at_cell = work::rounds_at(e.db.conn(), w.id, &cell.id)?;
    // A round the engine itself cut off and a run that ended without a return are not returns sent back, and a work
    // the owner let go on after it paused counts afresh.
    let resumed = work::resumed_at(e.db.conn(), w.id)?;
    if let Some(why) = failed::waiting(e.retry_wait, &at_cell, resumed, &section) {
        return Ok(Advance::Waits(why));
    }
    let sent_back = failed::sent_back(&at_cell, resumed);
    let pause_after = section.sent_back.pause_after as usize;
    if sent_back.len() >= pause_after {
        let reason = sent_back
            .first()
            .map(|r| r.rejected.clone().unwrap_or_default().to_string())
            .unwrap_or_default();
        let why = format!(
            "{} returns of cell {} in a row were sent back; the last: {reason}",
            sent_back.len(),
            cell.id
        );
        return failed::pause(e, w, &why);
    }
    // The advisor runs one level above the item's: it is the engineer, with a higher model.
    let line_of_cell = cell.line.clone().unwrap_or_else(|| "engineer".to_string());
    let level_name = e.config.config.level_of_line(&item.level, &line_of_cell);
    let (level, cli) = e
        .config
        .config
        .level(&level_name)
        .map(|(l, c)| (l.clone(), c.clone()))
        .ok_or_else(|| format!("item {}: no such level in the configuration: {}", item.id, level_name))?;
    let cli_name = level.cli.clone();
    let Composed {
        line,
        session,
        line_rounds,
        given,
        interjections,
        columns,
        number,
        msg,
    } = compose(e, w, g, cell, &item)?;
    // A session the CLI has started is resumed; one it never started is started under the id kept for it.
    let used = line_rounds
        .iter()
        .any(|r| r.usage.as_ref().is_some_and(|u| !u.is_null()));
    let head = w.worktree.as_deref().and_then(|wt| git::head(wt).ok());
    let change = w
        .worktree
        .as_deref()
        .and_then(|wt| values::change_hash(wt, &project.main));
    let ids: Vec<i64> = interjections
        .iter()
        .filter_map(|i| i["interjection"].as_i64())
        .collect();
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
    let wt = w.worktree.clone().unwrap_or_else(|| project.checkout.clone());
    let req = Request {
        work: w.id,
        cell: cell.id.clone(),
        line: line.clone(),
        cli,
        model: level.model.clone(),
        effort: level.effort.clone(),
        worktree: wt,
        config_dir: e.cli_home(&cli_name),
        resume,
        new_session: new_session.clone(),
        prompt: msg.text.clone(),
        schema: cell.schema.clone().unwrap_or_else(|| json!({ "type": "object" })),
        refused: e.config.config.install.agents.permissions.refused(&line),
    };
    let cli_version = e.cli_version(&cli_name);
    // The record keeps each large text once, in `contents`: the conditional inputs given, the task, the
    // structured-output schema.
    let mut kept = msg.kept.clone();
    let call = kept_call(agent::call(&req, &agent::command_line(&req), &cli_version), &mut kept);
    let input = json!({
        "message": msg.stored,
        "conditional": msg.record(),
        "call": call,
        "engine": { "head": head, "change": change, "columns": columns, "interjections": ids },
    });
    let round =
        e.db.write(Actor::Engine, |tx| {
            db::keep(tx, &kept)?;
            work::start_numbered(
                tx,
                Some(number),
                w.id,
                &cell.id,
                Some(sid),
                &input,
                &commit,
                Some(&level.model),
            )
            .map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    (e.say)(&format!("work {} cell {}: round {round} of the {line}", w.id, cell.id));
    let reply = e.agents.run(&req, &|what: &str| (e.activity)(w.id, round, what));
    if reply.session != new_session && !reply.session.is_empty() {
        let s = reply.session.clone();
        e.db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE sessions SET cli_session = ?2 WHERE id = ?1", params![sid, s])?;
            Ok(())
        })
        .map_err(sql)?;
    }
    let ended = Ended {
        usage: Some(round_cost(reply.usage.clone(), &line_rounds)).filter(|u| !u.is_null()),
        model: reply.model.clone(),
        compactions: reply.compactions,
        command: (!reply.command.is_empty()).then(|| reply.command.clone()),
        ..Ended::default()
    };
    let Some(output) = reply.output.clone() else {
        let f = Failed {
            w,
            cell,
            round,
            section: &section,
        };
        return f.handle(e, ended, &reply);
    };
    e.returned();
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

/// A round's call as its record keeps it: the structured-output schema the command line gives, the same for every
/// round of a graph, kept once in `contents` and referred to there.
fn kept_call(mut call: Value, kept: &mut Vec<(String, String)>) -> Value {
    if let Some(args) = call["command"].as_array_mut()
        && let Some(i) = args.iter().position(|a| a == "--json-schema").map(|i| i + 1)
        && let Some(schema) = args.get(i).and_then(Value::as_str).map(str::to_string)
        && schema.len() >= db::STORED
    {
        let h = db::content_hash(&schema);
        args[i] = json!({ "content": h });
        if !kept.iter().any(|(k, _)| *k == h) {
            kept.push((h, schema));
        }
    }
    call
}

/// What an agent's round at a cell is built from, and the message it is sent.
struct Composed {
    line: String,
    /// The line's session: its id and the CLI's id for it; none before the line's first round.
    session: Option<(i64, String)>,
    /// The session's rounds, oldest first.
    line_rounds: Vec<Round>,
    given: Map<String, Value>,
    /// The interjections waiting for the line.
    interjections: Vec<Value>,
    columns: Vec<(String, String)>,
    /// The number the round gets.
    number: i64,
    msg: message::Message,
}

/// Builds the message of an agent's round at a cell from the work as it stands, running and recording nothing.
fn compose(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell, item: &Item) -> Result<Composed, String> {
    let line = cell.line.clone().unwrap_or_else(|| "engineer".to_string());
    let session: Option<(i64, String)> =
        e.db.conn()
            .query_row(
                "SELECT id, cli_session FROM sessions WHERE work = ?1 AND line = ?2",
                params![w.id, line],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
    let work_rounds = work::rounds(e.db.conn(), w.id)?;
    let line_rounds: Vec<Round> = match &session {
        Some((sid, _)) => work_rounds
            .iter()
            .filter(|r| r.session == Some(*sid))
            .cloned()
            .collect(),
        None => Vec::new(),
    };
    let held = Held::of(&line_rounds);
    let registry = message::registry(&work_rounds);
    let gathered = inputs::gather(e, w, g, cell, item)?;
    let given = values::given(e, w, g, cell, item, &gathered)?;
    let mut interjections: Vec<Value> = inputs::db_rows(e, w, item, "waiting_interjections", Some("work"))?
        .into_iter()
        .filter(|r| r["line"] == line.as_str())
        .collect();
    let columns = message::columns(e, w, g, cell, item);
    let number = work::next_round(e.db.conn())?;
    let msg = message::Context {
        e,
        w,
        g,
        cell,
        item,
        round: number,
        rounds: &work_rounds,
        gathered: &gathered,
        given: &given,
        columns: &columns,
        held: &held,
        registry: &registry,
    }
    .build();
    // A short message gives no interjection: those the round it goes on from was given are still this round's, and
    // any that came since wait for the next whole message.
    if let Some(prev) = msg.continues {
        let given: Vec<Value> = work_rounds
            .iter()
            .find(|r| r.id == prev)
            .and_then(|r| r.input["engine"]["interjections"].as_array().cloned())
            .unwrap_or_default();
        interjections.retain(|i| given.contains(&i["interjection"]));
    }
    Ok(Composed {
        line,
        session,
        line_rounds,
        given,
        interjections,
        columns,
        number,
        msg,
    })
}

/// What a work's next round would be, as the engine would take it now, with nothing run or recorded: its cell, and
/// for an agent's round the message the agent would be sent, or why no agent is asked.
pub struct NextRound {
    pub cell: String,
    pub title: String,
    /// agent, engine, tool or owner.
    pub kind: String,
    pub line: Option<String>,
    pub message: Option<String>,
    pub step: Option<String>,
    /// For an agent's cell whose need the project does not answer: why no agent is asked, the engine answering the
    /// cell itself as its round would.
    pub not_asked: Option<String>,
    /// For a tool cell: what each tool asked for will do.
    pub tools: Vec<String>,
    /// For a cell whose judgment now takes a row that waits, such as the merge gate waiting for the owner's ruling
    /// on a design card: what it waits for. No round is offered until that comes.
    pub waits: Option<String>,
    /// The design card it waits for a ruling on, when it does.
    pub design_card: Option<i64>,
    /// The ask of the owner it waits for an answer to, when it does.
    pub ask: Option<i64>,
}

pub fn next_round(e: &mut Engine, work: i64) -> Result<NextRound, String> {
    let w = e.work(work)?;
    if w.ended.is_some() || w.paused.is_some() || e.done(&w)? {
        return Err(format!("work {work} has no next round: it is ended, paused or done"));
    }
    let g = e.graph_of(&w)?;
    let cell = g
        .cell(&w.pos.at)
        .cloned()
        .ok_or_else(|| format!("work {work} is at no such cell: {}", w.pos.at))?;
    // A gate whose need the project does not answer is answered by the engine, as its round does: no agent runs, and
    // there is no message.
    let mut not_asked = None;
    if let Some(need) = cell.needs.as_deref().filter(|_| cell.kind == "agent")
        && !need_met(&e.project_of(&w)?, need)
    {
        not_asked = Some(why_not_asked(e, &w, need)?);
    }
    let agent = cell.kind == "agent" && not_asked.is_none();
    let message = if agent { Some(next_message(e, work)?.text) } else { None };
    // A judgment that would take a row that waits records nothing new: the work waits for what the row says.
    let mut waits = None;
    let mut design_card = None;
    let mut ask = None;
    if matches!(cell.step.as_deref(), Some("ask_owner" | "ask_architect"))
        && let Some(r) = work::open_round(e.db.conn(), w.id)?.filter(|r| r.cell.as_deref() == Some(cell.id.as_str()))
    {
        let asked: Option<(i64, Option<String>)> =
            e.db.conn()
                .query_row("SELECT id, answer FROM owner_asks WHERE round = ?1", [r.id], |x| {
                    Ok((x.get(0)?, x.get(1)?))
                })
                .ok();
        if let Some((id, None)) = asked {
            waits = Some(format!("your answer to ask {id}"));
            ask = Some(id);
        }
    }
    if cell.kind == "engine" && cell.step.is_none() && !cell.rows.is_empty() {
        let item = e.item_of(&w)?;
        let gathered = inputs::gather(e, &w, &g, &cell, &item)?;
        let given = values::given(e, &w, &g, &cell, &item, &gathered)?;
        if let Ok(row) = g.take(&cell.id, &json!({}), &given)
            && row.waits
        {
            let what = values::gates_waiting_for(e, &w, &item)?;
            design_card = what
                .as_deref()
                .filter(|x| x.starts_with("the owner's ruling on design card"))
                .and(values::design_card(e, &w)?.map(|c| c.id));
            waits = Some(what.unwrap_or_else(|| row.note.clone().unwrap_or_else(|| row.name.clone())));
        }
    }
    let tools = if cell.kind == "tool" {
        let item = e.item_of(&w)?;
        let gathered = inputs::gather(e, &w, &g, &cell, &item)?;
        tools::asked(&gathered).iter().map(tools::describe).collect()
    } else {
        Vec::new()
    };
    Ok(NextRound {
        waits,
        design_card,
        ask,
        tools,
        title: cell.title.clone(),
        kind: cell.kind.clone(),
        line: cell
            .line
            .clone()
            .or_else(|| agent.then(|| "engineer".to_string()))
            .filter(|_| agent),
        step: cell.step.clone(),
        message,
        not_asked,
        cell: cell.id,
    })
}

/// The message the work's next round would be sent, when the work is at an agent's cell: built as its round builds
/// it, with nothing run or recorded.
pub fn next_message(e: &mut Engine, work: i64) -> Result<message::Message, String> {
    let w = e.work(work)?;
    let g = e.graph_of(&w)?;
    let cell = g
        .cell(&w.pos.at)
        .filter(|c| c.kind == "agent")
        .cloned()
        .ok_or_else(|| format!("work {work} is at cell {}, which is not an agent's", w.pos.at))?;
    let item = e.item_of(&w)?;
    Ok(compose(e, &w, &g, &cell, &item)?.msg)
}

/// The CLI reports what its session has cost so far, the rounds it resumed from included; a round's own cost is what
/// that grew by since the session's last round that reported it.
fn round_cost(mut usage: Value, line_rounds: &[Round]) -> Value {
    let Some(total) = usage["session_cost_usd"].as_f64() else {
        return usage;
    };
    let before = line_rounds
        .iter()
        .rev()
        .find_map(|r| r.usage.as_ref().and_then(|u| u["session_cost_usd"].as_f64()));
    usage["cost_usd"] = json!(match before {
        Some(b) if b <= total => total - b,
        _ => total,
    });
    usage
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

/// Why a gate whose need the project's configuration does not answer is not asked, as the engine's answer says it.
fn why_not_asked(e: &mut Engine, w: &Work, need: &str) -> Result<String, String> {
    let desc = e
        .flow(w.flow_version)?
        .engine
        .needs
        .get(need)
        .cloned()
        .unwrap_or_default();
    Ok(format!(
        "Not asked: this question needs what this project does not have. {desc}"
    ))
}

/// A gate whose need the project's configuration does not answer is not asked: the engine answers not applicable
/// where the gate's answer has that, and no otherwise, and says why.
fn not_applicable(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    need: &str,
) -> Result<Advance, String> {
    let why = why_not_asked(e, w, need)?;
    let has_answer = cell
        .schema
        .as_ref()
        .and_then(|s| s.pointer(&format!("/properties/{}/anyOf/0/properties/answer", g.within)))
        .is_some();
    let own = if has_answer {
        json!({ "answer": { "verdict": "not_applicable", "why": why } })
    } else {
        json!({})
    };
    let mut output = Map::new();
    output.insert(g.within.clone(), own);
    output.insert(
        "progress".into(),
        json!(format!("The engine answered not applicable: {why}")),
    );
    output.insert("side_reports".into(), json!([]));
    output.insert("tell_owner".into(), json!([]));
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
    // Starting a work takes the queue's next item and decides nothing, so its round records no input.
    let starts = step.as_deref() == Some("start_work");
    let gathered = if starts {
        Vec::new()
    } else {
        inputs::gather(e, w, g, cell, &item)?
    };
    let given = values::given(e, w, g, cell, &item, &gathered)?;
    let commit = e.config.commit.clone();
    let input = if starts {
        json!({})
    } else {
        json!({ "inputs": recorded_inputs(&gathered), "given": given })
    };
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

/// Cell 0: where the work runs, and the id the engineer's session will have. An item worked on here before takes over
/// its latest earlier work's branch, worktree (reused when it is there, made afresh from the branch otherwise) and
/// pull request; an item imported with a branch takes that branch and the pull request imported with it; any other
/// gets a new branch from main.
fn start_work(e: &mut Engine, w: &Work, item: &Item) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let hosts = &e.config.config.install.machines.hosts;
    let machine = hosts.keys().next().ok_or("the configuration names no machine")?;
    let fresh = config::worktrees(&e.data).join(format!("{}-{}", item.project, w.id));
    if let Some(parent) = fresh.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let earlier: Option<(Option<String>, String, Option<i64>)> =
        e.db.conn()
            .query_row(
                "SELECT worktree, branch, pr FROM works
                 WHERE item = ?1 AND id <> ?2 AND branch IS NOT NULL ORDER BY id DESC LIMIT 1",
                params![item.id, w.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
    let (dir, branch, pr, head) = match (earlier, &item.imported_branch) {
        (Some((worktree, branch, pr)), _) => {
            let dir = worktree
                .map(std::path::PathBuf::from)
                .filter(|d| d.join(".git").exists())
                .unwrap_or(fresh);
            let head = git::take_over_worktree(&project.checkout, &dir, &branch)?;
            (dir, branch, pr, head)
        }
        (None, Some(branch)) => {
            let head = git::take_over_worktree(&project.checkout, &fresh, branch)?;
            (fresh, branch.clone(), item.imported_pr, head)
        }
        (None, None) => {
            let branch = format!("wf-agents/{}-{}", item.id, w.id);
            let head = git::create_worktree(&project.checkout, &fresh, &branch, &project.main)?;
            (fresh, branch, None, head)
        }
    };
    let mut place = json!({
        "machine": machine,
        "worktree": dir.display().to_string(),
        "branch": branch,
        "head": head,
        "session": agent::new_session_id(),
    });
    if let Some(n) = pr {
        place["pr"] = json!(n);
    }
    Ok(json!({ "place": place }))
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

/// Cells 15 and 16: the question becomes an ask of the owner, and the work waits for the answer. In this version the
/// owner answers for the architect too, in the architect's form.
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
        let asked: Option<(i64, Option<String>)> =
            e.db.conn()
                .query_row("SELECT id, answer FROM owner_asks WHERE round = ?1", [r.id], |x| {
                    Ok((x.get(0)?, x.get(1)?))
                })
                .ok();
        let Some((id, answer)) = asked else {
            return Err(format!("round {} asks, but put no ask to the owner", r.id));
        };
        let Some(answer) = answer else {
            return Ok(Advance::Waits(format!("ask {id} waits for the owner")));
        };
        let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
        let text = answer["text"].as_str().unwrap_or_default().to_string();
        let output = if to == "owner" {
            json!({ "reply": { "text": text } })
        } else {
            match answer.get("reply").filter(|x| x.is_object()) {
                Some(reply) => json!({ "reply": reply }),
                None => {
                    json!({ "reply": { "text": text, "why": answer["why"].as_str().unwrap_or("The owner answered for the architect.") } })
                }
            }
        };
        let gathered = inputs::gather(e, w, g, cell, item)?;
        let given = values::given(e, w, g, cell, item, &gathered)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None);
    }
    let gathered = inputs::gather(e, w, g, cell, item)?;
    // The ask holds what the cells handed in: the question, how far the cell got, what failed. The history and the
    // other tables the cell reads are in the database for the owner to look at.
    let asked: Vec<Value> = gathered
        .iter()
        .filter(|x| x.value.is_some() && x.input.from.iter().all(|s| matches!(s, Src::Place(..))))
        .map(|x| json!({ "what": x.input.what, "value": x.value }))
        .collect();
    let about = what_is_asked(&asked);
    let body = json!({ "to": to, "work": w.id, "item": item.id, "title": item.title, "asked": asked });
    let input = json!({ "inputs": recorded_inputs(&gathered) });
    let commit = e.config.commit.clone();
    let whom = if to == "owner" {
        "you".to_string()
    } else {
        "the architect, for whom you answer in this version".to_string()
    };
    let text = format!("Work {} on item {} ({}) asks {whom}", w.id, item.id, item.title);
    let id =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            tx.execute(
                "INSERT INTO owner_asks (round, body) VALUES (?1, ?2)",
                params![round, body.to_string()],
            )?;
            let id = tx.last_insert_rowid();
            actions::notify_ask(tx, w.id, id, &format!("{text}{about} (ask {id})")).map_err(db::Error::Abort)?;
            Ok(id)
        })
        .map_err(sql)?;
    Ok(Advance::Waits(format!("ask {id} waits for the owner")))
}

/// What an ask is about, for the notice that tells the owner of it: the kind of its first point, in words, and the
/// first sentence of what that point says; nothing when the ask holds no point.
fn what_is_asked(asked: &[Value]) -> String {
    let point = asked
        .iter()
        .find_map(|a| a["value"]["points"].as_array().and_then(|ps| ps.first()).cloned());
    let Some(point) = point else { return String::new() };
    let kind = point["kind"].as_str().unwrap_or_default().replace('_', " ");
    let what = point["what"].as_str().unwrap_or_default();
    format!(" ({kind}): {}", first_sentence(what))
}

/// The first sentence of a text, at most 200 characters: up to its first full stop, question or exclamation mark, or
/// line break.
pub fn first_sentence(text: &str) -> String {
    let text = text.trim();
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            break;
        }
        out.push(c);
        // The full stop, exclamation and question marks of Chinese and Japanese text end a sentence on their own.
        let ends = matches!(c, '\u{3002}' | '\u{ff01}' | '\u{ff1f}')
            || (matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|n| n.is_whitespace()));
        if ends {
            break;
        }
    }
    if out.chars().count() > 200 {
        out = out.chars().take(199).collect::<String>() + "…";
    }
    out
}

/// Cell 18: what runs next, from what the design chose, what the review accepted, whether the change changed since
/// each side last looked, and what the fix answered.
fn choose_checks(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let reviews = inputs::answered(e, w, g, "5a")?;
    let adversarial = inputs::answered(e, w, g, "6")?;
    let fixes = inputs::answered(e, w, g, "8")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    // The notice of a new ask says its kind and the first sentence of what it asks.
    #[test]
    fn an_ask_is_told_by_its_kind_and_first_sentence() {
        let asked = vec![json!({ "what": "w", "value": { "to": "owner", "points": [
            { "kind": "blocked_by_permission", "what": "Please run a probe. Why: the lock.\n\npython3 - <<'EOF'\nEOF" }
        ] } })];
        assert_eq!(what_is_asked(&asked), " (blocked by permission): Please run a probe.");
        assert_eq!(first_sentence("a\u{3002}b"), "a\u{3002}");
        assert_eq!(first_sentence("Run it. Then paste"), "Run it.");
        assert_eq!(first_sentence("v1.2 is out\nmore"), "v1.2 is out");
        assert_eq!(what_is_asked(&[]), "");
    }

    fn round(usage: Value) -> Round {
        Round {
            id: 1,
            cell: None,
            session: Some(1),
            input: Value::Null,
            output: None,
            transition: None,
            rejected: None,
            compactions: 0,
            started_at: 0,
            ended_at: Some(0),
            usage: Some(usage),
        }
    }

    #[test]
    fn a_round_costs_what_its_session_grew_by() {
        let u = |c: f64| json!({ "session_cost_usd": c });
        assert_eq!(round_cost(u(1.5), &[])["cost_usd"], 1.5);
        let before = [round(u(1.5)), round(json!({ "started": true }))];
        assert_eq!(round_cost(u(4.0), &before)["cost_usd"], 2.5);
        // A session the CLI counted afresh costs what it reports.
        assert_eq!(round_cost(u(0.5), &before)["cost_usd"], 0.5);
        assert_eq!(round_cost(json!({ "started": true }), &[]), json!({ "started": true }));
    }
}
