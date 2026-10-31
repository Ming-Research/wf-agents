//! The owner's conversations as the messages view shows them, the one place the owner acts: their list, in an order
//! that never changes for what waits (the one-to-ones with the architects and wf-agents' own first, then a group for
//! each work, newest first, then the groups of connected projects), with what waits for the owner in each and what is
//! unread; and one in full, its entries in time order, with what the engine writes from the flow beside what was said
//! (a bubble for each stage a work entered, an architect's matter cards, the policy taking effect) and where the
//! owner's words stand with the architect, what waits for the owner, and how far the owner has read it. A work's group
//! is a chat: what its members said, and the work's own bubble for each stage it entered; nothing the engine tells of
//! the work is an entry of its own there.

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use db::rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

use crate::read::{self, Error, Flows};
use crate::{
    Conversation, ConversationDetail, ConversationEntry, Forwarded, Forwarding, Handled, MadeThing, MatterProgress,
    Misses, PlanChange, ReviewFindings, StageCard, StageNote, WordsState,
};

type Result<T> = std::result::Result<T, Error>;

/// What a work's group and the one-to-one with an architect leave out of what `conversation_entries` gathers for them,
/// as a condition on the view's columns: the engine's notices, which the bubble of the stage the work was at, the
/// bubble of the matter the architect has in hand and their heads say instead; in a work's group, the side reports a
/// work's lines handed in, which are the architect's to read in its queue, and what a helper of the work (the review,
/// the adversarial testing, the advisor) told the owner before only the engineer and the architect could, which its
/// round's progress note in the stage's bubble says.
const LEFT_OUT: &str = "NOT ((conversation LIKE 'work:%' OR conversation LIKE 'architect:%') AND source = 'notice')
    AND NOT (conversation LIKE 'work:%' AND (source = 'report'
      OR (source = 'message' AND sender NOT IN ('engineer', 'architect', 'policy'))))";

/// What the conversations need of the configuration: its projects, in order, the groups of two projects or more it
/// connects through each project's `other_projects`, and how long, in milliseconds, an architect waits for the owner's
/// reply to the question it put last (`agents.owner_reply_wait`).
#[derive(Debug, Clone, Default)]
pub struct Talks {
    pub projects: Vec<String>,
    pub sets: Vec<Vec<String>>,
    pub reply_wait: i64,
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
            sets: sets.into_values().filter(|s| s.len() > 1).collect(),
            reply_wait: i64::try_from(c.install.agents.owner_reply_wait)
                .unwrap_or(i64::MAX / 1000)
                .saturating_mul(1000),
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
    // An ask the owner answered is read: the owner answered it where it was asked. Only what is said to the owner is
    // unread until the owner reads it: what the members say to each other is no news the owner must come to.
    let mut st = conn.prepare(&format!(
        "SELECT conversation, at, sender, waiting, fault IS NOT NULL,
                source = 'ask' AND id IN (SELECT id FROM owner_asks WHERE answer IS NOT NULL), recipient = 'owner'
         FROM conversation_entries WHERE {LEFT_OUT}"
    ))?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, bool>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, bool>(5)?,
            r.get::<_, bool>(6)?,
        ))
    })?;
    for row in rows {
        let (raw, at, sender, waiting, fault, answered, to_owner) = row?;
        let key = t.key(&raw);
        let read = read_to.get(&key).copied().unwrap_or(0);
        let x = tally.entry(key).or_default();
        if waiting {
            x.waiting += 1;
        } else if !fault && !answered && to_owner && sender != "owner" && at > read {
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
/// of its asks), and where the owner's words in it stand with the architect; the open blockers the owner holds in it,
/// with their forms; in the one-to-one with an architect, the questions it pinned for the owner's reply and the one it
/// waits on now; and how far the owner has read it.
pub fn conversation(conn: &Connection, flows: &Flows, t: &Talks, key: &str) -> Result<ConversationDetail> {
    let select = "SELECT conversation, source, id, sender, recipient, text, at, work, round, ask, design_card, matter,
                    read_at, waiting, pending, undelivered, answerable, fault
                  FROM conversation_entries";
    let (sql, arg) = if key.starts_with("projects:") {
        (format!("{select} WHERE conversation LIKE 'architects:%'"), None)
    } else {
        (format!("{select} WHERE conversation = ?1 AND {LEFT_OUT}"), Some(key))
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
                images: Vec::new(),
                ask_detail: None,
                stage: None,
                matter_card: None,
                words_state: None,
                policy: None,
                plan: None,
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
    words_states(conn, &mut entries)?;
    images(conn, &mut entries)?;
    // Each ask in full.
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
    }
    // In the one-to-one with an architect, the questions it pinned for the owner's reply, newest first, and the one it
    // waits on now, read as the engine reads them.
    let (questions, waits_on) = match key.strip_prefix("architect:") {
        Some(project) => {
            let mut st = conn.prepare(db::QUESTIONS_PENDING)?;
            let pinned: Vec<i64> = st
                .query_map([project], |r| r.get("question"))?
                .collect::<db::rusqlite::Result<_>>()?;
            let questions: Vec<_> = pinned
                .iter()
                .filter_map(|id| asks.iter().find(|a| a.id == *id).cloned())
                .collect();
            let waits_on = db::question_waited_on(conn, project, t.reply_wait, db::now())
                .map_err(|e| Error::Failed(e.to_string()))?;
            (questions, waits_on)
        }
        None => (Vec::new(), None),
    };
    let waiting: Vec<_> = asks
        .iter()
        .filter(|a| entries.iter().any(|e| e.source == "ask" && e.id == a.id && e.waiting))
        .filter(|a| !questions.iter().any(|q| q.id == a.id))
        .cloned()
        .collect();
    let mut design_cards = Vec::new();
    if let Some(work) = key.strip_prefix("work:").and_then(|w| w.parse::<i64>().ok()) {
        // A design card is in the bubble of the stage that handed it in, and the one the work waits on pinned under the
        // head: its entry only counts what waits for the owner in the list.
        entries.retain(|e| e.source != "card");
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
    if let Some(project) = key.strip_prefix("architect:") {
        entries.extend(matter_cards(conn, flows, project)?);
        entries.extend(plans(conn, project)?);
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
        questions,
        waits_on,
        read_to,
        forwarding: if key == "wf-agents" {
            Some(Forwarding {
                projects: t.projects.clone(),
                auto: db::fault_forwarding(conn).map_err(|e| Error::Failed(e.to_string()))?,
            })
        } else {
            None
        },
    })
}

/// What a project's architect does now with the owner's words: the matter its work going on has in hand, with that
/// work, its latest round and the cell it is at.
struct WithWords {
    in_hand: Option<(i64, i64, Option<i64>, String)>,
}

/// What the architect of `project` does now with the owner's words, read once for a conversation.
fn with_words(conn: &Connection, project: &str) -> Result<WithWords> {
    let work: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, at FROM works WHERE graph = 'architect' AND project = ?1 AND ended IS NULL
             ORDER BY id DESC LIMIT 1",
            [project],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let mut in_hand = None;
    if let Some((w, at)) = work
        && let Some(m) = read::handling(conn, w)?
    {
        let round: Option<i64> = conn.query_row("SELECT max(n) FROM rounds WHERE work = ?1", [w], |r| r.get(0))?;
        in_hand = Some((m, w, round, at));
    }
    Ok(WithWords { in_hand })
}

/// Where the owner's words of one matter stand with the architect they were said to, once it took them up.
struct WordsNow {
    project: String,
    state: &'static str,
    /// While the architect has them in hand: its work, the work's latest round and the cell it is at.
    chart: Option<(i64, Option<i64>, String)>,
    /// For a talk a flow version before held about them, the latest entry of it a turn took.
    last_entry: Option<i64>,
}

/// Where the owner's words of `matter` stand, as [`words_states`] says it; none for a matter of another kind, or one
/// that waits in the queue untaken.
fn words_now(conn: &Connection, architects: &mut HashMap<String, WithWords>, matter: i64) -> Result<Option<WordsNow>> {
    let Some((project, handled, withdrawn, asked)) = conn
        .query_row(
            "SELECT m.project, m.handled_round IS NOT NULL, m.withdrawn IS NOT NULL,
               m.asked IS NOT NULL
                 AND NOT EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL)
             FROM architect_matters m WHERE m.id = ?1 AND m.kind = 'owner_words'",
            [matter],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let a = match architects.entry(project.clone()) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => v.insert(with_words(conn, &project)?),
    };
    let now = a.in_hand.as_ref().filter(|h| h.0 == matter);
    let state = if handled {
        "replied"
    } else if withdrawn {
        "withdrawn"
    } else if now.is_some() {
        "replying"
    } else if asked {
        "asked"
    } else {
        return Ok(None);
    };
    let chart = now.filter(|_| state == "replying").map(|h| (h.1, h.2, h.3.clone()));
    let last_entry: Option<i64> = conn.query_row(
        "SELECT max(id) FROM talk_entries WHERE matter = ?1 AND taken_round IS NOT NULL",
        [matter],
        |r| r.get(0),
    )?;
    Ok(Some(WordsNow {
        project,
        state,
        chart,
        last_entry,
    }))
}

/// Where the owner's words in a conversation stand with the architect they were said to, once it took them up, in the
/// one-to-one and in a work's group alike, for the one line under their bubble, since they have no matter card:
/// replying while it has them in hand and has not replied yet, replied once handled, asked while a question of the
/// architect's to the owner holds them, withdrawn. The line is under the owner's words; for a talk a flow version
/// before held about them, under the latest of its entries a turn took. Words it does not have yet, which their bubble
/// says are not taken up, have none, as have words said to every architect of a group at once, each of which takes them
/// on its own. Words the architect has are no longer pending.
fn words_states(conn: &Connection, entries: &mut [ConversationEntry]) -> Result<()> {
    let mut architects: HashMap<String, WithWords> = HashMap::new();
    let mut talks: HashMap<i64, Option<WordsNow>> = HashMap::new();
    for e in entries.iter_mut() {
        let owner_said = e.sender == "owner"
            && matches!(e.source.as_str(), "words" | "talk" | "interjection")
            && e.recipient != "architects";
        let Some(matter) = e.matter.filter(|_| owner_said) else {
            continue;
        };
        let now = match talks.entry(matter) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(words_now(conn, &mut architects, matter)?),
        };
        let Some(now) = now else {
            continue;
        };
        let entry = e.source == "talk";
        if entry && now.last_entry.is_none_or(|l| e.id > l) {
            continue;
        }
        // Words for the architect alone; words said to the engineer too stay pending until it takes them in.
        if e.recipient == "architect" {
            e.pending = false;
        }
        let latest = match now.last_entry {
            Some(l) => entry && e.id == l,
            None => !entry,
        };
        if !latest {
            continue;
        }
        e.words_state = Some(WordsState {
            state: now.state.into(),
            project: now.project.clone(),
            architect_work: now.chart.as_ref().map(|c| c.0),
            round: now.chart.as_ref().and_then(|c| c.1),
            cell: now.chart.as_ref().map(|c| c.2.clone()),
        });
    }
    Ok(())
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

/// The images the owner sent with the owner's words each entry holds: an interjection's own or those of the matter
/// holding its words, the words of a matter, and the owner's reply to an engineer's question; an answer to an ask has
/// its images in the ask.
fn images(conn: &Connection, entries: &mut [ConversationEntry]) -> Result<()> {
    use db::attachments::{Words, of, of_interjection};
    for e in entries.iter_mut().filter(|e| e.sender == "owner") {
        e.images = match e.source.as_str() {
            "interjection" => of_interjection(conn, e.id)?,
            "words" | "reply" => of(conn, Words::Matter(e.id))?,
            _ => Vec::new(),
        };
    }
    Ok(())
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
        images: Vec::new(),
        ask_detail: None,
        stage: None,
        matter_card: None,
        words_state: None,
        policy: None,
        plan: None,
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

/// A round of a work, as the bubbles of its stages read it.
struct Step {
    n: i64,
    cell: Option<String>,
    transition: Option<String>,
    started_at: i64,
    ended_at: Option<i64>,
    cost: f64,
    line: Option<String>,
    progress: Option<String>,
    /// What a review's round handed back, its one form, for the findings it left open.
    review: Option<Value>,
    /// The merge commit a merge handed back.
    commit: Option<String>,
}

impl Step {
    /// When it ended, or started while it runs.
    fn end(&self) -> i64 {
        self.ended_at.unwrap_or(self.started_at)
    }
}

/// A run of a work's rounds at one stage, by the places of its first and last round among the work's rounds.
struct Stretch {
    stage: String,
    from: usize,
    to: usize,
}

/// How many findings a review's return leaves open, as the merge's judge reads them: those of its columns that found
/// something or could not verify, a column of either with no finding listed counting one; and at a re-review, the
/// findings of the round before it holds not fixed or insists on.
fn open_findings(review: &Value) -> i64 {
    let columns = review["columns"].as_array().map_or(0, |cs| {
        cs.iter()
            .filter(|c| matches!(c["verdict"].as_str(), Some("findings" | "cannot_verify")))
            .map(|c| c["findings"].as_array().map_or(1, |f| f.len().max(1)) as i64)
            .sum::<i64>()
    });
    let again = review["previous"].as_array().map_or(0, |p| {
        p.iter()
            .filter(|x| matches!(x["verdict"].as_str(), Some("not_fixed" | "insist")))
            .count() as i64
    });
    columns + again
}

/// The words of a notice of the engine's about a work, as the bubble of the stage the work was at says them, which do
/// not name the work: the engine names it for its notifications log ("Work 8 stopped: ...", "Work 5 on item 3 (Parse
/// it) is paused: ...", "Main broke after work 5 (item 3: Parse it) merged ..."), while in the work's own group it goes
/// without saying, and the sentence starts at the word after it.
fn unnamed(text: &str, work: i64, item: Option<(i64, &str)>) -> String {
    let mut heads = Vec::new();
    if let Some((id, title)) = item {
        heads.push(format!("Work {work} on item {id} ({title}) is "));
        heads.push(format!("Work {work} on item {id} is "));
    }
    heads.push(format!("Work {work} "));
    let mut out = heads
        .iter()
        .find_map(|h| text.strip_prefix(h.as_str()))
        .unwrap_or(text)
        .to_string();
    if let Some((id, title)) = item {
        out = out.replace(&format!("work {work} (item {id}: {title})"), "this work");
    }
    let mut chars = out.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Whether a notice of the engine's tells how the work stands rather than of something that happened to it: a pause or
/// a stop, which the group says where it says where the work is; or what said again what the band and the head say (a
/// design card waiting, new or replacing another, the work ready to merge but held), which the engine no longer writes.
fn as_it_stands(words: &str, text: &str) -> bool {
    words.starts_with("Paused")
        || words.starts_with("Stopped")
        || (text.starts_with("Design card ") && text.contains(" of work "))
        || text.contains(" is ready to merge, but ")
}

/// The bubbles of the stages an engineer's work entered, posted when it came into each: one for each stretch of its
/// rounds at one stage that runs anything but the engine's own steps (research, design, implementation, testing and
/// fixing, the merge, watching main), from the round that came into the stage to the round that left it. A cell at no
/// stage (asking, tools, a pause, answers) is at the stage the work stopped at. A stage the work comes back to is a
/// stretch of its own, counted among the work's stretches at that stage. A stretch of the engine's own steps alone that
/// sent the work straight back to a stage before it (the merge's judge finding the reviews' findings open) has no bubble:
/// its sending back is how the stretch before it ended. Each says when it ran and what it cost; while the work is at it,
/// that it is; once over, how it ended, with why it was sent back and what its reviews found, the pull request and the
/// commit that merged the work, or why the work ended and the work that took it over; what its rounds said they did;
/// what the engine told of the work meanwhile; what its rounds handed back that the flow shows on the item; and the
/// design cards they handed in.
fn stage_cards(conn: &Connection, flows: &Flows, work: i64) -> Result<Vec<ConversationEntry>> {
    type Found = (
        i64,
        String,
        bool,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let found: Option<Found> = conn
        .query_row(
            &format!(
                "SELECT w.flow_version, w.at, w.ended IS NOT NULL OR w.at IN {}, w.ended, w.pr, w.item, i.title
                 FROM works w LEFT JOIN items i ON i.id = w.item WHERE w.id = ?1 AND w.graph = 'engineer'",
                over_sql()
            ),
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
        .optional()?;
    let Some((version, at, over, ended, pr, item, title)) = found else {
        return Ok(Vec::new());
    };
    let Some(g) = flows.graph(conn, version, "engineer") else {
        return Ok(Vec::new());
    };
    let within = format!("$.{}", g.within);
    let mut st = conn.prepare(
        "SELECT r.n, r.cell, r.transition, r.started_at, r.ended_at, coalesce(json_extract(r.usage, '$.cost_usd'), 0),
                s.line, CAST(json_extract(r.output, '$.progress') AS TEXT),
                CASE WHEN s.line = 'review' THEN json_extract(r.output, ?2) END,
                CAST(json_extract(r.output, '$.result.merge_commit') AS TEXT)
         FROM rounds r LEFT JOIN sessions s ON s.id = r.session WHERE r.work = ?1 ORDER BY r.n",
    )?;
    let steps: Vec<Step> = st
        .query_map(db::rusqlite::params![work, within], |r| {
            Ok(Step {
                n: r.get(0)?,
                cell: r.get(1)?,
                transition: r.get(2)?,
                started_at: r.get(3)?,
                ended_at: r.get(4)?,
                cost: r.get(5)?,
                line: r.get(6)?,
                progress: r.get::<_, Option<String>>(7)?.filter(|p| !p.trim().is_empty()),
                review: read::json(r.get(8)?).filter(Value::is_object),
                commit: r.get(9)?,
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
    // A stage's place in the flow: where its first cell stands among the graph's cells.
    let order = |stage: &str| {
        g.cells
            .iter()
            .position(|c| c.stage.as_deref() == Some(stage))
            .unwrap_or(usize::MAX)
    };
    let mut stretches: Vec<Stretch> = Vec::new();
    let mut now: Option<String> = None;
    for (i, s) in steps.iter().enumerate() {
        let stage = stage_of(s.cell.as_deref()).or_else(|| now.clone());
        if stage != now {
            now = stage.clone();
            if let Some(name) = stage.filter(|n| carded.contains(n)) {
                stretches.push(Stretch {
                    stage: name,
                    from: i,
                    to: i,
                });
            }
        }
        if let Some(x) = stretches.last_mut().filter(|x| Some(&x.stage) == now.as_ref()) {
            x.to = i;
        }
    }
    // The stretch the work is at now: the last, while the work goes on and its cell is at that stretch's stage.
    let at_stage = stage_of(Some(&at)).or_else(|| now.clone());
    let current = if over {
        None
    } else {
        stretches
            .len()
            .checked_sub(1)
            .filter(|&k| Some(&stretches[k].stage) == at_stage.as_ref())
    };
    // The stage a stretch sent the work to: that of the cell the row it left by leads to, else that of the stretch
    // after it; and whether that stage comes before its own.
    let went = |k: usize| -> Option<String> {
        let last = &steps[stretches[k].to];
        let to = last.cell.as_deref().and_then(|c| g.cell(c)).and_then(|c| {
            c.rows
                .iter()
                .find(|r| Some(&r.name) == last.transition.as_ref())
                .map(|r| r.to.clone())
        });
        to.and_then(|t| stage_of(Some(&t)))
            .or_else(|| stretches.get(k + 1).map(|x| x.stage.clone()))
    };
    let back = |k: usize| went(k).is_some_and(|s| order(&s) < order(&stretches[k].stage));
    let engine_only = |x: &Stretch| {
        steps[x.from..=x.to].iter().all(|s| {
            s.cell
                .as_deref()
                .and_then(|c| g.cell(c))
                .is_some_and(|c| c.kind == "engine")
        })
    };
    let folded: Vec<bool> = (0..stretches.len())
        .map(|k| k > 0 && Some(k) != current && engine_only(&stretches[k]) && back(k))
        .collect();
    // Why a row was taken: its note in the flow, else its name in words.
    let row_why = |s: &Step| -> Option<String> {
        let row = s.transition.as_deref()?;
        let note = s
            .cell
            .as_deref()
            .and_then(|c| g.cell(c))
            .and_then(|c| c.rows.iter().find(|r| r.name == row))
            .and_then(|r| r.note.clone());
        Some(note.unwrap_or_else(|| row.replace('_', " ")))
    };
    let mut st = conn
        .prepare("SELECT text, at FROM engine_notices WHERE work = ?1 AND ask IS NULL AND fault IS NULL ORDER BY id")?;
    let notices: Vec<(String, i64)> = st
        .query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<db::rusqlite::Result<_>>()?;
    let named = item.zip(title.as_deref());
    // The work that took the item over from this one, once it ended.
    let next_work: Option<i64> = match item {
        Some(i) => conn.query_row(
            "SELECT min(id) FROM works WHERE item = ?1 AND id > ?2 AND graph = 'engineer'",
            [i, work],
            |r| r.get(0),
        )?,
        None => None,
    };
    let all_cards = read::design_cards(conn, flows, "WHERE c.work = ?1", &[work])?;
    let kept: Vec<usize> = (0..stretches.len()).filter(|&k| !folded[k]).collect();
    let mut passes: HashMap<String, i64> = HashMap::new();
    let mut out = Vec::new();
    for (place, &k) in kept.iter().enumerate() {
        let x = &stretches[k];
        let rounds = &steps[x.from..=x.to];
        let first = &rounds[0];
        let last = &rounds[rounds.len() - 1];
        let pass = passes.entry(x.stage.clone()).or_insert(0);
        let again = *pass;
        *pass += 1;
        let is_current = Some(k) == current;
        let sent_by = folded
            .get(k + 1)
            .copied()
            .unwrap_or(false)
            .then(|| &steps[stretches[k + 1].to]);
        let leaving = last.cell.as_deref().and_then(|c| g.cell(c));
        let reviews = rounds.iter().any(|s| s.review.is_some());
        let (outcome, why) = if is_current {
            (None, None)
        } else if let Some(s) = sent_by {
            (Some("sent_back".to_string()), row_why(s))
        } else if place + 1 == kept.len() && ended.is_some() {
            let why = ended
                .as_deref()
                .map(|e| e.strip_prefix("stopped: ").unwrap_or(e).to_string());
            (Some("ended".to_string()), why)
        } else if back(k) {
            (Some("sent_back".to_string()), row_why(last))
        } else if let (Some(_), Some(row)) = (leaving.filter(|c| c.gate || c.kind == "tool"), &last.transition) {
            (Some(row.clone()), None)
        } else if reviews {
            (Some("no_findings".to_string()), None)
        } else {
            (Some("done".to_string()), None)
        };
        let findings = if outcome.as_deref() == Some("sent_back") {
            // The newest return of each review in the stretch, as the merge's judge reads them.
            let mut newest: BTreeMap<&str, &Step> = BTreeMap::new();
            for s in rounds {
                if let (Some(c), Some(v)) = (s.cell.as_deref(), &s.review)
                    && v["columns"].is_array()
                {
                    newest.insert(c, s);
                }
            }
            let mut list: Vec<ReviewFindings> = Vec::new();
            for (c, s) in newest {
                let n = s.review.as_ref().map_or(0, open_findings);
                if n > 0 {
                    list.push(ReviewFindings {
                        review: g.cell(c).map_or_else(|| c.to_string(), |x| x.title.clone()),
                        round: s.n,
                        findings: n,
                    });
                }
            }
            list.sort_by_key(|f| f.round);
            list
        } else {
            Vec::new()
        };
        // A sending back its reviews' findings say needs no other why.
        let why = if findings.is_empty() { why } else { None };
        let merged = outcome.as_deref() == Some("merged");
        let commit = if merged {
            rounds.iter().rev().find_map(|s| s.commit.clone())
        } else {
            None
        };
        let taken_over_by = if outcome.as_deref() == Some("ended") {
            next_work
        } else {
            None
        };
        let notes: Vec<StageNote> = rounds
            .iter()
            .filter_map(|s| {
                Some(StageNote {
                    round: s.n,
                    line: s.line.clone(),
                    text: s.progress.clone()?,
                })
            })
            .collect();
        // What the engine told of the work from when the work came into the stage to when it came into the next; what it
        // told before the first stage, with the first.
        let from = if place == 0 { i64::MIN } else { first.started_at };
        let until = kept
            .get(place + 1)
            .map_or(i64::MAX, |&j| steps[stretches[j].from].started_at);
        let events: Vec<String> = notices
            .iter()
            .filter(|(_, at)| *at >= from && *at < until)
            .filter_map(|(text, _)| {
                let words = unnamed(text, work, named);
                (!as_it_stands(&words, text)).then_some(words)
            })
            .collect();
        let card = StageCard {
            stage: x.stage.clone(),
            again,
            first: first.n,
            last: last.n,
            started_at: first.started_at,
            ended_at: if is_current { None } else { Some(last.end()) },
            cost_usd: rounds.iter().map(|s| s.cost).sum(),
            current: is_current,
            outcome,
            why,
            findings,
            pr: if merged { pr } else { None },
            commit,
            taken_over_by,
            notes,
            events,
            shown: read::shown_where(
                conn,
                flows,
                "w.id = ?1 AND r.n BETWEEN ?2 AND ?3",
                &[work, first.n, last.n],
            )?,
            design_cards: all_cards
                .iter()
                .filter(|c| c.round >= first.n && c.round <= last.n)
                .cloned()
                .collect(),
        };
        out.push(ConversationEntry {
            source: "stage".into(),
            id: place as i64 + 1,
            sender: "engineer".into(),
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
/// queue has no card: the queue in the panel beside the one-to-one lists it. The owner's words have none either: a card
/// would repeat their bubble, under which a line says where they stand (`words_states`). A matter that changed nothing
/// has none once done: the queue's list of those handled keeps it, with why. A matter done lists what it made by name
/// (`made_by`).
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
           FROM architect_matters m LEFT JOIN taken t ON t.matter = m.id
           WHERE m.project = ?1 AND m.kind <> 'owner_words')
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
        let handled = handled.get(&id).cloned();
        // A matter that changed nothing leaves the conversation once it is done; the queue in the panel beside it lists
        // it among those handled, with why.
        if handled.as_ref().is_some_and(|h| h.nothing) {
            continue;
        }
        let made = if done { made_by(conn, id)? } else { Vec::new() };
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
            handled,
            withdrawn,
            asked,
            fix: read::json(fix),
            made,
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

/// What an architect's matter made that the owner reads by name: the observations it recorded, which the engine names
/// the matter in where they came from ("... (matter 87)"), and the items its batch made, cancelled or moved in the
/// queue, each by its number and its title.
fn made_by(conn: &Connection, matter: i64) -> Result<Vec<MadeThing>> {
    let mut out = Vec::new();
    let mut st = conn.prepare("SELECT id, coalesce(title, what) FROM observations WHERE source LIKE ?1 ORDER BY id")?;
    let recorded: Vec<(i64, String)> = st
        .query_map([format!("%(matter {matter})")], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<db::rusqlite::Result<_>>()?;
    for (id, what) in recorded {
        out.push(MadeThing {
            kind: "observation".into(),
            change: "new".into(),
            id,
            title: what,
        });
    }
    let mut st = conn.prepare(
        "SELECT change, outcome FROM architect_item_changes WHERE matter = ?1 AND state = 'made' ORDER BY id",
    )?;
    let changes: Vec<(String, Option<String>)> = st
        .query_map([matter], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<db::rusqlite::Result<_>>()?;
    let number = |v: &Value| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()));
    for (change, outcome) in changes {
        let Some(c) = read::json(Some(change)) else {
            continue;
        };
        let (what, id) = match c["kind"].as_str() {
            Some("create_item") => ("new", read::json(outcome).and_then(|o| number(&o["created"]))),
            Some("cancel") => ("cancelled", number(&c["item"])),
            Some("move") => ("reordered", number(&c["item"])),
            _ => continue,
        };
        let Some(id) = id else {
            continue;
        };
        let title: Option<String> = conn
            .query_row("SELECT title FROM items WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        out.push(MadeThing {
            kind: "item".into(),
            change: what.into(),
            id,
            title: title.unwrap_or_else(|| c["title"].as_str().unwrap_or_default().to_string()),
        });
    }
    Ok(out)
}

/// The versions of the plan in a project's architect's notebook, each a bubble of the architect's in its one-to-one when
/// the round that wrote it ended: what changed from the version before, as the first line the new one has that the one
/// before had not, and the plan whole.
fn plans(conn: &Connection, project: &str) -> Result<Vec<ConversationEntry>> {
    let mut st = conn.prepare(
        "SELECT e.id, e.text, e.work, e.round, coalesce(r.ended_at, r.started_at)
         FROM notebook_edits e JOIN rounds r ON r.work = e.work AND r.n = e.round
         WHERE e.project = ?1 AND e.section = 'plan' ORDER BY e.id",
    )?;
    let rows: Vec<(i64, String, i64, i64, i64)> = st
        .query_map([project], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<db::rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    let mut before: Option<String> = None;
    for (id, text, work, round, at) in rows {
        let changed = before.as_ref().map(|old| {
            let had: BTreeSet<&str> = old.lines().map(str::trim).collect();
            text.lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !had.contains(l))
                .unwrap_or_default()
                .to_string()
        });
        out.push(ConversationEntry {
            source: "plan".into(),
            id,
            sender: "architect".into(),
            recipient: "architect".into(),
            at,
            work: Some(work),
            round: Some(round),
            plan: Some(PlanChange {
                changed: changed.filter(|c| !c.is_empty()),
                plan: text.clone(),
                work,
                round,
            }),
            ..blank()
        });
        before = Some(text);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // In the bubble of the stage a work was at, a notice of the engine's does not name the work, which the engine names
    // for its notifications log: the sentence starts at the word after the name, and a name inside it is "this work".
    #[test]
    fn a_notice_in_its_work_s_bubble_does_not_name_the_work() {
        let item = Some((3, "Parse it"));
        assert_eq!(
            unnamed("Work 5 on item 3 (Parse it) is paused: no return", 5, item),
            "Paused: no return"
        );
        assert_eq!(
            unnamed("Work 5 on item 3 is stopped by the architect: wrong way", 5, item),
            "Stopped by the architect: wrong way"
        );
        assert_eq!(
            unnamed("Work 5 stopped: the owner started it over.", 5, item),
            "Stopped: the owner started it over."
        );
        assert_eq!(
            unnamed(
                "Main broke after work 5 (item 3: Parse it) merged, and the full CI did not catch it.",
                5,
                item
            ),
            "Main broke after this work merged, and the full CI did not catch it."
        );
        // Another work's number is not this work's name.
        assert_eq!(unnamed("Work 51 stopped.", 5, item), "Work 51 stopped.");
        assert!(as_it_stands(
            "Paused: no return",
            "Work 5 on item 3 (Parse it) is paused: no return"
        ));
        assert!(!as_it_stands("Passed the limit.", "Work 5 passed the limit."));
    }
}
