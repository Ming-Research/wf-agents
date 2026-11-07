//! The owner's decisions in an item: what the architect or an engineer hands in as a decision, checked against the
//! records its words are from, and written as one line of the item's `approach` under the head `Decisions:`. The line
//! is the engine's, in one place: its date, who decided and the words quoted are not typed by the agent that hands the
//! decision in but taken from the records it names, so that a line says only what a record says.
//!
//! A decision names the records its words are from, each by where they are kept (`matter`, `ask` or `interjection`),
//! its number and the part of its words that decides: the quote must be found, as it is, in the record's words, which
//! the engine reads from the record again. Who decided is what the record says of who spoke; a part of a delegate's
//! words that it declared the owner's own (`relayed`) is told as the owner's, relayed by the delegate and not checked,
//! which no one can check, since the owner said it in the delegate's session. A decision on several records is told by
//! the least certain of its quotations, and a quotation of another source than the line's says its own.

use db::rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The head the decisions of an item stand under in its `approach`.
pub const HEAD: &str = "Decisions:";

/// What every line of a decision starts with.
pub const LINE: &str = "Decision (";

/// Who decided, as the records the words are from say it, from the most certain to the least: the owner, declared at
/// the page; the owner's own words, which a delegate that speaks for the owner marked as such in what it said, relayed
/// by the delegate and checked by no one, since the owner said them in the delegate's session and not through the
/// interface; a delegate that speaks for the owner, so that whether the owner said it is the delegate's word; a record
/// kept before who speaks was declared, which says nothing and is never told as the owner's; and the architect, whose
/// reply to an open question is its own judgment, even when it carries an answer of the owner's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Owner,
    Relayed,
    Delegate,
    Unrecorded,
    Architect,
}

impl Source {
    /// How a line of a decision names it, in its head and beside a quotation of another source.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Owner => "owner",
            Source::Relayed => "owner relayed by the delegate, not checked",
            Source::Delegate => "delegate",
            Source::Unrecorded => "unrecorded",
            Source::Architect => "architect",
        }
    }

    /// What a record says of who spoke, as the owner's words and answers keep the speaker the caller declared.
    pub fn declared(speaker: Option<&str>) -> Source {
        match speaker {
            Some("owner") => Source::Owner,
            Some("delegate") => Source::Delegate,
            _ => Source::Unrecorded,
        }
    }
}

/// A record a decision's words are from: who it says spoke, the words it holds that a quote may be found in, when they
/// were said, in milliseconds, and the parts of the words a delegate declared to be the owner's own (`relayed`; none
/// for any other speaker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub source: Source,
    pub texts: Vec<String>,
    pub at: i64,
    pub relayed: Vec<String>,
}

impl Record {
    /// Who a part of the words is told as having said it: the owner, relayed by the delegate, when a delegate said the
    /// record and declared a part of it that holds the whole of `words` the owner's own; otherwise the record's own.
    /// A part of the words that reaches beyond the declared part, or lies across two, is the delegate's.
    pub fn source_of(&self, words: &str) -> Source {
        if self.source == Source::Delegate && self.relayed.iter().any(|r| r.contains(words)) {
            Source::Relayed
        } else {
            self.source
        }
    }
}

/// The parts of the words a record declares to be the owner's own, as the record keeps them: a JSON array of strings,
/// or the text of one (the column of an interjection); none for anything else.
fn relayed_of(v: &Value) -> Vec<String> {
    let parsed;
    let v = match v {
        Value::String(s) => {
            parsed = serde_json::from_str::<Value>(s).unwrap_or(Value::Null);
            &parsed
        }
        v => v,
    };
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r.as_str().map(str::to_string))
        .collect()
}

fn strings(vs: &[&Value]) -> Vec<String> {
    vs.iter()
        .filter_map(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// The record a decision names, of a project: the owner's words, the owner's answer to an engineer's question or an
/// answer to an open question (`matter`), the owner's answer to an ask (`ask`), or what the owner or the architect said
/// to a work (`interjection`). An error says why it is none: there is no such record, it is another project's, it holds
/// no words a decision may quote, or it keeps no time to date the decision by.
pub fn record(conn: &Connection, project: &str, from: &str, id: i64) -> Result<Record, String> {
    match from {
        "matter" => {
            type Row = (String, String, Option<String>, String, i64, Option<String>);
            let row: Option<Row> = conn
                .query_row(
                    "SELECT project, kind, detail, body, at, reply FROM architect_matters WHERE id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .optional()
                .map_err(sql)?;
            let Some((of, kind, detail, body, at, reply)) = row else {
                return Err(format!("there is no matter {id}"));
            };
            if of != project {
                return Err(format!("matter {id} is of {of}, and the decision is of {project}"));
            }
            let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let reply: Value = reply.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or(Value::Null);
            match (kind.as_str(), detail.as_deref()) {
                ("owner_words", Some("release")) => Err(format!(
                    "matter {id} is the engine's note that the owner started a work, not words the owner said"
                )),
                ("owner_words", _) => Ok(Record {
                    source: Source::declared(body["speaker"].as_str()),
                    texts: strings(&[&body["text"], &body["answer"]]),
                    at,
                    relayed: relayed_of(&body["relayed"]),
                }),
                ("open_question", _) => {
                    let texts = strings(&[&body["answer"]]);
                    if texts.is_empty() {
                        return Err(format!("open question {id} has no answer yet"));
                    }
                    // The time of the answer, not of the question: a decision is dated by when its words were said.
                    let Some(at) = body["answered_at"].as_i64() else {
                        return Err(format!("open question {id} has no recorded time of its answer"));
                    };
                    Ok(Record {
                        source: Source::Architect,
                        texts,
                        at,
                        relayed: Vec::new(),
                    })
                }
                // The owner's answer in the work's group to a question of the engineer's is the matter's reply.
                ("engineer_question", _) if reply["by"] == "owner" => {
                    let Some(at) = reply["at"].as_i64() else {
                        return Err(format!("the owner's answer to question {id} has no recorded time"));
                    };
                    Ok(Record {
                        source: Source::declared(reply["speaker"].as_str()),
                        texts: strings(&[&reply["text"]]),
                        at,
                        relayed: relayed_of(&reply["relayed"]),
                    })
                }
                ("engineer_question", _) => Err(format!(
                    "question {id} has no answer of the owner's: only the owner's words hold the words of a decision"
                )),
                (other, _) => Err(format!(
                    "matter {id} is of the kind {other}: only the owner's words, the owner's answer to an engineer's \
                     question and the answer to an open question hold the words of a decision"
                )),
            }
        }
        "ask" => {
            let row: Option<(Option<String>, Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT a.answer, w.project, i.project FROM work_asks a JOIN works w ON w.id = a.work
                     LEFT JOIN items i ON i.id = w.item WHERE a.id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .map_err(sql)?;
            let Some((answer, wp, ip)) = row else {
                return Err(format!("there is no ask {id}"));
            };
            let of = wp.or(ip).unwrap_or_default();
            if of != project {
                return Err(format!("ask {id} is of {of}, and the decision is of {project}"));
            }
            let answer: Value = answer
                .and_then(|a| serde_json::from_str(&a).ok())
                .unwrap_or(Value::Null);
            if answer["by"] != "owner" {
                return Err(format!("ask {id} has no answer of the owner's"));
            }
            let Some(at) = answer["at"].as_i64() else {
                return Err(format!("the answer of ask {id} has no recorded time"));
            };
            Ok(Record {
                source: Source::declared(answer["speaker"].as_str()),
                texts: strings(&[&answer["text"]]),
                at,
                relayed: relayed_of(&answer["relayed"]),
            })
        }
        "interjection" => {
            type Row = (
                String,
                String,
                Option<i64>,
                Option<String>,
                Option<String>,
                i64,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            );
            let row: Option<Row> = conn
                .query_row(
                    "SELECT j.speaker, j.text, j.matter, w.project, i.project, j.at,
                       (SELECT json_extract(m.body, '$.speaker') FROM architect_matters m WHERE m.id = j.matter),
                       j.declared,
                       (SELECT json_extract(m.body, '$.relayed') FROM architect_matters m WHERE m.id = j.matter),
                       j.relayed
                     FROM line_sayings j JOIN works w ON w.id = j.work LEFT JOIN items i ON i.id = w.item
                     WHERE j.id = ?1",
                    [id],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                            r.get(6)?,
                            r.get(7)?,
                            r.get(8)?,
                            r.get(9)?,
                        ))
                    },
                )
                .optional()
                .map_err(sql)?;
            let Some((speaker, text, matter, wp, ip, at, of_matter, declared, matter_relayed, own_relayed)) = row
            else {
                return Err(format!("there is no interjection {id}"));
            };
            let of = wp.or(ip).unwrap_or_default();
            if of != project {
                return Err(format!(
                    "interjection {id} is of {of}, and the decision is of {project}"
                ));
            }
            // Words a matter holds say who spoke as the matter does; those said to the engineer alone, as the row does.
            let source = match (speaker.as_str(), matter) {
                ("architect", _) => Source::Architect,
                (_, Some(_)) => Source::declared(of_matter.as_deref()),
                _ => Source::declared(declared.as_deref()),
            };
            // The parts a delegate declared come from where it declared them, as for who spoke.
            let relayed = match (speaker.as_str(), matter) {
                ("architect", _) => None,
                (_, Some(_)) => matter_relayed,
                _ => own_relayed,
            };
            Ok(Record {
                source,
                texts: strings(&[&Value::String(text)]),
                at,
                relayed: relayed.map(|r| relayed_of(&Value::String(r))).unwrap_or_default(),
            })
        }
        other => Err(format!(
            "a decision's words are from a matter, an ask or an interjection, not from {other}"
        )),
    }
}

/// A text on one line, its runs of white space each one space.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A field of a line: on one line, without the sentence terminal it ends with, since the line puts its own.
fn field(s: &str) -> String {
    let f = flat(s);
    db::without_final_terminal(&f).trim_end().to_string()
}

/// The date of a time in milliseconds, `YYYY-MM-DD`, in UTC.
pub fn date_of(ms: i64) -> String {
    crate::utc(ms).chars().take(10).collect()
}

/// One piece of a decision's words as the record has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub from: String,
    pub id: i64,
    pub words: String,
    pub at: i64,
    /// Who the records say said these words: the record's speaker, or the owner relayed by the delegate for a part of
    /// a delegate's words that it declared the owner's own.
    pub source: Source,
}

impl Said {
    /// How the line quotes it, which the check that a record is not quoted twice finds again.
    fn quoted(&self) -> String {
        format!("\"{}\" ({} {}, ", flat(&self.words), self.from, self.id)
    }
}

/// A decision as the engine understands it once its records are read: who decided, when, the words from the records and
/// the line that goes into the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub said: Vec<Said>,
    pub line: String,
}

fn text<'a>(d: &'a Value, k: &str) -> &'a str {
    d[k].as_str().unwrap_or_default().trim()
}

/// The records a decision quotes, each as `(from, id)` with its quote; an entry that names no record is skipped here
/// and said by [`resolve`].
pub fn quotes(d: &Value) -> Vec<(String, i64, String)> {
    d["words"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| {
            (
                text(w, "from").to_string(),
                w["id"].as_i64().unwrap_or_default(),
                text(w, "quote").to_string(),
            )
        })
        .collect()
}

/// A decision read against the records: its words found in them, who decided and when, and the line that records it,
/// or every reason it cannot be recorded. `project` is the project the decision is of, `writer` who writes a reason the
/// decision does not give in so many words (`architect` or `engineer`), and `item` the item it is recorded in, when it
/// is one that exists: a record already quoted there by the same words is recorded already.
pub fn resolve(
    conn: &Connection,
    project: &str,
    d: &Value,
    writer: &str,
    item: Option<i64>,
) -> Result<Resolved, Vec<String>> {
    let mut problems = Vec::new();
    for k in ["what", "situation", "reason", "replaces"] {
        if text(d, k).is_empty() {
            problems.push(format!(
                "the decision has no {k}{}",
                if k == "replaces" {
                    ": say none when it replaces nothing"
                } else {
                    ""
                }
            ));
        }
    }
    let by = text(d, "reason_by");
    if by != "said" && by != "writer" {
        problems.push(format!("reason_by is said or writer, not \"{by}\""));
    }
    let approach: Option<String> = match item {
        Some(i) => conn
            .query_row("SELECT approach FROM items WHERE id = ?1", [i], |r| r.get(0))
            .optional()
            .map_err(|e| vec![sql(e)])?
            .flatten(),
        None => None,
    };
    let recorded: Vec<String> = approach
        .as_deref()
        .unwrap_or_default()
        .lines()
        .filter(|l| is_line(l))
        .map(str::to_string)
        .collect();
    let mut said: Vec<Said> = Vec::new();
    let mut source: Option<Source> = None;
    let words = quotes(d);
    if words.is_empty() {
        problems.push("the decision quotes no words: name the record and the part of its words that decides".into());
    }
    for (from, id, quote) in words {
        let record = match record(conn, project, &from, id) {
            Ok(r) => r,
            Err(why) => {
                problems.push(why);
                continue;
            }
        };
        if quote.is_empty() {
            problems.push(format!("the quote from {from} {id} is empty"));
            continue;
        }
        // The words are the record's own, found as the quote has them: nothing is normalised, so a full-width mark is
        // not a half-width one. What is written is the record's text, not the quote.
        let found = record
            .texts
            .iter()
            .find_map(|t| t.find(&quote).map(|at| t[at..at + quote.len()].to_string()));
        let Some(found) = found else {
            let held = record
                .texts
                .iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(" and ");
            problems.push(format!(
                "\"{quote}\" is not found as it is in the words of {from} {id}, which are {held}: quote a part of \
                 them letter for letter"
            ));
            continue;
        };
        let one = Said {
            from: from.clone(),
            id,
            source: record.source_of(&found),
            words: found,
            at: record.at,
        };
        if said.iter().any(|s| s.quoted() == one.quoted()) {
            problems.push(format!(
                "\"{}\" of {from} {id} is quoted twice in this decision",
                one.words
            ));
        } else if recorded.iter().any(|l| l.contains(&one.quoted())) {
            problems.push(format!(
                "\"{}\" of {from} {id} is recorded in the item already, in a line of its decisions: a second decision \
                 from the same record quotes another part of its words",
                one.words
            ));
        }
        source = Some(source.map_or(one.source, |s| s.max(one.source)));
        said.push(one);
    }
    // The line's head is the least certain of the sources of what it quotes.
    let source = source.unwrap_or(Source::Unrecorded);
    if by == "writer" && source == Source::Architect {
        problems.push(
            "the reason is the architect's own answer, so there is no one to have accepted a reason you wrote: give the \
             reason as the answer gives it, reason_by said"
                .into(),
        );
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let at = said.iter().map(|s| s.at).max().unwrap_or_default();
    let date = date_of(at);
    // Who accepted the reason is the one the records say spoke, never someone more certain than they are.
    let by = if by == "writer" {
        let accepted = match source {
            Source::Owner => "the owner",
            Source::Relayed => "the owner, relayed by the delegate",
            Source::Delegate => "the delegate",
            Source::Unrecorded | Source::Architect => "whoever spoke, not recorded",
        };
        format!(" ({writer}'s, accepted by {accepted})")
    } else {
        String::new()
    };
    let quoted = said
        .iter()
        .map(|s| {
            // A quotation of another source than the line's head says its own, after its date.
            let own = if s.source == source {
                String::new()
            } else {
                format!(", {}", s.source.as_str())
            };
            format!("\"{}\" ({} {}, {}{own})", flat(&s.words), s.from, s.id, date_of(s.at))
        })
        .collect::<Vec<_>>()
        .join("; ");
    let line = format!(
        "{LINE}{date}, {}): {}. Situation: {}. Reason{by}: {}. Replaces: {}. Words: {quoted}",
        source.as_str(),
        field(text(d, "what")),
        field(text(d, "situation")),
        field(text(d, "reason")),
        field(text(d, "replaces")),
    );
    Ok(Resolved { said, line })
}

/// Whether a line of a text is a line of a decision: it starts with [`LINE`], leading blanks and one list mark `- `
/// aside. One rule for every check, so that a line indented or listed by hand is neither written nor missed by one of
/// them.
pub fn is_line(line: &str) -> bool {
    let l = line.trim_start();
    l.strip_prefix("- ").unwrap_or(l).starts_with(LINE)
}

/// Who decided, as the head of a line of a decision says it: the words between its date and the parenthesis that
/// closes the head, such as `owner` or `architect`; empty for a line whose head says none.
pub fn decided_by(line: &str) -> String {
    let l = line.trim_start();
    let l = l.strip_prefix("- ").unwrap_or(l);
    l.strip_prefix(LINE)
        .and_then(|rest| rest.split_once(')'))
        .and_then(|(head, _)| head.split_once(", "))
        .map(|(_, by)| by.trim().to_string())
        .unwrap_or_default()
}

/// The prefix of the numbers the decisions of an item are listed by, for a design to answer each: L1, L2, …
pub const LISTED: &str = "L";

/// The decisions an item's `approach` holds, in their order, as a design is given them to answer each: the number it
/// answers it by (`decision`, L1 on), the line as it stands (`line`) and who decided as its head says (`by`). Empty for
/// an item with none.
pub fn listed(approach: &str) -> Vec<Value> {
    approach
        .lines()
        .filter(|l| is_line(l))
        .enumerate()
        .map(|(i, l)| {
            json!({
                "decision": format!("{LISTED}{}", i + 1),
                "line": l.trim(),
                "by": decided_by(l),
            })
        })
        .collect()
}

/// Whether a line of an item's text is a head: it ends in a colon, which a line of a decision never does.
fn is_head(line: &str) -> bool {
    line.trim_end().ends_with(':')
}

/// An item's `approach` with a line of a decision put under its head `Decisions:`, after the last line of the block
/// under it, which ends at a blank line or the next head. With no such head it is made, after what `Approach:` holds and
/// before `Learned:`, or at the end of the text with no `Learned:`; the text with none at all is the head and the line.
/// Every other byte stands as it was.
pub fn insert(approach: &str, line: &str) -> String {
    if approach.trim().is_empty() {
        return format!("{HEAD}\n{line}");
    }
    let mut lines: Vec<&str> = approach.split('\n').collect();
    if let Some(head) = lines.iter().position(|l| l.trim_end() == HEAD) {
        let mut at = head + 1;
        while at < lines.len() && !lines[at].trim().is_empty() && !is_head(lines[at]) {
            at += 1;
        }
        lines.insert(at, line);
        return lines.join("\n");
    }
    // After the last line that is not blank before `Learned:`, or in the whole text with none.
    let end = lines
        .iter()
        .position(|l| l.trim_end() == "Learned:")
        .unwrap_or(lines.len());
    let mut at = end;
    while at > 0 && lines[at - 1].trim().is_empty() {
        at -= 1;
    }
    lines.insert(at, line);
    lines.insert(at, HEAD);
    lines.join("\n")
}

/// An item's `approach` with the lines of decisions another item's `approach` holds and it does not, put under its
/// head in the order the other has them, as when the item is kept in a merge of the other into it. Every other byte
/// stands as it was; the text itself when there is nothing to carry.
pub fn carry(approach: &str, from: &str) -> String {
    let mut out = approach.to_string();
    for l in from.lines().filter(|l| is_line(l)) {
        if !out.lines().any(|had| had == l) {
            out = insert(&out, l);
        }
    }
    out
}

/// The `approach` of the items a batch of changes touches as the batch leaves it so far, which the check of a change
/// and the write of it both judge a text by: an item's text as the database has it, with the lines of decisions a
/// merge of the batch carried into it and the text an earlier edit of the batch gave it.
#[derive(Default)]
pub struct Approaches {
    now: std::collections::BTreeMap<i64, String>,
}

impl Approaches {
    /// The text of an item's `approach` now: the batch's, or the database's with none; empty with no such item.
    pub fn of(&self, conn: &Connection, item: i64) -> String {
        if let Some(t) = self.now.get(&item) {
            return t.clone();
        }
        conn.query_row("SELECT approach FROM items WHERE id = ?1", [item], |r| {
            r.get::<_, Option<String>>(0)
        })
        .ok()
        .flatten()
        .unwrap_or_default()
    }

    /// An item is kept in a merge of another into it: the lines of decisions the other holds that it does not are its.
    pub fn merged(&mut self, conn: &Connection, keep: i64, absorbed: i64) {
        let carried = carry(&self.of(conn, keep), &self.of(conn, absorbed));
        self.now.insert(keep, carried);
    }

    /// An edit of the batch writes an item's `approach` whole.
    pub fn written(&mut self, item: i64, text: &str) {
        self.now.insert(item, text.to_string());
    }
}

/// Whether a text holds a line that starts a decision which an earlier text of the same item does not hold: such a line
/// is written by the engine from a decision, never by hand.
pub fn new_decision_lines(before: &str, after: &str) -> Vec<String> {
    let had: Vec<&str> = before.lines().collect();
    after
        .lines()
        .filter(|l| is_line(l) && !had.contains(l))
        .map(str::to_string)
        .collect()
}

/// What a text that replaces an item's does not keep of the decisions the item has: each line of a decision it leaves
/// out, the head `Decisions:` when it drops it, and the order of the lines when it moves them. The lines the item has
/// stand letter for letter, in the order they were recorded; an empty list when it keeps them all.
pub fn unkept(before: &str, after: &str) -> Vec<String> {
    let lines = |t: &str| -> Vec<String> { t.lines().filter(|l| is_line(l)).map(str::to_string).collect() };
    let (had, now) = (lines(before), lines(after));
    let mut out: Vec<String> = had
        .iter()
        .filter(|l| !now.contains(l))
        .map(|l| format!("it leaves out the line \"{l}\" of a decision the item has"))
        .collect();
    let head = |t: &str| t.lines().any(|l| l.trim_end() == HEAD);
    if !had.is_empty() && head(before) && !head(after) {
        out.push(format!(
            "it leaves out the head {HEAD} the item's decisions stand under"
        ));
    }
    // The lines it keeps, in the order the item has them.
    let kept: Vec<&String> = had.iter().filter(|l| now.contains(l)).collect();
    let in_text: Vec<&String> = now.iter().filter(|l| had.contains(l)).collect();
    if kept != in_text {
        out.push("it moves the lines of the item's decisions out of the order they were recorded in".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::rusqlite::params;
    use db::{Actor, Db};

    /// A database with a project's records a decision may quote: matter 1 the owner's words said at the page, 2 the same
    /// said by a delegate, 3 words kept before who speaks was declared, 4 the answer to an open question, 5 the words
    /// of another project, 6 words of the engine's note, 7 to 10 questions of an engineer or an open question (see
    /// below), ask 1 answered by the owner with an item's work, ask 2 answered with no time kept, and interjections 4
    /// and 5 said to that work's engineer alone, by a delegate and with no speaker kept: the hub numbers its asks and
    /// the words said to a line as one.
    fn records() -> Db {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            let matter = |project: &str, kind: &str, detail: &str, body: &str, at: i64| {
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, detail, what, body, at) VALUES (?1, ?2, ?3, 'w', ?4, ?5)",
                    params![project, kind, detail, body, at],
                )
            };
            // 2026-11-02 12:00 UTC.
            let day = 1_793_620_800_000;
            matter("p", "owner_words", "said", r#"{"text":"No browser check. Keep it small.","where":"page","speaker":"owner"}"#, day)?;
            matter("p", "owner_words", "said", r#"{"text":"Yes, do that.","where":"page","speaker":"delegate"}"#, day)?;
            matter("p", "owner_words", "said", r#"{"text":"Old words.","where":"page"}"#, day + 86_400_000)?;
            // Raised on the day, answered two days after it.
            matter("p", "open_question", "", &format!(r#"{{"question":"Which?","answer":"Option one: skip the check.\n\nIt is small.","answered_at":{}}}"#, day + 2 * 86_400_000), day)?;
            matter("q", "owner_words", "said", r#"{"text":"Elsewhere.","where":"page","speaker":"owner"}"#, day)?;
            matter("p", "owner_words", "release", r#"{"text":"I started work 3."}"#, day)?;
            // 7: the owner's answer to an engineer's question, declared by a delegate, on the day after; 8: the architect's
            // reply to one; 9: a question with no reply; 10: an open question answered before its time was kept.
            for _ in 0..3 {
                matter("p", "engineer_question", "split", "{}", day)?;
            }
            matter("p", "open_question", "", r#"{"question":"Which?","answer":"Yes."}"#, day)?;
            // 11: words a delegate said, one part of them declared the owner's own; 12: an engineer's question the
            // owner answered through a delegate, with a part declared; 13: the words a delegate said to the architect,
            // which interjection 7 copies to the engineer.
            matter("p", "owner_words", "said", r#"{"text":"Yes. Keep the parser small. Then ship it.","where":"page","speaker":"delegate","relayed":["Keep the parser small."]}"#, day)?;
            matter("p", "engineer_question", "split", "{}", day)?;
            matter("p", "owner_words", "said", r#"{"text":"Hold the merge. Then ask me.","where":"work","work":1,"speaker":"delegate","relayed":["Hold the merge."]}"#, day)?;
            tx.execute(
                "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
                params![12, format!(r#"{{"by":"owner","at":{day},"text":"Split it. Two items.","speaker":"delegate","relayed":["Two items."]}}"#)],
            )?;
            tx.execute(
                "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
                params![7, format!(r#"{{"by":"owner","at":{},"text":"Split it in two.","speaker":"delegate"}}"#, day + 86_400_000)],
            )?;
            tx.execute(
                "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
                params![8, r#"{"by":"architect","at":1,"text":"No."}"#],
            )?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
                [],
            )?;
            tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'A3')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'I1', '{}', 'c', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO messages (kind, sender, work, round, text, body, answer, at) VALUES ('ask', 'engineer', 1, 1, '', '{}', ?1, 0)",
                [format!(r#"{{"by":"owner","at":{day},"text":"Keep the old name.","speaker":"owner"}}"#)],
            )?;
            // Ask 2 has no time kept; interjection 4 was said by a delegate to the engineer alone, 5 with no speaker kept.
            tx.execute(
                "INSERT INTO messages (kind, sender, work, round, text, body, answer, at) VALUES ('ask', 'engineer', 1, 1, '', '{}', ?1, 0)",
                [r#"{"by":"owner","text":"No time.","speaker":"owner"}"#],
            )?;
            tx.execute(
                "INSERT INTO messages (kind, sender, work, round, text, body, answer, at) VALUES ('ask', 'engineer', 1, 1, '', '{}', ?1, 0)",
                [format!(r#"{{"by":"owner","at":{day},"text":"Approved. Go on.","speaker":"delegate","relayed":["Go on."]}}"#)],
            )?;
            fn owner(text: &str) -> db::ToLine<'_> {
                db::ToLine::new(db::Sender::Owner, 1, "engineer", text)
            }
            db::say_to_line(
                tx,
                &db::ToLine {
                    speaker: Some("delegate"),
                    ..owner("Say it to the engineer.")
                },
                None,
            )?;
            db::say_to_line(tx, &owner("Old words to the engineer."), None)?;
            // Interjection 6 is said to the engineer alone with a part declared; 7 is the copy of matter 13's words.
            db::say_to_line(
                tx,
                &db::ToLine {
                    speaker: Some("delegate"),
                    relayed: Some(json!(["Hold it."])),
                    ..owner("Fine. Hold it. Wait.")
                },
                None,
            )?;
            db::say_to_line(
                tx,
                &db::ToLine {
                    informs: true,
                    ..owner("Said to the architect, for you to know: Hold the merge. Then ask me.")
                },
                Some(13),
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    fn decision(words: &[(&str, i64, &str)]) -> Value {
        json!({
            "what": "No browser check for this item.",
            "situation": "The item adds a diagram fit.",
            "reason": "It is small.",
            "reason_by": "said",
            "replaces": "none",
            "words": words.iter().map(|(f, i, q)| json!({ "from": f, "id": i, "quote": q })).collect::<Vec<_>>(),
        })
    }

    /// Who a decision's line says decided, from its head: `Decision (<date>, <who>):`.
    fn head_source(r: &Resolved) -> &'static str {
        let rest = r.line.split_once(", ").map_or("", |x| x.1);
        [
            Source::Owner,
            Source::Relayed,
            Source::Delegate,
            Source::Unrecorded,
            Source::Architect,
        ]
        .into_iter()
        .map(Source::as_str)
        .find(|s| rest.starts_with(&format!("{s}):")))
        .unwrap_or_default()
    }

    // Who decided is what the record says: the owner's words as declared at the page, a delegate's as the delegate's,
    // words kept before the speaker was declared as unrecorded, an answer to an open question as the architect's, and
    // an ask's answer as it was declared. A decision on several records is told by the least certain of them.
    #[test]
    fn who_decided_is_what_the_record_says_and_the_least_certain_of_several_holds() {
        let db = records();
        let who = |words: &[(&str, i64, &str)]| {
            head_source(&resolve(db.conn(), "p", &decision(words), "architect", None).unwrap())
        };
        assert_eq!(who(&[("matter", 1, "No browser check")]), "owner");
        assert_eq!(who(&[("matter", 2, "Yes, do that.")]), "delegate");
        assert_eq!(who(&[("matter", 3, "Old words.")]), "unrecorded");
        assert_eq!(who(&[("matter", 4, "skip the check")]), "architect");
        assert_eq!(who(&[("ask", 1, "Keep the old name.")]), "owner");
        // One record that says less decides for all of them, whichever order they come in.
        assert_eq!(who(&[("matter", 1, "Keep it small"), ("matter", 2, "Yes")]), "delegate");
        assert_eq!(who(&[("matter", 3, "Old"), ("matter", 2, "Yes")]), "unrecorded");
        assert_eq!(
            who(&[
                ("matter", 4, "small"),
                ("matter", 3, "Old"),
                ("matter", 1, "No browser")
            ]),
            "architect"
        );
        assert_eq!(who(&[("matter", 1, "No browser"), ("ask", 1, "Keep")]), "owner");
    }

    // The owner's answer to an engineer's question and the words said to the engineer alone are records of their own,
    // each telling who spoke as it was declared; the architect's reply to a question and a question with none are not
    // the owner's words.
    #[test]
    fn answers_to_the_engineer_and_words_said_to_it_alone_tell_who_spoke_as_declared() {
        let db = records();
        let who = |from: &str, id: i64, quote: &str| {
            resolve(db.conn(), "p", &decision(&[(from, id, quote)]), "architect", None).map(|r| head_source(&r))
        };
        assert_eq!(who("matter", 7, "in two"), Ok("delegate"));
        assert_eq!(who("interjection", 4, "to the engineer"), Ok("delegate"));
        assert_eq!(who("interjection", 5, "Old words"), Ok("unrecorded"));
        let refused = |from: &str, id: i64| {
            resolve(db.conn(), "p", &decision(&[(from, id, "x")]), "architect", None)
                .unwrap_err()
                .join("\n")
        };
        assert!(refused("matter", 8).contains("has no answer of the owner's"));
        assert!(refused("matter", 9).contains("has no answer of the owner's"));
        // The answer is dated by when the owner gave it, not by when the engineer asked.
        let line = resolve(
            db.conn(),
            "p",
            &decision(&[("matter", 7, "Split it")]),
            "architect",
            None,
        )
        .unwrap()
        .line;
        assert!(line.starts_with("Decision (2026-11-03, delegate)"), "{line}");
    }

    // A part of a delegate's words that it declared to be the owner's own is told as the owner's, relayed by the delegate
    // and not checked; the rest of the words, a part that reaches beyond the declared part and one across two are the
    // delegate's. Each of the five kinds of record that keeps the declaration is read: the owner's words, an answer to an
    // engineer's question, an ask's answer, words said to the engineer alone, and the copy of the words said to the
    // architect that a matter holds.
    #[test]
    fn a_part_a_delegate_relayed_is_the_owner_s_relayed_and_the_rest_the_delegate_s() {
        let db = records();
        let relayed = "owner relayed by the delegate, not checked";
        let sources = |words: &[(&str, i64, &str)]| -> Vec<&'static str> {
            resolve(db.conn(), "p", &decision(words), "architect", None)
                .unwrap()
                .said
                .iter()
                .map(|s| s.source.as_str())
                .collect()
        };
        // Matter 11: the declared part, a piece of it, the words around it, across its edge, and across both edges.
        assert_eq!(sources(&[("matter", 11, "Keep the parser small.")]), [relayed]);
        assert_eq!(sources(&[("matter", 11, "parser small")]), [relayed]);
        assert_eq!(sources(&[("matter", 11, "Then ship it.")]), ["delegate"]);
        assert_eq!(sources(&[("matter", 11, "Yes. Keep the parser small.")]), ["delegate"]);
        assert_eq!(sources(&[("matter", 11, "small. Then")]), ["delegate"]);
        let all = "Yes. Keep the parser small. Then ship it.";
        assert_eq!(sources(&[("matter", 11, all)]), ["delegate"]);
        // The other four kinds of record.
        assert_eq!(sources(&[("matter", 12, "Two items.")]), [relayed]);
        assert_eq!(sources(&[("matter", 12, "Split it.")]), ["delegate"]);
        assert_eq!(sources(&[("ask", 3, "Go on.")]), [relayed]);
        assert_eq!(sources(&[("ask", 3, "Approved.")]), ["delegate"]);
        assert_eq!(sources(&[("interjection", 6, "Hold it.")]), [relayed]);
        assert_eq!(sources(&[("interjection", 6, "Fine.")]), ["delegate"]);
        assert_eq!(sources(&[("interjection", 7, "Hold the merge.")]), [relayed]);
        assert_eq!(sources(&[("interjection", 7, "Then ask me.")]), ["delegate"]);
        // Words nobody declared a delegate's own, the owner's at the page and words kept before who spoke was declared
        // have no relayed part, whatever they hold.
        let owner = Record {
            source: Source::Owner,
            texts: vec!["Keep it.".into()],
            at: 0,
            relayed: vec!["Keep it.".into()],
        };
        assert_eq!(owner.source_of("Keep it."), Source::Owner);
        assert_eq!(sources(&[("matter", 2, "Yes, do that.")]), ["delegate"]);
        assert_eq!(sources(&[("matter", 3, "Old words.")]), ["unrecorded"]);
    }

    // A line's head is the least certain source of what it quotes, with the owner's relayed words between the owner's own
    // and the delegate's; a quotation of another source than the head's says its own after its date, and the head tells
    // who accepted a reason the writer gave.
    #[test]
    fn the_line_names_the_source_of_each_quotation_that_is_not_its_head_s() {
        let db = records();
        let relayed = "owner relayed by the delegate, not checked";
        let line = |words: &[(&str, i64, &str)], by: &str| {
            let mut d = decision(words);
            d["reason_by"] = json!(by);
            resolve(db.conn(), "p", &d, "engineer", None).unwrap().line
        };
        let small = ("matter", 11, "Keep the parser small.");
        let words_of = |l: &str| l.split("Words: ").nth(1).unwrap_or_default().to_string();
        // The owner's own words and a relayed part: the head is the relayed, and the owner's quotation says so.
        let l = line(&[small, ("matter", 1, "No browser check")], "said");
        assert!(l.starts_with(&format!("Decision (2026-11-02, {relayed}): ")), "{l}");
        let quoted = "\"Keep the parser small.\" (matter 11, 2026-11-02";
        assert_eq!(
            words_of(&l),
            format!("{quoted}); \"No browser check\" (matter 1, 2026-11-02, owner)")
        );
        // A relayed part and the rest of the same message: the delegate's is the head, the relayed part says its own.
        let l = line(&[small, ("matter", 11, "Then ship it.")], "said");
        assert!(l.starts_with("Decision (2026-11-02, delegate): "), "{l}");
        assert_eq!(
            words_of(&l),
            format!("{quoted}, {relayed}); \"Then ship it.\" (matter 11, 2026-11-02)")
        );
        // Only a relayed part: no quotation names a source of its own, and the writer's reason was accepted as relayed.
        let l = line(&[small], "writer");
        let accepted = "Reason (engineer's, accepted by the owner, relayed by the delegate): ";
        assert!(l.contains(accepted), "{l}");
        assert_eq!(words_of(&l), format!("{quoted})"));
    }

    // A decision is dated by when its words were said. An open question's answer comes after the question, so the date
    // is the answer's; a record with no time kept is refused, never dated to the epoch.
    #[test]
    fn a_decision_is_dated_by_its_words_and_a_record_with_no_time_is_refused() {
        let db = records();
        let line = resolve(
            db.conn(),
            "p",
            &decision(&[("matter", 4, "skip the check")]),
            "architect",
            None,
        )
        .unwrap()
        .line;
        assert!(line.starts_with("Decision (2026-11-04, architect)"), "{line}");
        assert!(line.ends_with("(matter 4, 2026-11-04)"), "{line}");
        let refused = |from: &str, id: i64, quote: &str| {
            resolve(db.conn(), "p", &decision(&[(from, id, quote)]), "architect", None)
                .unwrap_err()
                .join("\n")
        };
        assert!(refused("matter", 10, "Yes.").contains("has no recorded time of its answer"));
        assert!(refused("ask", 2, "No time.").contains("has no recorded time"));
    }

    // The words written are the record's, and the line is the engine's: the date is the latest of the words, each quote
    // carries its record and its date, and a reason the writer gave says who wrote it. The same decision gives the same
    // line whoever hands it in.
    #[test]
    fn the_line_is_made_from_the_records() {
        let db = records();
        let mut d = decision(&[("matter", 1, "  No browser check. "), ("matter", 3, "Old words.")]);
        d["reason_by"] = json!("writer");
        d["what"] = json!("No browser check\nfor this item.");
        let r = resolve(db.conn(), "p", &d, "architect", None).unwrap();
        assert_eq!(
            r.line,
            "Decision (2026-11-03, unrecorded): No browser check for this item. Situation: The item adds a diagram fit. \
             Reason (architect's, accepted by whoever spoke, not recorded): It is small. Replaces: none. \
             Words: \"No browser check.\" (matter 1, 2026-11-02, owner); \"Old words.\" (matter 3, 2026-11-03)"
        );
        assert_eq!(r.said[0].words, "No browser check.");
        // An engineer's decision names the engineer as the one who wrote the reason.
        let e = resolve(db.conn(), "p", &d, "engineer", None).unwrap();
        assert!(
            e.line
                .contains("Reason (engineer's, accepted by whoever spoke, not recorded)"),
            "{}",
            e.line
        );
        // The one who accepted the reason is the one the record says spoke: the owner, or a delegate.
        for (matter, quote, accepted) in [
            (1, "No browser check.", "the owner"),
            (2, "Yes, do that.", "the delegate"),
        ] {
            let mut d = decision(&[("matter", matter, quote)]);
            d["reason_by"] = json!("writer");
            let line = resolve(db.conn(), "p", &d, "engineer", None).unwrap().line;
            assert!(
                line.contains(&format!("(engineer's, accepted by {accepted})")),
                "{line}"
            );
        }
    }

    // A quote must be found as it is, in the words of a record that exists, is the project's and holds a decision's
    // words; each reason is told at once, with the words the record holds, so that it can be quoted again as they are.
    #[test]
    fn a_quote_that_is_not_the_record_s_is_refused() {
        let db = records();
        let refused = |words: &[(&str, i64, &str)]| {
            resolve(db.conn(), "p", &decision(words), "architect", None)
                .unwrap_err()
                .join("\n")
        };
        // Full-width and half-width marks are not the same letters, and nothing is normalised.
        let wide = refused(&[("matter", 1, "No browser check\u{ff0e} Keep it small.")]);
        assert!(
            wide.contains("is not found as it is") && wide.contains("\"No browser check. Keep it small.\""),
            "{wide}"
        );
        assert!(refused(&[("matter", 1, "no browser check")]).contains("is not found"));
        assert!(refused(&[("matter", 1, "   ")]).contains("is empty"));
        assert!(refused(&[]).contains("quotes no words"));
        assert!(refused(&[("matter", 99, "x")]).contains("there is no matter 99"));
        assert!(refused(&[("matter", 5, "Elsewhere.")]).contains("is of q"));
        assert!(refused(&[("matter", 6, "I started work 3.")]).contains("not words the owner said"));
        assert!(refused(&[("ask", 4, "x")]).contains("there is no ask 4"));
        assert!(refused(&[("note", 1, "x")]).contains("not from note"));
        assert!(refused(&[("matter", 1, "Keep it small"), ("matter", 1, "Keep it small")]).contains("quoted twice"));
        // Several reasons come at once.
        let mut d = decision(&[("matter", 99, "x")]);
        d["situation"] = json!("");
        d["replaces"] = json!(" ");
        d["reason_by"] = json!("owner");
        let all = resolve(db.conn(), "p", &d, "architect", None).unwrap_err();
        assert_eq!(all.len(), 4, "{all:?}");
        // A reason the writer gave cannot be accepted by an architect that gave the answer itself.
        let mut d = decision(&[("matter", 4, "skip the check")]);
        d["reason_by"] = json!("writer");
        assert!(resolve(db.conn(), "p", &d, "architect", None).unwrap_err()[0].contains("no one to have accepted"));
    }

    // A record already quoted in the item by the same words is recorded: a second decision from the same record quotes
    // another part of it.
    #[test]
    fn words_recorded_in_the_item_already_are_not_recorded_again() {
        let mut db = records();
        let line = resolve(
            db.conn(),
            "p",
            &decision(&[("matter", 1, "No browser check.")]),
            "architect",
            None,
        )
        .unwrap()
        .line;
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE items SET approach = ?1 WHERE id = 1",
                [insert("Approach:\n- Do it.", &line)],
            )?;
            Ok(())
        })
        .unwrap();
        let again = resolve(
            db.conn(),
            "p",
            &decision(&[("matter", 1, "No browser check.")]),
            "architect",
            Some(1),
        );
        assert!(again.unwrap_err()[0].contains("recorded in the item already"));
        // The same quote in a longer part, or another part of the record, is a decision of its own.
        assert!(
            resolve(
                db.conn(),
                "p",
                &decision(&[("matter", 1, "Keep it small.")]),
                "architect",
                Some(1)
            )
            .is_ok()
        );
        assert!(
            resolve(
                db.conn(),
                "p",
                &decision(&[("matter", 1, "No browser check. Keep")]),
                "architect",
                Some(1)
            )
            .is_ok()
        );
        // An item that is not there yet, or none, has nothing recorded.
        assert!(
            resolve(
                db.conn(),
                "p",
                &decision(&[("matter", 1, "No browser check.")]),
                "architect",
                None
            )
            .is_ok()
        );
    }

    // The line goes under the head `Decisions:` after the last line of its block, which ends at a blank line or the
    // next head; with no head it is made after the approach and before `Learned:`; every other byte stays.
    #[test]
    fn a_line_goes_under_the_head_of_decisions_and_nothing_else_moves() {
        let l = "Decision (2026-11-02, owner): New.";
        // Under the head, after the lines of its block, at the end of the text, at a blank line and at the next head.
        assert_eq!(
            insert(
                "Approach:\n- Do it.\nDecisions:\nDecision (a)\nDecision (b)\n\nLearned:\nCommits: abc\n",
                l
            ),
            "Approach:\n- Do it.\nDecisions:\nDecision (a)\nDecision (b)\nDecision (2026-11-02, owner): New.\n\nLearned:\nCommits: abc\n"
        );
        assert_eq!(
            insert("Decisions:\nDecision (a)", l),
            "Decisions:\nDecision (a)\nDecision (2026-11-02, owner): New."
        );
        assert_eq!(
            insert("Decisions:\nDecision (a)\nLearned:\nCommits: abc", l),
            "Decisions:\nDecision (a)\nDecision (2026-11-02, owner): New.\nLearned:\nCommits: abc"
        );
        // An empty block takes the line right under the head.
        assert_eq!(
            insert("A\nDecisions:\n\nLearned:", l),
            "A\nDecisions:\nDecision (2026-11-02, owner): New.\n\nLearned:"
        );
        // No head: made before `Learned:`, after the blank lines that set it off; or at the end; or alone.
        assert_eq!(
            insert("Approach:\n- Do it.\n\nLearned:\nFact (x)\n", l),
            "Approach:\n- Do it.\nDecisions:\nDecision (2026-11-02, owner): New.\n\nLearned:\nFact (x)\n"
        );
        assert_eq!(
            insert("Approach:\n- Do it.\n", l),
            "Approach:\n- Do it.\nDecisions:\nDecision (2026-11-02, owner): New.\n"
        );
        assert_eq!(
            insert("Approach:\n- Do it.", l),
            "Approach:\n- Do it.\nDecisions:\nDecision (2026-11-02, owner): New."
        );
        assert_eq!(
            insert("Learned:\nCommits: abc", l),
            "Decisions:\nDecision (2026-11-02, owner): New.\nLearned:\nCommits: abc"
        );
        assert_eq!(insert("", l), "Decisions:\nDecision (2026-11-02, owner): New.");
        assert_eq!(insert("  \n", l), "Decisions:\nDecision (2026-11-02, owner): New.");
        // A second line goes after the first.
        let once = insert("Approach:\n- Do it.", l);
        assert_eq!(
            insert(&once, "Decision (2026-11-03, owner): Next."),
            "Approach:\n- Do it.\nDecisions:\nDecision (2026-11-02, owner): New.\nDecision (2026-11-03, owner): Next."
        );
        // A head with its line ending (CR) is the head.
        assert_eq!(
            insert("A\r\nDecisions:\r\nDecision (a)\r\n", l),
            "A\r\nDecisions:\r\nDecision (a)\r\nDecision (2026-11-02, owner): New.\n"
        );
    }

    // A line that starts a decision which the earlier text did not hold is new; one the earlier text had, wherever it
    // stands, is not.
    #[test]
    fn a_new_line_of_a_decision_is_told_from_one_the_text_had() {
        let before = "Approach:\nDecisions:\nDecision (2026-11-02, owner): A.\n";
        assert!(new_decision_lines(before, before).is_empty());
        assert!(new_decision_lines(before, "Decisions:\nDecision (2026-11-02, owner): A.\nLearned:\n").is_empty());
        assert_eq!(
            new_decision_lines(
                before,
                &format!("{before}Decision (2026-11-03, owner): B.\nNote: Decision (x)")
            ),
            ["Decision (2026-11-03, owner): B."]
        );
        assert_eq!(
            new_decision_lines("", "Decision (2026-11-03, owner): B."),
            ["Decision (2026-11-03, owner): B."]
        );
        // A line indented by hand is a line of a decision as much as one that is not, to every check.
        assert_eq!(
            new_decision_lines(before, &format!("{before}  Decision (2026-11-03, owner): B.")),
            ["  Decision (2026-11-03, owner): B."]
        );
        assert!(is_line("\t Decision (2026-11-03, owner): B."));
        assert!(!is_line("Note: Decision (x)"));
        // A line listed by hand, as the items written with a list mark under their head have it, is one too; a mark of
        // another kind, or two, is not.
        assert!(is_line("- Decision (2026-11-03, owner): B."));
        assert!(is_line("  - Decision (2026-11-03, owner): B."));
        assert!(!is_line("* Decision (2026-11-03, owner): B."));
        assert!(!is_line("- - Decision (2026-11-03, owner): B."));
        assert!(!is_line("-Decision (2026-11-03, owner): B."));
        assert_eq!(
            unkept("Decisions:\n  Decision (2026-11-03, owner): B.", "Decisions:").len(),
            1
        );
    }

    // A design is given the item's decisions in their order, numbered from L1, each line as it stands with who decided as
    // its head says, a line listed by hand among them; the rest of the text is not one. An item with none lists none.
    #[test]
    fn an_item_s_decisions_are_listed_by_number_with_who_decided() {
        let approach = "Approach:\n- Do it.\nDecisions:\n- Decision (2026-11-03, owner): One path. Situation: s.\n\
                        Decision (2026-11-04, owner relayed by the delegate, not checked): Two. Words: \"x (y)\"\n\
                        Decision (2026-11-05, architect): Three.\nLearned:\nFact (2026-11-05): Decision (x) is not one";
        assert_eq!(
            listed(approach),
            [
                json!({ "decision": "L1", "line": "- Decision (2026-11-03, owner): One path. Situation: s.", "by": "owner" }),
                json!({
                    "decision": "L2",
                    "line": "Decision (2026-11-04, owner relayed by the delegate, not checked): Two. Words: \"x (y)\"",
                    "by": "owner relayed by the delegate, not checked",
                }),
                json!({ "decision": "L3", "line": "Decision (2026-11-05, architect): Three.", "by": "architect" }),
            ]
        );
        assert!(listed("Approach:\n- Do it.").is_empty());
        assert_eq!(decided_by("Decision (no head"), "");
    }

    // A merge carries the lines of the absorbed item's decisions into the kept one, in their order, under its head, and
    // those the kept item has already stay as they are.
    #[test]
    fn the_decisions_of_an_absorbed_item_go_to_the_one_kept() {
        let a = "Decision (2026-11-01, owner): A.";
        let b = "Decision (2026-11-02, owner): B.";
        let absorbed = format!("Approach:\n- Other.\nDecisions:\n{a}\n{b}\n");
        // Nothing to carry: the text stands.
        assert_eq!(
            carry("Approach:\n- Do it.", "Approach:\n- Other."),
            "Approach:\n- Do it."
        );
        // The kept item has the first already: only the second is added, after it.
        assert_eq!(
            carry(&format!("Approach:\n- Do it.\nDecisions:\n{a}\nLearned:\nx"), &absorbed),
            format!("Approach:\n- Do it.\nDecisions:\n{a}\n{b}\nLearned:\nx")
        );
        // The kept item has no head or text: both are made, in the order the absorbed item has them.
        assert_eq!(carry("", &absorbed), format!("Decisions:\n{a}\n{b}"));
        assert_eq!(
            carry("Approach:\n- Do it.", &absorbed),
            format!("Approach:\n- Do it.\nDecisions:\n{a}\n{b}")
        );
    }

    // The text of an item as a batch leaves it: the database's, with the lines a merge carried into it, and then the text
    // an edit of the batch gave it; an item the database does not hold is empty.
    #[test]
    fn the_text_of_an_item_follows_the_merges_and_edits_of_a_batch() {
        let line = "Decision (2026-11-01, owner): A.";
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            let gone = format!("Approach:\n- Gone.\nDecisions:\n{line}");
            for approach in ["Approach:\n- Keep.", gone.as_str()] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, approach, deliver, level)
                     VALUES ('p', 't', 'x', 'y', ?1, 'change', 'l')",
                    [approach],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let mut a = Approaches::default();
        assert_eq!(a.of(db.conn(), 1), "Approach:\n- Keep.");
        a.merged(db.conn(), 1, 2);
        assert_eq!(a.of(db.conn(), 1), format!("Approach:\n- Keep.\nDecisions:\n{line}"));
        // A text written over it after the merge leaves the carried line out.
        assert_eq!(unkept(&a.of(db.conn(), 1), "Approach:\n- New.").len(), 2);
        a.written(1, "Approach:\n- New.");
        assert_eq!(a.of(db.conn(), 1), "Approach:\n- New.");
        assert_eq!(a.of(db.conn(), 99), "");
    }

    // A text that replaces an item's keeps its decisions: the lines letter for letter, in order, under their head; one
    // that drops a line, the head, or moves the lines is told which, and a text that only moves what is around them,
    // or adds to it, keeps them.
    #[test]
    fn a_text_that_replaces_an_item_s_keeps_the_decisions_it_has() {
        let before =
            "Approach:\n- Do it.\nDecisions:\nDecision (2026-11-01, owner): A.\nDecision (2026-11-02, owner): B.\n";
        assert!(unkept(before, before).is_empty());
        assert!(unkept("", "Approach:\n- New.").is_empty());
        assert!(unkept(before, &before.replace("- Do it.", "- Do it first.\n- Then more.")).is_empty());
        // A line left out.
        let dropped = unkept(
            before,
            "Approach:\n- Do it.\nDecisions:\nDecision (2026-11-02, owner): B.\n",
        );
        assert_eq!(dropped.len(), 1, "{dropped:?}");
        assert!(dropped[0].contains("Decision (2026-11-01, owner): A."), "{dropped:?}");
        // A line changed by a letter is a line left out, and a line new by hand is not this check's.
        assert_eq!(unkept(before, &before.replace("owner): B.", "owner): B!")).len(), 1);
        // The head dropped, the lines kept.
        let headless = unkept(before, &before.replace("Decisions:\n", ""));
        assert_eq!(headless.len(), 1, "{headless:?}");
        assert!(headless[0].contains("head Decisions:"), "{headless:?}");
        // The lines swapped.
        let moved = unkept(
            before,
            "Approach:\n- Do it.\nDecisions:\nDecision (2026-11-02, owner): B.\nDecision (2026-11-01, owner): A.\n",
        );
        assert_eq!(moved.len(), 1, "{moved:?}");
        assert!(moved[0].contains("out of the order"), "{moved:?}");
        // An item with no decisions needs no head.
        assert!(unkept("Approach:\n- Do it.", "Approach:\n- Other.").is_empty());
    }
}
