//! What an agent returns at the end of a round, read against the cell it was entered for. Every return of a flow is in
//! one form, made of the flow and of what is written beside it ([`flow::form`]), which the agents' program is given
//! with every call: one of two kinds, a return that asks the engine for calls or one that does the cell's work (design
//! document `data-and-context`, 30 and 31). A return is put into the form's shape where it is not in it but means it,
//! as the old engine read one, and then read: what it dealt with, by the names of the cell's triggers, each worked out
//! to the block it stands on (9); each block it writes in `outputs`, a choice with its value and its line of why kept
//! as its content (50); the calls it asks for, each request of a tool one call; and what it says beside its work, its
//! questions, observations and reply, which are kept with it, its questions becoming data of their own once its run
//! finishes (32 to 36). What only its reading can see wrong with it against the cell is found as it is read and kept
//! with it: a return of both kinds, a name in `dealt_with` that is none of the cell's triggers, a tool the cell may not
//! ask for, a name in `meaning_unchanged` it does not write, a block it writes in `outputs` that the cell does not
//! write there or in a shape it does not take, and a question that recommends none of its options, that has the name
//! of a question the engine itself asks, or that the cell asks where no answer runs it. The run's check names these
//! with everything else wrong with the return, in one send-back (crate `decide`, module `run`). Why what came back
//! cannot be read at all is said in the words of `agents/return/sent_back.toml`.

use decide::BlockId;
use decide::run::Unmet;
use engine::Came;
use flow::agents::{Agents, ReturnKind, fill};
use flow::{CellSpec, Compiled};
use serde::Deserialize;
use serde_json::{Map, Value};
use work::{Ask, Handed, Question, Said, Way};

/// The fields of the kind that does the cell's work that are lists, as the engine reads them.
const LISTS: [&str; 4] = ["dealt_with", "meaning_unchanged", "questions", "observations"];

/// The field that holds the blocks a cell writes, as the engine reads it.
const OUTPUTS: &str = flow::agents::OUTPUTS;

/// The fields of a question, of either shape, as the engine reads them.
const QUESTION_FIELDS: [&str; 6] = ["name", "waits", "recommends", "ask", "options", "withdrawn"];

/// A return as the agent writes it: one of the two kinds, or, where it mixes them, both, which the run's check sends
/// back.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Return {
    calls: Option<Map<String, Value>>,
    outcome: Option<Outcome>,
}

/// A return that does the cell's work, as the agent writes it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    dealt_with: Vec<String>,
    meaning_unchanged: Vec<String>,
    questions: Vec<QuestionIn>,
    observations: Vec<String>,
    reply: Option<String>,
    outputs: Map<String, Value>,
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
    recommends: u64,
    ask: String,
    options: Vec<WayIn>,
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

/// A return put into the shape of the form when it is not in it but means it, as the old engine read one
/// (`as_taken`): the whole form, or the one object that wraps it, written as a string; the form wrapped one level in an
/// object of one key that is no kind of return; a kind, or the blocks a cell writes, written as a string; a kind left
/// out or null; a list left out, null or written as a string; and, in a question, the null fields of the other shape. A
/// content is never changed and a field the form requires of an item never made up, so what this does not reach fails
/// as it is read. The record of the round keeps the return as the command line printed it (`kept`), before this.
fn normalised(structured: &Value) -> Value {
    let kinds = [ReturnKind::Calls.name(), ReturnKind::Outcome.name()];
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
    if let Some(Value::Object(calls)) = o.get_mut(ReturnKind::Calls.name()) {
        for requests in calls.values_mut() {
            *requests = list_of(Some(requests.take()));
        }
    }
    if let Some(Value::Object(outcome)) = o.get_mut(ReturnKind::Outcome.name()) {
        for name in LISTS {
            let list = list_of(outcome.remove(name));
            outcome.insert(name.to_string(), list);
        }
        let outputs = table_of(outcome.remove(OUTPUTS));
        outcome.insert(OUTPUTS.to_string(), outputs);
        if outcome.get("reply").is_some_and(Value::is_null) {
            outcome.remove("reply");
        }
        if let Some(Value::Array(questions)) = outcome.get_mut("questions") {
            for question in questions {
                if let Value::Object(question) = question {
                    // Only the fields of the two shapes; a null of any other name is an unknown field and fails.
                    question.retain(|name, v| !v.is_null() || !QUESTION_FIELDS.contains(&name.as_str()));
                }
            }
        }
    }
    form
}

/// What came back, from the structured return, read against the cell it was entered for in the flow, in the words of
/// what is written beside the flows; or why it is not one of the form's, in those words.
pub fn came(structured: &Value, flow: &Compiled, cell: &CellSpec, words: &Agents) -> Result<Came, String> {
    let unread = &words.sent_back.unread;
    let r: Return = serde_json::from_value(normalised(structured))
        .map_err(|e| fill(&unread.not_of_its_form, &[("error", &e.to_string())]))?;
    let mut found = Vec::new();
    let mut came = Came {
        // A return that asks for calls and does no work deals with nothing.
        dealt_with: Some(Vec::new()),
        ..Came::default()
    };
    // A return is of one kind, never both, whatever each holds.
    let both = r.calls.is_some() && r.outcome.is_some();
    if let Some(outcome) = r.outcome {
        came.dealt_with = Some(dealt_with(&outcome.dealt_with, cell, &mut found));
        came.writes = writes(&outcome, flow, cell, &mut found);
        for name in &outcome.meaning_unchanged {
            if !outcome.outputs.contains_key(name) {
                found.push(Unmet::UnchangedNotWritten {
                    block: BlockId(name.clone()),
                });
            }
        }
        came.said = said(outcome);
        questions(&came.said.questions, flow, cell, words, &mut found);
    }
    if let Some(calls) = r.calls {
        came.asks = asks(calls, cell, &mut found, words)?;
    }
    if both {
        found.push(Unmet::AsksAndWorks);
    }
    came.found = found;
    Ok(came)
}

/// What is wrong with the questions the return asks that its reading sees, in the order it wrote them: one that
/// recommends none of its options, or has none; one with the name of the question the engine itself asks; and, of a
/// cell that no answer runs, every question, which no answer could bring back to it.
fn questions(asked: &[Question], flow: &Compiled, cell: &CellSpec, words: &Agents, found: &mut Vec<Unmet>) {
    let answered = (cell.triggers.iter())
        .any(|t| flow.blocks.get(&t.block).and_then(|b| b.writers.outside.as_deref()) == Some(flow::ANSWER));
    for question in asked {
        let name = || question.name().to_string();
        if question.name() == words.asks.protected.name {
            found.push(Unmet::QuestionTaken { name: name() });
            continue;
        }
        let Question::Asked {
            recommends, options, ..
        } = question
        else {
            continue;
        };
        if !recommends.is_some_and(|r| r >= 1 && r <= options.len() as u64) {
            found.push(Unmet::NotAnOption { name: name() });
        }
        if !answered {
            found.push(Unmet::NotAnswerable { name: name() });
        }
    }
}

/// The blocks what the return dealt with stand on, each name a trigger of the cell, in the order it names them; a name
/// that is none of them is found. A name written twice stands twice, which the run's check finds; two triggers of the
/// cell that stand on one block, each named once, stand on it once.
fn dealt_with(names: &[String], cell: &CellSpec, found: &mut Vec<Unmet>) -> Vec<BlockId> {
    let mut blocks: Vec<BlockId> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let Some(trigger) = cell.triggers.iter().find(|t| &t.name == name) else {
            found.push(Unmet::NotATrigger { name: name.clone() });
            continue;
        };
        let again = names[..i].contains(name);
        if again || !blocks.contains(&trigger.block) {
            blocks.push(trigger.block.clone());
        }
    }
    blocks
}

/// Each block the return writes, in the order the cell writes them, then any other in the order of its name: one the
/// cell writes there, as a string or as a choice's value with its line of why, which is kept as its content. A block
/// the cell does not write in `outputs`, as one it writes through a field of its own, and one in a shape neither a
/// block nor a choice has, is found.
fn writes(outcome: &Outcome, flow: &Compiled, cell: &CellSpec, found: &mut Vec<Unmet>) -> Vec<Handed> {
    let place = flow.flow.place(&cell.id);
    let outputs = place.map_or(&[][..], |p| &flow.flow.cells()[p].outputs[..]);
    let mut names: Vec<&String> = outcome.outputs.keys().collect();
    names.sort_by_key(|name| {
        let at = outputs.iter().position(|b| b.as_str() == name.as_str());
        (at.unwrap_or(usize::MAX), name.as_str())
    });
    let mut handed = Vec::new();
    for name in names {
        let block = BlockId(name.clone());
        let declared = flow.blocks.get(&block);
        let through_a_field = declared.is_some_and(|d| d.field.is_some());
        if !outputs.contains(&block) || through_a_field {
            found.push(Unmet::NotAnOutput { block });
            continue;
        }
        let matters = !outcome.meaning_unchanged.contains(name);
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
    handed
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
    calls: Map<String, Value>,
    cell: &CellSpec,
    found: &mut Vec<Unmet>,
    words: &Agents,
) -> Result<Vec<Ask>, String> {
    let mut tools: Vec<(String, Value)> = calls.into_iter().collect();
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
            asks.push(Ask::Tool {
                name: name.clone(),
                input: request.to_string(),
            });
        }
    }
    Ok(asks)
}

/// What the return says beside its work.
fn said(outcome: Outcome) -> Said {
    let questions = (outcome.questions.into_iter())
        .map(|q| match q {
            QuestionIn::Asked(q) => Question::Asked {
                name: q.name,
                waits: q.waits,
                recommends: Some(q.recommends),
                ask: q.ask,
                options: (q.options.into_iter())
                    .map(|o| Way {
                        does: o.does,
                        follows: o.follows,
                    })
                    .collect(),
                about: None,
            },
            QuestionIn::Withdrawn(q) => Question::Withdrawn {
                name: q.name,
                why: q.withdrawn,
            },
        })
        .collect();
    Said {
        questions,
        observations: outcome.observations,
        reply: outcome.reply,
    }
}

#[cfg(test)]
pub(crate) mod tests;
