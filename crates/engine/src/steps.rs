//! What the engine does at each kind of cell: an agent's round, its own steps, the tool cells, and asking the owner.

use db::Actor;
use db::rusqlite::{OptionalExtension, Transaction, params};
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph, CompiledRow};
use serde_json::{Map, Value, json};

use crate::agent::{self, Request};
use crate::failed::{self, Failed};
use crate::inputs::{self, Gathered};
use crate::message;
use crate::work::{self, Ended, Item, Round, RoundId, Work};
use crate::{ARCHITECT, Advance, Engine, actions, architect, checks, git, tools, values, watch};
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

/// Why a round's session starts afresh, in words, for the round's record and the owner's page.
pub fn new_session_why(
    why: sessions::Start,
    cell: &CompiledCell,
    limit: u64,
    model: &str,
    cache: Option<i64>,
) -> String {
    match why {
        sessions::Start::Line => "no session of its line going on yet".into(),
        sessions::Start::Cell => format!("cell {} opens one, at its first round", cell.id),
        sessions::Start::Limit => format!("context past the limit ({}K)", limit / 1000),
        sessions::Start::Model => format!("the model changed to {model}"),
        sessions::Start::Account => "the agents' login account changed".into(),
        sessions::Start::Expired => format!(
            "the prompt cache expired (the session's last call was {} minutes or more ago)",
            cache.unwrap_or_default() / 60_000
        ),
        sessions::Start::Carried => "the architect goes on in the session it had".into(),
        sessions::Start::Owner => "the owner asked for one".into(),
        sessions::Start::Fork => "forked from the base".into(),
    }
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

/// The commit of the project's main a step's round reads the project at ([`inputs::pin_main`]), main fetched once for
/// it: for a cell on a project's architect's line, by the fetch that brings the architect's worktree to main
/// ([`architect::ready`]), so that the worktree the architect reads files in and the inputs the round is given are of
/// one commit, though main moves on between two fetches; for any other cell by the fetch of `pin_main`. Where the
/// worktree cannot be brought to main, the step stops as a step of the engine's before a round does
/// ([`failed::networked`]): a failure of the network that passes by itself is waited out and the step taken again, the
/// work waits while the network is down for it to be up again, and any other failure pauses the work, with why.
fn pin_round(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
    section: &flow::format::Rounds,
    took: &[failed::Took],
) -> Result<Result<inputs::Pin, Advance>, String> {
    if item.is_none() {
        match failed::networked(e, w, cell, section, took, |e| architect::ready(e, w, g, cell))? {
            Ok(Some(commit)) => {
                let commit = inputs::reads_main(cell).then_some(commit);
                return Ok(Ok(inputs::Pin {
                    commit,
                    fetch_failed: None,
                }));
            }
            Ok(None) => {}
            Err(stop) => return Ok(Err(stop)),
        }
    }
    inputs::pin_main(e, w, cell, item, true).map(Ok)
}

/// The item a work is on, for a step that needs one.
fn needed<'i>(item: &'i Option<Item>, w: &Work) -> Result<&'i Item, String> {
    item.as_ref()
        .ok_or_else(|| format!("work {} is on no item, and this step is about one", w.id))
}

pub fn agent(e: &mut Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Advance, String> {
    close_interrupted(e, w)?;
    // The time the round's session is chosen at, which the message is built at too, so that a session whose cache
    // expires between them is not told apart by them.
    let now = db::now();
    let item = e.subject(w)?;
    // A work on an item runs in its project; a work of the architect is on no item and in no project.
    let project = match &item {
        Some(_) => Some(e.project_of(w)?),
        None => None,
    };
    // The commit of the project's main this step's round reads the project at, pinned once ([`pin_round`]): main is
    // fetched once to find it, before any input is gathered, so that the inputs that read the project's files on main
    // are the project as the remote has it now, and every one of them, with the checks of the return, is of that one
    // commit. When the fetch of a cell that has such an input fails, the round goes on at the commit an earlier fetch
    // left, and its record says why. A cell with no such input does not read main, and fetches only where it is an
    // architect's on a line, whose worktree is brought to main.
    let mut pinned: Option<inputs::Pin> = None;
    let section = failed::section(e, g)?;
    // What the work took since it last took a row from outside the engine, or since the owner last let it go on after
    // it paused: every row that runs again is counted in these, and a run waits after the one before.
    let took = failed::since_outside(e, w, g)?;
    // A table the cell must answer that has no row leaves its round nothing to answer: no round runs, and the engine
    // takes the cell's row for it, recorded with why.
    if let Some(row) = crate::row(cell, flow::format::NOTHING_TO_ANSWER).cloned() {
        let pin = match pin_round(e, w, g, cell, item.as_ref(), &section, &took)? {
            Ok(pin) => pin,
            Err(stop) => return Ok(stop),
        };
        let gathered = inputs::gather(e, w, g, cell, item.as_ref(), pin.commit.as_deref())?;
        let kept = pin.clone();
        pinned = Some(pin);
        if inputs::table(&gathered).is_empty() {
            let why = format!(
                "the table cell {} answers has no row, so its round could answer nothing",
                cell.id
            );
            let mut input = json!({ "engine": { "nothing_to_answer": why }, "inputs": recorded_inputs(&gathered) });
            kept.record(&mut input["engine"]);
            let commit = e.config.commit.clone();
            let round =
                e.db.write(Actor::Engine, |tx| {
                    work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
                })
                .map_err(sql)?;
            e.say(&format!("work {} cell {}: {why}", w.id, cell.id));
            return finish(
                e,
                w,
                g,
                cell,
                round,
                json!({}),
                &row,
                Ended::default(),
                &[],
                None,
                &Map::new(),
            );
        }
    }
    // The cell's rounds and when the owner last let the work go on, as the run guard judges the round by them.
    let at_cell = work::rounds_at(e.db.conn(), w.id, &cell.id)?;
    let resumed = work::resumed_at(e.db.conn(), w.id)?;
    if let Some(why) = failed::waiting(e.retry_wait, e.backoff_unit, &took, &section) {
        return Ok(Advance::Waits(why));
    }
    let sent_back = took
        .iter()
        .filter(|t| t.row.as_deref() == Some("sent_back"))
        .collect::<Vec<_>>();
    let pause_after = section.sent_back.pause_after as usize;
    if sent_back.len() >= pause_after {
        let reason = sent_back
            .first()
            .map(|t| failed::sent_back_reason(e, t.round))
            .unwrap_or_default();
        let why = format!(
            "{} returns in a row were sent back, the last at cell {}: {reason}",
            sent_back.len(),
            cell.id
        );
        return failed::pause(e, w, &why);
    }
    // A round of a project's architect reads the project at the latest main: for a cell on its line, its graph's worktree
    // is brought there first, by the one fetch the round makes. A failure of the network that passes by itself is
    // waited out and the step taken again, up to the bound of the flow's `step_network`; while the network is down,
    // every work waits for it; any other failure waits for the owner, with why.
    let pin = match pinned {
        Some(pin) => pin,
        None => match pin_round(e, w, g, cell, item.as_ref(), &section, &took)? {
            Ok(pin) => pin,
            Err(stop) => return Ok(stop),
        },
    };
    let main = pin.commit.clone();
    // The branch takes the latest main in before the cell's first round each time the work comes to it, for a cell
    // told what that brought (`main_changes`): the commit the round is pinned to, with no fetch of its own.
    e.caught_up = None;
    if crate::catch_up::reads(cell)
        && let Some(wt) = w.worktree.as_deref()
        && item.is_some()
        && crate::catch_up::first_round(e, w, g, cell)?
    {
        let project = e.project_of(w)?;
        e.caught_up = Some((w.id, crate::catch_up::bring(wt, &project.main, main.as_deref())));
    }
    let line_of_cell = crate::line_of(g, cell);
    let (level_name, level, _) = level_of(e, g, cell, item.as_ref()).map_err(|x| format!("work {}: {x}", w.id))?;
    let cli_name = level.cli.clone();
    let Composed {
        line,
        plan,
        line_rounds,
        given,
        mut interjections,
        columns,
        table,
        decisions,
        number,
        msg,
        whole,
        system,
        starts_with,
    } = compose(e, w, g, cell, item.as_ref(), main.as_deref(), now)?;
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
    let mut ids: Vec<i64> = interjections
        .iter()
        .filter_map(|i| i["interjection"].as_i64())
        .collect();
    let commit = e.config.commit.clone();
    // The session manager chose the session: the line's going on, resumed once the CLI started it and else started
    // under the id kept for it, which the architect's line takes up from the project's previous architect work at its
    // first round in a work; or a new one, which ends the one the line had going on; a new one the owner asked for
    // meets the ask.
    // A round whose session starts afresh says why in its record.
    let new_why = match &plan {
        Plan::New(why) => Some(new_session_why(
            *why,
            cell,
            e.config.config.install.agents.context_limit,
            &level.model,
            e.runs_on(&level).on().cache,
        )),
        Plan::Fork { .. } => Some(new_session_why(
            sessions::Start::Fork,
            cell,
            e.config.config.install.agents.context_limit,
            &level.model,
            e.runs_on(&level).on().cache,
        )),
        Plan::Going { .. } => None,
    };
    // The matter a session of a line whose sessions are each a matter's is for: the matter in hand.
    let matter = match cell.fork_of {
        Some(_) if !cell.whole_run => architect::picked(e.db.conn(), w.id)?,
        _ => None,
    };
    let (sid, resume, new_session, fork) = match plan {
        Plan::Going {
            id,
            cli_session,
            resume,
            carried,
            fork_from,
            ..
        } => {
            let id = if carried {
                e.db.write(Actor::Engine, |tx| {
                    sessions::choose::carry_over(tx, w.id, id).map_err(db::Error::Abort)
                })
                .map_err(sql)?
            } else {
                id
            };
            match fork_from {
                // A fork whose first round never started the CLI's session forks the base under its id now.
                Some(base) => (id, Some(base), cli_session, true),
                None => (id, resume.then(|| cli_session.clone()), cli_session, false),
            }
        }
        Plan::Fork { base, base_cli, .. } => {
            let fresh = agent::new_session_id();
            let id =
                e.db.write(Actor::Engine, |tx| {
                    sessions::choose::open_for(
                        tx,
                        w.id,
                        &line,
                        &cli_name,
                        &fresh,
                        sessions::Start::Fork,
                        matter,
                        Some(base),
                    )
                    .map_err(db::Error::Abort)
                })
                .map_err(sql)?;
            (id, Some(base_cli), fresh, true)
        }
        Plan::New(why) => {
            let fresh = agent::new_session_id();
            let id =
                e.db.write(Actor::Engine, |tx| {
                    sessions::choose::open_for(tx, w.id, &line, &cli_name, &fresh, why, matter, None)
                        .map_err(db::Error::Abort)
                })
                .map_err(sql)?;
            if why == sessions::Start::Owner {
                e.sessions().met(w.id, w.project.as_deref());
            }
            (id, None, fresh, false)
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
    // After each tool use the command line's hook has this program hand in the interjections waiting for the line, to a
    // cell that takes them.
    let deliver = std::env::current_exe().ok().map(|exe| {
        let n = number.n.to_string();
        let work = w.id.to_string();
        let data = e.data.display().to_string();
        let exe = exe.display().to_string();
        let mut words = vec![
            exe.as_str(),
            "deliver",
            "--data",
            data.as_str(),
            "--work",
            work.as_str(),
            "--round",
            n.as_str(),
        ];
        if crate::takes_interjections(cell) {
            words.extend(["--line", line_of_cell.as_str()]);
        }
        words.iter().map(|x| agent::quote(x)).collect::<Vec<_>>().join(" ")
    });
    // The images the message gives, after its text, each after its address.
    let mut images = Vec::new();
    for i in msg.images.iter().filter(|i| i.given) {
        let (media_type, bytes) = db::attachments::bytes(e.db.conn(), i.attachment)
            .map_err(sql)?
            .ok_or_else(|| format!("{number}: image {} is not kept", i.attachment))?;
        images.push(agent::Image {
            address: db::attachments::address(i.attachment),
            media_type,
            bytes,
        });
    }
    let req = Request {
        resume,
        fork,
        system,
        new_session: new_session.clone(),
        prompt: msg.text.clone(),
        images,
        regive: Some(given_again.clone()),
        deliver,
        ..cell_request(e, w, g, cell, item.as_ref())?
    };
    let cli_version = e.cli_version(&cli_name);
    // The record keeps each large text once, in `contents`: the conditional inputs given, the task, the
    // structured-output schema, the system prompt and each of its parts.
    let mut kept = msg.kept.clone();
    let mut call = kept_call(
        agent::call(&req, &agent::command_line(&req), &cli_version, crate::VERSION),
        &mut kept,
    );
    let (system_parts, system_texts) = sessions::system::record(&starts_with);
    for (k, t) in system_texts {
        if !kept.iter().any(|(x, _)| *x == k) {
            kept.push((k, t));
        }
    }
    // The level the round runs at, which the configuration maps to its model: the owner reads the round by it.
    call["level"] = Value::String(level_name.clone());
    // How the guard will judge the round, from the rounds kept as it starts, which its record keeps.
    let started = watch::Start {
        graph: &g.name,
        cell: &cell.id,
        section: &section,
        at_cell: &at_cell,
        resumed,
        model: &level.model,
        message: &msg.text,
        session: &line_rounds,
    }
    .run(e);
    let mut input = json!({
        "message": msg.stored,
        "conditional": msg.record(),
        "call": call,
        "engine": {
            "head": head, "columns": columns, "table": table, "interjections": ids,
            "new_session": new_why, "guard": started.lines().record(),
        },
    });
    pin.record(&mut input["engine"]);
    // The item's decisions as numbered for this round, which a later cell's check reads a number of it by.
    if let Some(d) = &decisions {
        input["engine"]["decisions"] = d.clone();
    }
    // Where the owner's images were given, by their attachments, and those only referred to.
    if !msg.images.is_empty() {
        input["images"] = msg.record_images();
    }
    // The round that starts a session with a system prompt records its parts, which the session holds to its end.
    if !starts_with.is_empty() {
        input["system"] = system_parts;
    }
    // A failed run's cause that another work's round met after the loop dispatched this step stops this round too, as
    // the owner was told: it does not start, and the work goes on from its cell once the wait is over.
    if e.halted() {
        let _ = std::fs::remove_file(&given_again);
        return Ok(Advance::Waits("a failed run's cause stops every work".to_string()));
    }
    // A fork reads its base from the prompt cache only up to a call the cache keeps under what the fork puts before its
    // messages: the session manager warms the base first when the cache may not keep its end so, and the fork and every
    // fork after it read it ([`crate::warm::before_fork`]).
    if req.fork {
        let runs = e.runs_on(&level);
        crate::warm::before_fork(e, w, &req, &input["call"], &cli_version, &runs)?;
    }
    let round =
        e.db.write(Actor::Engine, |tx| {
            db::keep(tx, &kept)?;
            work::start_numbered(tx, number, &cell.id, Some(sid), &input, &commit, Some(&level.model))
                .map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    e.say(&format!("work {} cell {}: {round} of the {line}", w.id, cell.id));
    let watching = watch::Watch::begin(e, round, started);
    let reply = e.sessions().run(&req, &|seen| watching.see(seen));
    let watched = watching.end();
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
    // A run stopped never reached the command line's report of what it used: its calls tell it.
    let usage = match &reply.stopped {
        Some(_) => watched.totals().usage(reply.usage.clone()),
        None => sessions::cost::own_cost(reply.usage.clone(), before),
    };
    let ended = Ended {
        usage: Some(usage).filter(|u| !u.is_null()),
        model: reply.model.clone(),
        compactions: reply.compactions,
        command: (!reply.command.is_empty()).then(|| reply.command.clone()),
        ..Ended::default()
    };
    // What the round came to joins the rounds later ones are judged by, unless it went past a signal that stops a round.
    let spent = ended.usage.as_ref().and_then(|u| u["cost_usd"].as_f64());
    if let Some(sample) = watched.sample(&cell.id, reply.model.as_deref(), spent) {
        let kept = e.db.write(Actor::Engine, |tx| {
            sessions::guard::keep_sample(tx, round, &g.name, &sample)?;
            Ok(())
        });
        if let Err(x) = kept {
            e.say(&format!("{round}: what it came to could not be kept: {x}"));
        }
    }
    // A work asked to stop while its round ran, to start over or by its item's cancel, hands in nothing: its round stays
    // open, with what it cost kept on it, for the loop to cut off once this step has ended, since only the loop does what
    // the engine does to a work from outside its step. So for a round the run guard stopped: the work is not run again.
    if db::stop_asked(e.db.conn(), w.id).map_err(sql)? {
        let usage = ended.usage.as_ref().map(Value::to_string);
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET usage = coalesce(usage, ?3) WHERE work = ?1 AND n = ?2",
                params![round.work, round.n, usage],
            )?;
            Ok(())
        })
        .map_err(sql)?;
        e.say(&format!(
            "work {} was asked to stop while {round} ran: what it handed in is not taken",
            w.id
        ));
        return Ok(Advance::Stopped);
    }
    // A round that resumed its session within the cache's lifetime and whose first call still read less than half of it
    // from the cache missed the cache: a fault of wf-agents, which the owner is told of, unless what comes before the
    // session's messages changed since its round before, which the miss is kept for, as its cost, with no notice.
    if req.resume.is_some()
        && let Some(usage) = &ended.usage
        && let Some(life) = req.cli.kind.cache_lifetime()
        && let Some(miss) = sessions::miss::missed(
            e.db.conn(),
            &new_session,
            &line_rounds,
            round,
            &input["call"],
            usage,
            ended.model.as_deref(),
            life,
        )?
    {
        crate::faults::cache_miss(e, w, round, &cell.id, &line, &miss)?;
    }
    let mut reply = reply;
    let mut ended = ended;
    // The attempts the command line refused in a run that handed a return in at last are kept on the round, each by
    // why: the faults of shape the agent put right within the run, which no return sent back shows.
    if reply.output.is_some() && !reply.refused.is_empty() {
        let why = Value::Array(reply.refused.iter().map(|a| json!(a.why)).collect()).to_string();
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.refused', json(?3))
                 WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                params![round.work, round.n, why],
            )?;
            Ok(())
        })
        .map_err(sql)?;
    }
    // A return the command line gave up on is repaired within the round, or handed in again by the agent, as the flow's
    // kind of failed run says; the round never runs again from its start.
    let repairs = section
        .failed
        .kinds
        .iter()
        .any(|k| k.name == crate::unparsed::KIND && k.handle == flow::format::Handle::Repair);
    if reply.output.is_none()
        && repairs
        && reply.stopped.is_none()
        && agent::failure_kind(reply.error.as_deref().unwrap_or_default()) == crate::unparsed::KIND
    {
        // A repair is checked as the return it gives would be taken.
        let accepts = |e: &Engine, candidate: &Value| -> Result<(), String> {
            let out = crate::unparsed::as_taken(g, cell, candidate.clone()).out;
            g.take(&cell.id, &out, &given)?;
            let ctx = checks::Ctx {
                e,
                w,
                g,
                cell,
                item: item.as_ref(),
                project: project.as_ref(),
                output: &out,
                unwrapped: g.unwrap(&out),
                head_start: head.clone(),
                columns: &columns,
                table: &table,
                interjections: &interjections,
                main: main.as_deref(),
            };
            checks::run(&ctx)
                .map(|_| ())
                .map_err(|problems| checks::reason(&problems))
        };
        let r = crate::unparsed::recover(e, &req, &reply, round, cell, &accepts);
        let mut record = r.record;
        // The round's model time is that of every run it made: the first run's, the repair's and the hand-in again's.
        let mut api_ms = sessions::cli::api_ms_sum(&[reply.usage[sessions::cli::API_MS].clone(), r.repair_api_ms]);
        if let Some(again) = r.again {
            // The hand-in again resumed the round's session: it cost what the session grew by since the first run.
            let was = reply.usage["session_cost_usd"].as_f64();
            let cost = match (again.usage["session_cost_usd"].as_f64(), was) {
                (Some(a), Some(b)) if a >= b => a - b,
                (Some(a), _) => a,
                _ => 0.0,
            };
            record["again"]["cost_usd"] = json!(cost);
            let compactions = reply.compactions + again.compactions;
            reply = again;
            reply.compactions = compactions;
            api_ms = sessions::cli::api_ms_sum(&[
                api_ms.unwrap_or(Value::Null),
                reply.usage[sessions::cli::API_MS].clone(),
            ]);
        }
        let mut usage = sessions::cost::own_cost(reply.usage.clone(), before);
        if let Some(c) = usage["cost_usd"].as_f64() {
            usage["cost_usd"] = json!(c + r.repair_cost);
        }
        if let Some(ms) = api_ms {
            usage[sessions::cli::API_MS] = ms;
        }
        ended = Ended {
            usage: Some(usage).filter(|u| !u.is_null()),
            model: reply.model.clone().or(ended.model),
            compactions: reply.compactions,
            ..ended
        };
        // The record keeps each attempt's whole text once, in `contents`.
        let mut kept = Vec::new();
        for a in record["attempts"].as_array_mut().into_iter().flatten() {
            if let Some(text) = a["text"].as_str().filter(|t| t.len() >= db::STORED).map(str::to_string) {
                let h = db::content_hash(&text);
                a["text"] = json!({ "content": h });
                kept.push((h, text));
            }
        }
        let recorded = record.to_string();
        e.db.write(Actor::Engine, |tx| {
            db::keep(tx, &kept)?;
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.unparsed', json(?3))
                 WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                params![round.work, round.n, recorded],
            )?;
            Ok(())
        })
        .map_err(sql)?;
        match r.output {
            Some(v) => {
                e.say(&format!("{round}: its return was repaired or handed in again"));
                reply.output = Some(v);
                // A repair reads the text of an attempt; a hand-in again through the tool keeps its own source.
                reply.output_from.get_or_insert(agent::Source::Text);
            }
            // The agent's hand-in again failed for another cause, which is handled as that kind of failed run.
            None if reply
                .error
                .as_deref()
                .is_some_and(|x| agent::failure_kind(x) != crate::unparsed::KIND) => {}
            None => {
                let why = format!(
                    "cell {}: the agent's return could not be parsed as JSON, and neither a repair nor handing it in \
                     again gave one",
                    cell.id
                );
                let ended = Ended {
                    rejected: Some(json!({ "failed": { "kind": crate::unparsed::KIND, "what": why } })),
                    transition: None,
                    ..ended
                };
                e.db.write(Actor::Engine, |tx| {
                    work::finish_round(tx, round, &ended).map_err(db::Error::Abort)
                })
                .map_err(sql)?;
                return failed::pause(e, w, &why);
            }
        }
    }
    let Some(output) = reply.output.clone() else {
        let f = Failed {
            w,
            g,
            cell,
            round,
            section: &section,
            guard: watched.stop_record(),
        };
        return f.handle(e, ended, &reply);
    };
    e.returned();
    // Every return is taken one way, whichever way it came: a form the agent wrote as a string holding its JSON taken as
    // the form, a field every return holds that it wrote inside the field its forms are under moved up beside it, and a
    // field the flow lets it leave out that it wrote as null left out. The round records a form taken from text, so
    // that how often models still write it shows.
    let crate::unparsed::Taken {
        out: output,
        from_text,
        foreign_nulls,
    } = crate::unparsed::as_taken(g, cell, output);
    // Where the return was read from (the tool the command line checked it in, or the text of the run's result, which
    // no check of the command line saw), and the nulls taken as left out for being fields of another form: what shows
    // how a return reached the engine with a fault the command line refuses.
    let mut kept = serde_json::Map::new();
    if let Some(from) = reply.output_from {
        kept.insert("return_from".into(), json!(from.name()));
    }
    if !foreign_nulls.is_empty() {
        kept.insert("foreign_nulls".into(), json!(foreign_nulls));
    }
    if !kept.is_empty() {
        let patch = json!({ "engine": kept }).to_string();
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET input = json_patch(input, ?3)
                 WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                params![round.work, round.n, patch],
            )?;
            Ok(())
        })
        .map_err(sql)?;
    }
    if from_text {
        e.say(&format!(
            "{round}: its {} was handed in as the JSON text of it, and is taken as the object",
            g.within
        ));
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.taken_from_text', ?3)
                 WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                params![round.work, round.n, g.within],
            )?;
            Ok(())
        })
        .map_err(sql)?;
    }
    let row = match g.take_or_places(&cell.id, &output, &given) {
        Ok(r) => r.clone(),
        Err(not_taken) => {
            let problems: Vec<checks::Problem> = not_taken.places.iter().map(checks::Problem::format).collect();
            return reject(
                e,
                w,
                &section,
                round,
                Ended {
                    output: Some(output),
                    ..ended
                },
                checks::record("format", not_taken.text, &problems),
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
        main: main.as_deref(),
    };
    let aside = match checks::run(&ctx) {
        Ok(aside) => aside,
        Err(problems) => {
            return reject(
                e,
                w,
                &section,
                round,
                Ended {
                    output: Some(output),
                    ..ended
                },
                checks::record("check", checks::reason(&problems), &problems),
            );
        }
    };
    // What only the checks the work says it changes found is kept on the round, for the reviews to judge.
    let set_aside = crate::set_aside::record(e.db.conn(), w.id, &aside)?;
    if !aside.is_empty() {
        e.say(&format!(
            "{round}: taken with {} problems set aside, found only by checks the work says it changes:\n{}",
            aside.len(),
            checks::reason(&aside)
        ));
    }
    let ended = Ended { set_aside, ..ended };
    finish(e, w, g, cell, round, output, &row, ended, &ids, head_after, &given)
}

/// A round's call as its record keeps it: the structured-output schema the command line gives and the system prompt it
/// appends, each the same for every round of a session, kept once in `contents` and referred to there.
pub(crate) fn kept_call(mut call: Value, kept: &mut Vec<(String, String)>) -> Value {
    for flag in ["--json-schema", sessions::system::FLAG] {
        if let Some(args) = call["command"].as_array_mut()
            && let Some(i) = args.iter().position(|a| a == flag).map(|i| i + 1)
            && let Some(text) = args.get(i).and_then(Value::as_str).map(str::to_string)
            && text.len() >= db::STORED
        {
            let h = db::content_hash(&text);
            args[i] = json!({ "content": h });
            if !kept.iter().any(|(k, _)| *k == h) {
                kept.push((h, text));
            }
        }
    }
    call
}

/// The directory a cell's round runs in, which the command line's tools and git work in: the work's worktree; for a
/// work on an item that has none yet, the project's checkout; for a project's architect, its graph's worktree, and for
/// a cell of the architect's on no line an empty directory of its own ([`architect::workdir`]). What tells an agent
/// where git runs (`history_sources`, in [`crate::inputs`]) reads it here, so that it is where the round really runs.
pub fn round_directory(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    project: Option<&config::Project>,
) -> Result<std::path::PathBuf, String> {
    match (&w.worktree, project) {
        (Some(wt), _) => Ok(wt.clone()),
        (None, Some(p)) => Ok(p.checkout.clone()),
        (None, None) => Ok(architect::workdir(&e.data, g, cell, architect::project_of(w)?)),
    }
}

/// What an agent's round at a cell is sent with, but for its message, its session and its hooks: the CLI and the model
/// of the level the cell runs at, the CLI's configuration directory, the effort, the structured-output schemas, the
/// tools and commands its line refuses, the directories outside where it runs that it may read, and where it runs: a
/// work in its worktree, a project's architect in its graph's worktree at the project's main, and a cell of the
/// architect's on no line in an empty directory ([`architect::workdir`]). What of
/// it the command line puts before the session is the same for every cell of a line, and of the lines forked together
/// with it ([`sessions::cli::forked_together`]), so that a fork reads its base from the prompt cache, but for the
/// effort, which the flow's check keeps from turning thinking on or off, or the line's session reads nothing from the
/// prompt cache at a change of cell; tests hold every line of the flow to that.
pub fn cell_request(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
) -> Result<Request, String> {
    let (_, level, cli) = level_of(e, g, cell, item).map_err(|x| format!("work {}: {x}", w.id))?;
    let line = crate::line_of(g, cell);
    let project = match item {
        Some(_) => Some(e.project_of(w)?),
        None => None,
    };
    let worktree = round_directory(e, w, g, cell, project.as_ref())?;
    let readable = readable_dirs(e);
    let permissions = &e.config.config.install.agents.permissions;
    let (allowed, refused) = (permissions.allowed(&line, &cli), permissions.refused(&line, &cli));
    Ok(Request {
        work: w.id,
        cell: cell.id.clone(),
        cli,
        model: level.model.clone(),
        effort: sessions::choose::effort(cell),
        worktree,
        config_dir: e.cli_home(&level.cli),
        resume: None,
        fork: false,
        system: None,
        new_session: String::new(),
        prompt: String::new(),
        images: Vec::new(),
        schema: cell.schema.clone().unwrap_or_else(|| json!({ "type": "object" })),
        envelope: sessions::cli::envelope(g, cell.line.as_deref()),
        allowed,
        refused,
        readable,
        replay: None,
        regive: None,
        deliver: None,
        bare: false,
        line,
    })
}

/// The level an agent's round at a cell runs at, its name with what it is and its CLI: the item's, or one above for the
/// advisor, which is the engineer with a higher model; for the architect's base, each matter's session and the second
/// eye, the level the installation names for the architect, so that a fork runs on the model of the base it forks.
pub fn level_of(
    e: &Engine,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
) -> Result<(String, config::Level, config::Cli), String> {
    let name = match item {
        Some(i) => e.config.config.level_of_line(&i.level, &crate::line_of(g, cell)),
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
    /// The session's rounds, oldest first: those of earlier architect works too, for the architect's line; none for a
    /// new session.
    line_rounds: Vec<Round>,
    given: Map<String, Value>,
    /// The interjections waiting for the line.
    interjections: Vec<Value>,
    columns: Vec<(String, String)>,
    /// The rows of the cell's numbered table, each with its number.
    table: Vec<Value>,
    /// The item's decisions the round is given, each with its number, for a cell that reads them
    /// ([`inputs::decisions_given`]).
    decisions: Option<Value>,
    /// The round it will be.
    number: RoundId,
    msg: message::Message,
    /// The message as a session that holds nothing but its system prompt is sent it: what a compaction within the round
    /// gives again.
    whole: String,
    /// The system prompt the round's command line carries: the session's own, or, for the round that starts it, what
    /// every session of the line is given alike ([`message::line_parts`]); none for a session that started without one.
    system: Option<String>,
    /// The parts of the system prompt, for the round that starts a session with them, which its record keeps.
    starts_with: Vec<(String, Value)>,
}

/// Builds the message of an agent's round at a cell from the work as it stands, running and recording nothing. The
/// session manager chooses the session it runs in and says what that session holds; the architect's line goes on in
/// one CLI session across the architect's works, so what its session holds is read from the rounds of every work that
/// ran in it.
fn compose(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
    main: Option<&str>,
    now: i64,
) -> Result<Composed, String> {
    let line = sessions::choose::line_of(cell);
    let (_, level, cli) = level_of(e, g, cell, item)?;
    let runs = e.runs_on(&level);
    let limit = e.config.config.install.agents.context_limit;
    let architect = w.project.as_deref().filter(|_| g.name == ARCHITECT);
    let fork = crate::base::fork_of(e, w, cell, cli.kind.forks())?;
    let mut plan = e
        .sessions()
        .plan(e.db.conn(), w.id, architect, cell, limit, runs.on(), now, fork.as_ref())?;
    // The rounds the agent ran or was told of: a step of the engine's before one that failed in passing is neither.
    let work_rounds: Vec<Round> = work::rounds(e.db.conn(), w.id)?
        .into_iter()
        .filter(|r| !failed::was_step(r))
        .collect();
    let number = work::next_round(e.db.conn(), w.id)?;
    // A fork holds what its base held: the base's rounds are its session's from its start.
    let (line_rounds, held) = match &mut plan {
        Plan::Going { rounds, held, .. } | Plan::Fork { rounds, held, .. } => {
            (std::mem::take(rounds), *std::mem::take(held))
        }
        Plan::New(_) => (Vec::new(), Held::default()),
    };
    // A session that has run no round yet starts with the system prompt every session of its line is given alike; one
    // going on carries the one it started with, or none.
    let (system, starts_with, held) = if line_rounds.is_empty() {
        let named = item
            .map(|i| i.project.as_str())
            .or(w.project.as_deref())
            .unwrap_or_default();
        let parts = message::line_parts(e, g, cell, named);
        let held = Held::with_system(&sessions::system::versions(&parts, number));
        (Some(sessions::system::text(&parts)), parts, held)
    } else {
        (
            sessions::system::of_session(e.db.conn(), &line_rounds),
            Vec::new(),
            held,
        )
    };
    let mut known: Vec<Round> = line_rounds.iter().filter(|r| r.id.work != w.id).cloned().collect();
    known.extend(work_rounds.iter().cloned());
    let registry = message::registry(&known);
    let gathered = inputs::gather(e, w, g, cell, item, main)?;
    let given = values::given(e, w, g, cell, item, &gathered)?;
    // The interjections waiting for the cell's line, when the cell takes them.
    let speaks_to = crate::line_of(g, cell);
    let mut interjections: Vec<Value> = if crate::takes_interjections(cell) {
        inputs::db_rows(e, w, item, "waiting_interjections", Some("work"))?
            .into_iter()
            .filter(|r| r["line"] == speaks_to.as_str())
            .collect()
    } else {
        Vec::new()
    };
    let message::ReviewColumns {
        columns,
        renumbered: columns_renumbered,
    } = message::columns(e, w, g, cell, item);
    let table = inputs::table(&gathered);
    let decisions = inputs::decisions_given(&gathered);
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
        columns_renumbered,
        table: &table,
        held: &held,
        registry: &registry,
    };
    let msg = context.build();
    let nothing = Held::with_system(&held.system);
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
        decisions,
        number,
        msg,
        whole,
        system,
        starts_with,
    })
}

/// The message the work's next round would be sent, when the work is at an agent's cell: built as its round builds
/// it, with nothing run or recorded, and so with no fetch: the project is read at the commit `origin/<main>` is at in
/// its checkout now.
pub fn next_message(e: &mut Engine, work: i64) -> Result<message::Message, String> {
    let w = e.work(work)?;
    let g = e.graph_of(&w)?;
    let cell = g
        .cell(&w.pos.at)
        .filter(|c| c.kind == "agent")
        .cloned()
        .ok_or_else(|| format!("work {work} is at cell {}, which is not an agent's", w.pos.at))?;
    let item = e.subject(&w)?;
    let pin = inputs::pin_main(e, &w, &cell, item.as_ref(), false)?;
    Ok(compose(e, &w, &g, &cell, item.as_ref(), pin.commit.as_deref(), db::now())?.msg)
}

/// Ends a round whose return is sent back, and tells the owner once if the work passes the limit its project sets on
/// the counter that counts the returns sent back.
fn reject(
    e: &mut Engine,
    w: &Work,
    section: &flow::format::Rounds,
    round: RoundId,
    ended: Ended,
    why: Value,
) -> Result<Advance, String> {
    e.say(&format!("{round} sent back: {}", db::without_problems(&why)));
    let ended = Ended {
        rejected: Some(why),
        transition: None,
        ..ended
    };
    e.db.write(Actor::Engine, |tx| {
        work::finish_round(tx, round, &ended).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    if let Some(counter) = &section.sent_back.counts {
        crate::limits::tell_past(e, w, counter)?;
    }
    Ok(Advance::Moved)
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
        projects: e.config.config.projects.clone(),
        caller_line: w
            .pos
            .callers
            .last()
            .and_then(|c| g.cell(c))
            .map(|c| crate::line_of(g, c)),
        row: row.clone(),
        to: moved.pos.clone(),
        to_title: g.cell(&moved.pos.at).map(|c| c.title.clone()).unwrap_or_default(),
        blockers: g.blockers.clone(),
    };
    let ended = Ended {
        output: Some(output),
        ..ended.taking(row)
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
    e.say(&format!(
        "work {} cell {} took \"{}\"{did}: now at {}",
        w.id, cell.id, row.name, moved.pos.at
    ));
    // Past a counter's limit the row is taken all the same, and the owner is told once.
    if let Some(counter) = row.counts.as_deref() {
        crate::limits::tell_past(e, w, counter)?;
    }
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
    e.say(&format!("work {} done", w.id));
    Ok(Advance::Moved)
}

/// The step of an engine's, a tool's or the owner's cell. `slotted` says whether the step is held to its machine's
/// slots, as every step of a work on an item is but one the owner asked for: a work that waits, holding no slot, then
/// stays where it waits for as long as going on would take a slot its machine does not have free ([`no_slot`]).
pub fn engine(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    slotted: bool,
) -> Result<Advance, String> {
    let item = e.subject(w)?;
    let open = close_interrupted(e, w)?;
    let step = cell.step.clone();
    if step.as_deref() == Some("bring_up") {
        return bring_up(e, w, g, cell, None);
    }
    if step.as_deref() == Some(architect::STOP) {
        return architect::stop_work(e, w, g, cell);
    }
    match step.as_deref() {
        Some("ask_owner") if w.graph == ARCHITECT => {
            return architect::ask_owner(e, w, g, cell, open);
        }
        Some("ask_owner") => return ask(e, w, g, cell, needed(&item, w)?, open, slotted),
        Some("ask_architect") => {
            let item = needed(&item, w)?;
            // A question an earlier engine put to the owner, who answered for the architect, is still the owner's to
            // answer; any other is queued for the architect.
            let stood_in = match open.as_ref().filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
                Some(r) => architect::owner_ask_of(e, r.id)?.is_some(),
                None => false,
            };
            if stood_in {
                return ask(e, w, g, cell, item, open, slotted);
            }
            return architect::ask_architect(e, w, g, cell, item, open, slotted);
        }
        _ => {}
    }
    // The agent paused: the work waits, paused by the engine, for the owner's Start; once the owner started it again
    // since it came here, it goes back.
    if step.as_deref() == Some("wait_for_start") {
        let came = work::rounds(e.db.conn(), w.id)?.last().and_then(|r| r.ended_at);
        let resumed = work::resumed_at(e.db.conn(), w.id)?;
        if !resumed.is_some_and(|r| came.is_none_or(|c| r > c)) {
            let caller = w.pos.callers.last().cloned().unwrap_or_default();
            let budget = crate::limits::ended_used_up(&e.config.config, e.db.conn(), w.id)?;
            let why = match budget {
                Some(minutes) => format!(
                    "cell {caller} paused: the work's time budget for a stretch ({minutes} minutes) is used up; start \
                     the work for another stretch"
                ),
                None => format!("cell {caller} paused; start the work for it to carry on"),
            };
            // A work on an item that used up its budget has its project's architect look at where it is stuck.
            if let Some(minutes) = budget {
                e.db.write(Actor::Engine, |tx| {
                    crate::limits::queue_stuck(tx, w.id, minutes).map_err(db::Error::Abort)
                })
                .map_err(sql)?;
            }
            e.pause(w.id, &why)?;
            return Ok(Advance::Stopped);
        }
    }
    // Where an item waits to start, which no work stands at but one made on a flow version that has it: what holds the
    // item back, as the engine works it out for every item that waits.
    if step.as_deref() == Some("wait_to_start") {
        let item = needed(&item, w)?;
        let changing = architect::held_by_checks(e.db.conn())?;
        let free = e.free_slots()?;
        let reason = e.waits_to_start(item.id, &item.project, &changing, free)?;
        let mut given = Map::new();
        given.insert("start".into(), json!(reason));
        let row = g.take(&cell.id, &json!({}), &given)?.clone();
        if row.waits {
            return wait_by(e, w, g, cell, &row, json!({ "given": given }));
        }
        let commit = e.config.commit.clone();
        let input = json!({ "given": given });
        let round =
            e.db.write(Actor::Engine, |tx| {
                work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
            })
            .map_err(sql)?;
        return finish(
            e,
            w,
            g,
            cell,
            round,
            json!({}),
            &row,
            Ended::default(),
            &[],
            None,
            &given,
        );
    }
    // Starting a work takes the queue's next item and decides nothing, so its round records no input.
    let starts = step.as_deref() == Some("start_work");
    let gathered = if starts {
        Vec::new()
    } else {
        inputs::gather(e, w, g, cell, item.as_ref(), None)?
    };
    let given = values::given(e, w, g, cell, item.as_ref(), &gathered)?;
    // A work behind another in its project's merge queue waits its turn by the merge cell's row that waits, holding no
    // slot; a flow version without that row has it wait the same, unrecorded. Its turn come, it merges only once it may
    // take a slot. The turn is read once, with the values that choose the row the merge takes afterwards: the row is
    // chosen by the turn this step acted on, whatever the queue became meanwhile.
    if step.as_deref() == Some("merge") {
        // A flow version from before the cell's rows read the turn does not give it with the other values.
        let turn = match given.get("turn").and_then(Value::as_str) {
            Some(turn) => turn.to_string(),
            None => values::value(e, w, g, item.as_ref(), "turn")?,
        };
        if turn == "behind" {
            let Some(row) = crate::row(cell, "behind").cloned() else {
                return Ok(Advance::Waits(format!(
                    "work {} waits its turn in the merge queue",
                    w.id
                )));
            };
            return wait_by(e, w, g, cell, &row, json!({ "given": { "turn": "behind" } }));
        }
        if slotted && e.waiting(w)? && !e.granted(w.id) {
            return Ok(waits_for_slot(w));
        }
    }
    let commit = e.config.commit.clone();
    let input = if starts {
        json!({})
    } else {
        json!({ "inputs": recorded_inputs(&gathered), "given": given })
    };
    // A step that only chooses the row is taken at once; so is where a project's architect waits for a work to start,
    // which no work of the architect stands at, its works starting where its row leads: a work there would take the row
    // for whether a matter is due for its project.
    let quick = matches!(step.as_deref(), None | Some("wait_for_start") | Some("wait_for_matter"));
    if quick {
        // What such a step returns is nothing but at the merge gate, which hands the fix the findings set aside.
        let output = crate::set_aside::gate_return(cell, &gathered, &given);
        let row = g.take(&cell.id, &output, &given)?.clone();
        if row.waits && waits_again(e, w, cell, &row)? {
            return Ok(Advance::Waits(format!("cell {} \"{}\"", cell.id, row.name)));
        }
        if no_slot(e, w, g, &row, slotted)? {
            return Ok(waits_for_slot(w));
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
    e.say(&format!(
        "work {} cell {}: {}",
        w.id,
        cell.id,
        step.clone().unwrap_or_default()
    ));
    let output = match step.as_deref() {
        Some("start_work") => start_work(e, w, needed(&item, w)?)?,
        Some("run_tools") => {
            let out = tools::run_tools(e, w, item.as_ref(), round, &gathered)?;
            // A tool that failed for want of the network while the network is down is no failure of the work's: no
            // question goes to the architect; every work waits for the network, and this cell runs again then.
            if let Some(what) = network_failure(&out)
                && e.network_down()
            {
                return e.wait_offline(w.id, &format!("cell {}: {what}", cell.id));
            }
            out
        }
        Some("check_records") => {
            let project = e.project_of(w)?;
            let wt = w
                .worktree
                .as_deref()
                .ok_or_else(|| format!("work {} has no worktree to check the records in", w.id))?;
            let fork = git::fork_point(wt, &project.main)?;
            crate::reasoning::check(e, &project, wt, &fork)?
        }
        Some("merge") => tools::merge(e, w, needed(&item, w)?, round)?,
        Some("end_architect_work") => {
            // The run is over: the session its matters shared ends with it, and the next run forks anew.
            let lines: Vec<&str> = g
                .cells
                .iter()
                .filter(|c| c.whole_run)
                .filter_map(|c| c.line.as_deref())
                .collect();
            e.db.write(Actor::Engine, |tx| {
                for line in &lines {
                    sessions::choose::end_line(tx, w.id, line).map_err(db::Error::Abort)?;
                }
                Ok(())
            })
            .map_err(sql)?;
            json!({})
        }
        Some("end_work") | Some("stop_work") => json!({}),
        Some("look_at_queue") => architect::look_at_queue(e, w, cell)?,
        Some("check_main") => crate::main_watch::check(e, w, round)?,
        Some(other) => return Err(format!("no such engine step: {other}")),
        None => json!({}),
    };
    if cell.rows.is_empty() {
        let output = match step.as_deref() {
            Some("end_work") => end_work(e, w, g, cell)?,
            _ => output,
        };
        return finish_end(e, w, round, output, &|_| Ok(()));
    }
    let row = g.take(&cell.id, &output, &given)?.clone();
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given)
}

/// What brought a work to its graph's cell for a work on an older flow version, as the engine's loop worked it out: the
/// version it was on, why the version the engine runs could not take it on where it stood, and whether it stays there
/// stuck, for the owner to decide, though it has a cell to go back to.
pub struct Brought {
    pub from: i64,
    pub why: String,
    pub stuck: bool,
}

/// The step of the cell a work on an older flow version comes to (a graph's `version`, `bring_up`): its round records
/// the versions, the cell the work goes back to and why, and the rounds the work does again from (`redoes_from`), and
/// takes `went_back`, to that cell, or, given `stuck`, the row that keeps the work there and pauses it, or the one that
/// ends it at the graph's end for a work stopped before its end, whose step then runs at once, whatever holds the work
/// otherwise: that step is how it ends. The engine's loop runs it as it brings the work there (`brought`); a work there
/// again, as when the owner started one stuck, runs it as its step, by what its last round there said: one with a cell
/// to go back to goes back, the owner's Start being the word to.
pub fn bring_up(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    brought: Option<Brought>,
) -> Result<Advance, String> {
    let (from, why, stuck) = match brought {
        Some(b) => (b.from, b.why, b.stuck),
        None => {
            let last = work::rounds_at(e.db.conn(), w.id, &cell.id)?
                .into_iter()
                .find_map(|r| r.output);
            let from = last.as_ref().and_then(|o| o["from"].as_i64()).unwrap_or(w.flow_version);
            let why = last
                .as_ref()
                .and_then(|o| o["why"].as_str())
                .unwrap_or("the engine brought it here")
                .to_string();
            (from, why, false)
        }
    };
    let Arrival {
        given,
        output,
        row,
        pause,
        ..
    } = arrival(e, w, g, cell, from, &why, stuck)?;
    let commit = e.config.commit.clone();
    let input = json!({ "given": given });
    let round =
        e.db.write(Actor::Engine, |tx| {
            work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    let moved = finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given)?;
    if let Some(pause) = pause {
        e.pause(w.id, &pause)?;
    }
    let now = e.work(w.id)?;
    if let Some(end) = g.cancel.as_deref().filter(|c| *c == now.pos.at).and_then(|c| g.cell(c)) {
        return engine(e, &now, g, end, false);
    }
    Ok(moved)
}

/// What the step of the cell a work on an older flow version comes to does with a work standing there, worked out
/// without writing: what it is given (`brought_up`), what its round records (the versions, the cell the work goes back
/// to, why, and the round it does again from), the row it takes by them, the rounds it does again, and, for a row that
/// pauses the work, what it is paused for. [`bring_up`] carries it out; what installing a flow would do
/// ([`crate::flow_cost`]) reads it for a work standing there as the engine would leave it.
pub(crate) struct Arrival {
    pub given: Map<String, Value>,
    pub output: Value,
    pub row: CompiledRow,
    /// The first round the work does again and how many it does again, from the latest stretch of the cell it goes
    /// back to on: none when it does not go back, or does no round again.
    pub redone: Option<(RoundId, usize)>,
    /// What the work is paused for, when the row pauses it.
    pub pause: Option<String>,
}

/// What the step of the cell a work on an older flow version comes to does with work `w` standing there
/// ([`Arrival`]), brought from version `from` for the reason `why`, and `stuck` when the engine keeps it there though it
/// has a cell to go back to.
pub(crate) fn arrival(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    from: i64,
    why: &str,
    stuck: bool,
) -> Result<Arrival, String> {
    let mut given = Map::new();
    let back = values::value(e, w, g, None, "brought_up")?;
    let brought_up = if stuck { "stuck".to_string() } else { back };
    given.insert("brought_up".into(), json!(brought_up));
    let back_to = w.pos.stopped.clone().filter(|c| g.cell(c).is_some());
    let rounds = work::rounds(e.db.conn(), w.id)?;
    let redone = back_to
        .as_deref()
        .filter(|_| brought_up == "went_back")
        .map(|t| crate::versions::redone_from(g, &rounds, t))
        .and_then(|i| rounds.get(i).map(|r| (r.id, rounds.len() - i)));
    let output = json!({
        "from": from, "to": e.flow_now, "back_to": back_to, "why": why, "redoes_from": redone.map(|(r, _)| r.n),
    });
    let row = g.take(&cell.id, &output, &given)?.clone();
    let pause = if row.pauses {
        // A paused work is brought to a newer version only as it goes on, so the owner's Start is what looks again.
        let again = "Start it once a flow version that has a cell it walked is installed, and it looks again";
        // An engineer's work whose merge stands can be neither started over nor cancelled.
        let ends = match w.item {
            Some(item) if w.graph != ARCHITECT => !db::merge_stands(e.db.conn(), item).map_err(|x| x.to_string())?,
            _ => false,
        };
        let then = match (&back_to, ends) {
            (Some(t), _) => format!("Start takes it back to cell {t} all the same"),
            (None, false) => again.to_string(),
            (None, true) => format!("Start it over, or cancel its item; or {again}"),
        };
        let now = e.flow_now;
        Some(format!("it cannot go on in flow version {now}: {why}. {then}"))
    } else {
        None
    };
    Ok(Arrival {
        given,
        output,
        row,
        redone,
        pause,
    })
}

/// The first tool of a tool step's output that failed reaching the network or GitHub's API, with what it reported.
fn network_failure(out: &Value) -> Option<String> {
    out["failed"]
        .as_array()?
        .iter()
        .find(|f| f["kind"] == "network_or_api")
        .map(|f| {
            format!(
                "{}: {}",
                f["tool"].as_str().unwrap_or_default(),
                f["what"].as_str().unwrap_or_default()
            )
        })
}

/// Whether a work, in a step held to its machine's slots (`slotted`), stays where it waits instead of going on by
/// `row`: it waits, holding no slot; the row would take it where it holds one, since the row does not wait again nor
/// lead to an ask of the owner or the architect, where the work waits holding none; and the loop granted its step no
/// slot, which only the loop does, so that two steps never take the same free slot.
pub fn no_slot(e: &mut Engine, w: &Work, g: &CompiledGraph, row: &CompiledRow, slotted: bool) -> Result<bool, String> {
    if !slotted || row.waits {
        return Ok(false);
    }
    let next = g.step(&w.pos, row)?;
    let asks = g
        .cell(&next.at)
        .is_some_and(|c| matches!(c.step.as_deref(), Some("ask_owner" | "ask_architect")));
    Ok(!asks && e.waiting(w)? && !e.granted(w.id))
}

/// A work that waits on where it waited, what it waited for come, until the loop grants it a slot; the loop keeps that
/// it does until its next step, for the owner's page.
pub fn waits_for_slot(w: &Work) -> Advance {
    Advance::Held(format!(
        "work {} waits for a free slot to go on from cell {}",
        w.id, w.pos.at
    ))
}

/// Whether a row that waits was the last the work took, at this cell: taken again with nothing new, it is not
/// recorded again.
fn waits_again(e: &Engine, w: &Work, cell: &CompiledCell, row: &CompiledRow) -> Result<bool, String> {
    let last = work::rounds(e.db.conn(), w.id)?.pop();
    Ok(last.is_some_and(|r| {
        r.cell.as_deref() == Some(cell.id.as_str()) && r.transition.as_deref() == Some(row.name.as_str())
    }))
}

/// Takes a row that waits, which the engine chose without running the cell's step: recorded once, with what chose it.
fn wait_by(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    row: &CompiledRow,
    input: Value,
) -> Result<Advance, String> {
    if waits_again(e, w, cell, row)? {
        return Ok(Advance::Waits(format!("cell {} \"{}\"", cell.id, row.name)));
    }
    let commit = e.config.commit.clone();
    let round =
        e.db.write(Actor::Engine, |tx| {
            work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
    finish(
        e,
        w,
        g,
        cell,
        round,
        json!({}),
        row,
        Ended::default(),
        &[],
        None,
        &Map::new(),
    )
}

/// Cell `S1`: the session manager sets up where the work runs, on a machine with a free slot, and the id the
/// engineer's session will have.
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

/// Cell `E1`: the work ends as every work does ([`crate::ending::end`]): its sessions end, and its worktree is removed
/// once the round is written; the branch stays. A work that delivered a report and handed in what to do next queues it
/// for the architect, as a matter of its own, when the cell names that (`queue_followup`), as a flow version before
/// the cell named what it does did too. What was said to the work and never handed in stays so: no round runs off the
/// graph to answer it, the way on no longer changes, and the owner's page marks it on the words themselves as not
/// received, in place of the notice a flow version's `tell_undelivered` named, which tells nothing now.
fn end_work(e: &mut Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let numbers = numbers(e, w, g)?;
    let follows = cell.actions.is_empty() || cell.actions.iter().any(|a| a == "queue_followup");
    let followup = if follows {
        report_followup(e, w, &project)?
    } else {
        None
    };
    let projects = &e.config.config.projects;
    e.db.write(Actor::Engine, |tx| {
        crate::ending::end(tx, w.id, crate::ending::End::Done, projects).map_err(db::Error::Abort)?;
        if let Some(m) = &followup {
            db::queue_matter(tx, m)?;
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(json!({ "numbers": numbers }))
}

/// What the engine records of a work as it ends (cell `E1`'s `numbers`), the data the flow's convergence is judged by:
/// for each stage its rounds went through, in the graph's order, how long they ran, what they cost and how many ran,
/// a round at a cell of no stage (running tools, asking) counting to the stage the work was at; and each time the work
/// went back to an earlier stage, the round, the cell it left, the cell it went to and why, as the row it took counts
/// it, or by the row's name where it counts nothing.
fn numbers(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Value, String> {
    let mut order: Vec<String> = Vec::new();
    for c in &g.cells {
        if let Some(s) = &c.stage
            && !order.contains(s)
        {
            order.push(s.clone());
        }
    }
    let at = |s: &str| order.iter().position(|x| x == s);
    let mut stages: Vec<(String, i64, f64, i64)> = Vec::new();
    let mut went_back = Vec::new();
    let mut current: Option<String> = None;
    for r in work::rounds(e.db.conn(), w.id)? {
        let Some(c) = r.cell.as_deref().and_then(|id| g.cell(id)) else {
            continue;
        };
        if c.stage.is_some() {
            current.clone_from(&c.stage);
        }
        let Some(stage) = current.clone() else { continue };
        let ms = r.ended_at.map_or(0, |end| (end - r.started_at).max(0));
        let cost = r.usage.as_ref().and_then(|u| u["cost_usd"].as_f64()).unwrap_or(0.0);
        match stages.iter_mut().find(|x| x.0 == stage) {
            Some(x) => {
                x.1 += ms;
                x.2 += cost;
                x.3 += 1;
            }
            None => stages.push((stage.clone(), ms, cost, 1)),
        }
        let row = r
            .transition
            .as_deref()
            .and_then(|t| c.rows.iter().find(|x| x.name == t));
        let Some(row) = row else { continue };
        let to = g.cell(&row.to).and_then(|t| t.stage.clone());
        if let (Some(to), Some(from)) = (to.as_deref().and_then(at), at(&stage))
            && to < from
        {
            went_back.push(json!({
                "round": r.id.to_string(),
                "from": c.id,
                "to": row.to,
                "why": row.counts.clone().unwrap_or_else(|| row.name.clone()),
            }));
        }
    }
    stages.sort_by_key(|x| at(&x.0));
    let stages: Vec<Value> = stages
        .into_iter()
        .map(|(stage, ms, cost, n)| {
            json!({ "stage": stage, "minutes": ms / 60_000, "cost": format!("{cost:.2}"), "rounds": n })
        })
        .collect();
    Ok(json!({ "stages": stages, "went_back": went_back }))
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
             WHERE work = ?1 AND cell = 'M2' AND transition = 'merged' ORDER BY n DESC LIMIT 1",
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

/// Cell `A3` of a flow version from before the engineer's graph lost its hand-over to the owner: the question becomes an
/// ask of the owner, and the work waits for the answer, holding no slot; once the answer came, it goes on when its
/// machine has a slot free, in a step held to the slots (`slotted`, [`no_slot`]). An ask of cell `A2` that an earlier
/// engine put to the owner, who answered for the architect in the architect's form, is completed by the owner's answer
/// the same way. A work standing at `A3` is taken over by the version the engine runs when it next goes on, which has
/// no such cell, so that this runs only for a work the engine has not brought up yet.
fn ask(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    open: Option<Round>,
    slotted: bool,
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
            // What was said to the work before the owner answered takes it on to choose the way on: the ask needs no
            // answer then.
            if let Some(row) = crate::row(cell, "interjection").cloned() {
                let gathered = inputs::gather(e, w, g, cell, Some(item), None)?;
                let given = values::given(e, w, g, cell, Some(item), &gathered)?;
                if given.get("interjection").and_then(Value::as_str) == Some("yes") {
                    if no_slot(e, w, g, &row, slotted)? {
                        return Ok(waits_for_slot(w));
                    }
                    return finish(
                        e,
                        w,
                        g,
                        cell,
                        r.id,
                        json!({}),
                        &row,
                        Ended::default(),
                        &[],
                        None,
                        &given,
                    );
                }
            }
            return Ok(Advance::Waits(format!("ask {id} waits for the owner")));
        };
        let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
        // The answer's words end with the line that names the images the owner sent with them.
        let images = db::attachments::of(e.db.conn(), db::attachments::Words::Ask(id)).map_err(sql)?;
        let text = db::attachments::with_named(answer["text"].as_str().unwrap_or_default(), &images);
        let output = if to == "owner" {
            // The owner resolves it, or hands it back to the architect with a decision for it to carry out.
            match answer["to"].as_str().filter(|t| *t == "architect") {
                Some(t) => json!({ "reply": { "text": text, "to": t } }),
                None => json!({ "reply": { "text": text } }),
            }
        } else {
            match answer.get("reply").filter(|x| x.is_object()) {
                Some(reply) => {
                    let mut reply = reply.clone();
                    if !images.is_empty() {
                        reply["text"] = json!(text);
                    }
                    json!({ "reply": reply })
                }
                None => {
                    json!({ "reply": { "text": text, "why": answer["why"].as_str().unwrap_or("The owner answered for the architect.") } })
                }
            }
        };
        let gathered = inputs::gather(e, w, g, cell, Some(item), None)?;
        let mut given = values::given(e, w, g, cell, Some(item), &gathered)?;
        // The owner's answer came: what was said to the work meanwhile reaches it at its next round.
        if given.contains_key("interjection") {
            given.insert("interjection".into(), json!("no"));
        }
        let row = g.take(&cell.id, &output, &given)?.clone();
        if no_slot(e, w, g, &row, slotted)? {
            return Ok(waits_for_slot(w));
        }
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    if to != "owner" {
        return Err(format!(
            "{}: a question to the architect is queued for it, not put to the owner",
            cell.id
        ));
    }
    let gathered = inputs::gather(e, w, g, cell, Some(item), None)?;
    // The ask holds what the cells handed in: the question, how far the cell got, what failed. The history and the
    // other tables the cell reads are in the database for the owner to look at.
    let mut asked: Vec<Value> = gathered
        .iter()
        .filter(|x| x.value.is_some() && x.input.from.iter().all(|s| matches!(s, Src::Place(..))))
        .map(|x| json!({ "what": x.input.what, "value": x.value }))
        .collect();
    let about = what_is_asked(&asked);
    // A question handed to the owner says how it came: every hand-over, with why and what each holder found.
    let moves: Option<Value> = match crate::blockers::open(e.db.conn(), w.id)? {
        Some(b) => {
            e.db.conn()
                .query_row("SELECT moves FROM blockers WHERE id = ?1", [b.id], |r| {
                    r.get::<_, String>(0)
                })
                .ok()
                .and_then(|m| serde_json::from_str::<Value>(&m).ok())
                .filter(|m| m.as_array().is_some_and(|ms| !ms.is_empty()))
        }
        None => None,
    };
    let handed = moves.as_ref().and_then(|m| m.as_array()?.last().cloned());
    if let Some(m) = &moves {
        asked.push(json!({
            "what": "How the question came to you: every hand-over, with why and what each holder found",
            "value": m,
        }));
    }
    let body = json!({ "to": to, "work": w.id, "item": item.id, "title": item.title, "asked": asked });
    let input = json!({ "inputs": recorded_inputs(&gathered) });
    let commit = e.config.commit.clone();
    // An ask that came of a flow anomaly reports a fault of wf-agents: the flow's ways on did not fit the situation.
    let anomaly = crate::faults::came_of_anomaly(e.db.conn(), w.id, g)?;
    let text = if anomaly {
        format!(
            "Work {} on item {} ({}) met a flow anomaly, a fault of wf-agents: the way on its engineer chose does not \
             fit the situation, and it asks you how to go on",
            w.id, item.id, item.title
        )
    } else if let Some(m) = &handed {
        format!(
            "Work {} on item {} ({}) waits on a question the {} handed to you: {}{about}",
            w.id,
            item.id,
            item.title,
            m["from"]["holder"].as_str().unwrap_or("architect"),
            m["why"].as_str().unwrap_or_default()
        )
    } else {
        format!("Work {} on item {} ({}) asks you{about}", w.id, item.id, item.title)
    };
    let id =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![round.work, round.n, body.to_string()],
            )?;
            let id = tx.last_insert_rowid();
            // The ask stands for the work's blocker before the owner while the owner holds it.
            if let Some(b) = crate::blockers::open(tx, w.id).map_err(db::Error::Abort)?
                && b.holder == "owner"
            {
                crate::blockers::set_ask(tx, b.id, id).map_err(db::Error::Abort)?;
            }
            let told = format!("{text} (ask {id})");
            if anomaly {
                actions::notify_fault_ask(tx, w.id, id, crate::faults::FLOW_ANOMALY, &told)
            } else {
                actions::notify_ask(tx, w.id, id, &told)
            }
            .map_err(db::Error::Abort)?;
            Ok(id)
        })
        .map_err(sql)?;
    Ok(Advance::Waits(format!("ask {id} waits for the owner")))
}

/// What an ask is about, for the notice that tells the owner of it: the first sentence of what its first point says;
/// nothing when the ask holds no point. An ask carries no kind; a kind one recorded before still holds is not said.
pub fn what_is_asked(asked: &[Value]) -> String {
    let point = asked
        .iter()
        .find_map(|a| a["value"]["points"].as_array().and_then(|ps| ps.first()).cloned());
    let Some(point) = point else { return String::new() };
    let what = point["what"].as_str().unwrap_or_default();
    format!(": {}", db::one_sentence(what))
}

/// The directories outside the worktree an agent may read: where the engine saves a tool's output too long to give
/// inline, and the logs of red checks it names, made when missing; and the installation's copy of the reasoning
/// system, whose check a cell that works on the records runs itself, when there is one. Each by its real path, so
/// that the CLI's rules match.
fn readable_dirs(e: &Engine) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = ["tool-output", "logs"]
        .iter()
        .filter_map(|name| {
            let dir = e.data.join(name);
            std::fs::create_dir_all(&dir).ok()?;
            std::fs::canonicalize(&dir).ok()
        })
        .collect();
    out.extend(std::fs::canonicalize(crate::reasoning::system(e)).ok());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The notice of a new ask says the first sentence of what it asks, and no kind, even of an ask recorded with one.
    #[test]
    fn an_ask_is_told_by_its_first_sentence() {
        let asked = vec![json!({ "what": "w", "value": { "to": "owner", "points": [
            { "what": "Please run a probe. Why: the lock.\n\npython3 - <<'EOF'\nEOF" }
        ] } })];
        assert_eq!(what_is_asked(&asked), ": Please run a probe.");
        let before = vec![json!({ "value": { "points": [{ "kind": "blocked_by_permission", "what": "Run it." }] } })];
        assert_eq!(what_is_asked(&before), ": Run it.");
        assert_eq!(what_is_asked(&[]), "");
    }
}
