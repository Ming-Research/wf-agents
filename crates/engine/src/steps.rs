//! What the engine does at each kind of cell: an agent's round, its own steps, the tool cells, and asking the owner.

use db::Actor;
use db::rusqlite::{OptionalExtension, Transaction, params};
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph, CompiledRow};
use serde_json::{Map, Value, json};

use crate::agent::{self, Request};
use crate::failed::{self, Failed};
use crate::inputs::{self, Gathered, at_path};
use crate::message;
use crate::work::{self, Ended, Item, Round, RoundId, Work};
use crate::{ARCHITECT, Advance, Engine, OBSERVER, actions, architect, checks, git, tools, values};
use sessions::{Held, Plan};

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

/// The item a work is on, for a step that needs one.
fn needed<'i>(item: &'i Option<Item>, w: &Work) -> Result<&'i Item, String> {
    item.as_ref()
        .ok_or_else(|| format!("work {} is on no item, and this step is about one", w.id))
}

pub fn agent(e: &mut Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Advance, String> {
    close_interrupted(e, w)?;
    // A new session of the architect's line in the middle of a wake understands the project first.
    if crate::architect::understand_first(e, w, g, cell)? {
        return crate::architect::back_to_understanding(e, w, g, cell);
    }
    let item = e.subject(w)?;
    // A work on an item runs in its project; a wake of the architect is on no item and in no project.
    let project = match &item {
        Some(_) => Some(e.project_of(w)?),
        None => None,
    };
    if let Some(need) = &cell.needs
        && !project.as_ref().is_some_and(|p| need_met(p, need))
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
    let line_of_cell = crate::line_of(g, cell);
    let (level_name, level, cli) = level_of(e, g, cell, item.as_ref()).map_err(|x| format!("work {}: {x}", w.id))?;
    let cli_name = level.cli.clone();
    let Composed {
        line,
        plan,
        line_rounds,
        given,
        mut interjections,
        columns,
        table,
        number,
        msg,
        whole,
    } = compose(e, w, g, cell, item.as_ref())?;
    // A message that would give its session whole what the session already holds is the engine's fault: it is not
    // sent, and the work waits for the owner.
    if !msg.repeats.is_empty() {
        let why = format!(
            "cell {}: the message of {number} was not sent, since it would repeat what its session holds: {}",
            cell.id,
            msg.repeats.join("; ")
        );
        return failed::pause(e, w, &why);
    }
    let head = w.worktree.as_deref().and_then(|wt| git::head(wt).ok());
    let change = w
        .worktree
        .as_deref()
        .zip(project.as_ref())
        .and_then(|(wt, p)| values::change_hash(wt, &p.main));
    let mut ids: Vec<i64> = interjections
        .iter()
        .filter_map(|i| i["interjection"].as_i64())
        .collect();
    let commit = e.config.commit.clone();
    // The session manager chose the session: the line's going on, resumed once the CLI started it and else started
    // under the id kept for it, or a new one, which ends the one the line had going on; a new one the owner asked for
    // meets the ask.
    let (sid, resume, new_session) = match plan {
        Plan::Going {
            id,
            cli_session,
            resume: true,
            ..
        } => (id, Some(cli_session.clone()), cli_session),
        Plan::Going { id, cli_session, .. } => (id, None, cli_session),
        Plan::New(why) => {
            let fresh = agent::new_session_id();
            let id =
                e.db.write(Actor::Engine, |tx| {
                    sessions::choose::open(tx, w.id, &line, &cli_name, &fresh, why).map_err(db::Error::Abort)
                })
                .map_err(sql)?;
            if why == sessions::Start::Owner {
                e.sessions.met(w.id, w.project.as_deref());
            }
            (id, None, fresh)
        }
    };
    // What the cell is given, whole, which a compaction within the round gives the session again at once.
    let given_again = std::env::temp_dir().join(format!("wf-agents-given-{number}-{}.md", agent::new_session_id()));
    std::fs::write(
        &given_again,
        format!(
            "Your session was compacted in the middle of {number}: what its cell is given, again, whole. Carry on with \
             the round's work.\n\n{whole}"
        ),
    )
    .map_err(|x| format!("{}: {x}", given_again.display()))?;
    // After each tool use the command line's hook has this program hand in the interjections waiting for the line.
    let deliver = std::env::current_exe().ok().map(|exe| {
        let n = number.n.to_string();
        let work = w.id.to_string();
        let data = e.data.display().to_string();
        let exe = exe.display().to_string();
        let words = [
            exe.as_str(),
            "deliver",
            "--data",
            data.as_str(),
            "--work",
            work.as_str(),
            "--round",
            n.as_str(),
            "--line",
            line_of_cell.as_str(),
        ];
        words.iter().map(|x| agent::quote(x)).collect::<Vec<_>>().join(" ")
    });
    // A work runs in its worktree; a project's architect in a directory of its own, with its project's checkout to
    // read.
    let wt = match (&w.worktree, &project) {
        (Some(wt), _) => wt.clone(),
        (None, Some(p)) => p.checkout.clone(),
        (None, None) => architect::made_workdir(&e.data, architect::project_of(w)?)?,
    };
    let mut readable = readable_dirs(e);
    if g.name == ARCHITECT {
        readable.extend(
            w.project
                .as_ref()
                .and_then(|p| e.config.config.projects.get(p))
                .and_then(|p| std::fs::canonicalize(&p.checkout).ok()),
        );
    }
    let req = Request {
        work: w.id,
        cell: cell.id.clone(),
        line: line_of_cell.clone(),
        cli,
        model: level.model.clone(),
        effort: sessions::choose::effort(cell),
        worktree: wt,
        config_dir: e.cli_home(&cli_name),
        resume,
        new_session: new_session.clone(),
        prompt: msg.text.clone(),
        schema: cell.schema.clone().unwrap_or_else(|| json!({ "type": "object" })),
        envelope: sessions::cli::envelope(g),
        refused: e.config.config.install.agents.permissions.refused(&line_of_cell),
        readable,
        replay: None,
        regive: Some(given_again.clone()),
        deliver,
    };
    let cli_version = e.cli_version(&cli_name);
    // The record keeps each large text once, in `contents`: the conditional inputs given, the task, the
    // structured-output schema.
    let mut kept = msg.kept.clone();
    let mut call = kept_call(
        agent::call(&req, &agent::command_line(&req), &cli_version, crate::VERSION),
        &mut kept,
    );
    // The level the round runs at, which the configuration maps to its model: the owner reads the round by it.
    call["level"] = Value::String(level_name.clone());
    let input = json!({
        "message": msg.stored,
        "conditional": msg.record(),
        "call": call,
        "engine": { "head": head, "change": change, "columns": columns, "table": table, "interjections": ids },
    });
    let round =
        e.db.write(Actor::Engine, |tx| {
            db::keep(tx, &kept)?;
            work::start_numbered(tx, number, &cell.id, Some(sid), &input, &commit, Some(&level.model))
                .map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    (e.say)(&format!("work {} cell {}: {round} of the {line}", w.id, cell.id));
    let reply = e
        .sessions
        .run(&req, &|what: &str| (e.activity)(round.work, round.n, what));
    let _ = std::fs::remove_file(&given_again);
    if reply.session != new_session && !reply.session.is_empty() {
        let s = reply.session.clone();
        e.db.write(Actor::Engine, |tx| {
            sessions::choose::rename(tx, sid, &s).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    }
    // A cell on no line has a session of its own for this round only.
    if cell.line.is_none() {
        e.db.write(Actor::Engine, |tx| {
            sessions::choose::end(tx, sid).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    }
    // The interjections the hook handed in while the round ran are this round's, as those its message gave are.
    let during = sessions::interject::delivered_during(&e.db, round.work, round.n, &ids)?;
    if !during.is_empty() {
        ids.extend(during.iter().filter_map(|i| i["interjection"].as_i64()));
        interjections.extend(during);
        let all = serde_json::to_string(&ids).unwrap_or_default();
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.interjections', json(?3))
                 WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                params![round.work, round.n, all],
            )?;
            Ok(())
        })
        .map_err(sql)?;
    }
    // What the session had cost before, its keep-warm pings included, so that the round's own cost is what it grew by.
    let before = sessions::cost::spent(e.db.conn(), &new_session, &line_rounds)?;
    let ended = Ended {
        usage: Some(sessions::cost::own_cost(reply.usage.clone(), before)).filter(|u| !u.is_null()),
        model: reply.model.clone(),
        compactions: reply.compactions,
        command: (!reply.command.is_empty()).then(|| reply.command.clone()),
        ..Ended::default()
    };
    // A round that resumed its session within the cache's lifetime and still wrote most of it to the cache again missed
    // the cache: a fault of wf-agents, which the owner is told of.
    if req.resume.is_some()
        && let Some(usage) = &ended.usage
        && let Some(miss) =
            sessions::miss::missed(e.db.conn(), &new_session, &line_rounds, round, &input["call"], usage)?
    {
        crate::faults::cache_miss(e, w, round, &cell.id, &line, &miss)?;
    }
    let Some(mut output) = reply.output.clone() else {
        let f = Failed {
            w,
            cell,
            round,
            section: &section,
        };
        return f.handle(e, ended, &reply);
    };
    e.returned();
    // A field the flow lets the return leave out that the agent wrote as null is left out.
    if let Some(s) = &cell.schema {
        flow::without_null_options(s, &mut output);
    }
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
        item: item.as_ref(),
        project: project.as_ref(),
        output: &output,
        unwrapped: g.unwrap(&output),
        head_start: head.clone(),
        columns: &columns,
        table: &table,
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
    finish(e, w, g, cell, round, output, &row, ended, &ids, head_after, &given)
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

/// The level an agent's round at a cell runs at, its name with what it is and its CLI: the item's, or one above for the
/// advisor, which is the engineer with a higher model; for the architect, the review of its item changes and the new
/// architects that read a policy, the level the installation names for the architect; for the search of
/// observations, the level it names for the observer.
pub fn level_of(
    e: &Engine,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
) -> Result<(String, config::Level, config::Cli), String> {
    let name = match item {
        Some(i) => e.config.config.level_of_line(&i.level, &crate::line_of(g, cell)),
        None if cell.line.as_deref() == Some(OBSERVER) => e.config.config.observer_level(),
        None => e.config.config.architect_level(),
    };
    let (level, cli) = e
        .config
        .config
        .level(&name)
        .map(|(l, c)| (l.clone(), c.clone()))
        .ok_or_else(|| format!("no such level in the configuration: {name}"))?;
    Ok((name, level, cli))
}

/// What an agent's round at a cell is built from, and the message it is sent.
struct Composed {
    /// The line the session is on: the cell's, or for a cell on no line the cell itself, whose session is its own.
    line: String,
    /// Where the round runs, as the session manager chose it.
    plan: Plan,
    /// The session's rounds, oldest first: those of earlier wakes too, for the architect's line; none for a new
    /// session.
    line_rounds: Vec<Round>,
    given: Map<String, Value>,
    /// The interjections waiting for the line.
    interjections: Vec<Value>,
    columns: Vec<(String, String)>,
    /// The rows of the cell's numbered table, each with its number.
    table: Vec<Value>,
    /// The round it will be.
    number: RoundId,
    msg: message::Message,
    /// The message as a session that holds nothing is sent it: what a compaction within the round gives again.
    whole: String,
}

/// Builds the message of an agent's round at a cell from the work as it stands, running and recording nothing. The
/// session manager chooses the session it runs in and says what that session holds; the architect's line goes on in
/// one CLI session across its wakes, so what its session holds is read from the rounds of every work that ran in it.
fn compose(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
) -> Result<Composed, String> {
    let line = sessions::choose::line_of(cell);
    let model = level_of(e, g, cell, item)?.1.model;
    let limit = e.config.config.install.agents.context_limit;
    let wake = w.project.as_deref().filter(|_| g.name == ARCHITECT);
    let mut plan = e.sessions.plan(e.db.conn(), w.id, wake, cell, limit, &model)?;
    let work_rounds = work::rounds(e.db.conn(), w.id)?;
    let (line_rounds, held) = match &mut plan {
        Plan::Going { rounds, held, .. } => (std::mem::take(rounds), *std::mem::take(held)),
        Plan::New(_) => (Vec::new(), Held::default()),
    };
    let mut known: Vec<Round> = line_rounds.iter().filter(|r| r.id.work != w.id).cloned().collect();
    known.extend(work_rounds.iter().cloned());
    let registry = message::registry(&known);
    let gathered = inputs::gather(e, w, g, cell, item)?;
    let given = values::given(e, w, g, cell, item, &gathered)?;
    let speaks_to = crate::line_of(g, cell);
    let mut interjections: Vec<Value> = inputs::db_rows(e, w, item, "waiting_interjections", Some("work"))?
        .into_iter()
        .filter(|r| r["line"] == speaks_to.as_str())
        .collect();
    let columns = message::columns(e, w, g, cell, item);
    let table = inputs::table(&gathered);
    let number = work::next_round(e.db.conn(), w.id)?;
    let context = message::Context {
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
        table: &table,
        held: &held,
        registry: &registry,
    };
    let msg = context.build();
    let nothing = Held::default();
    let whole = message::Context {
        held: &nothing,
        ..context
    }
    .build()
    .text;
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
        plan,
        line_rounds,
        given,
        interjections,
        columns,
        table,
        number,
        msg,
        whole,
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
    if w.ended.is_some() || e.done(&w)? {
        return Err(format!("work {work} has no next round: it is ended or done"));
    }
    // A paused work's next round is the one the owner's step runs; a work on an older flow version is taken over
    // instead of taking a step.
    if w.flow_version != e.flow_now {
        return Err(format!(
            "work {work} is on flow version {}, older than {}: when it goes on, a new work takes its item over",
            w.flow_version, e.flow_now
        ));
    }
    let g = e.graph_of(&w)?;
    let cell = g
        .cell(&w.pos.at)
        .cloned()
        .ok_or_else(|| format!("work {work} is at no such cell: {}", w.pos.at))?;
    let item = e.subject(&w)?;
    // A gate whose need the project does not answer is answered by the engine, as its round does: no agent runs, and
    // there is no message.
    let mut not_asked = None;
    if let Some(need) = cell.needs.as_deref().filter(|_| cell.kind == "agent")
        && !e.project_of_item(item.as_ref()).is_some_and(|p| need_met(&p, need))
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
                .query_row(
                    "SELECT id, answer FROM owner_asks WHERE work = ?1 AND round = ?2",
                    [r.id.work, r.id.n],
                    |x| Ok((x.get(0)?, x.get(1)?)),
                )
                .ok();
        match asked {
            Some((id, None)) => {
                waits = Some(format!("your answer to ask {id}"));
                ask = Some(id);
            }
            // A question to the architect waits for its reply.
            None if cell.step.as_deref() == Some("ask_architect") => {
                if let Some((matter, None)) = architect::question(e, r.id)? {
                    waits = Some(format!("the architect's reply to its question (matter {matter})"));
                }
            }
            _ => {}
        }
    }
    if cell.kind == "engine" && cell.step.is_none() && !cell.rows.is_empty() {
        let gathered = inputs::gather(e, &w, &g, &cell, item.as_ref())?;
        let given = values::given(e, &w, &g, &cell, item.as_ref(), &gathered)?;
        if let Ok(row) = g.take(&cell.id, &json!({}), &given)
            && row.waits
        {
            let what = match &item {
                Some(i) => values::gates_waiting_for(e, &w, i)?,
                None => None,
            };
            design_card = what
                .as_deref()
                .filter(|x| x.starts_with("the owner's ruling on design card"))
                .and(values::design_card(e, &w)?.map(|c| c.id));
            waits = Some(what.unwrap_or_else(|| row.note.clone().unwrap_or_else(|| row.name.clone())));
        }
    }
    let tools = if cell.kind == "tool" {
        let gathered = inputs::gather(e, &w, &g, &cell, item.as_ref())?;
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
        line: cell.line.clone().filter(|_| agent),
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
    let item = e.subject(&w)?;
    Ok(compose(e, &w, &g, &cell, item.as_ref())?.msg)
}

fn reject(e: &mut Engine, round: RoundId, ended: Ended, why: Value) -> Result<Advance, String> {
    (e.say)(&format!("{round} sent back: {why}"));
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
    let item = e.subject(w)?;
    let gathered = inputs::gather(e, w, g, cell, item.as_ref())?;
    let given = values::given(e, w, g, cell, item.as_ref(), &gathered)?;
    let row = g.take(&cell.id, &output, &given)?.clone();
    let commit = e.config.commit.clone();
    let input = json!({ "engine": { "not_applicable": need } });
    let round =
        e.db.write(Actor::Engine, |tx| {
            work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given)
}

/// Ends a round that took a row: the round, what the row and the cell's own actions do, and where the work goes,
/// in one transaction. `given` is what the rows were given, which an action at an input read by id reads.
#[allow(clippy::too_many_arguments)]
pub fn finish(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    round: RoundId,
    output: Value,
    row: &CompiledRow,
    ended: Ended,
    delivered: &[i64],
    head_after: Option<String>,
    given: &Map<String, Value>,
) -> Result<Advance, String> {
    let next = g.step(&w.pos, row)?;
    let mut moved = w.clone();
    moved.pos = next;
    // What an architect's round does is the architect's; every other step the engine's.
    let actor = if g.name == ARCHITECT && cell.kind == "agent" {
        Actor::Architect
    } else {
        Actor::Engine
    };
    let ctx = actions::Ctx {
        work: w.clone(),
        item: e.subject(w).ok().flatten(),
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
        given: given.clone(),
    };
    let ended = Ended {
        output: Some(output),
        transition: Some(row.name.clone()),
        ..ended
    };
    let mut done: Vec<String> = Vec::new();
    e.db.write(actor, |tx| {
        work::finish_round(tx, round, &ended).map_err(db::Error::Abort)?;
        if let Some(h) = &head_after {
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.head_after', ?3) WHERE work = ?1 AND n = ?2",
                params![round.work, round.n, h],
            )?;
        }
        for id in delivered {
            tx.execute(
                "UPDATE interjections SET delivered_round = ?2 WHERE id = ?1",
                params![id, round.n],
            )?;
        }
        for a in &cell.always {
            if let Some(name) = &a.action
                && ctx.at(&a.at).is_some()
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

/// What the end of a graph writes besides its round, in the round's transaction.
type AtEnd<'a> = &'a dyn Fn(&Transaction<'_>) -> Result<(), String>;

/// Ends the round at the graph's end, which has no rows: the work is done.
fn finish_end(e: &mut Engine, w: &Work, round: RoundId, output: Value, also: AtEnd<'_>) -> Result<Advance, String> {
    let ended = Ended {
        output: Some(output),
        ..Ended::default()
    };
    e.db.write(Actor::Engine, |tx| {
        work::finish_round(tx, round, &ended).map_err(db::Error::Abort)?;
        also(tx).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    (e.say)(&format!("work {} done", w.id));
    Ok(Advance::Moved)
}

pub fn engine(e: &mut Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Advance, String> {
    let item = e.subject(w)?;
    let open = close_interrupted(e, w)?;
    let step = cell.step.clone();
    match step.as_deref() {
        Some("ask_owner") if w.graph == ARCHITECT => return architect::ask_owner(e, w, g, cell, open),
        Some("ask_owner") => return ask(e, w, g, cell, needed(&item, w)?, open),
        Some("ask_architect") => {
            let item = needed(&item, w)?;
            // A question an earlier engine put to the owner, who answered for the architect, is still the owner's to
            // answer; any other is queued for the architect.
            let stood_in = match open.as_ref().filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
                Some(r) => architect::owner_ask_of(e, r.id)?.is_some(),
                None => false,
            };
            if stood_in {
                return ask(e, w, g, cell, item, open);
            }
            return architect::ask_architect(e, w, g, cell, item, open);
        }
        _ => {}
    }
    if step.as_deref() == Some("merge") {
        let ahead: Option<i64> =
            e.db.conn()
                .query_row(
                    "SELECT work FROM merge_queue WHERE project = ?1 LIMIT 1",
                    [&needed(&item, w)?.project],
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
        inputs::gather(e, w, g, cell, item.as_ref())?
    };
    let given = values::given(e, w, g, cell, item.as_ref(), &gathered)?;
    let commit = e.config.commit.clone();
    let input = if starts {
        json!({})
    } else {
        json!({ "inputs": recorded_inputs(&gathered), "given": given })
    };
    let quick = matches!(step.as_deref(), None | Some("choose_checks"));
    if quick {
        let output = match step.as_deref() {
            Some("choose_checks") => choose_checks(e, w, g)?,
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
        return finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given);
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
        Some("start_work") => start_work(e, w, needed(&item, w)?)?,
        Some("run_tools") => tools::run_tools(e, w, item.as_ref(), round, &gathered)?,
        Some("full_ci_on_draft") => tools::full_ci(e, w, needed(&item, w)?, round)?,
        Some("merge") => tools::merge(e, w, needed(&item, w)?, round)?,
        Some("watch_main") => tools::watch_main(e, w, g, needed(&item, w)?, round)?,
        Some("end_work") | Some("end_wake") => json!({}),
        Some("wake") => architect::wake(e, w)?,
        Some("look_at_queue") => architect::look_at_queue(e, w)?,
        Some("count_trials") => architect::count_trials(e, w)?,
        Some(other) => return Err(format!("no such engine step: {other}")),
        None => json!({}),
    };
    if cell.rows.is_empty() {
        let output = if step.as_deref() == Some("end_work") {
            end_work(e, w)?
        } else {
            output
        };
        return finish_end(e, w, round, output, &|_| Ok(()));
    }
    let row = g.take(&cell.id, &output, &given)?.clone();
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given)
}

/// Cell 0: the session manager sets up where the work runs, on a machine with a free slot, and the id the engineer's
/// session will have.
fn start_work(e: &mut Engine, w: &Work, item: &Item) -> Result<Value, String> {
    // An item above the highest level an item may be at is refused: the work pauses, saying why.
    e.config
        .config
        .item_level(&item.level)
        .map_err(|why| format!("item {}: {why}", item.id))?;
    let project = e.project_of(w)?;
    let free = e.slots_by_machine()?;
    let at = sessions::worktree::ItemPlace {
        id: item.id,
        project: &item.project,
        imported_branch: item.imported_branch.as_deref(),
        imported_pr: item.imported_pr,
    };
    let p = sessions::worktree::set_up(e.db.conn(), &e.data, &project, &at, w.id, &free)?;
    let mut place = json!({
        "machine": p.machine,
        "worktree": p.worktree.display().to_string(),
        "branch": p.branch,
        "head": p.head,
        "session": p.session,
    });
    if let Some(n) = p.pr {
        place["pr"] = json!(n);
    }
    Ok(json!({ "place": place }))
}

/// Cell 20: the work's sessions end, and the worktree is removed once the round is written; the branch stays. A work
/// that delivered a report and handed in what to do next queues it for the architect, as a matter of its own.
fn end_work(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let followup = report_followup(e, w, &project)?;
    e.db.write(Actor::Engine, |tx| {
        sessions::choose::end_work(tx, w.id).map_err(db::Error::Abort)?;
        if let Some(wt) = &w.worktree {
            let args = json!({ "checkout": project.checkout, "dir": wt });
            tx.execute(
                "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, 'remove_worktree', ?2, ?3)",
                params![w.id, args.to_string(), db::now()],
            )?;
        }
        if let Some(m) = &followup {
            db::queue_matter(tx, m)?;
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(json!({}))
}

/// What to do next that a work which delivered a report handed in, as a matter for the architect: the latest the work
/// handed in, as written, with the report's paths under `research/` and the commit the work merged, read where git has
/// that commit; none for a work that delivered a change, or handed in no what to do next.
fn report_followup(e: &Engine, w: &Work, project: &config::Project) -> Result<Option<db::Matter>, String> {
    let item = e.item_of(w)?;
    if item.deliver != "report" {
        return Ok(None);
    }
    let conn = e.db.conn();
    let latest: Option<(i64, String)> = conn
        .query_row(
            "SELECT round, followup FROM work_followups WHERE work = ?1 ORDER BY round DESC LIMIT 1",
            [w.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((round, followup)) = latest else {
        return Ok(None);
    };
    let commit: Option<String> = conn
        .query_row(
            "SELECT json_extract(output, '$.result.merge_commit') FROM rounds
             WHERE work = ?1 AND cell = '9' AND transition = 'merged' ORDER BY n DESC LIMIT 1",
            [w.id],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?
        .flatten();
    let reports: Vec<String> = commit
        .as_deref()
        .and_then(|c| {
            let dirs = [
                w.worktree.as_deref().map(std::path::Path::new),
                Some(project.checkout.as_path()),
            ];
            dirs.into_iter()
                .flatten()
                .find_map(|dir| git::changed(dir, &format!("{c}^"), c).ok())
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f.starts_with("research/"))
        .collect();
    Ok(Some(db::Matter {
        project: item.project.clone(),
        kind: "report".into(),
        detail: Some("followup".into()),
        what: format!(
            "What to do next after the report of item {} ({}): {}",
            item.id,
            item.title,
            db::one_sentence(&followup)
        ),
        body: json!({ "followup": followup, "reports": reports, "commit": commit }),
        item: Some(item.id),
        raised: Some((w.id, round)),
    }))
}

/// Cell 16: the question becomes an ask of the owner, and the work waits for the answer. An ask of cell 15 that an
/// earlier engine put to the owner, who answered for the architect in the architect's form, is completed by the
/// owner's answer the same way.
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
                .query_row(
                    "SELECT id, answer FROM owner_asks WHERE work = ?1 AND round = ?2",
                    [r.id.work, r.id.n],
                    |x| Ok((x.get(0)?, x.get(1)?)),
                )
                .ok();
        let Some((id, answer)) = asked else {
            return Err(format!("{} asks, but put no ask to the owner", r.id));
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
        let gathered = inputs::gather(e, w, g, cell, Some(item))?;
        let given = values::given(e, w, g, cell, Some(item), &gathered)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    if to != "owner" {
        return Err(format!(
            "{}: a question to the architect is queued for it, not put to the owner",
            cell.id
        ));
    }
    let gathered = inputs::gather(e, w, g, cell, Some(item))?;
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
    let text = format!("Work {} on item {} ({}) asks you", w.id, item.id, item.title);
    let id =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![round.work, round.n, body.to_string()],
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
pub fn what_is_asked(asked: &[Value]) -> String {
    let point = asked
        .iter()
        .find_map(|a| a["value"]["points"].as_array().and_then(|ps| ps.first()).cloned());
    let Some(point) = point else { return String::new() };
    let kind = point["kind"].as_str().unwrap_or_default().replace('_', " ");
    let what = point["what"].as_str().unwrap_or_default();
    format!(" ({kind}): {}", db::one_sentence(what))
}

/// The directories outside the worktree an agent may read: where the engine saves a tool's output too long to give
/// inline, and the logs of red checks it names; made when missing, by their real paths, so that the CLI's rules match.
fn readable_dirs(e: &Engine) -> Vec<std::path::PathBuf> {
    ["tool-output", "logs"]
        .iter()
        .filter_map(|name| {
            let dir = e.data.join(name);
            std::fs::create_dir_all(&dir).ok()?;
            std::fs::canonicalize(&dir).ok()
        })
        .collect()
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
        let since = rs.first().map(|r| r.id);
        fixes.iter().filter(|f| since.is_none_or(|s| f.id > s)).any(|f| {
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
        assert_eq!(what_is_asked(&[]), "");
    }
}
