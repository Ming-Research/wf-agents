//! A return whose JSON the command line could not parse, after all its attempts at it (`unparsed`, a kind of failed
//! run whose handling is to repair): never a passing failure, so the round is never run again from its start. Within
//! the round the engine first takes a whole return whose form the agent wrote as the JSON text of it, which the
//! command line refused, then closes a text that only lacks the brackets that close it, then has the installation's
//! lowest level repair any other fault of its syntax, once, a repair taken only when it keeps every other character as
//! it was, and then asks the agent to hand its return in again in its own session; what any of them gives is taken as
//! any return. Every return, repaired or not, has its form taken from the JSON text of it when the agent wrote it so,
//! and the fields every return holds that it wrote inside the field its forms are under moved up beside it, since the
//! command line no longer refuses them there.

use flow::{CompiledCell, CompiledGraph};
use serde_json::{Value, json};

use crate::Engine;
use crate::agent::{self, Reply, Request, Unparsed};
use crate::work::RoundId;

/// The kind of failed run, as the flow's rounds section names it, of a return the command line could not parse.
pub const KIND: &str = "unparsed";

/// The characters JSON's syntax uses, which a repair may add, remove or move, beside whitespace.
const SYNTAX: &str = "\"\\,:[]{}";

/// A return with each field every return holds that it wrote inside the field its forms are under (as `tell_owner`
/// and `progress` inside `outcome`) moved up beside that field, where the cell's schema has it, unless it is there
/// already; nothing else changes.
pub fn lifted(g: &CompiledGraph, cell: &CompiledCell, mut out: Value) -> Value {
    let common: Vec<String> = cell
        .schema
        .as_ref()
        .and_then(|s| s["properties"].as_object())
        .map(|p| p.keys().filter(|k| **k != g.within).cloned().collect())
        .unwrap_or_default();
    let Some(o) = out.as_object_mut() else { return out };
    let absent: Vec<&String> = common.iter().filter(|k| !o.contains_key(k.as_str())).collect();
    let mut moved = Vec::new();
    if let Some(inner) = o.get_mut(&g.within).and_then(Value::as_object_mut) {
        for k in absent {
            if let Some(v) = inner.shift_remove(k.as_str()) {
                moved.push((k.clone(), v));
            }
        }
    }
    o.extend(moved);
    out
}

/// A return whose field the forms are under holds a string holding the JSON of an object, as the model writes it at
/// times in place of the object, with that field the object; none when the field holds no such string. The command
/// line refuses the string, and the engine takes the object when it is handed it, from a run the command line gave up
/// on or another way, recording that it did (`engine.taken_from_text` on the round), for as long as the record shows
/// models still writing it.
pub fn form_from_text(g: &CompiledGraph, out: &Value) -> Option<Value> {
    let text = out.get(&g.within)?.as_str()?;
    let form = serde_json::from_str::<Value>(text.trim())
        .ok()
        .filter(Value::is_object)?;
    let mut out = out.clone();
    out[g.within.as_str()] = form;
    Some(out)
}

/// The text with the brackets that close it added at its end, when it lacks only those: a scan of its brackets
/// outside strings finds each closing one matching, and the text ending outside a string with some left open, after
/// which it parses as an object. None for a text whole already, or with any other fault.
pub fn closes(raw: &str) -> Option<String> {
    let mut open = Vec::new();
    let (mut in_string, mut escaped) = (false, false);
    for c in raw.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => open.push('}'),
            '[' => open.push(']'),
            '}' | ']' if open.pop() != Some(c) => return None,
            _ => {}
        }
    }
    if in_string || open.is_empty() {
        return None;
    }
    let closing: String = open.iter().rev().collect();
    let fixed = format!("{}{closing}", raw.trim_end());
    serde_json::from_str::<Value>(&fixed)
        .ok()
        .filter(Value::is_object)
        .map(|_| fixed)
}

/// Whether an attempt's text was cut off, which is never repaired since what is missing cannot be told: shorter than
/// the command line reported it (its count is of characters), or not ending in a closing brace once its whitespace is
/// trimmed.
pub fn cut_off(raw: &str, length: Option<usize>) -> bool {
    length.is_some_and(|n| raw.chars().count() < n) || !raw.trim_end().ends_with('}')
}

/// Whether a repair kept the content of the text it repaired, which cannot be parsed and so is compared as
/// characters: with each escape read as the character it stands for, and every character JSON's syntax uses and all
/// whitespace left out of both, the rest is the same, in the same order.
pub fn same_content(original: &str, repaired: &str) -> bool {
    content(original) == content(repaired)
}

fn content(text: &str) -> String {
    unescaped(text)
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control() && !SYNTAX.contains(*c))
        .collect()
}

/// A text with each escape JSON writes read as the character it stands for; an escape that is not one is left.
fn unescaped(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let hex = |at: usize| -> Option<u32> {
        let s: String = chars.get(at..at + 4)?.iter().collect();
        u32::from_str_radix(&s, 16).ok()
    };
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' || i + 1 >= chars.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let simple = match chars[i + 1] {
            'n' => Some('\n'),
            't' => Some('\t'),
            'r' => Some('\r'),
            'b' => Some('\u{8}'),
            'f' => Some('\u{c}'),
            '"' => Some('"'),
            '\\' => Some('\\'),
            '/' => Some('/'),
            _ => None,
        };
        if let Some(s) = simple {
            out.push(s);
            i += 2;
            continue;
        }
        if chars[i + 1] == 'u'
            && let Some(high) = hex(i + 2)
        {
            // A character beyond the basic plane is written as two escapes, a high surrogate and a low one.
            let pair =
                (0xD800..0xDC00).contains(&high) && chars.get(i + 6) == Some(&'\\') && chars.get(i + 7) == Some(&'u');
            if pair && let Some(low) = hex(i + 8).filter(|l| (0xDC00..0xE000).contains(l)) {
                if let Some(ch) = char::from_u32(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)) {
                    out.push(ch);
                    i += 12;
                    continue;
                }
            } else if let Some(ch) = char::from_u32(high) {
                out.push(ch);
                i += 6;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The JSON a repair answered with: the fenced block it wrote, or else its whole answer.
fn json_text(answer: &str) -> &str {
    let t = answer.trim();
    if let Some(start) = t.find("```") {
        let rest = &t[start + 3..];
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        if let Some(end) = rest.find("```") {
            return rest[..end].trim();
        }
    }
    t
}

/// What the engine's repair of a return the command line could not parse came to.
pub struct Recovery {
    /// The return to take: the last attempt whose form is the JSON text of it, closed, repaired or handed in again;
    /// none when nothing gave one.
    pub output: Option<Value>,
    /// The agent's run that handed its return in again, when it was asked to.
    pub again: Option<Reply>,
    /// What the round's record keeps of it: each attempt's whole text, and what each way of repairing came to.
    pub record: Value,
    /// What the repair by the lowest level cost, in dollars.
    pub repair_cost: f64,
}

/// Repairs, within its round, a return whose JSON the command line could not parse: takes the last attempt as it is
/// when its text is whole JSON whose form is the JSON text of it ([`form_from_text`]); closes the last attempt's text
/// when it only lacks closing brackets; else has the lowest level repair its syntax, taking the repair only when it
/// parses, keeps the content and `accepts` it (the cell's schema and checks, giving the return as it is taken); and
/// when neither applies to a text that is whole, or the repair is refused, asks the agent to hand its return in again
/// in its session. A text cut off, or none rebuilt, goes straight to the agent.
pub fn recover(
    e: &Engine,
    req: &Request,
    reply: &Reply,
    round: RoundId,
    g: &CompiledGraph,
    cell: &CompiledCell,
    accepts: &dyn Fn(&Engine, &Value) -> Result<Value, String>,
) -> Recovery {
    let attempts: Vec<Value> = reply
        .unparsed
        .iter()
        .map(|u| json!({ "id": u.id, "text": u.raw, "length": u.length, "why": u.why }))
        .collect();
    let mut record = json!({ "attempts": attempts });
    let mut repair_cost = 0.0;
    let last = reply.unparsed.last();
    let whole = last.and_then(|u| serde_json::from_str::<Value>(&u.raw).ok());
    match last.filter(|u| !cut_off(&u.raw, u.length)) {
        // A whole return the command line refused for its form held as the JSON text of it is taken as it is, its form
        // taken from the text as any return's is; any other whole return has no syntax to repair, and is handed in
        // again.
        Some(_) if whole.is_some() => {
            if let Some(v) = whole.filter(|v| form_from_text(g, v).is_some()) {
                record["form_as_text"] = json!(true);
                return Recovery {
                    output: Some(v),
                    again: None,
                    record,
                    repair_cost,
                };
            }
            record["repair"] = json!({ "not_tried": "the last attempt's text is whole JSON" });
        }
        Some(u) => {
            if let Some(fixed) = closes(&u.raw)
                && let Ok(v) = serde_json::from_str::<Value>(&fixed)
            {
                record["closed"] = json!({ "added": fixed[u.raw.trim_end().len()..] });
                return Recovery {
                    output: Some(lifted(g, cell, v)),
                    again: None,
                    record,
                    repair_cost,
                };
            }
            let (repaired, cost, taken) = repair(e, req, round, u, cell, accepts);
            record["repair"] = repaired;
            repair_cost = cost;
            if taken.is_some() {
                return Recovery {
                    output: taken,
                    again: None,
                    record,
                    repair_cost,
                };
            }
        }
        None => {
            let why = match last {
                None => "no attempt's text was rebuilt from the stream",
                Some(_) => "the last attempt's text was cut off",
            };
            record["repair"] = json!({ "not_tried": why });
        }
    }
    let message = format!(
        "# {round} · cell {}\n\nYour return could not be parsed as JSON, so the command line could not take it. Hand \
         it in again, whole, as format:{} says, as valid JSON: escape every quote (\\\") and every newline (\\n) inside \
         a text, and close every bracket.\n",
        cell.id, cell.id
    );
    let ask = Request {
        resume: Some(reply.session.clone()),
        prompt: message.clone(),
        regive: None,
        deliver: None,
        ..req.clone()
    };
    let again = e
        .sessions
        .run(&ask, &|what: &str| (e.activity)(round.work, round.n, what));
    record["again"] = json!({
        "message": message,
        "session": again.session,
        "handed_in": again.output.is_some(),
        "failed": again.error.as_deref().map(agent::failure_kind),
    });
    Recovery {
        output: again.output.clone().map(|v| lifted(g, cell, v)),
        again: Some(again),
        record,
        repair_cost,
    }
}

/// The one repair of an attempt's text by the installation's lowest level, in a new session with no tools: what the
/// record keeps of it, what it cost, and the return it gives when it is taken.
fn repair(
    e: &Engine,
    req: &Request,
    round: RoundId,
    u: &Unparsed,
    cell: &CompiledCell,
    accepts: &dyn Fn(&Engine, &Value) -> Result<Value, String>,
) -> (Value, f64, Option<Value>) {
    let name = e.config.config.lowest_level();
    let Some((level, cli)) = e.config.config.level(&name).map(|(l, c)| (l.clone(), c.clone())) else {
        let why = format!("no level of the configuration repairs it: {name}");
        return (json!({ "refused": why }), 0.0, None);
    };
    let schema = cell.schema.clone().unwrap_or(Value::Null);
    let prompt = format!(
        "The text below is a JSON object an agent wrote, which does not parse. Repair its JSON syntax only: escape \
         a quote or a newline inside a string, add or remove a comma, a colon or a bracket. Change no other character: \
         no word, no punctuation inside a string, no field and no order. Answer with the repaired JSON alone.\n\n\
         The schema the object fits:\n{schema}\n\nThe text:\n{}\n",
        u.raw
    );
    let ask = Request {
        work: req.work,
        cell: req.cell.clone(),
        line: "repair".into(),
        cli,
        model: level.model.clone(),
        effort: None,
        worktree: req.worktree.clone(),
        config_dir: e.cli_home(&level.cli),
        resume: None,
        new_session: agent::new_session_id(),
        prompt,
        schema: json!({}),
        envelope: Value::Null,
        refused: Vec::new(),
        readable: Vec::new(),
        replay: None,
        regive: None,
        deliver: None,
        bare: true,
    };
    let r = e
        .sessions
        .run(&ask, &|what: &str| (e.activity)(round.work, round.n, what));
    let cost = r.usage["session_cost_usd"].as_f64().unwrap_or(0.0);
    let mut record = json!({ "level": name, "model": level.model, "session": r.session, "cost_usd": cost });
    match taken(e, &r, u, accepts) {
        Ok(v) => {
            record["taken"] = json!(true);
            (record, cost, Some(v))
        }
        Err(why) => {
            record["refused"] = json!(why);
            (record, cost, None)
        }
    }
}

/// The return a repair gives, when it is taken: it parses, keeps the content of the text it repairs, and is accepted.
fn taken(
    e: &Engine,
    r: &Reply,
    u: &Unparsed,
    accepts: &dyn Fn(&Engine, &Value) -> Result<Value, String>,
) -> Result<Value, String> {
    let answer = r
        .result
        .as_deref()
        .ok_or_else(|| r.error.clone().unwrap_or_else(|| "the repair answered nothing".into()))?;
    let text = json_text(answer);
    let v: Value = serde_json::from_str(text).map_err(|x| format!("the repair does not parse: {x}"))?;
    if !same_content(&u.raw, text) {
        return Err("the repair changed the text beyond its syntax".into());
    }
    accepts(e, &v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Attempts at a return a trial's cell `P8` made, as its stream gave them: the first lacks its last closing brace;
    /// the second and third parse, with `tell_owner` and `progress` inside `outcome`; the fourth has them beside it.
    const MISSING_BRACE: &str = include_str!("../tests/unparsed/missing-last-brace.json.txt");
    const INSIDE_A: &str = include_str!("../tests/unparsed/common-fields-inside-a.json.txt");
    const INSIDE_B: &str = include_str!("../tests/unparsed/common-fields-inside-b.json.txt");
    const BESIDE: &str = include_str!("../tests/unparsed/common-fields-beside.json.txt");

    fn compiled() -> flow::Compiled {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap()
    }

    // The trial's attempt that lacks only its last brace is closed with exactly that brace, and its fields every return
    // holds, written inside `outcome`, move up beside it, as those of the attempts that parsed; one written beside it
    // stays as it is.
    #[test]
    fn a_text_that_lacks_only_closing_brackets_is_closed_and_its_fields_moved_up() {
        assert!(serde_json::from_str::<Value>(MISSING_BRACE).is_err());
        assert!(!cut_off(MISSING_BRACE, Some(MISSING_BRACE.chars().count())));
        let closed = closes(MISSING_BRACE).expect("closes");
        assert_eq!(&closed[MISSING_BRACE.trim_end().len()..], "}");
        let f = compiled();
        let g = f.graph("architect").unwrap();
        let cell = g.cell("P8").unwrap();
        for text in [closed.as_str(), INSIDE_A, INSIDE_B] {
            let v: Value = serde_json::from_str(text).unwrap();
            assert!(v["outcome"].get("tell_owner").is_some());
            let up = lifted(g, cell, v.clone());
            assert!(up["outcome"].get("tell_owner").is_none() && up["outcome"].get("progress").is_none());
            assert_eq!(up["tell_owner"], v["outcome"]["tell_owner"]);
            assert_eq!(up["progress"], v["outcome"]["progress"]);
            assert_eq!(up["outcome"]["revision"], v["outcome"]["revision"]);
        }
        let beside: Value = serde_json::from_str(BESIDE).unwrap();
        assert_eq!(lifted(g, cell, beside.clone()), beside);
        // A text whole already, or with any other fault, is not closed.
        assert_eq!(closes(BESIDE), None);
        assert_eq!(closes(r#"{"a": "b", "c": "d"#), None);
        assert_eq!(closes(r#"{"a": [1, 2}"#), None);
        assert_eq!(
            closes(r#"{"a": {"b": [1, "x]"]"#).as_deref(),
            Some(r#"{"a": {"b": [1, "x]"]}}"#)
        );
    }

    /// A word of the fixtures' text, in the owner's language: "change".
    const WORD: &str = "\u{6539}";

    /// The usual faults of a return written by hand, each injected into a valid return: what it does to the text, and
    /// the repair a model would make, which keeps the content.
    fn faults() -> Vec<(&'static str, String, String)> {
        let quote = (
            "an unescaped quote inside Chinese text",
            BESIDE.replacen(WORD, &format!("\"{WORD}\""), 1),
            BESIDE.replacen(WORD, &format!("\\\"{WORD}\\\""), 1),
        );
        let newline = (
            "a raw newline and a tab inside a string",
            BESIDE.replacen(WORD, &format!("{WORD}\n\t"), 1),
            BESIDE.replacen(WORD, &format!("{WORD}\\n\\t"), 1),
        );
        let comma = (
            "a trailing comma",
            format!("{},}}", BESIDE.trim_end().trim_end_matches('}')),
            BESIDE.to_string(),
        );
        vec![quote, newline, comma]
    }

    // A repair that changes only the syntax keeps the content: every usual fault, repaired, is the same text as its
    // original as characters go; a repair that changes a word, drops a field or turns an ASCII quote into a curly one
    // does not.
    #[test]
    fn only_a_repair_that_keeps_the_content_is_taken() {
        for (fault, broken, repaired) in faults() {
            assert!(serde_json::from_str::<Value>(&broken).is_err(), "{fault}");
            assert!(serde_json::from_str::<Value>(&repaired).is_ok(), "{fault}");
            assert!(same_content(&broken, &repaired), "{fault}");
            assert!(!cut_off(&broken, Some(broken.chars().count())), "{fault}");
        }
        let (_, broken, _) = &faults()[0];
        let curly = BESIDE.replacen(WORD, &format!("\u{201c}{WORD}\u{201d}"), 1);
        assert!(!same_content(broken, &curly));
        let dropped = BESIDE.replacen(WORD, "", 1);
        assert!(!same_content(broken, &dropped));
        let reworded = BESIDE.replacen(WORD, "\u{6362}", 1);
        assert!(!same_content(BESIDE, &reworded));
        // A character written as an escape is the same character.
        assert!(same_content("{\"a\": \"\u{4e2d}\"}", r#"{"a": "\u4e2d"}"#));
        assert!(same_content("{\"a\": \"\u{1f600}\"}", r#"{"a": "\ud83d\ude00"}"#));
        // The repair's answer is read from a fenced block when it wrote one.
        assert_eq!(json_text("Here:\n```json\n{\"a\": 1}\n```"), "{\"a\": 1}");
    }

    // A text cut off is never repaired: shorter than the command line reported it, or not ending in a closing brace.
    #[test]
    fn a_text_cut_off_is_not_repaired() {
        let n = BESIDE.chars().count();
        let cut: String = BESIDE.chars().take(n / 2).collect();
        assert!(cut_off(&cut, Some(n)));
        assert!(cut_off(&cut, None));
        // Rebuilt shorter than reported, though it happens to end in a brace.
        let short = format!("{cut}}}");
        assert!(cut_off(&short, Some(n)));
        assert!(!cut_off(BESIDE, Some(n)));
        assert!(!cut_off(BESIDE, None));
    }
}
