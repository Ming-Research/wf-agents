//! What an agent is sent as the engine enters a cell, as the owner's page shows it for the flow as it is written:
//! every part, in the order it is sent, with the text of each part that is written already, and where it comes from.
//!
//! The system prompt is the engine's, made as the engine makes it (`flow::system`), and the message is built by the
//! builder the engine sends it with (crate `context`), as the files define it: every form of each part, with the
//! places of a run open. Nothing here reads what a session holds, which is known only in a run: a session is sent what
//! it lacks, so the page shows one list for a cell, and which form of each part an entry sends is the builder's, tried
//! by its tests.
//!
//! Every word sent is read from the files of the flow and from those beside it, and what the page is given of each part
//! is its key and attributes there, as written; this code holds no word, only how they are put together: which part
//! comes from which file. The form of a return is the one the engine gives the agents' program, made by the generator
//! both use (`flow::form`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use flow::agents::{Agents, Part as PartIs};
use flow::system::{with_slots, written};
use flow::{CellSpec, Compiled, Kind};
use serde::Serialize;

use crate::view::InputKind;

/// Where what every agent is sent alike is read from; nothing where the page is served without it.
#[derive(Clone, Debug, Default)]
pub struct Alike {
    /// The directory that holds the flows, beside which are `agents/` and `project.toml`.
    pub root: Option<PathBuf>,
    /// The directory of the flow shown, whose cells' files a part may come from.
    pub flow: Option<PathBuf>,
    /// The project's own checkout, whose files the page shows as they are there now; none where the engine follows a
    /// script and has no project.
    pub project: Option<PathBuf>,
}

/// What a part is, for the page to know what it may do with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum What {
    /// The system prompt and the tools of the program the agent runs in, which are not the engine's.
    Program,
    /// A part of the system prompt of the session.
    System,
    /// The form of the return.
    Form,
    /// A part of a message that is no content of a block, nor the task, nor what the cell returns.
    Message,
    /// The content of a block the cell is given.
    Input,
    /// The blocks the cell writes.
    Outputs,
    /// The cell's task.
    Task,
}

/// How a part reaches the agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Carried {
    /// The program's own, which the engine does not send.
    Program,
    /// In the system prompt of the session: taken once, as the session begins.
    System,
    /// Given with every call of the program.
    Call,
    /// In a message, as the cell is entered, where the session lacks it.
    Message,
}

/// A piece of the text of a part: the system prompt's builder's, which the page shows as it is sent.
pub use flow::system::Piece;

/// One part of what an agent is sent, as the files say it: the key it has there, its attributes, the file its words
/// come from, and its text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub what: What,
    /// How it reaches the agent.
    pub carried: Carried,
    /// Its key in the files: a file of the project's by its name in `project.toml`, a set of rules, a file of the
    /// engine's words, `form`, a part of a message by its `part`, a block by its name.
    pub name: String,
    /// Of a part written anew each time, when it is sent at all: its `only`.
    pub only: Option<String>,
    /// The file its words come from.
    pub source: String,
    /// The block, of the content of a block the cell is given.
    pub block: Option<String>,
    /// The kind the cell gives that block in, as its file writes it; none where it says none.
    pub kind: Option<String>,
    /// The text sent, where it is written already.
    pub pieces: Vec<Piece>,
    /// What is sent in its place where the session holds it already; nothing where nothing is.
    pub held: Vec<Piece>,
    /// Of the content of a block, what is sent in its place where the session holds it and the cell never worked from
    /// it: `held_new`.
    pub held_new: Vec<Piece>,
    /// Of the content of a block, what is sent in place of its text where the session lacks what the text needs.
    pub whole: Vec<Piece>,
    /// Of the triggers' part, the line of each trigger of the cell, by the trigger's name, in the order the cell names
    /// them, which come after `pieces` for each trigger that ran the cell; and then `after`.
    pub items: Vec<(String, Vec<Piece>)>,
    pub after: Vec<Piece>,
}

/// What the page is given of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Given {
    Compiled {
        /// For each agent's cell, by its id, the parts in the order they are sent.
        cells: BTreeMap<String, Vec<Part>>,
        /// The names of the engine's tools, so that the page tells a tool in the form from a field.
        tools: Vec<String>,
        /// What each engine's cell's act writes into its blocks, by the cell's id.
        acts: BTreeMap<String, ActWords>,
        /// Each of the engine's tools as its file in `agents/tools/` writes it, by the tool's name.
        tool_files: BTreeMap<String, ActWords>,
        /// Each name a text sent to an agent gives between backticks that nothing of the flow, the return, the tools
        /// or the settings defines, with the file it is in.
        unknown: Vec<Unknown>,
    },
    /// The definition does not compile, or cannot be read: there is nothing to work it out of.
    NotCompiled,
    /// What the engine says to every agent cannot be read, or does not fit the flow: why.
    Unread { why: String },
}

/// What a file of the engine's words beside the flows says, as it is written: of an act, what it writes, in
/// `agents/acts/`; of a tool, what it is, in `agents/tools/`. Each key, dotted where it is in a table, with its text and
/// the places in it filled in a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActWords {
    pub source: String,
    pub words: Vec<(String, Vec<Piece>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unknown {
    pub name: String,
    pub source: String,
}

/// All of it, for the flow shown.
pub fn given(flow: &Compiled, alike: &Alike) -> Given {
    let Some(root) = &alike.root else {
        return Given::Unread {
            why: "the page was not told where the flows are".to_string(),
        };
    };
    let agents = match flow::agents::read(root) {
        Ok(agents) => agents,
        Err(why) => return Given::Unread { why },
    };
    let build = Build {
        flow,
        agents: &agents,
        alike,
    };
    let mut cells = BTreeMap::new();
    for cell in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        match build.parts(cell) {
            Ok(parts) => cells.insert(cell.id.as_str().to_string(), parts),
            Err(why) => return Given::Unread { why },
        };
    }
    let mut acts = BTreeMap::new();
    for cell in &flow.cells {
        let Some(act) = &cell.act else { continue };
        match act_words(&agents, act) {
            Ok(words) => acts.insert(cell.id.as_str().to_string(), words),
            Err(why) => return Given::Unread { why },
        };
    }
    let mut tool_files = BTreeMap::new();
    for tool in agents.tools.keys() {
        let file = format!("{}/tools/{tool}.toml", flow::agents::AGENTS_DIR);
        match file_words(&agents, &file) {
            Ok(words) => tool_files.insert(tool.clone(), words),
            Err(why) => return Given::Unread { why },
        };
    }
    let unknown = build.unknown(&cells);
    Given::Compiled {
        cells,
        tools: agents.tools.keys().cloned().collect(),
        acts,
        tool_files,
        unknown,
    }
}

/// The words of an act, read from its file as it is written, in its order.
fn act_words(agents: &flow::agents::Agents, act: &str) -> Result<ActWords, String> {
    let file = format!("{}/{}/{act}.toml", flow::agents::AGENTS_DIR, flow::agents::ACTS_DIR);
    file_words(agents, &file)
}

/// The words of a file of the engine's beside the flows, read as it is written, in its order: each key, dotted where it
/// is in a table, with its text.
fn file_words(agents: &flow::agents::Agents, file: &str) -> Result<ActWords, String> {
    let path = agents.root.join(file);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let table: toml::Table = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    fn flat(table: &toml::Table, before: &str, out: &mut Vec<(String, Vec<Piece>)>) {
        for (key, value) in table {
            let key = format!("{before}{key}");
            match value {
                toml::Value::Table(inner) => flat(inner, &format!("{key}."), out),
                toml::Value::String(text) => out.push((key, with_slots(text))),
                other => out.push((key, written(&other.to_string()))),
            }
        }
    }
    let mut words = Vec::new();
    flat(&table, "", &mut words);
    Ok(ActWords {
        source: agents.source(file),
        words,
    })
}

struct Build<'a> {
    flow: &'a Compiled,
    agents: &'a Agents,
    alike: &'a Alike,
}

impl<'a> Build<'a> {
    /// Each name between backticks in what the cells are sent, but for the project's own files, that nothing defines:
    /// no block, value of a choice, field, tool, part of the shape of either, kind of return or setting.
    fn unknown(&self, cells: &BTreeMap<String, Vec<Part>>) -> Vec<Unknown> {
        let agents = self.agents;
        let mut known: Vec<String> = vec!["calls".into(), "outcome".into()];
        for (name, block) in &self.flow.blocks {
            known.push(name.as_str().to_string());
            known.extend(block.values.iter().flatten().cloned());
        }
        for (name, field) in &agents.fields {
            known.push(name.clone());
            properties(&field.shape, &mut known);
        }
        for (name, tool) in &agents.tools {
            known.push(name.clone());
            properties(&tool.request, &mut known);
        }
        known.extend(agents.project.project.keys().map(|k| format!("project.{k}")));
        known.extend(
            self.flow
                .cells
                .iter()
                .flat_map(|c| c.triggers.iter().map(|t| t.name.clone())),
        );
        known.extend(agents.project.installation.names.iter().cloned());
        // The project's own files are the project's to word, and are not checked.
        let project: Vec<&String> = agents.project.files.values().map(|f| &f.path).collect();
        let mut out = std::collections::BTreeSet::new();
        for part in cells
            .values()
            .flatten()
            .filter(|p| !project.contains(&&p.source) && p.what != What::Form)
        {
            for piece in &part.pieces {
                let Piece::Text { text } = piece else { continue };
                for name in ticked_names(text) {
                    if !known.iter().any(|k| k == name) {
                        out.insert(Unknown {
                            name: name.to_string(),
                            source: part.source.clone(),
                        });
                    }
                }
            }
        }
        out.into_iter().collect()
    }

    fn source(&self, path: &str) -> String {
        self.agents.source(path)
    }

    /// The path of the file a cell's task comes from, from the directory that holds the flows' directory: the file the
    /// task is written in, where it has one of its own, else the cell's.
    fn task_source(&self, cell: &CellSpec) -> String {
        let flow = (self.alike.flow.as_deref())
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        match &cell.task_file {
            Some(file) => self.source(&format!("{flow}/{file}")),
            None => self.source(&format!("{flow}/cells/{}.toml", cell.id.as_str())),
        }
    }

    fn part(&self, what: What, carried: Carried, name: &str, source: String, pieces: Vec<Piece>) -> Part {
        Part {
            what,
            carried,
            name: name.to_string(),
            only: None,
            source,
            block: None,
            kind: None,
            pieces,
            held: Vec::new(),
            held_new: Vec::new(),
            whole: Vec::new(),
            items: Vec::new(),
            after: Vec::new(),
        }
    }

    fn parts(&self, cell: &CellSpec) -> Result<Vec<Part>, String> {
        let agents = self.agents;
        let session = self.source("agents/session.toml");
        let mut out = vec![self.part(What::Program, Carried::Program, "program", session, Vec::new())];
        // The form goes with the program's own tools, as a tool the program makes of it, which comes before the system
        // prompt in every request.
        // The form, as the engine gives it to the program: made by the one generator both use.
        let form = written(&flow::form::form(self.flow, agents)?);
        out.push(self.part(
            What::Form,
            Carried::Call,
            "form",
            self.source("agents/return/return.toml"),
            form,
        ));
        // The system prompt, as the engine makes it (`flow::system`), the project's files as they are in its checkout.
        let read = |path: &str| {
            let checkout = self.alike.project.as_ref();
            Ok(checkout.and_then(|checkout| std::fs::read_to_string(checkout.join(path)).ok()))
        };
        for part in flow::system::parts(agents, self.flow, cell, read).map_err(|wrong| wrong.join("; "))? {
            out.push(self.part(What::System, Carried::System, &part.name, part.source, part.pieces));
        }
        // The message as the engine's builder defines it, each form of each part with the places of a run open.
        for part in context::defined(self.flow, agents, cell) {
            let (what, source) = self.of_part(cell, part.part);
            out.push(Part {
                what,
                carried: Carried::Message,
                name: part.block.as_ref().map_or(part.part.name(), |b| b.as_str()).to_string(),
                only: part.only.map(|o| o.name().to_string()),
                source,
                block: part.block.map(|b| b.as_str().to_string()),
                kind: part.kind.map(|k| kind_name(InputKind::of(k)).to_string()),
                pieces: part.pieces,
                held: part.held,
                held_new: part.held_new,
                whole: part.whole,
                items: part.items,
                after: part.after,
            });
        }
        Ok(out)
    }

    /// What a part of the message is, for the page, and the file its words come from.
    fn of_part(&self, cell: &CellSpec, part: PartIs) -> (What, String) {
        match part {
            PartIs::Context => (What::Input, self.source("agents/message/given.toml")),
            PartIs::Outputs => (What::Outputs, self.source("agents/return/return.toml")),
            PartIs::Task => (What::Task, self.task_source(cell)),
            PartIs::Results => (What::Message, self.source("agents/message/results.toml")),
            PartIs::SentBack => (What::Message, self.source("agents/return/sent_back.toml")),
            PartIs::Taken | PartIs::Heading | PartIs::Work | PartIs::Main | PartIs::Triggers => {
                (What::Message, self.source("agents/message/message.toml"))
            }
        }
    }
}

/// The names of the properties of a shape, at any depth.
fn properties(shape: &toml::Value, out: &mut Vec<String>) {
    match shape {
        toml::Value::Table(table) => {
            if let Some(toml::Value::Table(props)) = table.get("properties") {
                out.extend(props.keys().cloned());
            }
            table.values().for_each(|v| properties(v, out));
        }
        toml::Value::Array(items) => items.iter().for_each(|v| properties(v, out)),
        _ => {}
    }
}

/// What stands between backticks in a text that has the form of a name: small letters, figures and underscores,
/// beginning with a letter, in parts joined by dots, as `plan_owner`, `owner.language` or `agents.subagents.most`.
fn ticked_names(text: &str) -> Vec<&str> {
    let is_name = |s: &str| {
        let mut parts = s.split('.');
        let ok = |p: &str| {
            p.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        };
        let first = parts.next().is_some_and(ok);
        let rest: Vec<&str> = parts.collect();
        first && rest.iter().all(|p| ok(p))
    };
    text.split('`').skip(1).step_by(2).filter(|s| is_name(s)).collect()
}

/// A kind as the files write it.
fn kind_name(kind: InputKind) -> &'static str {
    match kind {
        InputKind::Latest => "latest",
        InputKind::LastAndLatest => "last_and_latest",
        InputKind::All => "all",
    }
}
