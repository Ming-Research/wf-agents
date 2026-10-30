//! The message an agent's round is sent, and what the round's session already holds.
//!
//! A message is the round's inputs, of two kinds. The always inputs are given every time the cell is entered: what to
//! do this round (the cell's task), the addresses of the conditional inputs to work from by the number of each of the
//! cell's inputs, why the round happens and which format to hand in. A conditional input
//! is anything worked on: a rule, the item, a setting, another cell's output, a tool's results, an interjection, a
//! return format, what each numbered input is, the worktree. Each has an address, and is given only when the session
//! does not hold it: one the session already holds, by its content, is only referred to; one that is new or changed
//! is given; after a compaction the session is taken to hold nothing. What a session holds is worked out from the
//! record of what each of its rounds was given and handed back. After a return sent back the message only says why,
//! and that the return is to be rewritten.
//!
//! The standard context is what every agent needs once in a session: the return formats, the lists of inputs, the
//! rules, the project guide, the project's settings and the worktree. When one of them changed since the version a
//! session holds, the session is given only what changed: the lines of a text that differ, the entries of an object.
//! The project guide is read from main as the work's branch merged it, so the work's own edits never change it.
//!
//! Before a message is sent, it is checked for what it would give whole that its session already holds: content the
//! session got before, under any address, or a whole new version of an input of the standard context. Such a message
//! is not sent; it is the engine's fault, and the work is paused for the owner.
//!
//! What a session holds, giving it only what it lacks or only what changed, and the check before sending are the
//! session manager's ([`sessions::held`], [`sessions::give`]); this module writes the message from them.
//!
//! Addresses: a value a round produced is `<round>·<cell>·<path>`, the round named `R<work>-<n>`, by the earliest
//! round that produced the same content; a value the engine reads or computes for a round is `<round>·engine·<name>`,
//! by the earliest round it was the same for; and `rule:<name>`, `config:<setting>`, `item:<id>`, `design_card:<id>`,
//! `ask:<id>` (an ask of the owner), `ij:<id>`, `work:<id>` (`work:<graph>` for where an architect's line works),
//! `format:common`, `format:<cell>` and `inputs:<cell>`. A
//! round recorded before rounds were named by their work wrote a round's address as `R<number>·...`; such an address
//! names no round, and the engine never gives it again.

use std::collections::HashMap;

pub use flow::formats::{cell_format, cell_inputs, common_format, common_format_of, hash};
use flow::{AskNeed, CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};
pub use sessions::give::Conditional;
use sessions::give::{Giver, empty, render};
pub use sessions::held::{Held, registry, round_of};

use crate::inputs::{GUIDE, Gathered, Origin, Part};
use crate::work::{Item, Round, RoundId, Work};
use crate::{ARCHITECT, Engine};

/// An agent round's message: its text, word for word, and every conditional input it gives or refers to.
#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    /// The text as the round's record keeps it: the text itself, or its parts, each large one (a conditional input
    /// given, the cell's task) a reference to what `kept` holds.
    pub stored: Value,
    /// The large parts, each by the hash `contents` keeps it under.
    pub kept: Vec<(String, String)>,
    pub conditional: Vec<Conditional>,
    /// The round a short message goes on from, a return sent back or a round cut off midway: the session holds
    /// everything that round was given, and only the interjections it was given are this round's.
    pub continues: Option<RoundId>,
    /// What the message would give whole that its session already holds, each said with the input's address, its
    /// size and the round the session got it or its earlier version in; a message with any is not sent.
    pub repeats: Vec<String>,
}

impl Message {
    /// The conditional inputs as a round records them: each by its address and hash, given or referred to, whether
    /// only what changed was given, and for an input of the standard context the key its whole version is kept under.
    pub fn record(&self) -> Value {
        sessions::give::record(&self.conditional)
    }
}

/// Whether a value is the project's own context, which a message gives before anything else: the project's guide (its
/// AGENTS.md and design tree) and a setting of its configuration, such as its design goal.
fn project_own(o: &Origin) -> bool {
    matches!(o, Origin::Config(_)) || matches!(o, Origin::Engine(n) if n == GUIDE)
}

/// The kinds of question a project cannot meet, which its agents are not told of: those whose need it does not meet.
pub fn kinds_left_out<'g>(g: &'g CompiledGraph, project: Option<&config::Project>) -> Vec<&'g AskNeed> {
    g.ask_needs
        .iter()
        .filter(|k| !project.is_some_and(|p| p.has(&k.need).unwrap_or(false)))
        .collect()
}

/// A return format or a rule with the value of each setting it names written beside the setting's name, such as
/// the owner's language beside `owner.language`: the flow and the rules name a setting, never its value, which the
/// configuration holds. In a return format, which holds the schema as JSON, the value is written as JSON writes it
/// inside a string.
fn with_settings(e: &Engine, project: &str, text: String, in_json: bool) -> String {
    let mut out = text;
    // The fences around code are not names: without them, names are the text between pairs of backticks.
    for (name, value) in e.config.config.named_settings(project, &out.replace("```", "")) {
        let value = if in_json {
            let quoted = Value::String(value).to_string();
            quoted[1..quoted.len() - 1].to_string()
        } else {
            value
        };
        out = out.replace(&format!("`{name}`"), &format!("`{name}` ({value})"));
    }
    out
}

/// What a message is built from.
pub struct Context<'a> {
    pub e: &'a Engine,
    pub w: &'a Work,
    pub g: &'a CompiledGraph,
    pub cell: &'a CompiledCell,
    /// The item the work is on; none for a wake of the architect.
    pub item: Option<&'a Item>,
    /// This round.
    pub round: RoundId,
    /// Every round of the work so far, oldest first.
    pub rounds: &'a [Round],
    pub gathered: &'a [Gathered],
    /// The values the engine knows and rows recorded, which pick the lines the prompt adds.
    pub given: &'a Map<String, Value>,
    pub columns: &'a [(String, String)],
    /// The rows of the cell's numbered table, each with its number, as [`crate::inputs::table`] makes them.
    pub table: &'a [Value],
    pub held: &'a Held,
    pub registry: &'a HashMap<String, Vec<String>>,
}

impl Context<'_> {
    /// The address of what the engine read or computed for this round under a name: the earliest round it was the
    /// same for.
    fn computed(&self, m: &Giver<'_>, name: &str, content: &Value) -> String {
        m.computed(self.round, name, content)
    }

    fn cell_of_round(&self, id: RoundId) -> Option<String> {
        self.rounds
            .iter()
            .find(|r| r.id == id)
            .and_then(|r| r.cell.clone())
            .or_else(|| {
                self.e
                    .db
                    .conn()
                    .query_row(
                        "SELECT cell FROM rounds WHERE work = ?1 AND n = ?2",
                        [id.work, id.n],
                        |r| r.get(0),
                    )
                    .ok()
            })
    }

    /// The conditional inputs one source of an input gave, each with its address.
    fn conditional_of(&self, m: &Giver<'_>, p: &Part) -> Vec<(String, Value)> {
        match &p.origin {
            Origin::Place { first, cell, path, .. } => {
                vec![(m.earliest(format!("{first}·{cell}·{path}"), &p.value), p.value.clone())]
            }
            Origin::Config(n) => vec![(format!("config:{n}"), p.value.clone())],
            Origin::Recorded(n) | Origin::Engine(n) => vec![(self.computed(m, n, &p.value), p.value.clone())],
            Origin::Db(rel) => {
                let rows = p.value.as_array().cloned().unwrap_or_default();
                match rel.as_str() {
                    "item_body" => rows.into_iter().map(|r| (format!("item:{}", r["item"]), r)).collect(),
                    "item_design_cards" | "parent_design_cards" | "work_design_card" => rows
                        .into_iter()
                        .map(|r| (format!("design_card:{}", r["design_card"]), r))
                        .collect(),
                    "item_asks" => rows.into_iter().map(|r| (format!("ask:{}", r["ask"]), r)).collect(),
                    "waiting_interjections" => rows
                        .into_iter()
                        .map(|r| {
                            let text = format!(
                                "From the {}: {}",
                                r["speaker"].as_str().unwrap_or("owner"),
                                r["text"].as_str().unwrap_or_default()
                            );
                            (format!("ij:{}", r["interjection"]), Value::String(text))
                        })
                        .collect(),
                    // The locked acceptance is what the round that locked it handed in.
                    "item_acceptance" => rows
                        .into_iter()
                        .filter_map(|r| {
                            let a = r.get("acceptance").filter(|a| !empty(a))?.clone();
                            let round = RoundId::new(r["acceptance_work"].as_i64()?, r["acceptance_round"].as_i64()?);
                            let path = match self.cell_of_round(round).as_deref() {
                                Some(c) if db::is_cell(crate::ENGINEER, "A1", c) => {
                                    format!("{c}·reply.new_acceptance")
                                }
                                Some(c) => format!("{c}·acceptance"),
                                None => "acceptance".to_string(),
                            };
                            Some((m.earliest(format!("{round}·{path}"), &a), a))
                        })
                        .collect(),
                    // The problem as the engineer understood it, once research handed it in.
                    "item_problem" => rows
                        .into_iter()
                        .map(|r| {
                            let id = r["item"].as_i64().unwrap_or_default();
                            let researched: Option<(RoundId, String)> = self
                                .e
                                .db
                                .conn()
                                .query_row(
                                    &format!(
                                        "SELECT r.work, r.n, r.cell FROM rounds r JOIN works w ON w.id = r.work
                                         WHERE w.item = ?1 AND r.cell IN {} AND r.transition IS NOT NULL
                                           AND json_extract(r.output, '$.outcome.understanding') IS NOT NULL
                                         ORDER BY r.work DESC, r.n DESC LIMIT 1",
                                        db::cells_sql(crate::ENGINEER, &["R1"])
                                    ),
                                    [id],
                                    |x| Ok((RoundId::new(x.get(0)?, x.get(1)?), x.get(2)?)),
                                )
                                .ok();
                            let address = match researched {
                                Some((k, c)) => format!("{k}·{c}·understanding.problem"),
                                None => format!("item:{id}·problem"),
                            };
                            (address, r["problem"].clone())
                        })
                        .collect(),
                    _ => vec![(self.computed(m, rel, &p.value), p.value.clone())],
                }
            }
        }
    }

    /// Why the cell's last return was sent back, when this round only rewrites it: the session holds everything else.
    /// A new session, such as one the owner asked for, holds nothing, and is given the whole message.
    fn sent_back(&self) -> Option<(RoundId, String)> {
        let r = self.rounds.last()?;
        if r.cell.as_deref() != Some(self.cell.id.as_str()) || r.transition.is_some() || self.held.rounds.is_empty() {
            return None;
        }
        self.reason_sent_back(r)
    }

    /// Why a round's return was sent back; none for a round that was not, cut off or ended without a return included.
    fn reason_sent_back(&self, r: &Round) -> Option<(RoundId, String)> {
        let rej = r.rejected.as_ref()?;
        if ["interrupted", "failed", "error"].iter().any(|k| rej.get(k).is_some()) {
            return None;
        }
        let reason = rej
            .as_object()
            .and_then(|o| o.values().next())
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
            .unwrap_or_default();
        Some((r.id, reason))
    }

    /// The cell's last round, when the engine stopped in the middle of it and the session it ran in goes on: a
    /// session that handed in a return before is resumed, and holds what that round was given.
    fn cut_off(&self) -> Option<RoundId> {
        let r = self.rounds.last()?;
        let here = r.cell.as_deref() == Some(self.cell.id.as_str()) && r.transition.is_none();
        let cut = r.rejected.as_ref().is_some_and(|x| x.get("interrupted").is_some());
        (here && cut && !self.held.rounds.is_empty()).then_some(r.id)
    }

    /// Why this round happens, and the conditional inputs it says it with.
    fn why(&self, m: &mut Giver<'_>, interjections: &[String]) -> Vec<String> {
        let c = self.cell;
        let mut out = Vec::new();
        let here = |r: &&Round| r.cell.as_deref() == Some(c.id.as_str());
        let earlier = self.rounds.iter().rev().find(here);
        match self.rounds.last() {
            Some(r) if here(&r) && r.transition.is_none() => {
                let rej = r.rejected.clone().unwrap_or(Value::Null);
                if rej.get("interrupted").is_some() {
                    out.push(format!(
                        "{} was cut off when the engine stopped: do its work again.",
                        r.id
                    ));
                } else if let Some(f) = rej.get("failed") {
                    // Only the kind: what the command line reported may quote the run's own return, which a new
                    // session, such as a reader that must see only the text, is never shown.
                    let kind = f.get("kind").and_then(Value::as_str).unwrap_or("failed");
                    out.push(format!("{} ended without a return ({kind}): do its work again.", r.id));
                } else if let Some(why) = rej.get("error").and_then(Value::as_str) {
                    out.push(format!("{} ended without a return ({why}): do its work again.", r.id));
                } else if let Some((_, reason)) = self.reason_sent_back(r) {
                    // A return sent back whose session does not go on, as a cell on no line's: the new session is
                    // given the whole message, and why the return before was sent back.
                    out.push(format!(
                        "The return of {} was sent back: {reason} Hand in this round's so that it fits.",
                        r.id
                    ));
                }
            }
            Some(r) => {
                let row = r.transition.as_deref().unwrap_or_default();
                let from = r.cell.as_deref().unwrap_or_default();
                // Back from a cell that only did something for this one, as waiting for the owner's Start after a
                // pause or handing on its answers: the round before is this cell's, whose row went there.
                let helper = self
                    .g
                    .cell(from)
                    .filter(|x| x.kind != "agent" && x.rows.iter().any(|y| y.name == row && y.to == "@caller"));
                let before = self.rounds.iter().rev().nth(1).filter(|p| here(p));
                let paused = here(&r) && c.rows.iter().any(|x| x.name == row && x.to == "@self");
                if paused {
                    out.push(format!("You paused in {}: carry on.", r.id));
                } else if let (Some(_), Some(p)) =
                    (helper.filter(|h| h.step.as_deref() == Some("wait_for_start")), before)
                {
                    out.push(format!(
                        "You paused in {}, and the owner started the work again: carry on.",
                        p.id
                    ));
                } else if let (Some(_), Some(p)) = (
                    helper.filter(|h| {
                        h.rows
                            .iter()
                            .any(|y| y.actions.iter().any(|d| d.name() == "deliver_answers"))
                    }),
                    before,
                ) {
                    out.push(format!("Your answers in {} went to who interjected: carry on.", p.id));
                } else {
                    let title = self.g.cell(from).map(|x| x.title.as_str()).unwrap_or_default();
                    out.push(format!("Came from cell {from} ({title}) by its row `{row}`, {}.", r.id));
                    // Back from asking: the answer it brought.
                    if self.g.asking.iter().any(|a| a == from) {
                        let since = earlier.map(|e| e.id);
                        let after = |x: &&Round| since.is_none_or(|s| x.id > s);
                        let answer = self.rounds.iter().rev().take_while(after).find_map(|x| {
                            let cell = x.cell.as_deref()?;
                            let kind = self.g.cell(cell)?.kind.as_str();
                            let out = x.output.as_ref().filter(|_| x.transition.is_some())?;
                            let out = if kind == "agent" {
                                self.g.unwrap(out)
                            } else {
                                out.clone()
                            };
                            let reply = out.get("reply").filter(|v| !empty(v))?.clone();
                            Some((x.id, cell.to_string(), reply))
                        });
                        if let Some((id, cell, reply)) = answer {
                            let a = m.add(m.earliest(format!("{id}·{cell}·reply"), &reply), reply);
                            out.push(format!("The answer to what you asked: {a}."));
                        }
                    }
                }
            }
            None => {}
        }
        if let Some(k) = self.held.compacted
            && self.held.rounds.is_empty()
        {
            let when = if self.held.compacted_waiting {
                format!("while it waited after {k}")
            } else {
                format!("in {k}")
            };
            out.push(format!(
                "Your session was compacted {when}: what this round needs is given again."
            ));
        }
        if !interjections.is_empty() {
            out.push(format!(
                "Said to you: {}. Answer in this round what needs an answer.",
                interjections.join(", ")
            ));
        }
        // The budget of the work's stretch used up while no tool use could ask the agent to pause: this round asks.
        let pauses = c.forms.iter().any(|f| f.field.as_deref() == Some("pause"));
        if pauses
            && let Ok(Some(minutes)) = crate::limits::used_up(&self.e.config.config, self.e.db.conn(), self.w.id, None)
        {
            out.push(crate::limits::ask_to_pause(minutes));
        }
        out
    }

    /// The message. After a return sent back it only says why, and that the return is to be rewritten.
    pub fn build(&self) -> Message {
        let (e, w, g, c, item) = (self.e, self.w, self.g, self.cell, self.item);
        let short = |text: String, prev: RoundId| Message {
            stored: Value::String(text.clone()),
            text,
            kept: Vec::new(),
            conditional: Vec::new(),
            continues: Some(prev),
            repeats: Vec::new(),
        };
        if let Some((prev, reason)) = self.sent_back() {
            let text = format!(
                "# {} · cell {}\n\nYour return was sent back: {reason}\nRewrite it (format:{}).\n",
                self.round, c.id, c.id
            );
            return short(text, prev);
        }
        if let Some(prev) = self.cut_off() {
            let text = format!(
                "# {} · cell {}\n\n{prev} was cut off midway. First look at the worktree as it is now (files \
                 changed, commits made), then go on with its work, and hand it in as format:{} says.\n",
                self.round, c.id, c.id
            );
            return short(text, prev);
        }
        let mut m = Giver::new(self.held, self.registry, e.db.conn(), self.rounds);
        // What every round needs from the start: the return formats (the query tool's description in them lists the
        // database's tables and views), what each numbered input of the cell is, the rules and the worktree; the value
        // of each setting the formats and the rules name is written beside its name, and the kinds of question the
        // project cannot meet are left out of the formats. A wake of an architect is for its project.
        let named = item
            .map(|i| i.project.as_str())
            .or(w.project.as_deref())
            .unwrap_or_default();
        let project = e.config.config.projects.get(named);
        let gone = kinds_left_out(g, project);
        // The forms every return of the session's line shares: only those some cell of the line has.
        let common = common_format_of(g, &sessions::format_cells(g, c), &gone);
        m.give(
            "format:common".into(),
            Value::String(with_settings(e, named, common, true)),
        );
        m.give(
            format!("format:{}", c.id),
            Value::String(with_settings(e, named, cell_format(g, c, &gone), true)),
        );
        let numbered = cell_inputs(c);
        if !numbered.is_empty() {
            m.give(format!("inputs:{}", c.id), Value::String(numbered));
        }
        // A project's own rules, the architect's of its project included.
        let mut rules = Vec::new();
        let own: Vec<String> = project.map(|p| p.rules.clone()).unwrap_or_default();
        for name in own {
            let Some(text) = e.config.config.rules.get(&name) else {
                continue;
            };
            let text = with_settings(e, named, text.trim().to_string(), false);
            rules.push(m.give(format!("rule:{name}"), Value::String(text)));
        }
        for r in &c.rules {
            if let Some(rule) = g.rules.iter().find(|x| x.name == r.name) {
                let lines: Vec<String> = rule.text.iter().map(|t| format!("- {t}")).collect();
                let text = with_settings(e, named, format!("{}\n\n{}", rule.desc, lines.join("\n")), false);
                rules.push(m.give(format!("rule:{}", rule.name), Value::String(text)));
            }
        }
        // A line the configuration keeps from changing anything only reads the worktree. An architect reads its
        // project in its graph's worktree at main; its cell on no line, which knows only what its inputs hold, runs in
        // an empty directory and is told of none.
        let line = crate::line_of(g, c);
        let read_only = e.config.config.install.agents.permissions.read_only.contains(&line);
        let workspace = if item.is_none() && c.line.is_none() {
            None
        } else if g.name == ARCHITECT || g.name == crate::POLICY {
            let text = architect_workspace(e, &g.name, named);
            Some(m.give(format!("work:{}", g.name), Value::String(text)))
        } else {
            workspace(w, project, read_only).map(|text| m.give(format!("work:{}", w.id), Value::String(text)))
        };
        // The inputs, each by its number in `inputs:<cell>` and the addresses of what it holds; what the agent reads
        // itself is said in full. A cell's own input that has nothing says none, so that it is not taken for one
        // missing; an input every agent's cell may have, such as tool results, is listed only when it has something.
        let mut work_from = Vec::new();
        let mut interjections = Vec::new();
        let mut first: Vec<String> = Vec::new();
        let mut number = 0;
        for x in self.gathered {
            if x.itself() {
                work_from.push(format!("Read yourself: {}", x.input.what));
                continue;
            }
            number += 1;
            // A numbered table is one input the engine computes from its sources, each row with its number.
            if x.input.numbered.is_some() {
                if self.table.is_empty() {
                    work_from.push(format!("{number}. none"));
                } else {
                    let content = Value::Array(self.table.to_vec());
                    let address = self.computed(&m, "table", &content);
                    work_from.push(format!("{number}. {}", m.add(address, content)));
                }
                continue;
            }
            let mut addrs: Vec<String> = Vec::new();
            for p in &x.parts {
                for (address, content) in self.conditional_of(&m, p) {
                    if empty(&content) {
                        continue;
                    }
                    let a = m.give(address, content);
                    if a.starts_with("ij:") && !interjections.contains(&a) {
                        interjections.push(a.clone());
                    }
                    if project_own(&p.origin) && !first.contains(&a) {
                        first.push(a.clone());
                    }
                    if !addrs.contains(&a) {
                        addrs.push(a);
                    }
                }
            }
            if !addrs.is_empty() {
                work_from.push(format!("{number}. {}", addrs.join(", ")));
            } else if !x.input.common {
                work_from.push(format!("{number}. none"));
            }
        }
        if !rules.is_empty() {
            work_from.push(format!("Rules: {}", rules.join(", ")));
        }
        if let Some(a) = workspace {
            work_from.push(format!("Worktree: {a}"));
        }
        let why = self.why(&mut m, &interjections);

        let mut out = Writer::default();
        out.line(&format!("# {} · cell {} · {}\n", self.round, c.id, c.title));
        if !why.is_empty() {
            out.line(&why.join("\n"));
            out.line("");
        }
        // The project's own context comes first: its guide and its settings, then the rest in the order added.
        let mut given: Vec<&Conditional> = m.list.iter().filter(|x| x.given).collect();
        given.sort_by_key(|x| first.iter().position(|a| *a == x.address).unwrap_or(first.len()));
        if !given.is_empty() {
            out.line("## Given\n");
            for x in given {
                out.line(&format!("### {}\n", x.address));
                out.keep(&render(&x.content));
                out.line("");
            }
        }
        let mut task: Vec<&str> = c.prompt.iter().map(String::as_str).collect();
        if let Some(pb) = &c.prompt_by
            && let Some(v) = self.given.get(&pb.by).and_then(Value::as_str)
            && let Some(lines) = pb.cases.get(v)
        {
            task.extend(lines.iter().map(String::as_str));
        }
        // A cell with no task of its own has no heading for one.
        if !task.is_empty() {
            out.line("## Task\n");
            out.keep(&task.join("\n"));
            out.line("");
        }
        if !work_from.is_empty() {
            out.line("## Work from\n");
            for l in &work_from {
                out.line(l);
            }
            out.line("");
        }
        if !self.columns.is_empty() {
            out.line("## Review table: answer each column by its number\n");
            for (id, col) in self.columns {
                out.line(&format!("{id}. {col}"));
            }
            out.line("");
        }
        out.line(&format!("Hand in one JSON object, as format:{} says.", c.id));
        let (text, stored, mut kept) = out.finish();
        let repeats = m.repeats();
        for (key, whole) in m.kept {
            if !kept.iter().any(|(k, _)| *k == key) {
                kept.push((key, whole));
            }
        }
        Message {
            text,
            stored,
            kept,
            repeats,
            conditional: m.list,
            continues: None,
        }
    }
}

/// A message as it is written, line by line, with the large parts the round's record keeps once.
#[derive(Default)]
struct Writer {
    text: String,
    /// The parts the record keeps, and the text written since the last large one.
    parts: Vec<Value>,
    since: String,
    kept: Vec<(String, String)>,
}

impl Writer {
    fn line(&mut self, s: &str) {
        self.text.push_str(s);
        self.text.push('\n');
        self.since.push_str(s);
        self.since.push('\n');
    }

    /// A line that may be large: the record refers to a large one by its hash, and keeps it once.
    fn keep(&mut self, s: &str) {
        if s.len() < db::STORED {
            return self.line(s);
        }
        self.text.push_str(s);
        self.text.push('\n');
        if !self.since.is_empty() {
            self.parts.push(Value::String(std::mem::take(&mut self.since)));
        }
        let h = db::content_hash(s);
        self.parts.push(json!({ "content": h }));
        self.since.push('\n');
        if !self.kept.iter().any(|(k, _)| *k == h) {
            self.kept.push((h, s.to_string()));
        }
    }

    /// The text, the text as the record keeps it, and what the record keeps once.
    fn finish(mut self) -> (String, Value, Vec<(String, String)>) {
        if self.parts.is_empty() {
            let text = self.text;
            return (text.clone(), Value::String(text), Vec::new());
        }
        if !self.since.is_empty() {
            self.parts.push(Value::String(std::mem::take(&mut self.since)));
        }
        (self.text, json!({ "parts": self.parts }), self.kept)
    }
}

/// Where a project's architect works, what `work:<graph>` gives: its graph's worktree, at the project's main and
/// brought to the latest main before each round, in which it only reads; other branches it reads there with git,
/// without switching, and other projects with the query tool.
fn architect_workspace(e: &Engine, graph: &str, project: &str) -> String {
    let dir = crate::architect::worktree(&e.data, graph, project);
    let main = e
        .config
        .config
        .projects
        .get(project)
        .map_or("main", |p| p.main.as_str());
    let mut out = format!(
        "You are the architect of the project {project}. Your working directory is {}, a worktree of your own at the \
         project's main, the branch {main}, which the engine brings to the latest main before each round: you only \
         read, and change and commit nothing, there or anywhere. Read another branch there without switching: `git \
         fetch origin <branch>`, then `git show origin/<branch>:<path>`, `git diff origin/{main}...origin/<branch>` \
         or `git log origin/<branch>`.",
        dir.display()
    );
    let others: Vec<&String> = e.config.config.projects.keys().filter(|n| *n != project).collect();
    if !others.is_empty() {
        let names: Vec<&str> = others.iter().map(|n| n.as_str()).collect();
        out.push_str(&format!(
            " The other projects ({}) have architects of their own: read their items with the query tool, and tell \
             their architects what they must know with a message.",
            names.join(", ")
        ));
    }
    out
}

/// Where the work's agents work: its worktree and branch, and the project's branch the flow calls main; what
/// `work:<id>` gives. An agent on a read-only line is told that it only reads the worktree. None for a work of no
/// project the configuration knows.
fn workspace(w: &Work, project: Option<&config::Project>, read_only: bool) -> Option<String> {
    let p = project?;
    let main = format!(
        "Main, wherever the flow says it, is the branch {}: the work branches from it, its PR merges into it, and a \
         diff against main is a diff against it.",
        p.main
    );
    Some(match (&w.worktree, &w.branch) {
        (Some(wt), Some(branch)) if read_only => format!(
            "Your working directory is the worktree {}, on the branch {branch}: you only read it, and change and \
             commit nothing there. {main}",
            wt.display()
        ),
        (Some(wt), Some(branch)) => format!(
            "Your working directory is the worktree {}, on the branch {branch}: commit there, and the engine pushes \
             when you ask it to with a tool. {main}",
            wt.display()
        ),
        _ => main,
    })
}

/// The columns the table review answers, each with its number, for the review of this round. A re-review that looks
/// at disputes only has none; one that looks at what changed has them all, since which columns a change concerns is
/// not worked out in this version. A work on no item reviews nothing.
pub fn columns(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
) -> Vec<(String, String)> {
    let Some(item) = item else { return Vec::new() };
    let rule = |name: &str| {
        g.rules
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    let mut cols: Vec<String> = Vec::new();
    match cell.id.as_str() {
        "T4" => {
            let review = crate::inputs::last_return(e, w, g, "T7", "plan")
                .ok()
                .flatten()
                .and_then(|r| crate::inputs::at_path(&r, "plan.review"));
            if review.as_ref().and_then(Value::as_str) == Some("disputes") {
                return Vec::new();
            }
            let table = rule("review_table");
            let project = e.config.config.projects.get(&item.project);
            if item.deliver == "report" {
                cols.extend(table.get(16).cloned());
            } else {
                cols.extend(table.iter().skip(2).take(7).cloned());
                for entry in project.map(|p| p.review_list.clone()).unwrap_or_default() {
                    cols.push(format!("From the review list in the project's configuration: {entry}"));
                }
            }
            let acceptance = item.acceptance.clone().unwrap_or(Value::Null);
            for a in acceptance.as_array().cloned().unwrap_or_default() {
                cols.push(format!(
                    "Acceptance condition {}: {} Can it be checked; is the evidence there, does it come from the head to be merged, and does it hold?",
                    a["id"].as_str().unwrap_or("?"),
                    a["condition"].as_str().unwrap_or_default()
                ));
            }
            if item.deliver != "report" {
                cols.push("The research's judgment that the work stands: does its reason hold?".into());
                cols.push("The judgment that there is no better design: does its reason hold?".into());
                for (c, what) in [
                    ("D4", "a gap in what is under test"),
                    ("D5", "unsure facts"),
                    ("D6", "architectural burden"),
                    ("D7", "cross-project dependency"),
                    ("D8", "splitting"),
                ] {
                    // A check the project lacks what it needs for was answered by the engine, not judged: no column.
                    let not_asked = g
                        .cell(c)
                        .and_then(|x| x.needs.as_deref())
                        .is_some_and(|need| project.is_some_and(|p| !p.has(need).unwrap_or(false)));
                    // The outcome the check went on by, as a gate names it; a flow version before gates named their
                    // outcomes answered no.
                    let on = g
                        .cell(c)
                        .and_then(|x| x.outcomes.first().cloned())
                        .unwrap_or_else(|| "no".into());
                    if g.cell(c).is_some() && !not_asked {
                        cols.push(format!(
                            "The check of {what} answered \"{on}\" or not applicable: does its reason hold?"
                        ));
                    }
                }
                let changes = crate::inputs::last_return(e, w, g, "I1", "deviations").ok().flatten();
                for d in changes
                    .and_then(|r| r["deviations"].as_array().cloned())
                    .unwrap_or_default()
                {
                    if d["kind"] != "surprise" {
                        cols.push(format!(
                            "The {} \"{}\": does its reason hold, and did the design change with it?",
                            d["kind"].as_str().unwrap_or_default().replace('_', " "),
                            d["what"].as_str().unwrap_or_default()
                        ));
                    }
                }
                // A return of a flow version before the answer was flat holds the test changes under end.
                let done = crate::inputs::last_return(e, w, g, "I1", "test_changes").ok().flatten();
                for t in done
                    .and_then(|r| {
                        r.get("test_changes")
                            .or_else(|| r["end"].get("test_changes"))
                            .and_then(Value::as_array)
                            .cloned()
                    })
                    .unwrap_or_default()
                {
                    let name = t["test"].as_str().unwrap_or_default();
                    match t["change"].as_str() {
                        Some("added") => cols.push(format!(
                            "The new test {name}: take the other side on its running time. Do other tests already catch what it catches, could it be smaller or faster, could it be folded into another test?"
                        )),
                        Some(c) => cols.push(format!(
                            "The test {name}, {c}: compared with before, is there a technical reason?"
                        )),
                        None => {}
                    }
                }
            }
        }
        "T5" => {
            let review = rule("design_tree_review");
            for i in [1, 2, 3, 4, 10, 11] {
                cols.extend(review.get(i).cloned());
            }
        }
        _ => {}
    }
    cols.into_iter()
        .enumerate()
        .map(|(i, c)| ((i + 1).to_string(), c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> CompiledGraph {
        let relations = db::relations(db::SCHEMA).unwrap();
        let f = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations,
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        f.graph(crate::ENGINEER).unwrap().clone()
    }

    // The forms every return shares hold every tool and every role any cell has, and show a return's shape and the
    // columns of every table the query tool reads; a cell's own format names the ones it has.
    #[test]
    fn the_common_format_holds_every_alternative_and_a_cell_says_which_it_has() {
        let g = graph();
        let common = common_format(&g, &[]);
        for t in [
            "\"ci\"",
            "\"ci_before\"",
            "\"gh\"",
            "\"query\"",
            "\"advisor\"",
            "\"architect\"",
            "\"owner\"",
        ] {
            assert!(common.contains(t), "{t}");
        }
        assert!(common.contains("leaves out `progress`"), "{common}");
        assert!(
            common.contains(
                r#"`{"tell_owner": […], "progress": "…", "side_reports": […], "flow_feedback": "…", "outcome": {…}}`"#
            ),
            "{common}"
        );
        // The query tool lists each table and view with its columns.
        assert!(common.contains("- items (id, project, title, "), "{common}");
        // The kinds of ask, the same under every role asked, are written once.
        assert_eq!(
            common.matches("The existing architecture or design has a flaw").count(),
            1,
            "{common}"
        );
        assert!(common.contains("\"$ref\":\"#/$defs/"), "{common}");
        let implement = cell_format(&g, g.cell("I1").unwrap(), &[]);
        assert!(
            implement.contains("`tools` (of ci, ci_before, measure, gh, query)"),
            "{implement}"
        );
        assert!(
            implement.contains("`ask` (to advisor, architect, owner)"),
            "{implement}"
        );
        let advisor = cell_format(&g, g.cell("A1").unwrap(), &[]);
        assert!(advisor.contains("`ask` (to architect, owner)"), "{advisor}");
        assert!(!advisor.contains("`redirect`"), "{advisor}");
    }

    // What the inputs every agent's cell has are is said once, in the common format; each cell's list of its inputs
    // names them only, and says in full what its own are.
    #[test]
    fn the_inputs_every_cell_has_are_described_once() {
        let g = graph();
        let common = common_format(&g, &[]);
        let implement = g.cell("I1").unwrap();
        let goal = implement
            .inputs
            .iter()
            .find(|x| x.name.as_deref() == Some("Design goal"))
            .unwrap();
        assert_eq!(common.matches(goal.what.as_str()).count(), 1, "{common}");
        let listed = cell_inputs(implement);
        assert!(!listed.contains(goal.what.as_str()), "{listed}");
        assert!(
            listed.contains("Design goal, which format:common describes"),
            "{listed}"
        );
        assert!(listed.contains(implement.inputs[0].what.as_str()), "{listed}");
    }

    // A kind of question whose need the project does not meet is left out of the formats, value and meaning, and the
    // other kinds stay.
    #[test]
    fn a_kind_the_project_cannot_meet_is_left_out_of_the_formats() {
        let g = graph();
        let gone = kinds_left_out(&g, None);
        assert!(gone.iter().any(|k| k.kind == "gap_under_test"), "{gone:?}");
        let common = common_format(&g, &gone);
        assert!(!common.contains("gap_under_test"), "{common}");
        assert!(!common.contains("rejects a natural way of writing"), "{common}");
        assert!(common.contains("\"unsure\""), "{common}");
        assert!(common_format(&g, &[]).contains("gap_under_test"));
    }
}
