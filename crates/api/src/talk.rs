//! The owner's conversations as the messages view shows them, the one place the owner acts: their list, in an order
//! that never changes for what waits (the one-to-ones with the architects and wf-agents' own first, then a group for
//! each work, newest first, then the groups of connected projects), with what waits for the owner in each and what is
//! unread; and one in full, its entries in time order, with what the engine writes from the flow beside what was said
//! (a work's stage cards, an architect's matter cards, the policy taking effect, wf-agents' observations), what waits
//! for the owner, and how far the owner has read it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use db::rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::read::{self, Error, Flows};
use crate::{
    Conversation, ConversationDetail, ConversationEntry, Forwarded, Forwarding, Handled, MatterProgress, Misses,
    StageCard, StageMove, TalkWait,
};

type Result<T> = std::result::Result<T, Error>;

/// What the conversations need of the configuration: its projects, in order, the project that is wf-agents itself,
/// the groups of two projects or more it connects through each project's `other_projects`, and how long, in seconds,
/// an architect waits for the owner's reply in a talk (`agents.owner_reply_wait`).
#[derive(Debug, Clone, Default)]
pub struct Talks {
    pub projects: Vec<String>,
    pub own: Option<String>,
    pub sets: Vec<Vec<String>>,
    pub reply_wait: u64,
}

impl Talks {
    pub fn of(c: &config::Config) -> Talks {
        let projects: Vec<String> = c.projects.keys().cloned().collect();
        // Which group each project is in: the projects joined through `other_projects`, either way.
        let mut group: BTreeMap<String, usize> = projects.iter().enumerate().map(|(i, p)| (p.clone(), i)).collect();
        for (name, p) in &c.projects {
            for o in &p.other_projects {
                let (Some(&a), Some(&b)) = (group.get(name), group.get(o)) else {
                    continue;
                };
                if a == b {
                    continue;
                }
                for g in group.values_mut() {
                    if *g == b {
                        *g = a;
                    }
                }
            }
        }
        let mut sets: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for (p, g) in &group {
            sets.entry(*g).or_default().push(p.clone());
        }
        Talks {
            projects,
            own: c.wf_agents_project(),
            sets: sets.into_values().filter(|s| s.len() > 1).collect(),
            reply_wait: c.install.agents.owner_reply_wait,
        }
    }

    /// The conversation an entry of `conversation_entries` belongs to: what the architects of projects say to each
    /// other, and the owner's words to them, go to the group of every project the configuration connects them with.
    fn key(&self, raw: &str) -> String {
        let Some(rest) = raw.strip_prefix("architects:") else {
            return raw.to_string();
        };
        let first = rest.split('|').next().unwrap_or_default();
        match self.sets.iter().find(|s| s.iter().any(|p| p == first)) {
            Some(s) => set_key(s),
            None => format!("projects:{rest}"),
        }
    }

    /// The projects of a group of projects, by its key.
    pub fn set_of(&self, key: &str) -> Option<Vec<String>> {
        let rest = key.strip_prefix("projects:")?;
        Some(rest.split('|').map(str::to_string).collect())
    }
}

fn set_key(set: &[String]) -> String {
    format!("projects:{}", set.join("|"))
}

/// What the list counts of a conversation.
#[derive(Default)]
struct Tally {
    waiting: i64,
    unread: i64,
    last_at: Option<i64>,
}

/// How far the owner has read each conversation, by its key.
fn reads(conn: &Connection) -> Result<HashMap<String, i64>> {
    let mut st = conn.prepare("SELECT conversation, at FROM conversation_reads")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<db::rusqlite::Result<_>>()?)
}

/// The cells of the engineer's graph where a work is over, as `conversation_entries` names its end.
fn over_sql() -> String {
    db::cells_sql("engineer", &["E1"])
}

/// The owner's conversations in the list's order, which waiting and new entries never change: the one-to-one with
/// each project's architect, in the configuration's order, and wf-agents' own; then a group for each engineer's work,
/// newest first, an ended one greyed in place; then a group for each set of projects the configuration connects. Each
/// with how many things in it wait for the owner, how many of its entries the owner has not read, and when its latest
/// came.
pub fn conversations(conn: &Connection, t: &Talks) -> Result<Vec<Conversation>> {
    let read_to = reads(conn)?;
    let mut tally: HashMap<String, Tally> = HashMap::new();
    // An ask the owner answered is read: the owner answered it where it was asked.
    let mut st = conn.prepare(
        "SELECT conversation, at, sender, waiting, fault IS NOT NULL,
                source = 'ask' AND id IN (SELECT id FROM owner_asks WHERE answer IS NOT NULL)
         FROM conversation_entries",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, bool>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, bool>(5)?,
        ))
    })?;
    for row in rows {
        let (raw, at, sender, waiting, fault, answered) = row?;
        let key = t.key(&raw);
        let read = read_to.get(&key).copied().unwrap_or(0);
        let x = tally.entry(key).or_default();
        if waiting {
            x.waiting += 1;
        } else if !fault && !answered && sender != "owner" && at > read {
            x.unread += 1;
        }
        x.last_at = Some(x.last_at.map_or(at, |l| l.max(at)));
    }
    let mut out = Vec::new();
    let one = |key: String, kind: &str, project: Option<String>, tally: &mut HashMap<String, Tally>| {
        let x = tally.remove(&key).unwrap_or_default();
        Conversation {
            projects: if kind == "projects" {
                t.set_of(&key).unwrap_or_default()
            } else {
                Vec::new()
            },
            key,
            kind: kind.to_string(),
            project,
            work: None,
            item: None,
            title: None,
            level: None,
            over: false,
            waiting: x.waiting,
            unread: x.unread,
            last_at: x.last_at,
        }
    };
    // The architects of the configuration's projects, then those of projects it no longer has whose entries remain.
    let mut architects: Vec<String> = t.projects.clone();
    let gone: BTreeSet<String> = tally
        .keys()
        .filter_map(|k| k.strip_prefix("architect:"))
        .filter(|p| !t.projects.iter().any(|x| x == p))
        .map(str::to_string)
        .collect();
    architects.extend(gone);
    for p in architects {
        out.push(one(format!("architect:{p}"), "architect", Some(p), &mut tally));
    }
    out.push(one("wf-agents".into(), "wf_agents", None, &mut tally));
    let mut st = conn.prepare(&format!(
        "SELECT w.id, w.item, i.title, i.level, i.project, w.ended IS NOT NULL OR w.at IN {}
         FROM works w JOIN items i ON i.id = w.item WHERE w.graph = 'engineer' ORDER BY w.id DESC",
        over_sql()
    ))?;
    let works = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, bool>(5)?,
        ))
    })?;
    for w in works {
        let (id, item, title, level, project, over) = w?;
        let key = format!("work:{id}");
        let x = tally.remove(&key).unwrap_or_default();
        out.push(Conversation {
            key,
            kind: "work".into(),
            project: Some(project),
            projects: Vec::new(),
            work: Some(id),
            item: Some(item),
            title: Some(title),
            level: Some(level),
            over,
            waiting: x.waiting,
            unread: x.unread,
            last_at: x.last_at,
        });
    }
    let mut groups: Vec<String> = t.sets.iter().map(|s| set_key(s)).collect();
    let left: BTreeSet<String> = tally
        .keys()
        .filter(|k| k.starts_with("projects:") && !groups.contains(k))
        .cloned()
        .collect();
    groups.extend(left);
    for key in groups {
        out.push(one(key, "projects", None, &mut tally));
    }
    Ok(out)
}

/// One of the owner's conversations in full: its entries in time order, with what the engine writes from the flow
/// beside what was said (a work's stage cards; an architect's matter cards and the policy in force, with the documents
/// of its asks; wf-agents' observations); the open blockers the owner holds in it, with their forms; and how far the
/// owner has read it.
pub fn conversation(conn: &Connection, flows: &Flows, t: &Talks, key: &str) -> Result<ConversationDetail> {
    let select = "SELECT conversation, source, id, sender, recipient, text, at, work, round, ask, design_card, matter,
                    read_at, waiting, pending, undelivered, answerable, fault
                  FROM conversation_entries";
    let (sql, arg) = if key.starts_with("projects:") {
        (format!("{select} WHERE conversation LIKE 'architects:%'"), None)
    } else {
        (format!("{select} WHERE conversation = ?1"), Some(key))
    };
    let mut st = conn.prepare(&sql)?;
    let map = |r: &db::rusqlite::Row<'_>| {
        Ok((
            r.get::<_, String>(0)?,
            ConversationEntry {
                source: r.get(1)?,
                id: r.get(2)?,
                sender: r.get(3)?,
                recipient: r.get(4)?,
                text: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                at: r.get(6)?,
                work: r.get(7)?,
                round: r.get(8)?,
                ask: r.get(9)?,
                design_card: r.get(10)?,
                matter: r.get(11)?,
                read_at: r.get(12)?,
                waiting: r.get(13)?,
                pending: r.get(14)?,
                undelivered: r.get(15)?,
                answerable: r.get(16)?,
                fault: r.get(17)?,
                misses: None,
                forwards: Vec::new(),
                ask_detail: None,
                revisions: None,
                stage: None,
                matter_card: None,
                policy: None,
                observation: None,
            },
        ))
    };
    let rows: Vec<(String, ConversationEntry)> = match arg {
        Some(k) => st.query_map([k], map)?.collect::<db::rusqlite::Result<_>>()?,
        None => st.query_map([], map)?.collect::<db::rusqlite::Result<_>>()?,
    };
    let mut entries: Vec<ConversationEntry> = rows
        .into_iter()
        .filter(|(raw, _)| t.key(raw) == key)
        .map(|(_, e)| e)
        .collect();
    if key.starts_with("architect:") {
        said_at_once(&mut entries);
    }
    misses(conn, &mut entries)?;
    forwards(conn, &mut entries)?;
    // Each ask in full, with its documents: the study of a new policy, the policy to approve with the corrections of
    // its text since the owner last read it.
    let ids: Vec<String> = entries
        .iter()
        .filter(|e| e.source == "ask")
        .map(|e| e.id.to_string())
        .collect();
    let asks = if ids.is_empty() {
        Vec::new()
    } else {
        read::asks(conn, &format!("WHERE a.id IN ({})", ids.join(", ")), &[])?
    };
    for e in entries.iter_mut().filter(|e| e.source == "ask") {
        e.ask_detail = asks.iter().find(|a| a.id == e.id).cloned();
        if let Some(a) = &e.ask_detail
            && a.body["kind"] == "policy_approval"
        {
            e.revisions = Some(revisions(conn, a.id, &a.body, a.work, a.round)?);
        }
    }
    let waiting: Vec<_> = asks
        .iter()
        .filter(|a| entries.iter().any(|e| e.source == "ask" && e.id == a.id && e.waiting))
        .cloned()
        .collect();
    let mut design_cards = Vec::new();
    if let Some(work) = key.strip_prefix("work:").and_then(|w| w.parse::<i64>().ok()) {
        entries.extend(stage_cards(conn, flows, work)?);
        // The design card a work waits for the owner's ruling on before its merge: the blocker the owner holds.
        design_cards = read::design_cards(
            conn,
            flows,
            "WHERE c.status = 'open' AND EXISTS (SELECT 1 FROM blockers b WHERE b.work = c.work AND b.kind = 'ruling'
               AND b.holder = 'owner' AND b.ended IS NULL) AND c.work = ?1",
            &[work],
        )?;
    }
    let talk = match key.strip_prefix("architect:") {
        Some(project) => waits_for_reply(conn, project, t.reply_wait)?,
        None => None,
    };
    if let Some(project) = key.strip_prefix("architect:") {
        // The owner's words a talk is about are taken up, while the talk is open or held for the owner's reply.
        if let Some((open, _)) = db::open_talk(conn, project).map_err(|e| Error::Failed(e.to_string()))? {
            for e in entries
                .iter_mut()
                .filter(|e| e.source == "words" && e.matter == Some(open))
            {
                e.pending = false;
            }
        }
        entries.extend(matter_cards(conn, flows, project)?);
        if let Some(p) = read::policy(conn, project)? {
            entries.push(ConversationEntry {
                source: "policy".into(),
                id: p.round,
                sender: "architect".into(),
                recipient: "owner".into(),
                text: String::new(),
                at: p.at.unwrap_or_default(),
                work: Some(p.work),
                round: Some(p.round),
                policy: Some(p),
                ..blank()
            });
        }
    }
    // The observations of a project, which the owner closes where its architect speaks to the owner: wf-agents' in its
    // own conversation, any other project's in its architect's one-to-one.
    let observed = match key.strip_prefix("architect:") {
        Some(p) if t.own.as_deref() != Some(p) => Some(p.to_string()),
        Some(_) => None,
        None if key == "wf-agents" => t.own.clone(),
        None => None,
    };
    if let Some(project) = observed {
        for o in read::observations(conn, &project)? {
            entries.push(ConversationEntry {
                source: "observation".into(),
                id: o.id,
                sender: "architect".into(),
                recipient: "owner".into(),
                text: o.what.clone(),
                at: o.created.unwrap_or_default(),
                observation: Some(o),
                ..blank()
            });
        }
    }
    entries.sort_by(|a, b| {
        a.at.cmp(&b.at)
            .then_with(|| a.source.cmp(&b.source))
            .then(a.id.cmp(&b.id))
    });
    let read_to: Option<i64> = conn
        .query_row(
            "SELECT at FROM conversation_reads WHERE conversation = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(ConversationDetail {
        key: key.to_string(),
        entries,
        asks: waiting,
        design_cards,
        read_to,
        forwarding: if key == "wf-agents" {
            Some(Forwarding {
                projects: t.projects.clone(),
                auto: db::fault_forwarding(conn).map_err(|e| Error::Failed(e.to_string()))?,
            })
        } else {
            None
        },
        talk,
    })
}

/// The talk with a project's architect that waits for the owner's reply: the one open, while its work waits at the
/// cell that waits for the reply (`H11`), until its reply's end and the wait (`wait`, seconds) later, or held out of the
/// queue once the wait ran out; none while the architect is answering, the owner said more since, or none is open.
fn waits_for_reply(conn: &Connection, project: &str, wait: u64) -> Result<Option<TalkWait>> {
    let Some((matter, held)) = db::open_talk(conn, project).map_err(|e| Error::Failed(e.to_string()))? else {
        return Ok(None);
    };
    // The owner said more in the one-to-one since the talk was held, or since its last reply: the architect has it.
    let since: Option<i64> = conn.query_row(
        &format!(
            "SELECT CASE WHEN ?2 THEN m.talk_held ELSE (
               SELECT max(r.ended_at) FROM rounds r JOIN works w ON w.id = r.work
               WHERE w.graph = 'architect' AND w.project = m.project AND w.ended IS NULL AND r.cell IN {}
                 AND r.transition IS NOT NULL) END
             FROM architect_matters m WHERE m.id = ?1",
            db::cells_sql("architect", &["H8"])
        ),
        params![matter, held],
        |r| r.get(0),
    )?;
    let said: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM architect_matters WHERE project = ?1 AND kind = 'owner_words'
           AND json_extract(body, '$.where') = 'page' AND id <> ?2 AND handled_round IS NULL AND withdrawn IS NULL
           AND at > coalesce(?3, 0))",
        params![project, matter, since],
        |r| r.get(0),
    )?;
    if said {
        return Ok(None);
    }
    if held {
        return Ok(Some(TalkWait { matter, until: None }));
    }
    let at: Option<String> = conn
        .query_row(
            "SELECT at FROM works WHERE graph = 'architect' AND project = ?1 AND ended IS NULL
             ORDER BY id DESC LIMIT 1",
            [project],
            |r| r.get(0),
        )
        .optional()?;
    if !at.is_some_and(|a| db::is_cell("architect", "H11", &a)) {
        return Ok(None);
    }
    let wait = i64::try_from(wait).unwrap_or(i64::MAX / 1000).saturating_mul(1000);
    Ok(Some(TalkWait {
        matter,
        until: since.map(|s| s.saturating_add(wait)),
    }))
}

/// What the owner said at once to the architect's work in hand and to its policy setting, an interjection into each,
/// as one entry said to both, shown once: the first, not taken up while either is not, and never received when either
/// was not.
fn said_at_once(entries: &mut Vec<ConversationEntry>) {
    let mut kept: Vec<ConversationEntry> = Vec::with_capacity(entries.len());
    for e in entries.drain(..) {
        let owner_said = |x: &ConversationEntry| x.source == "interjection" && x.sender == "owner";
        if owner_said(&e)
            && let Some(first) = kept
                .iter_mut()
                .find(|k| owner_said(k) && k.at == e.at && k.text == e.text && k.recipient != e.recipient)
        {
            first.recipient = "both".into();
            first.pending |= e.pending;
            first.undelivered |= e.undelivered;
            continue;
        }
        kept.push(e);
    }
    *entries = kept;
}

/// An entry with nothing in it, for the engine's cards to fill.
fn blank() -> ConversationEntry {
    ConversationEntry {
        source: String::new(),
        id: 0,
        sender: String::new(),
        recipient: String::new(),
        text: String::new(),
        at: 0,
        work: None,
        round: None,
        ask: None,
        design_card: None,
        matter: None,
        read_at: None,
        waiting: false,
        pending: false,
        undelivered: false,
        answerable: false,
        fault: None,
        misses: None,
        forwards: Vec::new(),
        ask_detail: None,
        revisions: None,
        stage: None,
        matter_card: None,
        policy: None,
        observation: None,
    }
}

/// The cache misses each notice of a fault of that kind holds.
fn misses(conn: &Connection, entries: &mut [ConversationEntry]) -> Result<()> {
    let mut tally = conn.prepare(
        "SELECT count(*), coalesce(sum(c.written), 0), coalesce(sum(c.cost_usd), 0.0), max(r.started_at)
         FROM cache_misses c JOIN rounds r ON r.work = c.work AND r.n = c.round WHERE c.notice = ?1",
    )?;
    for e in entries.iter_mut().filter(|e| e.source == "notice" && e.fault.is_some()) {
        let m = tally.query_row([e.id], |r| {
            Ok(Misses {
                count: r.get(0)?,
                written: r.get(1)?,
                cost_usd: r.get(2)?,
                last_at: r.get(3)?,
            })
        })?;
        e.misses = (m.count > 0).then_some(m);
    }
    Ok(())
}

/// Where each notice of a fault of wf-agents went: the architects it was forwarded to, in the order it went, by whom,
/// and how the matter it became stands with each.
fn forwards(conn: &Connection, entries: &mut [ConversationEntry]) -> Result<()> {
    let mut st = conn.prepare(
        "SELECT project, coalesce(json_extract(body, '$.by'), 'owner'), id,
           CASE WHEN handled_round IS NOT NULL THEN 'handled' WHEN withdrawn IS NOT NULL THEN 'withdrawn'
                ELSE 'queued' END,
           handled_work, handled_round
         FROM architect_matters WHERE notice = ?1 ORDER BY id",
    )?;
    for e in entries.iter_mut().filter(|e| e.source == "notice" && e.fault.is_some()) {
        let rows = st.query_map([e.id], |r| {
            Ok(Forwarded {
                project: r.get(0)?,
                by: r.get(1)?,
                matter: r.get(2)?,
                state: r.get(3)?,
                handled_work: r.get(4)?,
                handled_round: r.get(5)?,
            })
        })?;
        e.forwards = rows.collect::<db::rusqlite::Result<_>>()?;
    }
    Ok(())
}

/// The corrections of the text of a policy being set that came before an ask to approve it, since the setting's ask
/// before it (or since the text was written): each correcting round with why and its edits, oldest first. A setting
/// that is a stage of the architect's works, whose ask holds its matter, has them in whichever of those works; one a
/// flow version before ran in a work of its own, in that work.
fn revisions(conn: &Connection, ask: i64, body: &Value, work: i64, round: i64) -> Result<Value> {
    let held = body["holds"].as_array().and_then(|h| h.first()).and_then(Value::as_i64);
    if let Some(m) = held {
        let since: (i64, i64) = conn
            .query_row(
                "SELECT work, round FROM owner_asks a, json_each(a.body, '$.holds') h
                 WHERE h.value = ?1 AND a.id < ?2 AND json_extract(a.body, '$.kind') = 'policy_approval'
                 ORDER BY a.id DESC LIMIT 1",
                params![m, ask],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .unwrap_or((0, 0));
        let mut st = conn.prepare(
            "SELECT work, n, json_extract(output, '$.outcome.revision') FROM policy_setting_rounds
             WHERE matter = ?1 AND cell = 'P8' AND transition IS NOT NULL
               AND (work > ?2 OR (work = ?2 AND n > ?3)) AND (work < ?4 OR (work = ?4 AND n < ?5))
             ORDER BY work, n",
        )?;
        let rows = st.query_map(params![m, since.0, since.1, work, round], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (w, n, revision) = row?;
            if let Some(v) = read::json(revision) {
                out.push(json!({ "work": w, "round": n, "why": v["why"], "edits": v["edits"] }));
            }
        }
        return Ok(Value::Array(out));
    }
    let since: i64 = conn.query_row(
        "SELECT coalesce(max(round), 0) FROM owner_asks
         WHERE work = ?1 AND round < ?2 AND json_extract(body, '$.kind') = 'policy_approval'",
        params![work, round],
        |r| r.get(0),
    )?;
    let mut st = conn.prepare(&format!(
        "SELECT n, json_extract(output, '$.outcome.revision') FROM rounds
         WHERE work = ?1 AND n > ?2 AND n < ?3 AND transition IS NOT NULL AND cell IN {} ORDER BY n",
        db::cells_sql("architect", &["P8"])
    ))?;
    let rows = st.query_map(params![work, since, round], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (n, revision) = row?;
        if let Some(v) = read::json(revision) {
            out.push(json!({ "round": n, "why": v["why"], "edits": v["edits"] }));
        }
    }
    Ok(Value::Array(out))
}

/// A round of a work, as the stage cards read it.
struct Step {
    n: i64,
    cell: Option<String>,
    transition: Option<String>,
    started_at: i64,
    ended_at: Option<i64>,
    cost: f64,
    line: Option<String>,
    progress: Option<String>,
}

/// The stage cards of an engineer's work: one for each stretch of its rounds at one stage that runs anything but the
/// engine's own steps (research, design, implementation, testing and fixing, the merge, watching main), from the round
/// that came into the stage to the round that left it. A cell at no stage (asking, tools, a pause, answers) is at the
/// stage the work stopped at. A stage the work comes back to is a stretch of its own, which says what brought it back.
/// Each says when it ran and what it cost, the cell the work is at while it is at the stage, the row it left by, what
/// its rounds handed back that the flow shows on the item, the design cards they handed in, how many rounds fixed and
/// reviewed, and what its last agent's round said it did.
fn stage_cards(conn: &Connection, flows: &Flows, work: i64) -> Result<Vec<ConversationEntry>> {
    let found: Option<(i64, String, bool)> = conn
        .query_row(
            &format!(
                "SELECT flow_version, at, ended IS NOT NULL OR at IN {} FROM works WHERE id = ?1 AND graph = 'engineer'",
                over_sql()
            ),
            [work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((version, at, over)) = found else {
        return Ok(Vec::new());
    };
    let Some(g) = flows.graph(conn, version, "engineer") else {
        return Ok(Vec::new());
    };
    let mut st = conn.prepare(
        "SELECT r.n, r.cell, r.transition, r.started_at, r.ended_at, coalesce(json_extract(r.usage, '$.cost_usd'), 0),
                s.line, json_extract(r.output, '$.progress')
         FROM rounds r LEFT JOIN sessions s ON s.id = r.session WHERE r.work = ?1 ORDER BY r.n",
    )?;
    let steps: Vec<Step> = st
        .query_map([work], |r| {
            Ok(Step {
                n: r.get(0)?,
                cell: r.get(1)?,
                transition: r.get(2)?,
                started_at: r.get(3)?,
                ended_at: r.get(4)?,
                cost: r.get(5)?,
                line: r.get(6)?,
                progress: r.get(7)?,
            })
        })?
        .collect::<db::rusqlite::Result<_>>()?;
    // A stage runs more than the engine's steps when one of its cells is an agent's or a tool's.
    let carded: BTreeSet<String> = g
        .cells
        .iter()
        .filter(|c| c.kind != "engine")
        .filter_map(|c| c.stage.clone())
        .collect();
    let stage_of = |cell: Option<&str>| cell.and_then(|c| g.cell(c)).and_then(|c| c.stage.clone());
    let moved = |s: &Step| -> Option<StageMove> {
        let cell = s.cell.clone()?;
        let row = s.transition.clone()?;
        let c = g.cell(&cell);
        Some(StageMove {
            round: s.n,
            title: c.map(|c| c.title.clone()),
            note: c
                .and_then(|c| c.rows.iter().find(|x| x.name == row))
                .and_then(|x| x.note.clone()),
            cell,
            row,
            said: s.progress.clone().filter(|p| !p.trim().is_empty()),
        })
    };
    let mut cards: Vec<StageCard> = Vec::new();
    let mut again: HashMap<String, i64> = HashMap::new();
    let mut now: Option<String> = None;
    for (i, s) in steps.iter().enumerate() {
        let stage = stage_of(s.cell.as_deref()).or_else(|| now.clone());
        if stage != now {
            let before = i.checked_sub(1).and_then(|k| steps.get(k));
            if let (Some(card), Some(b)) = (cards.last_mut(), before)
                && Some(&card.stage) == now.as_ref()
            {
                card.left = moved(b);
            }
            now = stage.clone();
            if let Some(name) = stage.filter(|n| carded.contains(n)) {
                let k = again.entry(name.clone()).or_insert(0);
                cards.push(StageCard {
                    stage: name,
                    again: *k,
                    first: s.n,
                    last: s.n,
                    started_at: s.started_at,
                    ended_at: None,
                    cost_usd: 0.0,
                    current: false,
                    cell: None,
                    cell_title: None,
                    back: if *k > 0 { before.and_then(moved) } else { None },
                    left: None,
                    shown: Vec::new(),
                    design_cards: Vec::new(),
                    fixes: 0,
                    reviews: 0,
                    said: None,
                    said_round: None,
                });
                *k += 1;
            }
        }
        let Some(card) = cards.last_mut().filter(|c| Some(&c.stage) == now.as_ref()) else {
            continue;
        };
        card.last = s.n;
        card.ended_at = s.ended_at;
        card.cost_usd += s.cost;
        if s.cell.as_deref().is_some_and(|c| db::is_cell("engineer", "T6", c)) {
            card.fixes += 1;
        }
        if s.line.as_deref() == Some("review") && s.transition.is_some() {
            card.reviews += 1;
        }
        if s.line.as_deref() == Some("engineer")
            && let Some(p) = s.progress.as_ref().filter(|p| !p.trim().is_empty())
        {
            card.said = Some(p.clone());
            card.said_round = Some(s.n);
        }
    }
    // The stage the work is at now: the last card's, while the work goes on and its cell is at that stage.
    let at_stage = stage_of(Some(&at)).or(now);
    if let Some(card) = cards.last_mut()
        && !over
        && Some(&card.stage) == at_stage.as_ref()
    {
        card.current = true;
        card.ended_at = None;
        card.cell_title = g.cell(&at).map(|c| c.title.clone());
        card.cell = Some(at);
    }
    let all_cards = read::design_cards(conn, flows, "WHERE c.work = ?1", &[work])?;
    let mut out = Vec::new();
    for (k, mut card) in cards.into_iter().enumerate() {
        card.shown = read::shown_where(
            conn,
            flows,
            "w.id = ?1 AND r.n BETWEEN ?2 AND ?3",
            &[work, card.first, card.last],
        )?;
        card.design_cards = all_cards
            .iter()
            .filter(|c| c.round >= card.first && c.round <= card.last)
            .cloned()
            .collect();
        out.push(ConversationEntry {
            source: "stage".into(),
            id: k as i64 + 1,
            sender: "engine".into(),
            recipient: "owner".into(),
            at: card.started_at,
            work: Some(work),
            round: Some(card.first),
            stage: Some(card),
            ..blank()
        });
    }
    Ok(out)
}

/// The matter cards of a project's architect, the latest two hundred matters it took up, each where it stands: held by
/// an ask of the owner, worked on at the cell the architect's work is at, handled with what became of it, withdrawn
/// with why, or back in the queue; a matter about a work names that work, whose group holds it in full. A card comes
/// into the conversation when the architect takes its matter up, at the round that picked it (a lone matter at the
/// look at the queue, or one picked from many), and stands there while it changes, so that the cards read in the order
/// the architect took the matters up, the one in hand last. A matter it never picked comes in when it left the queue:
/// when it was handled without a pick (closed without a look), held by an ask, or withdrawn. One still waiting in the
/// queue has no card: the queue under the head lists it.
fn matter_cards(conn: &Connection, flows: &Flows, project: &str) -> Result<Vec<ConversationEntry>> {
    let handled: HashMap<i64, Handled> = read::matters_handled(conn, project)?
        .into_iter()
        .map(|h| (h.matter, h))
        .collect();
    // The architect's work going on, the cell it is at and the matter it handles there.
    let architect_work: Option<(i64, String, i64)> = conn
        .query_row(
            "SELECT id, at, flow_version FROM works WHERE graph = 'architect' AND project = ?1 AND ended IS NULL
             ORDER BY id DESC LIMIT 1",
            [project],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let working = match &architect_work {
        Some((w, at, version)) => read::handling(conn, *w)?.map(|m| {
            let title = flows
                .graph(conn, *version, "architect")
                .and_then(|g| g.cell(at).map(|c| c.title.clone()));
            let round: Option<i64> = conn
                .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [*w], |r| r.get(0))
                .ok()
                .flatten();
            (m, *w, at.clone(), title, round)
        }),
        None => None,
    };
    // Each matter's place in the conversation: the first round that picked it; else the round that handled it, the
    // round of the ask that holds it, or when it was withdrawn; none while it waits in the queue untaken.
    let mut st = conn.prepare(&format!(
        "WITH taken AS (
           SELECT CAST(coalesce(json_extract(r.output, '$.queue.matter'),
                                json_extract(r.output, '$.outcome.pick.matter')) AS INTEGER) AS matter,
                  min(coalesce(r.ended_at, r.started_at)) AS at
           FROM rounds r JOIN works w ON w.id = r.work
           WHERE w.graph = 'architect' AND w.project = ?1 AND r.transition IS NOT NULL AND r.cell IN {}
           GROUP BY 1)
         SELECT * FROM (
           SELECT m.id, m.kind, m.detail, m.what, m.item, m.withdrawn, m.asked, m.handled_round IS NOT NULL,
             m.asked IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL),
             (SELECT w.id FROM works w WHERE w.graph = 'engineer'
                AND w.id = coalesce(m.work, CAST(json_extract(m.body, '$.work') AS INTEGER))),
             CASE WHEN m.kind = 'set_straight' THEN json_extract(m.body, '$.fix') END,
             coalesce(t.at,
               (SELECT coalesce(r.ended_at, r.started_at) FROM rounds r
                WHERE r.work = m.handled_work AND r.n = m.handled_round),
               (SELECT coalesce(r.ended_at, r.started_at) FROM owner_asks a
                JOIN rounds r ON r.work = a.work AND r.n = a.round WHERE a.id = m.asked),
               CASE WHEN m.withdrawn IS NOT NULL THEN coalesce(
                 (SELECT min(c.at) FROM changes c
                  WHERE c.table_name = 'architect_matters' AND c.row_key = json_object('id', m.id)
                    AND json_extract(c.new_row, '$.withdrawn') IS NOT NULL), m.at) END) AS shown
           FROM architect_matters m LEFT JOIN taken t ON t.matter = m.id WHERE m.project = ?1)
         WHERE shown IS NOT NULL ORDER BY id DESC LIMIT 200",
        db::cells_sql("architect", &["Q0", "Q1"])
    ))?;
    let rows = st.query_map([project], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<i64>>(6)?,
            r.get::<_, bool>(7)?,
            r.get::<_, bool>(8)?,
            r.get::<_, Option<i64>>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, i64>(11)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, kind, detail, what, item, withdrawn, asked, done, held, work, fix, at) = row?;
        let now = working.as_ref().filter(|w| w.0 == id);
        let state = if done {
            "done"
        } else if withdrawn.is_some() {
            "withdrawn"
        } else if now.is_some() {
            "working"
        } else if held {
            "held"
        } else {
            "queued"
        };
        let card = MatterProgress {
            matter: id,
            kind,
            detail,
            what: what.clone(),
            state: state.into(),
            cell: now.map(|w| w.2.clone()),
            cell_title: now.and_then(|w| w.3.clone()),
            architect_work: now.map(|w| w.1),
            round: now.and_then(|w| w.4),
            work,
            item,
            handled: handled.get(&id).cloned(),
            withdrawn,
            asked,
            fix: read::json(fix),
        };
        out.push(ConversationEntry {
            source: "matter".into(),
            id,
            sender: "architect".into(),
            recipient: "owner".into(),
            text: what,
            at,
            work,
            matter: Some(id),
            matter_card: Some(card),
            ..blank()
        });
    }
    Ok(out)
}
