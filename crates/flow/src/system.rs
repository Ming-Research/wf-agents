//! The system prompt of a line's session, made from the files beside the flows: the parts `agents/session.toml` names,
//! in the order it names them. A file of the project that `project.toml` names, headed as `session.toml` says; the
//! sets of rules for the agents that the line takes; a text of the engine's own words; and how a cell returns, made from
//! `agents/return/` with the words of each field and of each tool the flow names. It is the same for every cell of a
//! line, whose sets of rules are the line's, and holds only what the line shares (principle
//! the-system-prompt-holds-what-a-line-shares).
//!
//! The engine gives it to a session as the session begins, and the owner's page shows it, both from [`parts`], so
//! that the page shows what is sent. Nothing here holds a word that is sent: only which part comes from which file, and
//! how the parts are put together.
//!
//! A part's text is kept in [`Piece`]s, what is sent as written apart from each place, `{name}`, that the engine fills
//! in a run; the page shows a place by its name. [`prompt`] puts the parts together as they are sent: each place
//! filled, and each setting a text names by its name between backticks given its value beside the name
//! ([`Settings`]), but in a file of the project, which is given as the project wrote it. Each part keeps the texts of
//! the files it is made of, each with its file ([`Said`]), so that [`check`] says what is wrong with a text against
//! the file that holds it, before any session begins; [`flow_texts`] are the flow's own texts sent in a message, which
//! name settings as well.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::agents::{
    AGENTS_DIR, Agents, InForm, Installation, LinePart, OUTPUTS, OfTheLine, PROJECT_FILE, Project, ProjectPart,
    RULES_FILE, ReturnKind, ReturnsPart, SessionPart, ToldAs, WordsPart, YOUR_LINE_FILE,
};
use crate::{CellSpec, Compiled, Kind};

/// The file, from the directory that holds the flows, that the part on returning is put together by.
const RETURN_FILE: &str = "agents/return/return.toml";

/// The file, from the directory that holds the flows, that says how a file of the project stands in the prompt.
const SESSION_FILE: &str = "agents/session.toml";

/// What a part of a line's system prompt is, as `agents/session.toml` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Of {
    /// A file of the project.
    Project,
    /// A set of rules for the agents that the line takes.
    Rules,
    /// A text of the engine's own words.
    Words,
    /// How a cell returns.
    Returns,
}

/// One part of a line's system prompt, as the files say it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Part {
    pub of: Of,
    /// Its key in the files: a file of the project by its name in `project.toml`, a set of rules by its name in
    /// `rules.toml`, a text of the engine's by its file in `agents/`, how a cell returns as `session.toml` names it.
    pub name: String,
    /// The file its words come from: a file of the project by its path in the project's checkout; any other by its
    /// path from the directory that holds the flows, with that directory's name before it.
    pub source: String,
    /// Its text; nothing of a file of the project that was not there to read.
    pub pieces: Vec<Piece>,
    /// Each text of the files beside the flows that the part is made of, with the places the builder fills filled:
    /// of a file of the project, the heading it stands under; of the engine's words, what fills `{your_line}` as well.
    pub texts: Vec<Said>,
}

/// A text of a file beside the flows, with that file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Said {
    /// The file, by its path from the directory that holds the flows, with that directory's name before it; or, of a
    /// flow's own text, the thing of the flow it is.
    pub source: String,
    pub text: String,
    /// Its places, `{name}`, are filled as it is sent; else it is sent as written, braces and all.
    pub places: bool,
}

/// A piece of the text of a part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "piece", rename_all = "camelCase")]
pub enum Piece {
    /// Sent as written.
    Text { text: String },
    /// A place filled in a run, by its name.
    Slot { name: String },
}

/// The parts of the system prompt of the session of the cell's line, in their order. A file of the project is read by
/// `read`, by its path in the project's checkout: its text, nothing where there is nothing to read it in, or why it
/// cannot be read. Or everything that keeps it from being made, as [`made`] says it.
pub fn parts(
    agents: &Agents,
    flow: &Compiled,
    cell: &CellSpec,
    read: impl Fn(&str) -> Result<Option<String>, String>,
) -> Result<Vec<Part>, Vec<String>> {
    let (parts, wrong) = made(agents, flow, cell, read);
    if wrong.is_empty() { Ok(parts) } else { Err(wrong) }
}

/// The parts of the system prompt of the session of the cell's line that can be made, in their order, and what keeps
/// the others from being made, each once: a file of the project that cannot be read, a set of rules the line takes or
/// a tool the flow names that nothing beside the flows defines.
pub fn made(
    agents: &Agents,
    flow: &Compiled,
    cell: &CellSpec,
    read: impl Fn(&str) -> Result<Option<String>, String>,
) -> (Vec<Part>, Vec<String>) {
    let session = &agents.session;
    let mut out = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    for part in &session.parts {
        match part {
            SessionPart::Project(ProjectPart { project }) => {
                let Some(file) = agents.project.files.get(project) else {
                    wrong.push(format!(
                        "{PROJECT_FILE} has no file `{project}`, which session.toml names"
                    ));
                    continue;
                };
                let heading = Said {
                    source: agents.source(SESSION_FILE),
                    text: as_written(&fill(&session.heading, &[("path", &file.path), ("text", "")])),
                    places: true,
                };
                let pieces = match read(&file.path) {
                    Ok(Some(text)) => fill(&session.heading, &[("path", &file.path), ("text", text.trim_end())]),
                    Ok(None) => Vec::new(),
                    Err(why) => {
                        wrong.push(why);
                        continue;
                    }
                };
                out.push(Part {
                    of: Of::Project,
                    name: project.clone(),
                    source: file.path.clone(),
                    pieces,
                    texts: vec![heading],
                });
            }
            SessionPart::Line(LinePart { line: OfTheLine::Rules }) => {
                for name in &cell.system {
                    match rules(agents, flow, name) {
                        Ok(part) => out.push(part),
                        Err(why) => wrong.push(why),
                    }
                }
            }
            SessionPart::Words(WordsPart { words }) => {
                let text = agents.words.get(words).map(String::as_str).unwrap_or_default();
                let source = agents.source(&format!("{AGENTS_DIR}/{words}"));
                let pieces = with_slots(text.trim_end());
                let mut texts = vec![Said {
                    source: source.clone(),
                    text: text.to_string(),
                    places: true,
                }];
                // What fills `{your_line}` is the engine's words as well, with the commands its place names filled.
                if pieces
                    .iter()
                    .any(|p| matches!(p, Piece::Slot { name } if name == "your_line"))
                {
                    let line = &agents.your_line;
                    let source = agents.source(&format!("{AGENTS_DIR}/{YOUR_LINE_FILE}"));
                    for text in [
                        &line.reads_only,
                        &line.writes,
                        &line.locks.replace("{commands}", ""),
                        &line.or,
                    ] {
                        texts.push(Said {
                            source: source.clone(),
                            text: text.clone(),
                            places: false,
                        });
                    }
                }
                out.push(Part {
                    of: Of::Words,
                    name: words.clone(),
                    source,
                    pieces,
                    texts,
                });
            }
            SessionPart::Returns(ReturnsPart { returns }) => {
                let (mut texts, filling, missing) = returning(agents, flow);
                wrong.extend(missing);
                let pieces = with_slots(&texts.iter().map(|s| s.text.as_str()).collect::<String>());
                // What fills a place of a tool's words is sent as well.
                texts.extend(filling);
                out.push(Part {
                    of: Of::Returns,
                    name: returns.name().to_string(),
                    source: agents.source(RETURN_FILE),
                    pieces,
                    texts,
                });
            }
        }
    }
    let mut once = Vec::new();
    for what in wrong {
        if !once.contains(&what) {
            once.push(what);
        }
    }
    (out, once)
}

/// A set of rules the line takes, by its name: one beside the flows, in `rules.toml`; of a flow written before it was
/// laid out one thing to a file, its own, in its `rules/`.
fn rules(agents: &Agents, flow: &Compiled, name: &str) -> Result<Part, String> {
    let (text, source) = match (flow.rules.get(name), agents.rules.get(name)) {
        (Some(text), _) => (text.as_str(), format!("rules/{name}.md")),
        (None, Some(set)) => {
            let dir = RULES_FILE.rsplit_once('/').map_or("", |(dir, _)| dir);
            (set.text.as_str(), agents.source(&format!("{dir}/{}", set.file)))
        }
        (None, None) => return Err(format!("{RULES_FILE} has no set `{name}`, which a line takes")),
    };
    Ok(Part {
        of: Of::Rules,
        name: name.to_string(),
        source: source.clone(),
        pieces: written(text.trim_end()),
        texts: vec![Said {
            source,
            text: text.to_string(),
            places: false,
        }],
    })
}

/// The names of the fields of a kind of return, in the order of the form.
fn fields_of(agents: &Agents, kind: ReturnKind) -> Vec<&str> {
    let form = &agents.returns.form;
    let order = match kind {
        ReturnKind::Calls => &form.calls,
        ReturnKind::Outcome => &form.outcome,
    };
    (order.iter())
        .filter_map(|place| match place {
            InForm::Field(name) => Some(name.as_str()),
            InForm::Group(_) => None,
        })
        .filter(|name| agents.fields.get(*name).is_some_and(|f| f.kinds.contains(&kind)))
        .collect()
}

/// How a cell returns, the part on returning of a line's system prompt: the same for every cell of the flow, each field
/// and each tool the flow names with its words, in the order `[how_you_return]` of `return.toml` sets. Or that the flow
/// names a tool that `agents/tools/` does not define.
pub fn how_you_return(agents: &Agents, flow: &Compiled) -> Result<Vec<Piece>, Vec<String>> {
    let (texts, _, wrong) = returning(agents, flow);
    if !wrong.is_empty() {
        return Err(wrong);
    }
    Ok(with_slots(&texts.iter().map(|s| s.text.as_str()).collect::<String>()))
}

/// The texts how a cell returns is made of, each with its file, in their order, which written one after another are
/// its text; the words that may fill a place of a tool's words, each with its tool's file; and each tool the flow
/// names that `agents/tools/` does not define.
fn returning(agents: &Agents, flow: &Compiled) -> (Vec<Said>, Vec<Said>, Vec<String>) {
    let told = &agents.returns.how_you_return;
    let ret = agents.source(RETURN_FILE);
    let field = |name: &str| agents.source(&format!("{AGENTS_DIR}/return/fields/{name}.toml"));
    let tool = |name: &str| agents.source(&format!("{AGENTS_DIR}/tools/{name}.toml"));
    let item = |words: &str| told.item.replace("{text}", words.trim_end());
    let mut said: Vec<Said> = Vec::new();
    // A paragraph after a blank line, an item of a list after a line's end, the words told with those of `outputs`
    // after a space.
    let mut put = |source: String, before: &str, text: &str| {
        let before = if said.is_empty() { "" } else { before };
        said.push(Said {
            source,
            text: format!("{before}{text}"),
            places: true,
        });
    };
    let mut in_order = fields_of(agents, ReturnKind::Outcome);
    for name in fields_of(agents, ReturnKind::Calls) {
        if !in_order.contains(&name) {
            in_order.push(name);
        }
    }
    let told_as = |as_: ToldAs| {
        (in_order.iter())
            .filter(move |name| agents.fields[**name].told == as_)
            .map(|name| (*name, &agents.fields[*name]))
    };
    put(ret.clone(), "\n\n", &told.heading);
    put(ret.clone(), "\n\n", &told.first);
    put(ret.clone(), "\n\n", told.kinds.trim_end());
    for (name, words) in told_as(ToldAs::Alone) {
        put(field(name), "\n\n", words.words.trim_end());
        if name == OUTPUTS {
            for (with, words) in told_as(ToldAs::WithOutputs) {
                put(field(with), " ", words.words.trim_end());
            }
        }
    }
    put(ret.clone(), "\n\n", told.every_time.trim_end());
    let listed: Vec<(&String, _)> = (told.listed.iter())
        .filter_map(|n| agents.fields.get(n).map(|f| (n, f)))
        .collect();
    if !listed.is_empty() {
        put(ret.clone(), "\n\n", told.fields.trim_end());
        for (i, (name, words)) in listed.iter().enumerate() {
            put(field(name), if i == 0 { "\n\n" } else { "\n" }, &item(&words.words));
        }
        for (name, words) in &listed {
            if let Some(also) = &words.also {
                put(field(name), "\n\n", also.trim_end());
            }
        }
    }
    let reads: Vec<String> = (told.reads.iter())
        .filter(|n| agents.fields.contains_key(n.as_str()))
        .map(|n| format!("`{n}`"))
        .collect();
    if !reads.is_empty() {
        put(
            ret.clone(),
            "\n\n",
            &told.owner_reads.replace("{fields}", &and(&reads, &told.and)),
        );
    }
    let mut wrong = Vec::new();
    let mut filling = Vec::new();
    // The tools of the flow's form, which every agent cell of it has.
    let tools = crate::form::tools(flow);
    if !tools.is_empty() {
        put(ret.clone(), "\n\n", told.calls.trim_end());
        put(ret.clone(), "\n\n", told.tools.trim_end());
        let mut first = true;
        for name in tools {
            match agents.tools.get(name) {
                Some(words) => {
                    put(tool(name), if first { "\n\n" } else { "\n" }, &item(&words.words));
                    first = false;
                    for text in words.places.values().flat_map(|ways| ways.values()) {
                        filling.push(Said {
                            source: tool(name),
                            text: text.clone(),
                            places: false,
                        });
                    }
                }
                None => wrong.push(format!(
                    "the flow names the tool `{name}`, which agents/tools/ does not have"
                )),
            }
        }
    }
    (said, filling, wrong)
}

/// What is wrong with the texts the parts are made of, each with the file that holds it, each once: a place that
/// `fills` says the engine does not fill, and, but in what heads a file of the project, a setting a text names that
/// has no value.
pub fn check(parts: &[Part], settings: &Settings, fills: impl Fn(&str) -> bool) -> Vec<String> {
    let mut wrong: Vec<String> = Vec::new();
    let mut said = |what: String| {
        if !wrong.contains(&what) {
            wrong.push(what);
        }
    };
    for part in parts {
        for text in &part.texts {
            for piece in with_slots(&text.text).into_iter().filter(|_| text.places) {
                if let Piece::Slot { name } = piece
                    && !fills(&name)
                {
                    said(format!("{}: the engine fills no place `{{{name}}}`", text.source));
                }
            }
            if part.of == Of::Project {
                continue;
            }
            for name in settings.named_in(&text.text).err().unwrap_or_default() {
                said(format!(
                    "{}: names the setting `{name}`, which has no value",
                    text.source
                ));
            }
        }
    }
    wrong
}

/// The flow's own texts that a message sends its agent cells, each with what of the flow it is: each agent cell's
/// task, what each of its triggers says it does, and the words of each block it writes.
pub fn flow_texts(flow: &Compiled) -> Vec<Said> {
    let mut out = Vec::new();
    for cell in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        let id = cell.id.as_str();
        if let Some(task) = &cell.task {
            out.push(Said {
                source: format!("cell {id}, its task"),
                text: task.clone(),
                places: false,
            });
        }
        for trigger in &cell.triggers {
            if let Some(does) = &trigger.does {
                out.push(Said {
                    source: format!("cell {id}, what it does for {}", trigger.name),
                    text: does.clone(),
                    places: false,
                });
            }
        }
    }
    for (name, block) in &flow.blocks {
        if let Some(what) = &block.what {
            out.push(Said {
                source: format!("block {}, its words", name.as_str()),
                text: what.clone(),
                places: false,
            });
        }
    }
    out
}

/// The names the parts of a line's system prompt that are not the line's sets of rules are given, as
/// `agents/session.toml` names them: a file of the project by its name in `project.toml`, a text of the engine's by its
/// file, how a cell returns by its name. A set of rules that took one of them would be told apart from it by nothing.
pub fn reserved(agents: &Agents) -> Vec<String> {
    (agents.session.parts.iter())
        .filter_map(|part| match part {
            SessionPart::Project(ProjectPart { project }) => Some(project.clone()),
            SessionPart::Words(WordsPart { words }) => Some(words.clone()),
            SessionPart::Returns(ReturnsPart { returns }) => Some(returns.name().to_string()),
            SessionPart::Line(_) => None,
        })
        .collect()
}

/// The settings a text sent to an agent may name, each by its name, with its value, or with none where the
/// installation has none for it: the project's, in `[project]` of `project.toml`, each as `project.<name>`; and the
/// installation's that `names` in `agents/installation.toml` beside the flows lists, whose values the installation
/// holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings(BTreeMap<String, Option<String>>);

impl Settings {
    /// The settings `project.toml` gives, and those of the installation `names` lists, each with the value `value`
    /// gives it.
    pub fn of(project: &Project, names: &Installation, installation: impl Fn(&str) -> Option<String>) -> Settings {
        let mut values = BTreeMap::new();
        for (name, value) in &project.project {
            values.insert(format!("project.{name}"), Some(value.clone()));
        }
        for name in &names.names {
            values.entry(name.clone()).or_insert_with(|| installation(name));
        }
        Settings(values)
    }

    /// The names of the settings, in the order of their names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// The text with the value of each setting it names written beside the name: `` `owner.language` `` becomes
    /// `` `owner.language` (Chinese) ``. A name is what stands between two backticks outside a block of code, where it
    /// is one of these settings; anything else between backticks is left as it is. Or every setting the text names
    /// that has no value, each once, in the order of the text: such a text is not given with the name bare, which the
    /// agent could not tell from a setting the owner left empty.
    pub fn named_in(&self, text: &str) -> Result<String, Vec<String>> {
        let mut out = String::with_capacity(text.len());
        let mut lacking: Vec<String> = Vec::new();
        let mut at = 0;
        for (start, end) in quoted(text) {
            let name = &text[start..end];
            match self.0.get(name) {
                None => {}
                Some(Some(value)) => {
                    out.push_str(&text[at..=end]);
                    out.push_str(&format!(" ({value})"));
                    at = end + 1;
                }
                Some(None) if !lacking.iter().any(|l| l == name) => lacking.push(name.to_string()),
                Some(None) => {}
            }
        }
        out.push_str(&text[at..]);
        if lacking.is_empty() { Ok(out) } else { Err(lacking) }
    }
}

/// The system prompt the parts make, as it is sent: each part's text with its places filled by `fill`, and, but in a
/// file of the project, each setting it names given its value beside its name; the parts apart by a blank line, a part
/// with no text left out. Or what is wrong with it, each once, with the file it is in: a place `fill` does not fill, a
/// setting named that has no value.
pub fn prompt(
    parts: &[Part],
    settings: &Settings,
    fill: impl Fn(&str) -> Option<String>,
) -> Result<String, Vec<String>> {
    let mut wrong: Vec<String> = Vec::new();
    let mut said = |what: String| {
        if !wrong.contains(&what) {
            wrong.push(what);
        }
    };
    let mut texts = Vec::new();
    for part in parts {
        let mut text = String::new();
        for piece in &part.pieces {
            match piece {
                Piece::Text { text: written } => text.push_str(written),
                Piece::Slot { name } => match fill(name) {
                    Some(value) => text.push_str(&value),
                    None => said(format!("{}: the engine fills no place `{{{name}}}`", part.source)),
                },
            }
        }
        if part.of != Of::Project {
            match settings.named_in(&text) {
                Ok(named) => text = named,
                Err(lacking) => {
                    for name in lacking {
                        said(format!(
                            "{}: names the setting `{name}`, which has no value",
                            part.source
                        ));
                    }
                }
            }
        }
        if !text.is_empty() {
            texts.push(text);
        }
    }
    if wrong.is_empty() {
        Ok(texts.join("\n\n"))
    } else {
        Err(wrong)
    }
}

/// Where each text between two backticks stands, from its first byte to the byte of the backtick that closes it. A
/// block of code, between fences of three backticks or more, holds none: what it holds is code, given as it is. A
/// backtick that nothing closes before a blank line or the end of the text opens nothing.
fn quoted(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut open: Option<usize> = None;
    let mut fenced = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' && open.is_some() {
            let line = bytes[i + 1..].iter().take_while(|b| **b != b'\n');
            if line.clone().all(|b| b.is_ascii_whitespace()) {
                open = None;
            }
        }
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
        if run >= 3 {
            fenced = !fenced;
            open = None;
        } else if run == 1 && !fenced {
            match open.take() {
                Some(start) => spans.push((start, i)),
                None => open = Some(i + 1),
            }
        }
        i += run;
    }
    spans
}

/// Names joined by commas, the last by `and`.
fn and(names: &[String], and: &str) -> String {
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{}{and}{last}", rest.join(", ")),
        None => String::new(),
    }
}

/// Pieces as the files write them, each place by its name.
pub fn as_written(pieces: &[Piece]) -> String {
    (pieces.iter())
        .map(|piece| match piece {
            Piece::Text { text } => text.clone(),
            Piece::Slot { name } => format!("{{{name}}}"),
        })
        .collect()
}

/// A text sent as written, or nothing where there is none.
pub fn written(text: &str) -> Vec<Piece> {
    if text.is_empty() {
        return Vec::new();
    }
    vec![Piece::Text { text: text.to_string() }]
}

/// A text with the places the flow fills in filled, each by its name; a place filled in a run stays one. What fills a
/// place is sent as it is: a place in it is not filled again.
pub fn fill(text: &str, known: &[(&str, &str)]) -> Vec<Piece> {
    let pieces = (with_slots(text).into_iter())
        .map(|piece| match &piece {
            Piece::Slot { name } => match known.iter().find(|(k, _)| k == name) {
                Some((_, value)) => Piece::Text {
                    text: value.to_string(),
                },
                None => piece,
            },
            Piece::Text { .. } => piece,
        })
        .collect();
    join(pieces)
}

/// Pieces with each run of text made one, and no empty text.
pub fn join(pieces: Vec<Piece>) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for piece in pieces {
        match (out.last_mut(), piece) {
            (Some(Piece::Text { text }), Piece::Text { text: more }) => text.push_str(&more),
            (_, piece) => out.push(piece),
        }
    }
    out.retain(|p| !matches!(p, Piece::Text { text } if text.is_empty()));
    out
}

/// A text with its places to fill in apart from what is sent as written, the places found as every reader of the
/// engine's words finds them ([`crate::agents::pieces`]).
pub fn with_slots(text: &str) -> Vec<Piece> {
    let pieces = (crate::agents::pieces(text).into_iter()).map(|piece| match piece {
        crate::agents::Piece::Text(text) => Piece::Text { text: text.to_string() },
        crate::agents::Piece::Place(name) => Piece::Slot { name: name.to_string() },
    });
    join(pieces.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Piece {
        Piece::Text { text: text.into() }
    }

    #[test]
    fn a_place_to_fill_in_is_told_apart_from_what_is_sent_as_written_and_a_lone_brace_is_sent() {
        assert_eq!(
            with_slots("Work here. {your_line} Write in {owner_language}."),
            vec![
                text("Work here. "),
                Piece::Slot {
                    name: "your_line".into()
                },
                text(" Write in "),
                Piece::Slot {
                    name: "owner_language".into()
                },
                text(".")
            ]
        );
        assert_eq!(with_slots("a {\"type\": 1} b {x"), vec![text("a {\"type\": 1} b {x")]);
        assert_eq!(with_slots("as {{name}} says"), vec![text("as {{name}} says")]);
        assert_eq!(with_slots(""), Vec::<Piece>::new());
    }

    #[test]
    fn what_fills_a_place_is_not_filled_again() {
        assert_eq!(
            fill("## {task}", &[("task", "say {{q}} and {x}")]),
            vec![text("## say {{q}} and {x}")]
        );
    }

    /// The settings of the tests: the project's `research` and `test_names`, and the installation's `owner.language`,
    /// `agents.subagents.most` and `github.checks`, the last with no value.
    fn settings() -> Settings {
        let project: Project = toml::from_str(
            "tree = \"design/\"\n[files]\n[project]\nresearch = \"research/\"\ntest_names = \"By its own name.\"\n",
        )
        .unwrap();
        let names = Installation {
            names: ["owner.language", "agents.subagents.most", "github.checks"]
                .map(String::from)
                .to_vec(),
            ..Default::default()
        };
        Settings::of(&project, &names, |name| match name {
            "owner.language" => Some("Chinese".to_string()),
            "agents.subagents.most" => Some("3".to_string()),
            _ => None,
        })
    }

    // Each setting a text names has its value beside its name, wherever it stands and however often; a name that is no
    // setting, as a path, a block or a key the installation has but the flow does not name, has none.
    #[test]
    fn each_named_setting_has_its_value_beside_it_and_an_unknown_one_has_none() {
        let text = "Write in `owner.language`; at most `agents.subagents.most`; name tests as `project.test_names` \
                    says, in `project.research`. Not `agents.slots`, `owner.voice`, `design/README.md`, `plan` or \
                    `owner.language is`. Again `owner.language`.";
        assert_eq!(
            settings().named_in(text).unwrap(),
            "Write in `owner.language` (Chinese); at most `agents.subagents.most` (3); name tests as \
             `project.test_names` (By its own name.) says, in `project.research` (research/). Not `agents.slots`, \
             `owner.voice`, `design/README.md`, `plan` or `owner.language is`. Again `owner.language` (Chinese)."
        );
        assert_eq!(
            settings().names().collect::<Vec<_>>(),
            [
                "agents.subagents.most",
                "github.checks",
                "owner.language",
                "project.research",
                "project.test_names"
            ]
        );
    }

    // A setting a text names that has no value is said, once, and the text is not given with it bare.
    #[test]
    fn a_named_setting_with_no_value_is_said_and_not_given_bare() {
        let text = "Among `github.checks`, in `owner.language`, again `github.checks`.";
        assert_eq!(settings().named_in(text).unwrap_err(), ["github.checks"]);
    }

    // What a block of code holds is code: no setting is named in it. A backtick nothing closes before a blank line opens
    // nothing, so that the setting after it is still named.
    #[test]
    fn neither_code_nor_a_stray_backtick_hides_or_makes_a_name() {
        let fenced = "```toml\nkey = `owner.language`\n```\nIn `owner.language`.";
        assert_eq!(
            settings().named_in(fenced).unwrap(),
            "```toml\nkey = `owner.language`\n```\nIn `owner.language` (Chinese)."
        );
        let stray = "A stray ` here.\n\nWrite in `owner.language`.";
        assert_eq!(
            settings().named_in(stray).unwrap(),
            "A stray ` here.\n\nWrite in `owner.language` (Chinese)."
        );
    }

    // The parts are put together with their places filled and their settings named, but in a file of the project,
    // which is given as written; a place nothing fills and a setting with no value are each said with the file.
    #[test]
    fn the_prompt_fills_the_places_and_names_the_settings_but_in_the_projects_files() {
        let part = |of, source: &str, pieces| Part {
            of,
            name: source.to_string(),
            source: source.to_string(),
            pieces,
            texts: Vec::new(),
        };
        let parts = [
            part(
                Of::Project,
                "AGENTS.md",
                written("# `AGENTS.md`\n\nIn `owner.language`."),
            ),
            part(Of::Project, "absent.md", Vec::new()),
            part(Of::Words, "w.md", with_slots("Here. {your_line} In `owner.language`.")),
        ];
        let fill = |name: &str| (name == "your_line").then(|| "You commit.".to_string());
        assert_eq!(
            prompt(&parts, &settings(), fill).unwrap(),
            "# `AGENTS.md`\n\nIn `owner.language`.\n\nHere. You commit. In `owner.language` (Chinese)."
        );
        let wrong = [part(
            Of::Rules,
            "r.md",
            with_slots("{other} `github.checks` {other} `github.checks`"),
        )];
        assert_eq!(
            prompt(&wrong, &settings(), fill).unwrap_err(),
            [
                "r.md: the engine fills no place `{other}`",
                "r.md: names the setting `github.checks`, which has no value"
            ]
        );
    }

    // The start check says what is wrong with each text against the file that holds it, each once: a place the engine
    // does not fill, where the text's places are filled, and a setting named that has no value, but in what heads a file
    // of the project.
    #[test]
    fn the_check_says_each_fault_against_the_file_that_holds_its_text() {
        let said = |source: &str, text: &str, places: bool| Said {
            source: source.to_string(),
            text: text.to_string(),
            places,
        };
        let part = |of, texts| Part {
            of,
            name: String::new(),
            source: "part.toml".to_string(),
            pieces: Vec::new(),
            texts,
        };
        let parts = [
            part(
                Of::Project,
                vec![said("session.toml", "# `AGENTS.md` {other} `github.checks`", true)],
            ),
            part(
                Of::Returns,
                vec![
                    said("return.toml", "{your_line} {store} `owner.language`", true),
                    said("fields/x.toml", " `github.checks` {other}", true),
                    said("tools/y.toml", " `github.checks` `github.checks`", true),
                ],
            ),
            part(Of::Rules, vec![said("rules/r.md", "A {brace} as written.", false)]),
        ];
        let fills = |name: &str| ["your_line", "store"].contains(&name);
        assert_eq!(
            check(&parts, &settings(), fills),
            [
                "session.toml: the engine fills no place `{other}`",
                "fields/x.toml: the engine fills no place `{other}`",
                "fields/x.toml: names the setting `github.checks`, which has no value",
                "tools/y.toml: names the setting `github.checks`, which has no value",
            ]
        );
    }
}
