//! The form of a return of a flow: the shape alone, which the agents' program is given with every call of an agent cell
//! of the flow, and so the same bytes on every call of a session (design document `data-and-context`, 30 and 31). It is
//! made of the definitions alone: the engine's fields as `agents/return/` defines them, which every agent cell has
//! alike, and the tools the flow names, each shaped as `agents/tools/` defines it. The blocks a cell writes go in the
//! field `outputs`, under their names, which the form does not list: the engine checks what a return writes against
//! the cell it was entered for, once it is read. The rules here name no cell, no datum, no field and no tool.

use crate::agents::{Agents, Group, InForm, ReturnKind};
use crate::compile::Compiled;
use crate::source::Kind;

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

/// The form of a return of the flow, as JSON, its members in the order the files give them and two spaces to a level:
/// one of two kinds, never both nor neither, each holding the fields `agents/return/return.toml` gives it in its order,
/// with a field there only where its own file says it is one of that kind's. The kind that asks for calls holds each
/// tool of the flow as a list of its requests, and is left out where the flow names no tool. Or why it cannot be made:
/// the flow names a tool that `agents/tools/` does not define.
pub fn form(flow: &Compiled, agents: &Agents) -> Result<String, String> {
    let tools = tools(flow);
    let mut requests = Vec::new();
    for tool in &tools {
        let Some(defined) = agents.tools.get(*tool) else {
            let file = agents.source("agents/tools/");
            return Err(format!(
                "the flow {} names the tool {tool}, which {file} does not have",
                flow.name
            ));
        };
        requests.push((*tool, &defined.request));
    }
    let kind = |kind: ReturnKind, order: &[InForm]| {
        let mut properties = toml::Table::new();
        let mut required = Vec::new();
        for place in order {
            match place {
                InForm::Group(Group::Tools) => {
                    for (tool, request) in &requests {
                        let list = table([("type", "array".into()), ("items", (*request).clone())]);
                        properties.insert(tool.to_string(), list);
                    }
                }
                InForm::Field(name) => {
                    let Some(field) = agents.fields.get(name).filter(|f| f.kinds.contains(&kind)) else {
                        continue;
                    };
                    properties.insert(name.clone(), field.shape.clone());
                    if field.required {
                        required.push(toml::Value::String(name.clone()));
                    }
                }
            }
        }
        let mut object = toml::Table::new();
        object.insert("type".into(), "object".into());
        object.insert("properties".into(), toml::Value::Table(properties));
        if !required.is_empty() {
            object.insert("required".into(), toml::Value::Array(required));
        }
        object.insert("additionalProperties".into(), false.into());
        toml::Value::Table(object)
    };
    let form = &agents.returns.form;
    let mut kinds = toml::Table::new();
    if !tools.is_empty() {
        kinds.insert(ReturnKind::Calls.name().into(), kind(ReturnKind::Calls, &form.calls));
    }
    kinds.insert(
        ReturnKind::Outcome.name().into(),
        kind(ReturnKind::Outcome, &form.outcome),
    );
    // One of the two kinds, never both nor neither: the service refuses a choice at the top of a form, so the kinds
    // are two properties of which the form takes exactly one.
    let top = table([
        ("type", "object".into()),
        ("properties", toml::Value::Table(kinds)),
        ("minProperties", 1.into()),
        ("maxProperties", 1.into()),
        ("additionalProperties", false.into()),
    ]);
    Ok(json(&top, 0))
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
