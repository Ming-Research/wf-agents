//! A work's entries, table by table: each list and each map of a [`Work`] is a table, read back in the order the
//! list or the map has.

use std::collections::BTreeMap;

use decide::{
    Abandoned, Answered, BlockId, CallId, Called, CellId, Done, Finished, LetGo, Pause, PutIn, Questioned, Returned,
    Round, RunId, Seq, Started, Write, Written,
};
use rusqlite::{Connection, Row, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use work::{Filled, Measured, Measures, Moment, Observed, Pinged, Ran, RanOn, Session, Spent, Time, Work};

use crate::{Divergence, Entry, Fault, WorkNumber};

/// Everything kept of the work.
pub(crate) fn load(db: &Connection, work: WorkNumber) -> Result<Work, Fault> {
    let w = work.0;
    let mut out = Work::default();
    let r = &mut out.records;

    let mut basis: BTreeMap<RunId, BTreeMap<BlockId, Option<Seq>>> = BTreeMap::new();
    for (run, block, version) in rows(db, "SELECT run, block, version FROM basis WHERE work = ?1", w, |row| {
        Ok((
            RunId(row.get(0)?),
            BlockId(row.get(1)?),
            row.get::<_, Option<u64>>(2)?.map(Seq),
        ))
    })? {
        basis.entry(run).or_default().insert(block, version);
    }
    let started = rows(
        db,
        "SELECT at, run, cell, because FROM started WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok((
                Seq(row.get(0)?),
                RunId(row.get(1)?),
                CellId(row.get(2)?),
                row.get::<_, String>(3)?,
            ))
        },
    )?;
    for (at, run, cell, because) in started {
        r.started.push(Started {
            run,
            cell,
            at,
            because: parse("started", &because)?,
            basis: basis.remove(&run).unwrap_or_default(),
        });
    }

    r.rounds = rows(db, "SELECT at, run FROM rounds WHERE work = ?1 ORDER BY at", w, |row| {
        Ok(Round {
            at: Seq(row.get(0)?),
            run: RunId(row.get(1)?),
        })
    })?;
    r.returned = rows(
        db,
        "SELECT at, run FROM returned WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(Returned {
                at: Seq(row.get(0)?),
                run: RunId(row.get(1)?),
            })
        },
    )?;

    let called = rows(
        db,
        "SELECT at, call, run, who FROM called WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok((
                Seq(row.get(0)?),
                CallId(row.get(1)?),
                RunId(row.get(2)?),
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    for (at, call, run, who) in called {
        let on = who.map(|who| from_name("called", who)).transpose()?;
        r.called.push(Called { call, run, on, at });
    }

    r.done = rows(db, "SELECT at, call FROM done WHERE work = ?1 ORDER BY at", w, |row| {
        Ok(Done {
            at: Seq(row.get(0)?),
            call: CallId(row.get(1)?),
        })
    })?;

    let mut writes: BTreeMap<RunId, Vec<Write>> = BTreeMap::new();
    for (run, write) in rows(
        db,
        "SELECT run, block, matters, value FROM writes WHERE work = ?1 ORDER BY run, n",
        w,
        |row| {
            let write = Write {
                block: BlockId(row.get(1)?),
                meaning_unchanged: !row.get::<_, bool>(2)?,
                value: row.get(3)?,
            };
            Ok((RunId(row.get(0)?), write))
        },
    )? {
        writes.entry(run).or_default().push(write);
    }
    let finished = rows(
        db,
        "SELECT at, run, dealt_with FROM finished WHERE work = ?1 ORDER BY at",
        w,
        |row| Ok((Seq(row.get(0)?), RunId(row.get(1)?), row.get::<_, Option<String>>(2)?)),
    )?;
    for (at, run, dealt_with) in finished {
        let writes = writes.remove(&run).unwrap_or_default();
        let dealt_with = dealt_with.map(|names| parse("finished", &names)).transpose()?;
        r.finished.push(Finished {
            run,
            at,
            writes,
            dealt_with,
        });
    }

    r.abandoned = rows(
        db,
        "SELECT at, run FROM abandoned WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(Abandoned {
                at: Seq(row.get(0)?),
                run: RunId(row.get(1)?),
            })
        },
    )?;

    r.put_in = rows(
        db,
        "SELECT at, run, block, version FROM put_in WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(PutIn {
                at: Seq(row.get(0)?),
                run: RunId(row.get(1)?),
                block: BlockId(row.get(2)?),
                version: Seq(row.get(3)?),
            })
        },
    )?;

    let written = rows(
        db,
        "SELECT at, block, writer, matters, value FROM written WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok((
                Seq(row.get(0)?),
                BlockId(row.get(1)?),
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        },
    )?;
    for (at, block, writer, matters, value) in written {
        r.written.push(Written {
            block,
            by: from_name("written", writer)?,
            matters,
            value,
            at,
        });
    }

    r.questions = rows(
        db,
        "SELECT at, name, cell, run, engine, waits, withdrawn FROM questions WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(Questioned {
                at: Seq(row.get(0)?),
                name: row.get(1)?,
                cell: CellId(row.get(2)?),
                run: RunId(row.get(3)?),
                engine: row.get(4)?,
                waits: row.get(5)?,
                withdrawn: row.get(6)?,
            })
        },
    )?;
    r.answers = rows(
        db,
        "SELECT at, question, version, option, why FROM answers WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(Answered {
                at: Seq(row.get(0)?),
                question: row.get(1)?,
                version: Seq(row.get(2)?),
                option: row.get(3)?,
                why: row.get(4)?,
            })
        },
    )?;

    r.let_go = rows(
        db,
        "SELECT at, cell FROM let_go WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(LetGo {
                at: Seq(row.get(0)?),
                cell: CellId(row.get(1)?),
            })
        },
    )?;
    let pauses = rows(
        db,
        "SELECT at, paused, pauser, why FROM pauses WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok((
                Seq(row.get(0)?),
                row.get::<_, bool>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;
    for (at, paused, pauser, why) in pauses {
        r.pauses.push(Pause {
            paused,
            at,
            by: from_name("pauses", pauser)?,
            why,
        });
    }
    let cancels = rows(
        db,
        "SELECT at, why FROM cancelled WHERE work = ?1 ORDER BY at",
        w,
        |row| Ok((Seq(row.get(0)?), row.get::<_, Option<String>>(1)?)),
    )?;
    for (at, why) in cancels {
        r.cancels.push(at);
        if let Some(why) = why {
            out.why_cancelled.insert(at, why);
        }
    }
    let reopens = rows(
        db,
        "SELECT at, why FROM reopened WHERE work = ?1 ORDER BY at",
        w,
        |row| Ok((Seq(row.get(0)?), row.get::<_, String>(1)?)),
    )?;
    for (at, why) in reopens {
        r.reopens.push(at);
        out.why_reopened.insert(at, why);
    }
    r.releases = rows(db, "SELECT at FROM released WHERE work = ?1 ORDER BY at", w, |row| {
        Ok(Seq(row.get(0)?))
    })?;

    out.contents = rows(
        db,
        "SELECT block, version, content FROM contents WHERE work = ?1",
        w,
        |row| Ok(((BlockId(row.get(0)?), Seq(row.get(1)?)), row.get(2)?)),
    )?
    .into_iter()
    .collect();

    for (round, given) in rows(db, "SELECT run, round, given FROM given WHERE work = ?1", w, |row| {
        Ok(((RunId(row.get(0)?), row.get(1)?), row.get::<_, String>(2)?))
    })? {
        out.given.insert(round, parse("given", &given)?);
    }
    let filled = rows(
        db,
        "SELECT at, run, round, attempt, filled FROM filled WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok((
                Seq(row.get(0)?),
                RunId(row.get(1)?),
                row.get(2)?,
                row.get(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    )?;
    for (at, run, round, attempt, fills) in filled {
        out.filled.push(Filled {
            at,
            run,
            round,
            attempt,
            fills: parse("filled", &fills)?,
        });
    }
    for (round, back) in rows(db, "SELECT run, round, back FROM back WHERE work = ?1", w, |row| {
        Ok(((RunId(row.get(0)?), row.get(1)?), row.get::<_, String>(2)?))
    })? {
        out.back.insert(round, parse("back", &back)?);
    }

    for (call, what) in rows(db, "SELECT call, what FROM calls WHERE work = ?1", w, |row| {
        Ok((CallId(row.get(0)?), row.get::<_, String>(1)?))
    })? {
        out.calls.insert(call, parse("calls", &what)?);
    }
    out.results = rows(db, "SELECT call, result FROM results WHERE work = ?1", w, |row| {
        Ok((CallId(row.get(0)?), row.get(1)?))
    })?
    .into_iter()
    .collect();

    out.spent = rows(
        db,
        "SELECT run, round, usd_micros, input_tokens, cached_tokens, output_tokens, context_tokens, compactions \
         FROM spent WHERE work = ?1",
        w,
        |row| {
            let spent = Spent {
                usd_micros: row.get(2)?,
                input_tokens: row.get(3)?,
                cached_tokens: row.get(4)?,
                output_tokens: row.get(5)?,
                context_tokens: row.get(6)?,
                compactions: row.get(7)?,
            };
            Ok(((RunId(row.get(0)?), row.get(1)?), spent))
        },
    )?
    .into_iter()
    .collect();

    out.sessions = rows(
        db,
        "SELECT at, line, why FROM sessions WHERE work = ?1 ORDER BY at",
        w,
        |row| Ok((Seq(row.get(0)?), row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
    )?
    .into_iter()
    .map(|(at, line, why)| {
        Ok(Session {
            at,
            line,
            why: parse("sessions", &why)?,
        })
    })
    .collect::<Result<_, Fault>>()?;
    out.ran = rows(
        db,
        "SELECT at, run, round, attempt, level, model, effort, account FROM ran WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(Ran {
                at: Seq(row.get(0)?),
                run: RunId(row.get(1)?),
                round: row.get(2)?,
                attempt: row.get(3)?,
                on: RanOn {
                    level: row.get(4)?,
                    model: row.get(5)?,
                    effort: row.get(6)?,
                    account: row.get(7)?,
                },
            })
        },
    )?;
    for (measured, before) in rows(
        db,
        "SELECT at, run, round, attempt, input_tokens, cache_read_tokens, cache_written_tokens, thinking_tokens, turns, \
         calls, first_read, first_context, added, given, before, received, crossed, priced FROM measured WHERE work = ?1 \
         ORDER BY at",
        w,
        |row| {
            let measures = Measures {
                input_tokens: row.get(4)?,
                cache_read_tokens: row.get(5)?,
                cache_written_tokens: row.get(6)?,
                thinking_tokens: row.get(7)?,
                turns: row.get(8)?,
                calls: row.get(9)?,
                first_read: row.get(10)?,
                first_context: row.get(11)?,
                added: row.get(12)?,
                given: row.get(13)?,
                before: Default::default(),
                received: row.get(15)?,
                crossed: row.get(16)?,
                priced: row.get(17)?,
            };
            let measured = Measured {
                at: Seq(row.get(0)?),
                run: RunId(row.get(1)?),
                round: row.get(2)?,
                attempt: row.get(3)?,
                measures,
            };
            Ok((measured, row.get::<_, String>(14)?))
        },
    )? {
        let mut measured = measured;
        measured.measures.before = parse("measured", &before)?;
        out.measured.push(measured);
    }
    out.pinged = rows(
        db,
        "SELECT at, line, session, usd_micros, input_tokens, cached_tokens, output_tokens, context_tokens, \
         compactions, failed FROM pinged WHERE work = ?1 ORDER BY at",
        w,
        |row| {
            Ok(Pinged {
                at: Seq(row.get(0)?),
                line: row.get(1)?,
                session: row.get(2)?,
                spent: Spent {
                    usd_micros: row.get(3)?,
                    input_tokens: row.get(4)?,
                    cached_tokens: row.get(5)?,
                    output_tokens: row.get(6)?,
                    context_tokens: row.get(7)?,
                    compactions: row.get(8)?,
                },
                failed: row.get(9)?,
            })
        },
    )?;

    out.moments = rows(
        db,
        "SELECT place, time FROM moments WHERE work = ?1 ORDER BY place",
        w,
        |row| {
            Ok(Moment {
                from: Seq(row.get(0)?),
                at: Time(row.get(1)?),
            })
        },
    )?;
    // In the order they were kept, which their numbers keep.
    out.observed = rows(
        db,
        "SELECT run, round, n FROM observations WHERE work = ?1 ORDER BY number",
        w,
        |row| {
            Ok(Observed {
                run: RunId(row.get(0)?),
                round: row.get(1)?,
                n: row.get(2)?,
            })
        },
    )?;
    Ok(out)
}

/// Keeps what the given work has that the store does not hold yet; or, where what the store holds is not where the
/// given work begins, keeps nothing and says where. Everything is checked before anything is written.
pub(crate) fn save(db: &Connection, work: WorkNumber, given: &Work) -> Result<(), Fault> {
    let w = work.0;
    let held = load(db, work)?;
    let (h, g) = (&held.records, &given.records);

    let started = grown("started", &h.started, &g.started, |e| Entry::At(e.at))?;
    let rounds = grown("rounds", &h.rounds, &g.rounds, |e| Entry::At(e.at))?;
    let returned = grown("returned", &h.returned, &g.returned, |e| Entry::At(e.at))?;
    let called = grown("called", &h.called, &g.called, |e| Entry::At(e.at))?;
    let done = grown("done", &h.done, &g.done, |e| Entry::At(e.at))?;
    let finished = grown("finished", &h.finished, &g.finished, |e| Entry::At(e.at))?;
    let abandoned = grown("abandoned", &h.abandoned, &g.abandoned, |e| Entry::At(e.at))?;
    let put_in = grown("put_in", &h.put_in, &g.put_in, |e| Entry::At(e.at))?;
    let written = grown("written", &h.written, &g.written, |e| Entry::At(e.at))?;
    let questions = grown("questions", &h.questions, &g.questions, |e| Entry::At(e.at))?;
    let answers = grown("answers", &h.answers, &g.answers, |e| Entry::At(e.at))?;
    let let_go = grown("let_go", &h.let_go, &g.let_go, |e| Entry::At(e.at))?;
    let pauses = grown("pauses", &h.pauses, &g.pauses, |e| Entry::At(e.at))?;
    // A cancel and a reopen are kept with why, which is theirs as much as their place.
    let with_why = |places: &[Seq], why: &BTreeMap<Seq, String>| -> Vec<(Seq, Option<String>)> {
        places.iter().map(|at| (*at, why.get(at).cloned())).collect()
    };
    let (held_cancels, given_cancels) = (
        with_why(&h.cancels, &held.why_cancelled),
        with_why(&g.cancels, &given.why_cancelled),
    );
    let cancels = grown("cancelled", &held_cancels, &given_cancels, |(at, _)| Entry::At(*at))?;
    let (held_reopens, given_reopens) = (
        with_why(&h.reopens, &held.why_reopened),
        with_why(&g.reopens, &given.why_reopened),
    );
    let reopens = grown("reopened", &held_reopens, &given_reopens, |(at, _)| Entry::At(*at))?;
    let releases = grown("released", &h.releases, &g.releases, |at| Entry::At(*at))?;
    let contents = gained("contents", &held.contents, &given.contents, |(block, at)| {
        Entry::Version {
            block: block.clone(),
            at: *at,
        }
    })?;
    let given_rounds = gained("given", &held.given, &given.given, round_entry)?;
    let filled = grown("filled", &held.filled, &given.filled, |f| Entry::At(f.at))?;
    let back = gained("back", &held.back, &given.back, round_entry)?;
    let calls = gained("calls", &held.calls, &given.calls, |c| Entry::Call(*c))?;
    let results = gained("results", &held.results, &given.results, |c| Entry::Call(*c))?;
    let spent = gained("spent", &held.spent, &given.spent, round_entry)?;
    let moments = grown("moments", &held.moments, &given.moments, |m| Entry::At(m.from))?;
    let sessions = grown("sessions", &held.sessions, &given.sessions, |s| Entry::At(s.at))?;
    let ran = grown("ran", &held.ran, &given.ran, |r| Entry::At(r.at))?;
    let observed = grown("observations", &held.observed, &given.observed, |o| Entry::Round {
        run: o.run,
        round: o.round,
    })?;
    let measured = grown("measured", &held.measured, &given.measured, |m| Entry::At(m.at))?;
    let pinged = grown("pinged", &held.pinged, &given.pinged, |p| Entry::At(p.at))?;

    // Each table after those its rows name.
    for s in started {
        let because = json("started", &s.because)?;
        db.execute(
            "INSERT INTO started (work, at, run, cell, because) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![w, s.at.0, s.run.0, s.cell.0, because],
        )?;
        for (block, version) in &s.basis {
            db.execute(
                "INSERT INTO basis (work, run, block, version) VALUES (?1, ?2, ?3, ?4)",
                params![w, s.run.0, block.0, version.map(|v| v.0)],
            )?;
        }
    }
    for e in rounds {
        db.execute(
            "INSERT INTO rounds (work, at, run) VALUES (?1, ?2, ?3)",
            params![w, e.at.0, e.run.0],
        )?;
    }
    for e in returned {
        db.execute(
            "INSERT INTO returned (work, at, run) VALUES (?1, ?2, ?3)",
            params![w, e.at.0, e.run.0],
        )?;
    }
    for c in called {
        let who = c.on.as_ref().map(|who| name("called", who)).transpose()?;
        db.execute(
            "INSERT INTO called (work, at, call, run, who) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![w, c.at.0, c.call.0, c.run.0, who],
        )?;
    }
    for d in done {
        db.execute(
            "INSERT INTO done (work, at, call) VALUES (?1, ?2, ?3)",
            params![w, d.at.0, d.call.0],
        )?;
    }
    for f in finished {
        let dealt_with = f.dealt_with.as_ref().map(|names| json("finished", names)).transpose()?;
        db.execute(
            "INSERT INTO finished (work, at, run, dealt_with) VALUES (?1, ?2, ?3, ?4)",
            params![w, f.at.0, f.run.0, dealt_with],
        )?;
        for (n, write) in (1..).zip(&f.writes) {
            db.execute(
                "INSERT INTO writes (work, run, n, block, matters, value) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![w, f.run.0, n, write.block.0, !write.meaning_unchanged, write.value],
            )?;
        }
    }
    for a in abandoned {
        db.execute(
            "INSERT INTO abandoned (work, at, run) VALUES (?1, ?2, ?3)",
            params![w, a.at.0, a.run.0],
        )?;
    }
    for d in put_in {
        db.execute(
            "INSERT INTO put_in (work, at, run, block, version) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![w, d.at.0, d.run.0, d.block.0, d.version.0],
        )?;
    }
    for e in written {
        let writer = name("written", &e.by)?;
        db.execute(
            "INSERT INTO written (work, at, block, writer, matters, value) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![w, e.at.0, e.block.0, writer, e.matters, e.value],
        )?;
    }
    for q in questions {
        db.execute(
            "INSERT INTO questions (work, at, name, cell, run, engine, waits, withdrawn) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![w, q.at.0, q.name, q.cell.0, q.run.0, q.engine, q.waits, q.withdrawn],
        )?;
    }
    for a in answers {
        db.execute(
            "INSERT INTO answers (work, at, question, version, option, why) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![w, a.at.0, a.question, a.version.0, a.option, a.why],
        )?;
    }
    for e in let_go {
        db.execute(
            "INSERT INTO let_go (work, at, cell) VALUES (?1, ?2, ?3)",
            params![w, e.at.0, e.cell.0],
        )?;
    }
    for e in pauses {
        let pauser = name("pauses", &e.by)?;
        db.execute(
            "INSERT INTO pauses (work, at, paused, pauser, why) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![w, e.at.0, e.paused, pauser, e.why],
        )?;
    }
    for (at, why) in cancels {
        db.execute(
            "INSERT INTO cancelled (work, at, why) VALUES (?1, ?2, ?3)",
            params![w, at.0, why],
        )?;
    }
    for (at, why) in reopens {
        db.execute(
            "INSERT INTO reopened (work, at, why) VALUES (?1, ?2, ?3)",
            params![w, at.0, why],
        )?;
    }
    for at in releases {
        db.execute("INSERT INTO released (work, at) VALUES (?1, ?2)", params![w, at.0])?;
    }
    for ((block, version), content) in contents {
        db.execute(
            "INSERT INTO contents (work, block, version, content) VALUES (?1, ?2, ?3, ?4)",
            params![w, block.0, version.0, content],
        )?;
    }
    for ((run, round), what) in given_rounds {
        db.execute(
            "INSERT INTO given (work, run, round, given) VALUES (?1, ?2, ?3, ?4)",
            params![w, run.0, round, json("given", what)?],
        )?;
    }
    for f in filled {
        db.execute(
            "INSERT INTO filled (work, at, run, round, attempt, filled) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![w, f.at.0, f.run.0, f.round, f.attempt, json("filled", &f.fills)?],
        )?;
    }
    for ((run, round), what) in back {
        db.execute(
            "INSERT INTO back (work, run, round, back) VALUES (?1, ?2, ?3, ?4)",
            params![w, run.0, round, json("back", what)?],
        )?;
    }
    // Each observation after what came back, which it names, and numbered by the store as it is kept.
    for o in observed {
        db.execute(
            "INSERT INTO observations (work, run, round, n) VALUES (?1, ?2, ?3, ?4)",
            params![w, o.run.0, o.round, o.n],
        )?;
    }
    for (call, what) in calls {
        db.execute(
            "INSERT INTO calls (work, call, what) VALUES (?1, ?2, ?3)",
            params![w, call.0, json("calls", what)?],
        )?;
    }
    for (call, result) in results {
        db.execute(
            "INSERT INTO results (work, call, result) VALUES (?1, ?2, ?3)",
            params![w, call.0, result],
        )?;
    }
    for ((run, round), s) in spent {
        db.execute(
            "INSERT INTO spent (work, run, round, usd_micros, input_tokens, cached_tokens, output_tokens, \
             context_tokens, compactions) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                w,
                run.0,
                round,
                s.usd_micros,
                s.input_tokens,
                s.cached_tokens,
                s.output_tokens,
                s.context_tokens,
                s.compactions
            ],
        )?;
    }
    for s in sessions {
        db.execute(
            "INSERT INTO sessions (work, at, line, why) VALUES (?1, ?2, ?3, ?4)",
            params![w, s.at.0, s.line, json("sessions", &s.why)?],
        )?;
    }
    for r in ran {
        db.execute(
            "INSERT INTO ran (work, at, run, round, attempt, level, model, effort, account) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                w,
                r.at.0,
                r.run.0,
                r.round,
                r.attempt,
                r.on.level,
                r.on.model,
                r.on.effort,
                r.on.account
            ],
        )?;
    }
    for m in measured {
        let n = &m.measures;
        db.execute(
            "INSERT INTO measured (work, at, run, round, attempt, input_tokens, cache_read_tokens, \
             cache_written_tokens, thinking_tokens, turns, calls, first_read, first_context, added, given, before, \
             received, crossed, priced) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                w,
                m.at.0,
                m.run.0,
                m.round,
                m.attempt,
                n.input_tokens,
                n.cache_read_tokens,
                n.cache_written_tokens,
                n.thinking_tokens,
                n.turns,
                n.calls,
                n.first_read,
                n.first_context,
                n.added,
                n.given,
                json("measured", &n.before)?,
                n.received,
                n.crossed,
                n.priced
            ],
        )?;
    }
    for p in pinged {
        let s = &p.spent;
        db.execute(
            "INSERT INTO pinged (work, at, line, session, usd_micros, input_tokens, cached_tokens, output_tokens, \
             context_tokens, compactions, failed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                w,
                p.at.0,
                p.line,
                p.session,
                s.usd_micros,
                s.input_tokens,
                s.cached_tokens,
                s.output_tokens,
                s.context_tokens,
                s.compactions,
                p.failed
            ],
        )?;
    }
    for m in moments {
        db.execute(
            "INSERT INTO moments (work, place, time) VALUES (?1, ?2, ?3)",
            params![w, m.from.0, m.at.0],
        )?;
    }
    Ok(())
}

/// The rows the query finds for the work, each as the closure reads it.
fn rows<T>(
    db: &Connection,
    sql: &str,
    work: u64,
    read: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, Fault> {
    let mut query = db.prepare(sql)?;
    let rows = query.query_map([work], read)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The entries of the given list after those held, where the held ones are where the list begins.
fn grown<'a, T: PartialEq>(
    table: &'static str,
    held: &[T],
    given: &'a [T],
    entry: impl Fn(&T) -> Entry,
) -> Result<&'a [T], Fault> {
    for (i, h) in held.iter().enumerate() {
        match given.get(i) {
            Some(g) if g == h => {}
            Some(_) => return Err(diverged(table, entry(h), Divergence::Changed)),
            None => return Err(diverged(table, entry(h), Divergence::Missing)),
        }
    }
    Ok(&given[held.len()..])
}

/// The entries of the given map whose keys are not held, where every held entry is in it as it is held.
fn gained<'a, K: Ord, V: PartialEq>(
    table: &'static str,
    held: &BTreeMap<K, V>,
    given: &'a BTreeMap<K, V>,
    entry: impl Fn(&K) -> Entry,
) -> Result<Vec<(&'a K, &'a V)>, Fault> {
    for (k, h) in held {
        match given.get(k) {
            Some(g) if g == h => {}
            Some(_) => return Err(diverged(table, entry(k), Divergence::Changed)),
            None => return Err(diverged(table, entry(k), Divergence::Missing)),
        }
    }
    Ok(given.iter().filter(|(k, _)| !held.contains_key(k)).collect())
}

fn round_entry(&(run, round): &(RunId, usize)) -> Entry {
    Entry::Round { run, round }
}

fn diverged(table: &'static str, entry: Entry, how: Divergence) -> Fault {
    Fault::Diverged { table, entry, how }
}

/// The value as JSON.
fn json<T: Serialize>(table: &'static str, value: &T) -> Result<String, Fault> {
    serde_json::to_string(value).map_err(|e| Fault::Value {
        table,
        message: e.to_string(),
    })
}

/// The value the JSON is.
fn parse<T: DeserializeOwned>(table: &'static str, text: &str) -> Result<T, Fault> {
    serde_json::from_str(text).map_err(|e| Fault::Value {
        table,
        message: e.to_string(),
    })
}

/// The name of a variant that carries no data, such as a person or a writer.
pub(crate) fn name<T: Serialize>(table: &'static str, value: &T) -> Result<String, Fault> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => Ok(name),
        Ok(other) => Err(Fault::Value {
            table,
            message: format!("not a name: {other}"),
        }),
        Err(e) => Err(Fault::Value {
            table,
            message: e.to_string(),
        }),
    }
}

/// The variant of this name.
pub(crate) fn from_name<T: DeserializeOwned>(table: &'static str, name: String) -> Result<T, Fault> {
    serde_json::from_value(serde_json::Value::String(name)).map_err(|e| Fault::Value {
        table,
        message: e.to_string(),
    })
}
