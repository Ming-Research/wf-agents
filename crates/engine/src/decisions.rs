//! The owner's decisions in an item: what the architect or an engineer hands in as a decision, checked against the
//! records its words are from, and written as one line of the item's `approach` under the head `Decisions:`. The line
//! is the engine's, in one place: its date, who decided and the words quoted are not typed by the agent that hands the
//! decision in but taken from the records it names, so that a line says only what a record says.
//!
//! A decision names the records its words are from, each by where they are kept (`matter`, `ask` or `interjection`),
//! its number and the part of its words that decides: the quote must be found, as it is, in the record's words, which
//! the engine reads from the record again. Who decided is what the record says of who spoke; a decision on several
//! records is told by the least certain of them.

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
/// the page; a delegate that speaks for the owner, so that whether the owner said it is the delegate's word; a record
/// kept before who speaks was declared, which says nothing and is never told as the owner's; and the architect, whose
/// reply to an open question is its own judgment, even when it carries an answer of the owner's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Owner,
    Delegate,
    Unrecorded,
    Architect,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Owner => "owner",
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

/// A record a decision's words are from: who it says spoke, the words it holds that a quote may be found in, and when
/// they were said, in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub source: Source,
    pub texts: Vec<String>,
    pub at: i64,
}

fn strings(vs: &[&Value]) -> Vec<String> {
    vs.iter()
        .filter_map(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// The record a decision names, of a project: the owner's words or an answer to an open question (`matter`), the
/// owner's answer to an ask (`ask`), or what the owner or the architect said to a work (`interjection`). An error says
/// why it is none: there is no such record, it is another project's, or it holds no words a decision may quote.
pub fn record(conn: &Connection, project: &str, from: &str, id: i64) -> Result<Record, String> {
    match from {
        "matter" => {
            let row: Option<(String, String, Option<String>, String, i64)> = conn
                .query_row(
                    "SELECT project, kind, detail, body, at FROM architect_matters WHERE id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .map_err(sql)?;
            let Some((of, kind, detail, body, at)) = row else {
                return Err(format!("there is no matter {id}"));
            };
            if of != project {
                return Err(format!("matter {id} is of {of}, and the decision is of {project}"));
            }
            let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            match (kind.as_str(), detail.as_deref()) {
                ("owner_words", Some("release")) => Err(format!(
                    "matter {id} is the engine's note that the owner started a work, not words the owner said"
                )),
                ("owner_words", _) => Ok(Record {
                    source: Source::declared(body["speaker"].as_str()),
                    texts: strings(&[&body["text"], &body["answer"]]),
                    at,
                }),
                ("open_question", _) => {
                    let texts = strings(&[&body["answer"]]);
                    if texts.is_empty() {
                        return Err(format!("open question {id} has no answer yet"));
                    }
                    Ok(Record {
                        source: Source::Architect,
                        texts,
                        at,
                    })
                }
                (other, _) => Err(format!(
                    "matter {id} is of the kind {other}: only the owner's words and the answer to an open question \
                     hold the words of a decision"
                )),
            }
        }
        "ask" => {
            let row: Option<(Option<String>, Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT a.answer, w.project, i.project FROM owner_asks a JOIN works w ON w.id = a.work
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
            Ok(Record {
                source: Source::declared(answer["speaker"].as_str()),
                texts: strings(&[&answer["text"]]),
                at: answer["at"].as_i64().unwrap_or_default(),
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
            );
            let row: Option<Row> = conn
                .query_row(
                    "SELECT j.speaker, j.text, j.matter, w.project, i.project,
                       coalesce((SELECT min(c.at) FROM changes c
                                 WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)), 0),
                       (SELECT json_extract(m.body, '$.speaker') FROM architect_matters m WHERE m.id = j.matter)
                     FROM interjections j JOIN works w ON w.id = j.work LEFT JOIN items i ON i.id = w.item
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
                        ))
                    },
                )
                .optional()
                .map_err(sql)?;
            let Some((speaker, text, matter, wp, ip, at, declared)) = row else {
                return Err(format!("there is no interjection {id}"));
            };
            let of = wp.or(ip).unwrap_or_default();
            if of != project {
                return Err(format!(
                    "interjection {id} is of {of}, and the decision is of {project}"
                ));
            }
            // What was said to the engineer alone keeps no speaker; words a matter holds say it as the matter does.
            let source = match (speaker.as_str(), matter) {
                ("architect", _) => Source::Architect,
                (_, Some(_)) => Source::declared(declared.as_deref()),
                _ => Source::Unrecorded,
            };
            Ok(Record {
                source,
                texts: strings(&[&Value::String(text)]),
                at,
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
    pub source: Source,
    pub date: String,
    pub said: Vec<Said>,
    pub line: String,
}

impl Resolved {
    /// What the batch keeps of it, so that the review reads what is written.
    pub fn to_json(&self) -> Value {
        json!({
            "source": self.source.as_str(),
            "date": self.date,
            "line": self.line,
            "words": self.said.iter().map(|s| json!({ "from": s.from, "id": s.id, "words": s.words })).collect::<Vec<_>>(),
        })
    }
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
        .filter(|l| l.starts_with(LINE))
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
        source = Some(source.map_or(record.source, |s| s.max(record.source)));
        said.push(one);
    }
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
    let by = if by == "writer" {
        format!(" ({writer}'s, accepted by the owner)")
    } else {
        String::new()
    };
    let quoted = said
        .iter()
        .map(|s| format!("\"{}\" ({} {}, {})", flat(&s.words), s.from, s.id, date_of(s.at)))
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
    Ok(Resolved {
        source,
        date,
        said,
        line,
    })
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

/// Whether a text holds a line that starts a decision which an earlier text of the same item does not hold: such a line
/// is written by the engine from a decision, never by hand.
pub fn new_decision_lines(before: &str, after: &str) -> Vec<String> {
    let had: Vec<&str> = before.lines().collect();
    after
        .lines()
        .filter(|l| l.starts_with(LINE) && !had.contains(l))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::rusqlite::params;
    use db::{Actor, Db};

    /// A database with a project's records a decision may quote: matter 1 the owner's words said at the page, 2 the same
    /// said by a delegate, 3 words kept before who speaks was declared, 4 the answer to an open question, 5 the words
    /// of another project, 6 words of the engine's note, and ask 1 answered by the owner with an item's work.
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
            matter("p", "open_question", "", r#"{"question":"Which?","answer":"Option one: skip the check.\n\nIt is small."}"#, day)?;
            matter("q", "owner_words", "said", r#"{"text":"Elsewhere.","where":"page","speaker":"owner"}"#, day)?;
            matter("p", "owner_words", "release", r#"{"text":"I started work 3."}"#, day)?;
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
                "INSERT INTO owner_asks (work, round, body, answer) VALUES (1, 1, '{}', ?1)",
                [format!(r#"{{"by":"owner","at":{day},"text":"Keep the old name.","speaker":"owner"}}"#)],
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

    // Who decided is what the record says: the owner's words as declared at the page, a delegate's as the delegate's,
    // words kept before the speaker was declared as unrecorded, an answer to an open question as the architect's, and
    // an ask's answer as it was declared. A decision on several records is told by the least certain of them.
    #[test]
    fn who_decided_is_what_the_record_says_and_the_least_certain_of_several_holds() {
        let db = records();
        let who = |words: &[(&str, i64, &str)]| {
            resolve(db.conn(), "p", &decision(words), "architect", None)
                .unwrap()
                .source
                .as_str()
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
        assert_eq!(r.date, "2026-11-03");
        assert_eq!(
            r.line,
            "Decision (2026-11-03, unrecorded): No browser check for this item. Situation: The item adds a diagram fit. \
             Reason (architect's, accepted by the owner): It is small. Replaces: none. \
             Words: \"No browser check.\" (matter 1, 2026-11-02); \"Old words.\" (matter 3, 2026-11-03)"
        );
        assert_eq!(r.said[0].words, "No browser check.");
        // An engineer's decision names the engineer as the one who wrote the reason.
        let e = resolve(db.conn(), "p", &d, "engineer", None).unwrap();
        assert!(
            e.line.contains("Reason (engineer's, accepted by the owner)"),
            "{}",
            e.line
        );
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
        assert!(refused(&[("matter", 9, "x")]).contains("there is no matter 9"));
        assert!(refused(&[("matter", 5, "Elsewhere.")]).contains("is of q"));
        assert!(refused(&[("matter", 6, "I started work 3.")]).contains("not words the owner said"));
        assert!(refused(&[("ask", 2, "x")]).contains("there is no ask 2"));
        assert!(refused(&[("note", 1, "x")]).contains("not from note"));
        assert!(refused(&[("matter", 1, "Keep it small"), ("matter", 1, "Keep it small")]).contains("quoted twice"));
        // Several reasons come at once.
        let mut d = decision(&[("matter", 9, "x")]);
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
    }
}
