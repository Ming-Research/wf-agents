//! What the System view shows: how the whole system stands now (each project's architect, and what waits for the
//! owner), and what it did over a period of whole weeks: what it delivered, what it spent and on what, where it spent
//! for nothing, and what went wrong. Everything is read from what the database keeps. A week starts on Monday at
//! midnight in the owner's time zone. The works by where they stand are the works list's, which the page reads as it
//! is.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use db::rusqlite::{self, Connection};

use crate::read::{Error, Flows, json, outcome, over, work_summaries};
use crate::{
    Anomaly, AnomalyKind, ArchitectNow, CellSpend, HaltTime, ItemAt, ItemSpend, LineSpend, Merged, OpenPr,
    RoundOutcome, System, SystemNow, SystemOutput, SystemSpend, SystemWaste, Waiting, Week, WorkCount,
};

type Result<T> = std::result::Result<T, Error>;

const DAY: i64 = 86_400_000;
const WEEK: i64 = 7 * DAY;

const ENGINEER: &str = "engineer";
const ARCHITECT: &str = "architect";

/// How many of the costliest cells the view shows.
const CELLS: usize = 10;

/// The most weeks a period holds: a hundred years, so that no period asked for overflows a time.
const MOST_WEEKS: i64 = 5200;

/// The counter of the rows a flow anomaly takes, and that of the rows a tool that failed takes, as the flow names them.
const FLOW_ANOMALIES: &str = "flow_anomalies";
const TOOL_FAILURES: &str = "tool_failures";

/// What the System view is read for.
pub struct SystemAsk<'a> {
    /// How many weeks the period holds, the week under way the last; none for all time.
    pub weeks: Option<i64>,
    /// The owner's time zone, in minutes east of UTC, which the weeks start in.
    pub tz: i64,
    /// The projects of the configuration, whose architects the view shows.
    pub projects: &'a [String],
    /// The limits of the flow's counters, by project and counter, as the configuration sets them.
    pub limits: &'a BTreeMap<String, BTreeMap<String, u64>>,
    /// When the period ends.
    pub now: i64,
}

/// How the whole system stands now, and what it did over the period asked for.
pub fn system(conn: &Connection, flows: &Flows, ask: &SystemAsk<'_>) -> Result<System> {
    let this_week = week_of(ask.now, ask.tz);
    let from = ask.weeks.map(|n| this_week - (n.clamp(1, MOST_WEEKS) - 1) * WEEK);
    let since = from.unwrap_or(i64::MIN);
    let works = works_of(conn)?;
    let rounds = rounds_since(conn, since)?;
    let pinged = pings_since(conn, since)?;
    let merges = merges(conn)?;
    let (made, cancelled) = item_events(conn)?;
    let changes = work_changes(conn)?;
    let merged_at: HashMap<i64, i64> = merges.iter().map(|m| (m.work, m.at)).collect();
    let spans = pr_spans(&works, &changes, &merged_at);

    // The weeks run from the period's start, or for all time from the week anything first happened in, to the week
    // under way.
    let first = from.unwrap_or_else(|| {
        rounds
            .iter()
            .map(|r| r.started_at)
            .chain(pinged.iter().map(|p| p.at))
            .chain(made.iter().map(|m| m.at))
            .min()
            .map_or(this_week, |t| week_of(t, ask.tz).min(this_week))
    });
    let slot = |t: i64| -> Option<usize> {
        let w = week_of(t, ask.tz);
        if (first..=this_week).contains(&w) {
            usize::try_from((w - first) / WEEK).ok()
        } else {
            None
        }
    };
    let mut weeks: Vec<Week> = (0..=(this_week - first) / WEEK)
        .map(|k| {
            let start = first + k * WEEK;
            Week {
                start,
                merged: 0,
                made: 0,
                cancelled: 0,
                open_prs: open_at(&spans, (start + WEEK).min(ask.now)),
                engineers_usd: 0.0,
                architects_usd: 0.0,
            }
        })
        .collect();
    for k in merges.iter().filter_map(|m| slot(m.at)) {
        weeks[k].merged += 1;
    }
    for k in made.iter().filter_map(|m| slot(m.at)) {
        weeks[k].made += 1;
    }
    for k in cancelled.iter().filter_map(|c| slot(c.at)) {
        weeks[k].cancelled += 1;
    }

    // Where the money went, and where it went for nothing, round by round and ping by ping.
    let mut lines: BTreeMap<(String, String), LineSpend> = BTreeMap::new();
    let mut items: BTreeMap<i64, ItemSum> = BTreeMap::new();
    let mut cells: HashMap<(String, String), CellSum> = HashMap::new();
    let (mut sent_back, mut failed, mut pings) = (Tally::new(), Tally::new(), Tally::new());
    let mut halted: BTreeMap<String, Halted> = BTreeMap::new();
    let (mut read, mut written, mut input) = (0, 0, 0);
    for r in &rounds {
        let w = works.get(&r.work);
        if let Some(k) = slot(r.started_at) {
            spent_in(&mut weeks[k], w, r.cost);
        }
        if let Some(line) = &r.line {
            let l = line_spend(&mut lines, line, &r.model);
            l.rounds += 1;
            l.cost_usd += r.cost;
        }
        if let Some(w) = w
            && let Some(item) = w.item
        {
            let s = items.entry(item).or_insert_with(|| ItemSum::of(w));
            s.works.insert(r.work);
            s.rounds += 1;
            s.cost += r.cost;
        }
        if let Some(cell) = &r.cell
            && r.cost > 0.0
        {
            let graph = w.map_or(ENGINEER, |w| w.graph.as_str());
            let c = cells.entry((graph.to_string(), code(graph, cell))).or_default();
            c.rounds += 1;
            c.cost += r.cost;
            c.version = w.map_or(0, |w| w.version);
            c.id = cell.clone();
        }
        read += r.read;
        written += r.written;
        input += r.input;
        match r.outcome {
            RoundOutcome::SentBack => tally(&mut sent_back, r.work, r.cost),
            RoundOutcome::Failed | RoundOutcome::Interrupted => tally(&mut failed, r.work, r.cost),
            _ => {}
        }
        if let Some((kind, until)) = &r.halt {
            let h = halted.entry(kind.clone()).or_default();
            h.spans.push((r.ended_at.unwrap_or(r.started_at), *until));
            h.runs += 1;
        }
    }
    for p in &pinged {
        let w = works.get(&p.work);
        if let Some(k) = slot(p.at) {
            spent_in(&mut weeks[k], w, p.cost);
        }
        let l = line_spend(&mut lines, &p.line, &p.model);
        l.pings += 1;
        l.cost_usd += p.cost;
        if let Some(w) = w
            && let Some(item) = w.item
        {
            let s = items.entry(item).or_insert_with(|| ItemSum::of(w));
            s.works.insert(p.work);
            s.cost += p.cost;
        }
        tally(&mut pings, p.work, p.cost);
    }

    let merged: Vec<Merged> = merges
        .iter()
        .rev()
        .filter(|m| m.at >= since)
        .filter_map(|m| {
            let w = works.get(&m.work)?;
            Some(Merged {
                item: w.item?,
                title: w.title.clone().unwrap_or_default(),
                work: m.work,
                started_at: m.started_at,
                merged_at: m.at,
                cost_usd: m.cost,
            })
        })
        .collect();
    let newest_first =
        |list: Vec<ItemAt>| -> Vec<ItemAt> { list.into_iter().rev().filter(|x| x.at >= since).collect() };
    let output = SystemOutput {
        merged,
        made: newest_first(made),
        cancelled: newest_first(cancelled),
        open_prs: open_now(&works, &spans),
    };

    let mut by_item: Vec<ItemSpend> = items
        .into_iter()
        .map(|(item, s)| ItemSpend {
            item,
            title: s.title,
            works: s.works.len() as i64,
            rounds: s.rounds,
            cost_usd: s.cost,
        })
        .collect();
    by_item.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd));
    let mut costliest: Vec<_> = cells.into_iter().collect();
    costliest.sort_by(|a, b| b.1.cost.total_cmp(&a.1.cost));
    costliest.truncate(CELLS);
    let latest: Option<i64> = conn.query_row("SELECT max(id) FROM flow_versions", [], |r| r.get(0))?;
    let cells: Vec<CellSpend> = costliest
        .into_iter()
        .map(|((graph, cell), s)| CellSpend {
            title: latest
                .and_then(|v| title_in(conn, flows, v, &graph, &cell))
                .or_else(|| title_in(conn, flows, s.version, &graph, &s.id)),
            graph,
            cell,
            rounds: s.rounds,
            cost_usd: s.cost,
        })
        .collect();

    let (misses, missed, miss_usd): (i64, i64, f64) = conn.query_row(
        "SELECT count(*), coalesce(sum(m.written), 0), coalesce(sum(m.cost_usd), 0) FROM cache_misses m
         JOIN rounds r ON r.work = m.work AND r.n = m.round WHERE r.started_at >= ?1",
        [since],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let every_halt: Vec<(i64, i64)> = halted.values().flat_map(|h| h.spans.iter().copied()).collect();
    let waste = SystemWaste {
        read,
        written,
        input,
        misses,
        missed,
        miss_usd,
        sent_back: counts(&works, sent_back),
        failed: counts(&works, failed),
        halted_ms: covered(&every_halt, since, ask.now),
        halts: halted
            .into_iter()
            .map(|(kind, h)| HaltTime {
                ms: covered(&h.spans, since, ask.now),
                runs: h.runs,
                kind,
            })
            .collect(),
        pings: counts(&works, pings),
    };

    let mut anomalies = counted(conn, flows, &works, ask, since)?;
    anomalies.extend(
        changes
            .stops
            .iter()
            .filter(|s| s.at >= since)
            .map(|s| anomaly(&works, s.work, None, s.at, AnomalyKind::Stopped { why: s.why.clone() })),
    );
    anomalies.extend(main_red(conn, &works, since)?);
    anomalies.sort_by_key(|a| std::cmp::Reverse(a.at));

    Ok(System {
        from,
        at: ask.now,
        now: SystemNow {
            architects: architects(conn, flows, ask.projects)?,
            // An ask waits from when the owner was told of it.
            asks: waiting(
                conn,
                "SELECT count(*), min(coalesce((SELECT min(n.at) FROM engine_notices n WHERE n.ask = a.id),
                   r.started_at))
                 FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round WHERE a.answer IS NULL",
            )?,
            // A design card waits from the end of the round that handed it in.
            design_cards: waiting(
                conn,
                "SELECT count(*), min(coalesce(r.ended_at, r.started_at))
                 FROM design_cards c JOIN rounds r ON r.work = c.work AND r.n = c.round WHERE c.status = 'open'",
            )?,
            faults: waiting(
                conn,
                "SELECT count(*), min(at) FROM engine_notices WHERE fault IS NOT NULL AND read_at IS NULL",
            )?,
        },
        weeks,
        output,
        spend: SystemSpend {
            by_line_model: lines.into_values().collect(),
            by_item,
            cells,
        },
        waste,
        anomalies,
    })
}

/// The start of the week `at` falls in: Monday at midnight, `tz` minutes east of UTC.
fn week_of(at: i64, tz: i64) -> i64 {
    let shift = tz * 60_000;
    let day = (at + shift).div_euclid(DAY);
    // 1970-01-01 was a Thursday, three days after a Monday.
    (day - (day + 3).rem_euclid(7)) * DAY - shift
}

/// How long the spans cover within `from` to `to`, each moment once.
fn covered(spans: &[(i64, i64)], from: i64, to: i64) -> i64 {
    let mut within: Vec<(i64, i64)> = spans
        .iter()
        .map(|&(a, b)| (a.max(from), b.min(to)))
        .filter(|(a, b)| b > a)
        .collect();
    within.sort_unstable();
    let (mut total, mut reach) = (0, i64::MIN);
    for (a, b) in within {
        let a = a.max(reach);
        if b > a {
            total += b - a;
        }
        reach = reach.max(b);
    }
    total
}

/// The cell a round ran at, by its code now: every id a flow version gave the cell reads as its code.
fn code(graph: &str, id: &str) -> String {
    db::code_of(graph, id).unwrap_or(id).to_string()
}

/// A cell's title, as a flow version has it.
fn title_in(conn: &Connection, flows: &Flows, version: i64, graph: &str, cell: &str) -> Option<String> {
    let flow = flows.get(conn, version).ok()?;
    Some(flow.graph(graph)?.cell(cell)?.title.clone())
}

/// A work, as the view names what came of it.
struct WorkOf {
    graph: String,
    version: i64,
    item: Option<i64>,
    title: Option<String>,
    project: Option<String>,
    /// The cell it is at, its pull request, and whether it was stopped before its end.
    at: String,
    pr: Option<i64>,
    ended: bool,
    /// When its first round started, and when its last round ended or started.
    first_at: Option<i64>,
    last_at: Option<i64>,
}

/// Every work, by its number.
fn works_of(conn: &Connection) -> Result<HashMap<i64, WorkOf>> {
    let mut st = conn.prepare(
        "SELECT w.id, w.graph, w.flow_version, w.item, i.title, coalesce(i.project, w.project), w.at, w.pr,
                w.ended IS NOT NULL,
                (SELECT min(x.started_at) FROM rounds x WHERE x.work = w.id),
                (SELECT max(coalesce(x.ended_at, x.started_at)) FROM rounds x WHERE x.work = w.id)
         FROM works w LEFT JOIN items i ON i.id = w.item",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get(0)?,
            WorkOf {
                graph: r.get(1)?,
                version: r.get(2)?,
                item: r.get(3)?,
                title: r.get(4)?,
                project: r.get(5)?,
                at: r.get(6)?,
                pr: r.get(7)?,
                ended: r.get(8)?,
                first_at: r.get(9)?,
                last_at: r.get(10)?,
            },
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A round of the period, with what the view counts of it.
struct Spent {
    work: i64,
    cell: Option<String>,
    outcome: RoundOutcome,
    /// For a failed run whose cause stops every work, that cause, and until when it said to wait.
    halt: Option<(String, i64)>,
    started_at: i64,
    ended_at: Option<i64>,
    model: String,
    /// The line of the session it ran in; none for the engine's own steps, tool cells and the owner.
    line: Option<String>,
    cost: f64,
    /// Its input tokens: given without the cache, read from it, and written to it.
    input: i64,
    read: i64,
    written: i64,
}

/// The rounds that started in the period, with what they cost.
fn rounds_since(conn: &Connection, since: i64) -> Result<Vec<Spent>> {
    let mut st = conn.prepare(
        "SELECT r.work, r.cell, r.transition, r.rejected, r.started_at, r.ended_at, coalesce(r.model, ''), s.line,
                coalesce(json_extract(r.usage, '$.cost_usd'), 0),
                CAST(coalesce(json_extract(r.usage, '$.usage.input_tokens'), 0) AS INTEGER),
                CAST(coalesce(json_extract(r.usage, '$.usage.cache_read_input_tokens'), 0) AS INTEGER),
                CAST(coalesce(json_extract(r.usage, '$.usage.cache_creation_input_tokens'), 0) AS INTEGER)
         FROM rounds r LEFT JOIN sessions s ON s.id = r.session
         WHERE r.started_at >= ?1 ORDER BY r.work, r.n",
    )?;
    let rows = st.query_map([since], |r| {
        let transition: Option<String> = r.get(2)?;
        let rejected = json(r.get(3)?);
        let ended_at: Option<i64> = r.get(5)?;
        let halt = rejected.as_ref().and_then(|x| {
            let f = &x["failed"];
            Some((f["kind"].as_str()?.to_string(), f["until"].as_i64()?))
        });
        Ok(Spent {
            work: r.get(0)?,
            cell: r.get(1)?,
            outcome: outcome(transition.as_deref(), rejected.as_ref(), ended_at.is_some()),
            halt,
            started_at: r.get(4)?,
            ended_at,
            model: r.get(6)?,
            line: r.get(7)?,
            cost: r.get(8)?,
            input: r.get(9)?,
            read: r.get(10)?,
            written: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A keep-warm ping of the period: its work, the line and model of its session, when and what it cost.
struct Pinged {
    work: i64,
    line: String,
    model: String,
    at: i64,
    cost: f64,
}

fn pings_since(conn: &Connection, since: i64) -> Result<Vec<Pinged>> {
    let mut st = conn.prepare(
        "SELECT s.work, s.line, coalesce(p.model, ''), p.started_at, coalesce(json_extract(p.usage, '$.cost_usd'), 0)
         FROM pings p JOIN sessions s ON s.id = p.session WHERE p.started_at >= ?1",
    )?;
    let rows = st.query_map([since], |r| {
        Ok(Pinged {
            work: r.get(0)?,
            line: r.get(1)?,
            model: r.get(2)?,
            at: r.get(3)?,
            cost: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A merge: the work that merged and when, when the work's first round started, and what the work cost, its keep-warm
/// pings included.
struct Merge {
    work: i64,
    at: i64,
    started_at: i64,
    cost: f64,
}

/// Every merge, the earliest first.
fn merges(conn: &Connection) -> Result<Vec<Merge>> {
    let mut st = conn.prepare(&format!(
        "SELECT r.work, r.ended_at, (SELECT min(x.started_at) FROM rounds x WHERE x.work = r.work),
                (SELECT coalesce(sum(json_extract(x.usage, '$.cost_usd')), 0) FROM rounds x WHERE x.work = r.work)
                + (SELECT coalesce(sum(json_extract(p.usage, '$.cost_usd')), 0)
                   FROM pings p JOIN sessions s ON s.id = p.session WHERE s.work = r.work)
         FROM rounds r WHERE r.cell IN {} AND r.transition = 'merged' AND r.ended_at IS NOT NULL
         ORDER BY r.ended_at",
        db::cells_sql(ENGINEER, &["M2"])
    ))?;
    let rows = st.query_map([], |r| {
        Ok(Merge {
            work: r.get(0)?,
            at: r.get(1)?,
            started_at: r.get(2)?,
            cost: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Each item made, and each item cancelled with why, as the changes to the items recorded when: the earliest first.
fn item_events(conn: &Connection) -> Result<(Vec<ItemAt>, Vec<ItemAt>)> {
    let read = |sql: &str| -> Result<Vec<ItemAt>> {
        let mut st = conn.prepare(sql)?;
        let rows = st.query_map([], |r| {
            Ok(ItemAt {
                item: r.get(0)?,
                title: r.get(1)?,
                at: r.get(2)?,
                why: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    };
    let made = read(
        "SELECT i.id, i.title, min(c.at), NULL FROM items i
         JOIN changes c ON c.table_name = 'items' AND c.row_key = json_object('id', i.id)
         WHERE c.old_row IS NULL GROUP BY i.id ORDER BY 3",
    )?;
    let cancelled = read(
        "SELECT i.id, i.title, min(c.at), i.cancelled FROM items i
         JOIN changes c ON c.table_name = 'items' AND c.row_key = json_object('id', i.id)
         WHERE i.cancelled IS NOT NULL AND json_extract(c.new_row, '$.cancelled') IS NOT NULL
           AND json_extract(c.old_row, '$.cancelled') IS NULL
         GROUP BY i.id ORDER BY 3",
    )?;
    Ok((made, cancelled))
}

/// What the changes to the works tell: when each work first recorded its pull request, when each was stopped before
/// its end, and each time the engine stopped one, saying why. The architect's stops are the architect's, not the
/// engine's.
struct WorkChanges {
    pr_at: HashMap<i64, i64>,
    ended_at: HashMap<i64, i64>,
    stops: Vec<Stop>,
}

/// A time the engine stopped a work, and why.
struct Stop {
    work: i64,
    at: i64,
    why: String,
}

fn work_changes(conn: &Connection) -> Result<WorkChanges> {
    let mut st = conn.prepare(
        "SELECT json_extract(c.row_key, '$.id'), c.at,
                json_extract(c.new_row, '$.pr') IS NOT NULL AND json_extract(c.old_row, '$.pr') IS NULL,
                json_extract(c.new_row, '$.ended') IS NOT NULL AND json_extract(c.old_row, '$.ended') IS NULL,
                json_extract(c.new_row, '$.paused_by') IS 'engine'
                  AND json_extract(c.old_row, '$.paused_by') IS NOT 'engine',
                json_extract(c.new_row, '$.paused')
         FROM changes c
         WHERE c.table_name = 'works' AND c.new_row IS NOT NULL
           AND (json_extract(c.new_row, '$.pr') IS NOT json_extract(c.old_row, '$.pr')
             OR json_extract(c.new_row, '$.ended') IS NOT json_extract(c.old_row, '$.ended')
             OR json_extract(c.new_row, '$.paused_by') IS NOT json_extract(c.old_row, '$.paused_by'))
         ORDER BY c.id",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, bool>(2)?,
            r.get::<_, bool>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut out = WorkChanges {
        pr_at: HashMap::new(),
        ended_at: HashMap::new(),
        stops: Vec::new(),
    };
    for row in rows {
        let (work, at, pr, ended, stopped, why) = row?;
        if pr {
            out.pr_at.entry(work).or_insert(at);
        }
        if ended {
            out.ended_at.entry(work).or_insert(at);
        }
        if stopped
            && let Some(why) = why
            && !why.starts_with(db::STOPPED_BY_ARCHITECT)
        {
            out.stops.push(Stop { work, at, why });
        }
    }
    Ok(out)
}

/// A pull request a work had, and when it was open: from when the work recorded it to when the work merged, was
/// stopped or came to the end of its graph; none while it is open still.
struct PrSpan {
    project: String,
    pr: i64,
    work: i64,
    from: i64,
    to: Option<i64>,
}

fn pr_spans(works: &HashMap<i64, WorkOf>, changes: &WorkChanges, merged: &HashMap<i64, i64>) -> Vec<PrSpan> {
    works
        .iter()
        .filter_map(|(&work, w)| {
            let pr = w.pr?;
            let from = changes.pr_at.get(&work).copied().or(w.first_at)?;
            let done = w.ended || db::is_cell(ENGINEER, "E1", &w.at);
            let to = merged
                .get(&work)
                .or_else(|| changes.ended_at.get(&work))
                .copied()
                .or_else(|| done.then_some(w.last_at.unwrap_or(from)));
            Some(PrSpan {
                project: w.project.clone().unwrap_or_default(),
                pr,
                work,
                from,
                to,
            })
        })
        .collect()
}

/// How many pull requests were open at `t`, each by its project and number: one a later work took over is open while
/// either work has it.
fn open_at(spans: &[PrSpan], t: i64) -> i64 {
    spans
        .iter()
        .filter(|s| s.from <= t && s.to.is_none_or(|end| end > t))
        .map(|s| (s.project.as_str(), s.pr))
        .collect::<HashSet<_>>()
        .len() as i64
}

/// The pull requests open now, the oldest first: each by the latest work that has it, open since a work first recorded
/// it.
fn open_now(works: &HashMap<i64, WorkOf>, spans: &[PrSpan]) -> Vec<OpenPr> {
    let open: BTreeSet<(&str, i64)> = spans
        .iter()
        .filter(|s| s.to.is_none())
        .map(|s| (s.project.as_str(), s.pr))
        .collect();
    let mut out: Vec<OpenPr> = open
        .into_iter()
        .filter_map(|(project, pr)| {
            let of: Vec<&PrSpan> = spans.iter().filter(|s| s.project == project && s.pr == pr).collect();
            let work = of.iter().filter(|s| s.to.is_none()).map(|s| s.work).max()?;
            let w = works.get(&work)?;
            Some(OpenPr {
                pr,
                project: w.project.clone(),
                work,
                item: w.item,
                title: w.title.clone(),
                since: of.iter().map(|s| s.from).min()?,
            })
        })
        .collect();
    out.sort_by_key(|p| p.since);
    out
}

/// What an item's works cost in the period.
struct ItemSum {
    title: String,
    works: HashSet<i64>,
    rounds: i64,
    cost: f64,
}

impl ItemSum {
    fn of(w: &WorkOf) -> ItemSum {
        ItemSum {
            title: w.title.clone().unwrap_or_default(),
            works: HashSet::new(),
            rounds: 0,
            cost: 0.0,
        }
    }
}

/// What a cell's rounds cost in the period, with the flow version and the id one of them ran at, which name the cell
/// when the newest version has none of its code.
#[derive(Default)]
struct CellSum {
    rounds: i64,
    cost: f64,
    version: i64,
    id: String,
}

/// How long one cause of failed runs stopped every work: from each failed run's end to when it said to try again.
#[derive(Default)]
struct Halted {
    spans: Vec<(i64, i64)>,
    runs: i64,
}

/// How many rounds or pings of each work, and what they cost.
type Tally = HashMap<i64, (i64, f64)>;

fn tally(t: &mut Tally, work: i64, cost: f64) {
    let e = t.entry(work).or_insert((0, 0.0));
    e.0 += 1;
    e.1 += cost;
}

/// Adds what a round or a ping of a work cost to its week: an architect's wake's or an engineer's work's.
fn spent_in(week: &mut Week, w: Option<&WorkOf>, cost: f64) {
    if w.is_some_and(|w| w.graph == ARCHITECT) {
        week.architects_usd += cost;
    } else {
        week.engineers_usd += cost;
    }
}

fn line_spend<'m>(lines: &'m mut BTreeMap<(String, String), LineSpend>, line: &str, model: &str) -> &'m mut LineSpend {
    lines
        .entry((line.to_string(), model.to_string()))
        .or_insert_with(|| LineSpend {
            line: line.to_string(),
            model: model.to_string(),
            rounds: 0,
            pings: 0,
            cost_usd: 0.0,
        })
}

/// The works of a tally, the costliest first.
fn counts(works: &HashMap<i64, WorkOf>, tally: Tally) -> Vec<WorkCount> {
    let mut out: Vec<WorkCount> = tally
        .into_iter()
        .map(|(work, (n, cost))| {
            let w = works.get(&work);
            WorkCount {
                work,
                graph: w.map(|w| w.graph.clone()).unwrap_or_default(),
                item: w.and_then(|w| w.item),
                title: w.and_then(|w| w.title.clone()),
                project: w.and_then(|w| w.project.clone()),
                n,
                cost_usd: cost,
            }
        })
        .collect();
    out.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd).then(a.work.cmp(&b.work)));
    out
}

fn anomaly(works: &HashMap<i64, WorkOf>, work: i64, round: Option<i64>, at: i64, what: AnomalyKind) -> Anomaly {
    let w = works.get(&work);
    Anomaly {
        work,
        graph: w.map(|w| w.graph.clone()).unwrap_or_default(),
        item: w.and_then(|w| w.item),
        title: w.and_then(|w| w.title.clone()),
        project: w.and_then(|w| w.project.clone()),
        round,
        at,
        what,
    }
}

/// A row a round took, and when.
struct Took {
    work: i64,
    n: i64,
    cell: String,
    row: String,
    at: i64,
    started_at: i64,
}

/// The counter a row counts, and whether it is the work's total time, whose limit is no count.
#[derive(Clone)]
struct Counted {
    name: String,
    time: bool,
}

/// The counter each row counts, by the flow version, the graph, the cell and the row.
type Counters = HashMap<(i64, String, String, String), Option<Counted>>;

fn counter_of(conn: &Connection, flows: &Flows, w: &WorkOf, cell: &str, row: &str) -> Option<Counted> {
    let flow = flows.get(conn, w.version).ok()?;
    let g = flow.graph(&w.graph)?;
    let name = g.cell(cell)?.rows.iter().find(|r| r.name == row)?.counts.clone()?;
    let time = g.counters.iter().any(|c| c.name == name && c.time);
    Some(Counted { name, time })
}

/// What the rows the period's rounds took tell, each counter counted over its work's whole life: a row counting a flow
/// anomaly, one counting a tool that failed, and the row that took a counter past the limit its project sets, as the
/// engine tells the owner of it; a limit of 0 sets none, and the work's total time is measured, not counted.
fn counted(
    conn: &Connection,
    flows: &Flows,
    works: &HashMap<i64, WorkOf>,
    ask: &SystemAsk<'_>,
    since: i64,
) -> Result<Vec<Anomaly>> {
    let mut st = conn.prepare(
        "SELECT work, n, cell, transition, coalesce(ended_at, started_at), started_at FROM rounds
         WHERE cell IS NOT NULL AND transition IS NOT NULL ORDER BY work, n",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Took {
            work: r.get(0)?,
            n: r.get(1)?,
            cell: r.get(2)?,
            row: r.get(3)?,
            at: r.get(4)?,
            started_at: r.get(5)?,
        })
    })?;
    let mut known: Counters = HashMap::new();
    let mut times: HashMap<(i64, String), u64> = HashMap::new();
    let mut out = Vec::new();
    for took in rows {
        let t = took?;
        let Some(w) = works.get(&t.work) else { continue };
        let Some(counter) = known
            .entry((w.version, w.graph.clone(), t.cell.clone(), t.row.clone()))
            .or_insert_with(|| counter_of(conn, flows, w, &t.cell, &t.row))
            .clone()
        else {
            continue;
        };
        let count = times.entry((t.work, counter.name.clone())).or_insert(0);
        *count += 1;
        if t.started_at < since {
            continue;
        }
        let at = |what: AnomalyKind| anomaly(works, t.work, Some(t.n), t.at, what);
        match counter.name.as_str() {
            FLOW_ANOMALIES => out.push(at(AnomalyKind::FlowAnomaly { row: t.row.clone() })),
            TOOL_FAILURES => out.push(at(AnomalyKind::ToolFailed { cell: t.cell.clone() })),
            _ => {}
        }
        let limit = w
            .project
            .as_ref()
            .and_then(|p| ask.limits.get(p))
            .and_then(|l| l.get(&counter.name))
            .filter(|l| **l > 0);
        if let Some(&limit) = limit
            && !counter.time
            && *count == limit + 1
        {
            out.push(at(AnomalyKind::Counter {
                counter: counter.name,
                limit: limit as i64,
            }));
        }
    }
    Ok(out)
}

/// The times main went red after a work merged in the period: how, and the checks red.
fn main_red(conn: &Connection, works: &HashMap<i64, WorkOf>, since: i64) -> Result<Vec<Anomaly>> {
    let mut st = conn.prepare(&format!(
        "SELECT work, n, coalesce(ended_at, started_at), json_extract(output, '$.main.kind'),
                json_extract(output, '$.main.checks')
         FROM rounds WHERE cell IN {} AND started_at >= ?1
           AND json_extract(output, '$.main.kind') IN ('environment_changed', 'broken_by_merge', 'known_red')
         ORDER BY work, n",
        db::cells_sql(ENGINEER, &["L1"])
    ))?;
    let rows = st.query_map([since], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (work, n, at, how, checks) = row?;
        let checks: Vec<String> = json(checks)
            .as_ref()
            .and_then(|c| c.as_array())
            .map(|cs| {
                cs.iter()
                    .filter(|c| c["result"] != "green")
                    .filter_map(|c| c["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        out.push(anomaly(works, work, Some(n), at, AnomalyKind::MainRed { how, checks }));
    }
    Ok(out)
}

/// Each project's architect now: its wake not over, while it is awake, and its queue. The projects are those of the
/// configuration, and any other a wake or a matter names.
fn architects(conn: &Connection, flows: &Flows, projects: &[String]) -> Result<Vec<ArchitectNow>> {
    let mut st = conn.prepare("SELECT project, count(*), min(at) FROM architect_queue GROUP BY project")?;
    let queued: HashMap<String, (i64, Option<i64>)> = st
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect::<rusqlite::Result<_>>()?;
    let mut awake: HashMap<String, (i64, bool)> = HashMap::new();
    for w in work_summaries(conn, flows, "WHERE w.graph = 'architect' AND w.ended IS NULL")? {
        if let Some(project) = &w.project
            && !over(&w)
        {
            awake.insert(project.clone(), (w.id, w.paused.is_some()));
        }
    }
    let names: BTreeSet<&String> = projects.iter().chain(queued.keys()).chain(awake.keys()).collect();
    Ok(names
        .into_iter()
        .map(|project| {
            let (queue, oldest) = queued.get(project).copied().unwrap_or((0, None));
            let (wake, paused) = awake
                .get(project)
                .map_or((None, false), |&(id, paused)| (Some(id), paused));
            ArchitectNow {
                project: project.clone(),
                wake,
                paused,
                queue,
                oldest,
            }
        })
        .collect())
}

/// Things that wait for the owner, as a query that gives how many and when the oldest came reads them.
fn waiting(conn: &Connection, sql: &str) -> Result<Waiting> {
    Ok(conn.query_row(sql, [], |r| {
        Ok(Waiting {
            n: r.get(0)?,
            oldest: r.get(1)?,
        })
    })?)
}

#[cfg(test)]
mod tests {
    use db::{Actor, Db};
    use serde_json::json;

    use super::*;

    #[test]
    fn a_week_starts_on_monday_at_midnight_in_the_owners_time_zone() {
        // 1970-01-01 was a Thursday: its week started on Monday 1969-12-29, and the next on Monday 1970-01-05.
        assert_eq!(week_of(0, 0), -3 * DAY);
        assert_eq!(week_of(4 * DAY - 1, 0), -3 * DAY);
        assert_eq!(week_of(4 * DAY, 0), 4 * DAY);
        // Eight hours east of UTC, Monday's midnight comes eight hours earlier.
        assert_eq!(week_of(0, 480), -3 * DAY - 8 * 3_600_000);
    }

    #[test]
    fn spans_cover_each_moment_once_within_the_period() {
        assert_eq!(covered(&[(0, 10), (5, 20), (30, 40)], 0, 35), 25);
        assert_eq!(covered(&[(0, 10)], 20, 30), 0);
    }

    // One work ran, was sent back, failed on a usage limit, took a row counting a flow anomaly past its limit, merged
    // and broke main, with a keep-warm ping, a cache miss and an ask of the owner; another holds a pull request open and
    // was stopped by the engine; an item was cancelled; the architect has a matter in its queue.
    #[test]
    fn the_system_view_counts_what_the_works_did() {
        let relations = db::relations(db::SCHEMA).unwrap();
        let compiled = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations,
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        let g = compiled.graph(ENGINEER).unwrap();
        let research = g.cell("R1").unwrap().title.clone();
        let (cell, row) = g
            .cells
            .iter()
            .find_map(|c| {
                c.rows
                    .iter()
                    .find(|r| r.counts.as_deref() == Some(FLOW_ANOMALIES))
                    .map(|r| (c.id.clone(), r.name.clone()))
            })
            .unwrap();
        let hour = 3_600_000;
        let base = db::now();
        let at = |hours: i64| base - hours * hour;
        let flow = compiled.json();
        let usage = json!({ "cost_usd": 1.0, "usage": {
            "input_tokens": 50, "cache_read_input_tokens": 900, "cache_creation_input_tokens": 50 } })
        .to_string();
        let sent = json!({ "cost_usd": 0.5 }).to_string();
        let failed = json!({ "failed": { "kind": "usage_limit", "what": "limit", "until": at(2) } }).to_string();
        let main = json!({ "main": { "kind": "broken_by_merge", "checks": [
            { "name": "test", "result": "red", "run": "r" }, { "name": "lint", "result": "green", "run": "r" }] } })
        .to_string();
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            let run = |sql: &str, args: &[&dyn db::rusqlite::ToSql]| tx.execute(sql, args).map(|_| ());
            run("INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)", &[&flow])?;
            for title in ["Merge it", "Drop it", "Open it"] {
                run(
                    "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', '1')",
                    &[&title],
                )?;
            }
            run("UPDATE items SET cancelled = 'not needed' WHERE id = 2", &[])?;
            run("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'E1')", &[])?;
            run("INSERT INTO works (item, flow_version, graph, at) VALUES (3, 1, 'engineer', 'I1')", &[])?;
            run("INSERT INTO sessions (work, line, cli, cli_session) VALUES (1, 'engineer', 'claude', 's1')", &[])?;
            run(
                "INSERT INTO rounds (work, n, cell, session, input, output, transition, config_commit, model, usage,
                   started_at, ended_at)
                 VALUES (1, 1, 'R1', 1, '{}', '{}', 'done', 'c', 'm', ?1, ?2, ?2)",
                &[&usage, &at(5)],
            )?;
            run(
                "INSERT INTO rounds (work, n, cell, session, input, rejected, config_commit, model, usage, started_at,
                   ended_at)
                 VALUES (1, 2, 'R1', 1, '{}', '{\"format\":\"no outcome\"}', 'c', 'm', ?1, ?2, ?2)",
                &[&sent, &at(4)],
            )?;
            run(
                "INSERT INTO rounds (work, n, cell, session, input, rejected, config_commit, model, started_at, ended_at)
                 VALUES (1, 3, 'R1', 1, '{}', ?1, 'c', 'm', ?2, ?2)",
                &[&failed, &at(3)],
            )?;
            // Twice, past the limit of one.
            for n in [4, 7] {
                run(
                    "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                     VALUES (1, ?1, ?2, '{}', '{}', ?3, 'c', ?4, ?4)",
                    &[&n, &cell, &row, &at(3)],
                )?;
            }
            run(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 5, 'M2', '{}', '{}', 'merged', 'c', ?1, ?1)",
                &[&at(2)],
            )?;
            run(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 6, 'L1', '{}', ?1, 'broken_by_merge', 'c', ?2, ?2)",
                &[&main, &at(1)],
            )?;
            run(
                "INSERT INTO pings (session, waited, model, usage, started_at, ended_at)
                 VALUES (1, '{\"cell\":\"R1\",\"on\":\"pause\",\"what\":\"x\"}', 'm', '{\"cost_usd\":0.25}', ?1, ?1)",
                &[&at(4)],
            )?;
            run(
                "INSERT INTO engine_notices (work, text, at, fault) VALUES (1, 'missed', ?1, 'cache_miss')",
                &[&at(4)],
            )?;
            run(
                "INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, notice)
                 VALUES (1, 1, 'engineer', '[]', 40, 100, 0.125, 1)",
                &[],
            )?;
            run("INSERT INTO owner_asks (work, round, body) VALUES (1, 2, '{}')", &[])?;
            run(
                "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('p', 'owner_words', 'Look', '{}', ?1)",
                &[&at(1)],
            )?;
            run("UPDATE works SET pr = 9 WHERE id = 2", &[])?;
            run(
                "UPDATE works SET paused = 'cell T2: red', paused_by = 'engine' WHERE id = 2",
                &[],
            )?;
            Ok(())
        })
        .unwrap();
        let limits = BTreeMap::from([("p".to_string(), BTreeMap::from([(FLOW_ANOMALIES.to_string(), 1_u64)]))]);
        let projects = ["p".to_string()];
        let now = db::now();
        let ask = SystemAsk {
            weeks: Some(2),
            tz: 0,
            projects: &projects,
            limits: &limits,
            now,
        };
        let s = system(db.conn(), &Flows::default(), &ask).unwrap();

        assert_eq!(s.from, Some(week_of(now, 0) - WEEK));
        assert_eq!(s.weeks.len(), 2);
        let sum = |f: fn(&Week) -> i64| s.weeks.iter().map(f).sum::<i64>();
        assert_eq!((sum(|w| w.merged), sum(|w| w.made), sum(|w| w.cancelled)), (1, 3, 1));
        assert_eq!(s.weeks[1].open_prs, 1);
        let spent: f64 = s.weeks.iter().map(|w| w.engineers_usd + w.architects_usd).sum();
        assert!((spent - 1.75).abs() < 1e-9, "{spent}");

        // What got done: the merge, with the work's time and cost; the items made and cancelled; the pull request open.
        assert_eq!(
            s.output.merged,
            [Merged {
                item: 1,
                title: "Merge it".into(),
                work: 1,
                started_at: at(5),
                merged_at: at(2),
                cost_usd: 1.75,
            }]
        );
        assert_eq!(s.output.made.len(), 3);
        assert_eq!(s.output.cancelled[0].why.as_deref(), Some("not needed"));
        let open: Vec<(i64, i64)> = s.output.open_prs.iter().map(|p| (p.pr, p.work)).collect();
        assert_eq!(open, [(9, 2)]);

        // Where the money went: by line and model, the ping counted; by item; by cell, its title the flow's.
        assert_eq!(
            s.spend.by_line_model,
            [LineSpend {
                line: "engineer".into(),
                model: "m".into(),
                rounds: 3,
                pings: 1,
                cost_usd: 1.75,
            }]
        );
        assert_eq!(
            s.spend.by_item,
            [ItemSpend {
                item: 1,
                title: "Merge it".into(),
                works: 1,
                rounds: 7,
                cost_usd: 1.75,
            }]
        );
        assert_eq!(
            s.spend.cells,
            [CellSpend {
                graph: ENGINEER.into(),
                cell: "R1".into(),
                title: Some(research),
                rounds: 2,
                cost_usd: 1.5,
            }]
        );

        // Where it went for nothing: the cache, the return sent back, the failed run and the hour it halted every work,
        // the ping.
        let w = &s.waste;
        assert_eq!((w.read, w.written, w.input), (900, 50, 50));
        assert_eq!((w.misses, w.missed, w.miss_usd), (1, 40, 0.125));
        let of =
            |list: &[WorkCount]| -> Vec<(i64, i64, f64)> { list.iter().map(|c| (c.work, c.n, c.cost_usd)).collect() };
        assert_eq!(of(&w.sent_back), [(1, 1, 0.5)]);
        assert_eq!(of(&w.failed), [(1, 1, 0.0)]);
        assert_eq!(of(&w.pings), [(1, 1, 0.25)]);
        assert_eq!(
            w.halts,
            [HaltTime {
                kind: "usage_limit".into(),
                ms: hour,
                runs: 1,
            }]
        );
        assert_eq!(w.halted_ms, hour);

        // What went wrong: the flow anomaly twice, the second past its limit.
        let what: Vec<&AnomalyKind> = s.anomalies.iter().map(|a| &a.what).collect();
        assert_eq!(what.len(), 5, "{what:?}");
        let past: Vec<Option<i64>> = s
            .anomalies
            .iter()
            .filter(|a| matches!(a.what, AnomalyKind::Counter { .. }))
            .map(|a| a.round)
            .collect();
        assert_eq!(past, [Some(7)]);
        for kind in [
            AnomalyKind::FlowAnomaly { row },
            AnomalyKind::Counter {
                counter: FLOW_ANOMALIES.into(),
                limit: 1,
            },
            AnomalyKind::MainRed {
                how: "broken_by_merge".into(),
                checks: vec!["test".into()],
            },
            AnomalyKind::Stopped {
                why: "cell T2: red".into(),
            },
        ] {
            assert!(what.contains(&&kind), "{kind:?} in {what:?}");
        }

        // Now: the architect asleep with its queue, and what waits for the owner.
        assert_eq!(
            s.now.architects,
            [ArchitectNow {
                project: "p".into(),
                wake: None,
                paused: false,
                queue: 1,
                oldest: Some(at(1)),
            }]
        );
        assert_eq!(
            s.now.asks,
            Waiting {
                n: 1,
                oldest: Some(at(4))
            }
        );
        assert_eq!(s.now.design_cards, Waiting { n: 0, oldest: None });
        assert_eq!(
            s.now.faults,
            Waiting {
                n: 1,
                oldest: Some(at(4))
            }
        );
    }
}
