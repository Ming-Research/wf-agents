//! What an agent returns at the end of a round, read against the cell it was entered for. Every return of a line is in
//! one form, made of the flow and of what is written beside it ([`flow::form`]), which the agents' program is given
//! with every call: one of two kinds, a return that asks the engine for tools or one that does the cell's work. A
//! return is put into the form's shape where it is not in it but means it, as the old engine read one, and then read:
//! what it dealt with, by the names of the cell's triggers, each worked out to the block it stands on, a group's
//! trigger to its group's name; each block it writes in `outputs`, a choice with its value and its line of why kept as
//! its content, and the record of the work's questions and the owner's answers, the block the engine's source of
//! answers writes ([`flow::ANSWER`]), read as the questions it asks, which are kept with it and, once its run finishes,
//! become data of their own and the record's entries; the tools it asks for, each request of a
//! tool one call; what it says beside its work, its observations, which are kept with it; and why it says the work
//! changes each file the project protects, which the engine's look at the work's branch takes as the round ends and
//! asks the owner with, and which is not kept with the return ([`crate::protected`]). What only its reading can see
//! wrong with it against the cell is found as it is read and kept with it: a return of both kinds, a name in
//! `dealt_with` that is none of the cell's triggers, a tool the cell may not ask for, a name in `meaning_unchanged` it
//! does not write, a block it writes in `outputs` that is not among the cell's outputs or in a shape it does not take,
//! the owner's words named as dealt with and not answered in the record of what is said, and a question that
//! recommends none of its options or has the name of a question the engine itself asks ([`engine::asked::unmet`],
//! which the test carriers find in a script's questions the same way). The run's check names these
//! with everything else wrong with the return, in one send-back (crate `decide`, module `run`). Why what came back
//! cannot be read at all is said in the words of `agents/return/sent_back.toml`.

use std::collections::BTreeMap;

use decide::BlockId;
use decide::run::Unmet;
use engine::Came;
use flow::agents::{Agents, ReturnKind, fill};
use flow::{CellSpec, Compiled};
use serde::Deserialize;
use serde_json::{Map, Value};
use work::{Handed, Question, Said, Tool, Way};

/// The fields of the kind that does the cell's work that are lists, as the engine reads them.
const LISTS: [&str; 3] = ["dealt_with", "meaning_unchanged", "observations"];

/// The field that holds the blocks a cell writes, as the engine reads it.
const OUTPUTS: &str = flow::agents::OUTPUTS;

/// The field that says why the work changes each file the project protects, as the engine reads it.
const PROTECTED_CHANGES: &str = "protected_changes";

/// The fields of a question, of either shape, as the engine reads them.
const QUESTION_FIELDS: [&str; 6] = ["name", "waits", "recommends", "ask", "options", "withdrawn"];

/// A return as the agent writes it: one of the two kinds, or, where it mixes them, both, which the run's check sends
/// back.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Return {
    tools: Option<Map<String, Value>>,
    outcome: Option<Outcome>,
}

/// A return that does the cell's work, as the agent writes it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    dealt_with: Vec<String>,
    meaning_unchanged: Vec<String>,
    observations: Vec<String>,
    protected_changes: BTreeMap<String, String>,
    outputs: Map<String, Value>,
}

/// What came back, as its reading finds it: what the engine takes of it, and, of a return that does the cell's work,
/// why it says the work changes each file the project protects, by the file's path, which the engine's look at the
/// work's branch as the round ends takes ([`crate::protected`]); none of a return that only asks for tools, which
/// cannot say it.
#[derive(Debug)]
pub struct Reading {
    pub came: Came,
    pub protected: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum QuestionIn {
    Asked(AskedIn),
    Withdrawn(WithdrawnIn),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AskedIn {
    name: String,
    waits: bool,
    #[serde(deserialize_with = "whole")]
    recommends: Option<u64>,
    ask: String,
    options: Vec<WayIn>,
}

/// The option a question recommends, a whole number written as one or as a float with nothing after the point, as the
/// shape's `integer` takes it ([`flow::form::fits`]); a negative one is no option, none, which the check of the
/// question's options finds. A number that is not whole is not of the shape.
fn whole<'de, D: serde::Deserializer<'de>>(written: D) -> Result<Option<u64>, D::Error> {
    let n = serde_json::Number::deserialize(written)?;
    if let Some(n) = n.as_u64() {
        return Ok(Some(n));
    }
    match (n.as_i64(), n.as_f64()) {
        (Some(_), _) => Ok(None),
        (None, Some(f)) if f.fract() == 0.0 && f < 0.0 => Ok(None),
        (None, Some(f)) if f.fract() == 0.0 && f <= u64::MAX as f64 => Ok(Some(f as u64)),
        _ => Err(serde::de::Error::custom(format!("{n} is no whole number"))),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WayIn {
    does: String,
    follows: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WithdrawnIn {
    name: String,
    withdrawn: String,
}

/// The object a text holds when nothing but whitespace and closing brackets follows it: the model at times writes a
/// form with a closing bracket too many. None for any other text, which may be content.
fn object_of(text: &str) -> Option<Value> {
    let text = text.trim();
    let mut values = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    let object = values.next()?.ok().filter(Value::is_object)?;
    let rest = &text[values.byte_offset()..];
    rest.chars()
        .all(|c| c.is_whitespace() || c == '}' || c == ']')
        .then_some(object)
}

/// The array a list of the form comes as: a string holding one is parsed, a list left out or null is empty.
fn list_of(value: Option<Value>) -> Value {
    match value {
        None | Some(Value::Null) => Value::Array(Vec::new()),
        Some(Value::String(text)) => match serde_json::from_str::<Value>(text.trim()) {
            Ok(list @ Value::Array(_)) => list,
            _ => Value::String(text),
        },
        Some(other) => other,
    }
}

/// The object a part of the form that is one comes as: a string holding one is read as it, one left out or null is
/// empty.
fn table_of(value: Option<Value>) -> Value {
    match value {
        None | Some(Value::Null) => Value::Object(Map::new()),
        Some(Value::String(text)) => object_of(&text).unwrap_or(Value::String(text)),
        Some(other) => other,
    }
}

/// Whether the block is a record of the work's questions and the owner's answers: one the engine's source of answers
/// writes ([`flow::ANSWER`]). What a cell writes of one is the questions it asks.
fn of_questions(flow: &Compiled, block: &BlockId) -> bool {
    flow.blocks.get(block).and_then(|b| b.writers.outside.as_deref()) == Some(flow::ANSWER)
}

/// The blocks among the cell's outputs that hold the questions it asks, by name.
fn questions_blocks<'f>(flow: &'f Compiled, cell: &CellSpec) -> Vec<&'f str> {
    let place = flow.flow.place(&cell.id);
    let outputs = place.map_or(&[][..], |p| &flow.flow.cells()[p].outputs[..]);
    (outputs.iter())
        .filter(|b| of_questions(flow, b))
        .map(|b| b.as_str())
        .collect()
}

/// A return put into the shape of the form when it is not in it but means it, as the old engine read one
/// (`as_taken`): the whole form, or the one object that wraps it, written as a string; the form wrapped one level in an
/// object of one key that is no kind of return; a kind, or the blocks a cell writes, written as a string; a kind left
/// out or null; a list left out, null or written as a string, the questions among the blocks included; and, in a
/// question, the null fields of the other shape. The questions are those of the blocks of `questions`. A content is
/// never changed and a field the form requires of an item never made up, so what this does not reach fails as it is
/// read. The record of the round keeps the return as the command line printed it (`kept`), before this.
fn normalised(structured: &Value, questions: &[&str]) -> Value {
    let kinds = [ReturnKind::Tools.name(), ReturnKind::Outcome.name()];
    let mut form = match structured {
        Value::String(text) => object_of(text).unwrap_or_else(|| structured.clone()),
        other => other.clone(),
    };
    let inner = match &form {
        Value::Object(o) if o.len() == 1 && !kinds.iter().any(|k| o.contains_key(*k)) => match o.values().next() {
            Some(Value::String(text)) => object_of(text),
            Some(inner @ Value::Object(_)) => Some(inner.clone()),
            _ => None,
        },
        _ => None,
    };
    if let Some(inner) = inner {
        form = inner;
    }
    let Value::Object(o) = &mut form else { return form };
    for kind in kinds {
        match o.remove(kind) {
            None | Some(Value::Null) => {}
            Some(value) => {
                o.insert(kind.to_string(), table_of(Some(value)));
            }
        }
    }
    if let Some(Value::Object(tools)) = o.get_mut(ReturnKind::Tools.name()) {
        for requests in tools.values_mut() {
            *requests = list_of(Some(requests.take()));
        }
    }
    if let Some(Value::Object(outcome)) = o.get_mut(ReturnKind::Outcome.name()) {
        for name in LISTS {
            let list = list_of(outcome.remove(name));
            outcome.insert(name.to_string(), list);
        }
        for name in [OUTPUTS, PROTECTED_CHANGES] {
            let table = table_of(outcome.remove(name));
            outcome.insert(name.to_string(), table);
        }
        if let Some(Value::Object(outputs)) = outcome.get_mut(OUTPUTS) {
            for name in questions {
                let Some(asked) = outputs.get_mut(*name) else { continue };
                *asked = list_of(Some(asked.take()));
                let Value::Array(asked) = asked else { continue };
                for question in asked {
                    if let Value::Object(question) = question {
                        // Only the fields of the two shapes; a null of any other name is an unknown field and fails.
                        question.retain(|name, v| !v.is_null() || !QUESTION_FIELDS.contains(&name.as_str()));
                    }
                }
            }
        }
    }
    form
}

/// What came back, from the structured return, read against the cell it was entered for in the flow, in the words of
/// what is written beside the flows; or why it is not one of the form's, in those words.
pub fn came(structured: &Value, flow: &Compiled, cell: &CellSpec, words: &Agents) -> Result<Reading, String> {
    let unread = &words.sent_back.unread;
    let not_of_its_form = |e: serde_json::Error| fill(&unread.not_of_its_form, &[("error", &e.to_string())]);
    let asking = questions_blocks(flow, cell);
    let r: Return = serde_json::from_value(normalised(structured, &asking)).map_err(not_of_its_form)?;
    let mut found = Vec::new();
    let mut came = Came {
        // A return that asks for tools and does no work deals with nothing.
        dealt_with: Some(Vec::new()),
        ..Came::default()
    };
    // A return is of one kind, never both, whatever each holds.
    let both = r.tools.is_some() && r.outcome.is_some();
    let mut protected = None;
    if let Some(mut outcome) = r.outcome {
        protected = Some(std::mem::take(&mut outcome.protected_changes));
        let (dealt_with, none) = cell.dealt_with(&outcome.dealt_with);
        found.extend(none.into_iter().map(|name| Unmet::NotATrigger { name }));
        came.dealt_with = Some(dealt_with);
        let (writes, asked) = writes(&outcome, flow, cell, &mut found);
        came.writes = writes;
        for name in &outcome.meaning_unchanged {
            if !outcome.outputs.contains_key(name) {
                found.push(Unmet::UnchangedNotWritten {
                    block: BlockId(name.clone()),
                });
            }
        }
        answered(&outcome, flow, cell, &mut found);
        // The questions a record holds, which are not of its shape where they cannot be read as questions.
        let mut questions = Vec::new();
        for (block, value) in asked {
            match serde_json::from_value::<Vec<QuestionIn>>(value) {
                Ok(asked) => questions.extend(asked.into_iter().map(question)),
                Err(_) => found.push(Unmet::NotOfItsShape { block }),
            }
        }
        came.said = Said {
            questions,
            observations: outcome.observations,
        };
        found.extend(engine::asked::unmet(&came.said.questions, words));
    }
    if let Some(tools) = r.tools {
        came.asks = asks(tools, cell, &mut found, words)?;
    }
    if both {
        found.push(Unmet::AsksAndWorks);
    }
    came.found = found;
    Ok(Reading { came, protected })
}

/// Of each trigger of the owner's words the return names as dealt with, in the order it names them, once each, where
/// the return writes no words in `outputs` into a record of what is said that the trigger has its agent read: the
/// owner's words are answered there (design document `data-and-context`, 38).
fn answered(outcome: &Outcome, flow: &Compiled, cell: &CellSpec, found: &mut Vec<Unmet>) {
    let words =
        |block: &BlockId| matches!(outcome.outputs.get(block.as_str()), Some(Value::String(w)) if !w.trim().is_empty());
    for (i, name) in outcome.dealt_with.iter().enumerate() {
        if outcome.dealt_with[..i].contains(name) {
            continue;
        }
        let Some(trigger) = (cell.triggers.iter()).find(|t| &t.name == name) else {
            continue;
        };
        if trigger.of_source(flow, flow::WORDS).is_some() && !trigger.records(flow, flow::WORDS).any(words) {
            found.push(Unmet::NotAnswered { name: name.clone() });
        }
    }
}

/// Each block the return writes, in the order the cell writes them, then any other in the order of its name: one the
/// cell writes there, as a string, as a choice's value with its line of why, or, of a block whose file gives it a
/// shape, as the JSON of that shape it holds ([`shaped`]), which is kept as its content, or as a text that holds
/// nothing, kept as it is. A block that is not among the cell's outputs, one in a shape neither a block nor a choice
/// has, and one not of the shape its file gives it, is found. Beside them, what the return writes of each record of the
/// work's questions, by the record, which is read as the questions it asks, none where it holds nothing: their entries
/// are the engine's to write.
fn writes(
    outcome: &Outcome,
    flow: &Compiled,
    cell: &CellSpec,
    found: &mut Vec<Unmet>,
) -> (Vec<Handed>, Vec<(BlockId, Value)>) {
    let place = flow.flow.place(&cell.id);
    let outputs = place.map_or(&[][..], |p| &flow.flow.cells()[p].outputs[..]);
    let mut names: Vec<&String> = outcome.outputs.keys().collect();
    names.sort_by_key(|name| {
        let at = outputs.iter().position(|b| b.as_str() == name.as_str());
        (at.unwrap_or(usize::MAX), name.as_str())
    });
    let mut handed = Vec::new();
    let mut asked = Vec::new();
    for name in names {
        let block = BlockId(name.clone());
        if !outputs.contains(&block) {
            found.push(Unmet::NotAnOutput { block });
            continue;
        }
        let declared = flow.blocks.get(&block);
        let matters = !outcome.meaning_unchanged.contains(name);
        // A text that holds nothing says nothing of a block of a shape: it is handed in holding nothing, for the run's
        // check to take as no write where it takes nothing away, and of the questions it asks none (decision
        // a-write-of-nothing-only-empties).
        if declared.is_some_and(|d| d.shape.is_some())
            && let Value::String(text) = &outcome.outputs[name]
            && decide::holds_nothing(None, text)
        {
            if !of_questions(flow, &block) {
                handed.push(Handed {
                    block,
                    content: text.clone(),
                    matters,
                    value: None,
                });
            }
            continue;
        }
        // A block of a shape is held to it as the return is read, the questions among them: the form gives the shape.
        if let Some(shape) = declared.and_then(|d| d.shape.as_ref()) {
            let Some(value) = shaped(&outcome.outputs[name], shape) else {
                found.push(Unmet::NotOfItsShape { block });
                continue;
            };
            if of_questions(flow, &block) {
                asked.push((block, value));
                continue;
            }
            handed.push(Handed {
                block,
                content: value.to_string(),
                matters,
                value: None,
            });
            continue;
        }
        if of_questions(flow, &block) {
            asked.push((block, outcome.outputs[name].clone()));
            continue;
        }
        let (content, value) = match &outcome.outputs[name] {
            Value::String(content) => (content.clone(), None),
            Value::Object(choice) => match (choice.len(), choice.get("value"), choice.get("why")) {
                (2, Some(Value::String(value)), Some(Value::String(why))) => (why.clone(), Some(value.clone())),
                _ => {
                    found.push(misshapen(declared.is_some_and(|d| d.values.is_some()), block));
                    continue;
                }
            },
            _ => {
                found.push(misshapen(declared.is_some_and(|d| d.values.is_some()), block));
                continue;
            }
        };
        handed.push(Handed {
            block,
            content,
            matters,
            value,
        });
    }
    (handed, asked)
}

/// What a block of a shape holds as the shape holds it: the value as it is, where it is of the shape; or, written as a
/// text, the JSON the text holds, where that is of it, as a return that means the form is put into its shape before it
/// is read (decision normalise-shape-only), such as one read from the stream after the command line gave up on it.
/// None where neither is.
fn shaped(value: &Value, shape: &flow::Shape) -> Option<Value> {
    if flow::form::fits(value, shape) {
        return Some(value.clone());
    }
    let Value::String(text) = value else {
        return None;
    };
    let held: Value = serde_json::from_str(text.trim()).ok()?;
    flow::form::fits(&held, shape).then_some(held)
}

/// What is wrong with a block written in a shape neither a block nor a choice has: a choice is not written as its value
/// and its line of why, and another block not as a string.
fn misshapen(choice: bool, block: BlockId) -> Unmet {
    if choice {
        Unmet::NoValue { block }
    } else {
        Unmet::NotAChoice { block }
    }
}

/// The calls the return asks for, each request of a tool one call, in the order the cell names its tools, then any
/// other in the order of its name; a tool the cell may not ask for is found. Or why they are not of the form: a tool's
/// requests that are no list.
fn asks(
    tools: Map<String, Value>,
    cell: &CellSpec,
    found: &mut Vec<Unmet>,
    words: &Agents,
) -> Result<Vec<Tool>, String> {
    let mut tools: Vec<(String, Value)> = tools.into_iter().collect();
    tools.sort_by_key(|(name, _)| {
        let at = cell.tools.iter().position(|t| t == name);
        (at.unwrap_or(usize::MAX), name.clone())
    });
    let mut asks = Vec::new();
    for (name, requests) in tools {
        let Value::Array(requests) = requests else {
            return Err(fill(&words.sent_back.unread.requests_not_a_list, &[("name", &name)]));
        };
        if !cell.tools.contains(&name) {
            found.push(Unmet::NotATool { name: name.clone() });
        }
        for request in requests {
            asks.push(Tool {
                name: name.clone(),
                input: request.to_string(),
            });
        }
    }
    Ok(asks)
}

/// A question as the return writes it, as the engine keeps it.
fn question(q: QuestionIn) -> Question {
    match q {
        QuestionIn::Asked(q) => Question::Asked {
            name: q.name,
            waits: q.waits,
            recommends: q.recommends,
            ask: q.ask,
            options: (q.options.into_iter())
                .map(|o| Way {
                    does: o.does,
                    follows: o.follows,
                })
                .collect(),
            about: None,
            reasons: BTreeMap::new(),
        },
        QuestionIn::Withdrawn(q) => Question::Withdrawn {
            name: q.name,
            why: q.withdrawn,
        },
    }
}

#[cfg(test)]
pub(crate) mod tests;
