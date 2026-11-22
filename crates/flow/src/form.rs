//! The form of a return of a line: the shape alone, which the agents' program is given with every call of an agent
//! cell of the line, and so the same bytes on every call of a session, which is a line's (decision
//! `one-form-for-a-line`). It is made of the definitions alone, in three parts: the engine's fields as `agents/return/`
//! defines them, which every agent cell has alike; the blocks the line's cells write, each a field of `outputs` under
//! its name, in its own shape; and the tools the flow names, each shaped as `agents/tools/` defines it, which the kind
//! of return that asks for tools holds and nothing else. The engine checks each return against the cell it was entered
//! for, once it is read. The rules here name no cell, no datum and no tool, and of the engine's fields only the one that
//! holds the blocks.

use crate::agents::{Agents, OUTPUTS, ReturnKind};
use crate::compile::{Block, Compiled, Shape};
use crate::source::Kind;
use decide::BlockId;

/// The engine's tools the flow's agent cells may ask for, each once, in the order the cells name them: the tools the
/// flow names, which every agent cell of it has.
pub fn tools(flow: &Compiled) -> Vec<&str> {
    let mut tools: Vec<&str> = Vec::new();
    for cell in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        for tool in &cell.tools {
            if !tools.contains(&tool.as_str()) {
                tools.push(tool);
            }
        }
    }
    tools
}

/// The blocks the agent cells of the line write, each once, in the order of the walk, and those of a cell in the order
/// its outputs name them.
pub fn blocks<'f>(flow: &'f Compiled, line: &str) -> Vec<&'f BlockId> {
    let mut blocks: Vec<&BlockId> = Vec::new();
    let of_line = (flow.cells.iter()).filter(|c| c.kind == Kind::Agent && c.line.as_deref() == Some(line));
    for cell in of_line {
        let Some(place) = flow.flow.place(&cell.id) else {
            continue;
        };
        for block in &flow.flow.cells()[place].outputs {
            if !blocks.contains(&block) {
                blocks.push(block);
            }
        }
    }
    blocks
}

/// The form of a return of the line's cells, as JSON, its members in the order the files give them and two spaces to
/// a level: one of two kinds, never both nor neither. The kind that does the cell's work holds the fields
/// `agents/return/return.toml` gives it, in its order, with `outputs` holding each block of the line ([`blocks`]) in its
/// own shape; the kind that asks for tools holds each tool of the flow as a list of its requests, and is left out where
/// the flow names no tool. Or why it cannot be made: the flow names a tool that `agents/tools/` does not define.
pub fn form(flow: &Compiled, agents: &Agents, line: &str) -> Result<String, String> {
    let made = made(flow, agents, line)?;
    let tools = made.tools.as_ref().map(|tools| kind(tools.clone(), Vec::new()));
    let outcome = made.outcome(Some(&made.blocks));
    Ok(json(&top(tools, outcome), 0))
}

/// The form of a return of the line's cells, in its three parts, each as JSON as [`form`] writes it: together they are
/// the form, and nothing is in two of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parts {
    /// The engine's fields: the form with `outputs` as its own file shapes it, holding no block, and the kind that asks
    /// for tools, where the flow names any, holding no tool.
    pub fields: String,
    /// The blocks of the line, as `outputs` holds them; none where the form has no `outputs`.
    pub blocks: Option<String>,
    /// The tools of the flow, as the kind that asks for tools holds them; none where the flow names no tool.
    pub tools: Option<String>,
}

/// The form of a return of the line's cells in its three parts, or why it cannot be made, as [`form`] says.
pub fn parts(flow: &Compiled, agents: &Agents, line: &str) -> Result<Parts, String> {
    let made = made(flow, agents, line)?;
    let empty = made.tools.as_ref().map(|_| kind(toml::Table::new(), Vec::new()));
    let has_outputs = made.fields.iter().any(|(name, _)| name == OUTPUTS);
    Ok(Parts {
        fields: json(&top(empty, made.outcome(None)), 0),
        blocks: has_outputs.then(|| json(&made.outputs(Some(&made.blocks)), 0)),
        tools: made.tools.map(|tools| json(&kind(tools, Vec::new()), 0)),
    })
}

/// What the form is made of: the engine's fields of the kind that does the cell's work, in their order, each as its
/// file shapes it, and those every return of it holds; the blocks of the line, each under its name in its shape; and
/// each tool of the flow as a list of its requests, none where it names none.
struct Made {
    fields: Vec<(String, toml::Value)>,
    required: Vec<toml::Value>,
    blocks: toml::Table,
    tools: Option<toml::Table>,
}

impl Made {
    /// The kind that does the cell's work, `outputs` holding these blocks, or none where it is to hold none.
    fn outcome(&self, blocks: Option<&toml::Table>) -> toml::Value {
        let mut properties = toml::Table::new();
        for (name, shape) in &self.fields {
            let shape = if name == OUTPUTS {
                self.outputs(blocks)
            } else {
                shape.clone()
            };
            properties.insert(name.clone(), shape);
        }
        kind(properties, self.required.clone())
    }

    /// The shape of `outputs` as its file gives it, holding these blocks under `properties` after its `type`, or none.
    fn outputs(&self, blocks: Option<&toml::Table>) -> toml::Value {
        let shape = (self.fields.iter())
            .find(|(name, _)| name == OUTPUTS)
            .map(|(_, shape)| shape);
        let Some(toml::Value::Table(shape)) = shape else {
            return shape.cloned().unwrap_or_else(|| toml::Value::Table(toml::Table::new()));
        };
        let Some(blocks) = blocks else {
            return toml::Value::Table(shape.clone());
        };
        let mut with = toml::Table::new();
        if !shape.contains_key("type") {
            with.insert("properties".into(), toml::Value::Table(blocks.clone()));
        }
        for (key, value) in shape {
            with.insert(key.clone(), value.clone());
            if key == "type" {
                with.insert("properties".into(), toml::Value::Table(blocks.clone()));
            }
        }
        toml::Value::Table(with)
    }
}

fn made(flow: &Compiled, agents: &Agents, line: &str) -> Result<Made, String> {
    let names = tools(flow);
    let mut tools = toml::Table::new();
    for tool in &names {
        let Some(defined) = agents.tools.get(*tool) else {
            let file = agents.source("agents/tools/");
            return Err(format!(
                "the flow {} names the tool {tool}, which {file} does not have",
                flow.name
            ));
        };
        let list = table([("type", "array".into()), ("items", defined.request.clone())]);
        tools.insert(tool.to_string(), list);
    }
    let mut fields = Vec::new();
    let mut required = Vec::new();
    for name in &agents.returns.form.outcome {
        let Some(field) = agents.fields.get(name) else { continue };
        fields.push((name.clone(), field.shape.clone()));
        if field.required {
            required.push(toml::Value::String(name.clone()));
        }
    }
    let mut blocks = toml::Table::new();
    for block in self::blocks(flow, line) {
        if let Some(declared) = flow.blocks.get(block) {
            blocks.insert(block.as_str().to_string(), shape(declared));
        }
    }
    Ok(Made {
        fields,
        required,
        blocks,
        tools: (!names.is_empty()).then_some(tools),
    })
}

/// The shape a return holds the block to: the one its file gives; of a choice, its value, one of its values, and a line
/// of why; of any other, a text.
fn shape(block: &Block) -> toml::Value {
    if let Some(Shape(shape)) = &block.shape {
        return shape.clone();
    }
    let Some(values) = &block.values else {
        return table([("type", "string".into())]);
    };
    let values = toml::Value::Array(values.iter().cloned().map(toml::Value::String).collect());
    let value = table([("type", "string".into()), ("enum", values)]);
    let why = table([("type", "string".into())]);
    let required = toml::Value::Array(vec!["value".into(), "why".into()]);
    table([
        ("type", "object".into()),
        ("properties", table([("value", value), ("why", why)])),
        ("required", required),
        ("additionalProperties", false.into()),
    ])
}

/// A kind of return: an object of these properties, and no other, those required where any is.
fn kind(properties: toml::Table, required: Vec<toml::Value>) -> toml::Value {
    let mut object = toml::Table::new();
    object.insert("type".into(), "object".into());
    object.insert("properties".into(), toml::Value::Table(properties));
    if !required.is_empty() {
        object.insert("required".into(), toml::Value::Array(required));
    }
    object.insert("additionalProperties".into(), false.into());
    toml::Value::Table(object)
}

/// The top of the form: one of the two kinds, never both nor neither. The service refuses a choice at the top of a form,
/// so the kinds are two properties of which the form takes exactly one.
fn top(tools: Option<toml::Value>, outcome: toml::Value) -> toml::Value {
    let mut kinds = toml::Table::new();
    if let Some(tools) = tools {
        kinds.insert(ReturnKind::Tools.name().into(), tools);
    }
    kinds.insert(ReturnKind::Outcome.name().into(), outcome);
    table([
        ("type", "object".into()),
        ("properties", toml::Value::Table(kinds)),
        ("minProperties", 1.into()),
        ("maxProperties", 1.into()),
        ("additionalProperties", false.into()),
    ])
}

fn table<const N: usize>(entries: [(&str, toml::Value); N]) -> toml::Value {
    toml::Value::Table(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// A value as JSON, written out with its members in the order they hold, two spaces to a level.
fn json(value: &toml::Value, depth: usize) -> String {
    let pad = "  ".repeat(depth + 1);
    let end = "  ".repeat(depth);
    let text = |s: &str| serde_json::to_string(s).unwrap_or_default();
    match value {
        toml::Value::String(s) => text(s),
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Datetime(d) => text(&d.to_string()),
        toml::Value::Array(items) if items.is_empty() => "[]".to_string(),
        toml::Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(|v| format!("{pad}{}", json(v, depth + 1))).collect();
            format!("[\n{}\n{end}]", inner.join(",\n"))
        }
        toml::Value::Table(t) if t.is_empty() => "{}".to_string(),
        toml::Value::Table(t) => {
            let inner: Vec<String> = (t.iter())
                .map(|(k, v)| format!("{pad}{}: {}", text(k), json(v, depth + 1)))
                .collect();
            format!("{{\n{}\n{end}}}", inner.join(",\n"))
        }
    }
}
