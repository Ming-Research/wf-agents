//! The message a cell's agent is sent as the engine enters the cell, built from the flow's files alone: its parts in
//! the order `message.toml` gives them, each datum of the cell's context as `given.toml` words it for the kind the cell
//! gives it in. The code holds no word an agent is sent, only how the words are put together.
//!
//! What is sent depends on the situation the entry is in ([`Situation`]): which of the parts the session keeps it
//! holds, whether the last return in it handed in blocks, which triggers ran the cell, and how the session stands with
//! each datum. The context manager works the situation out from the records (`crate::manager`); the page picks one.
//! The places a run fills ([`Values`]) are filled by the engine, and left open for the page, so that what the page
//! shows for a situation is what the engine sends in it.

use std::collections::BTreeMap;

use decide::run::Unmet;
use decide::{BlockId, CallId, Seq};
use flow::agents::{Agents, GivenKind, MessagePart, Only, Part};
use flow::{CellSpec, Compiled, InputKind};
use serde::{Deserialize, Serialize};

use crate::text::{self, Piece, fill, filled, join, put, with_slots, written};

/// How the session stands with a datum of the cell's context as the cell is entered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stands {
    /// No one has written it yet: it is left out.
    Unwritten,
    /// The session lacks its latest version and the version the cell last worked from.
    Lacks,
    /// The session holds the version the cell last worked from, and lacks a later one.
    HoldsLast,
    /// The session holds its latest version, and it is new to the cell since the version the cell last worked from.
    HoldsLatest,
    /// The session holds its latest version, and nothing of it is new to the cell since it last worked from it, or the
    /// cell worked from none of it: nothing is sent of it.
    Holds,
}

impl Stands {
    /// Its name, in the words the files use.
    pub fn name(self) -> &'static str {
        match self {
            Stands::Unwritten => "unwritten",
            Stands::Lacks => "lacks",
            Stands::HoldsLast => "holds_last",
            Stands::HoldsLatest => "holds_latest",
            Stands::Holds => "holds",
        }
    }
}

/// The situation an entry of a cell is in, which decides what of the message is sent and in which form.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Situation {
    /// The parts of the message the session keeps, those with a `kind`, of which it holds the latest, by `part`.
    pub holds: Vec<String>,
    /// The last return in the session handed in blocks.
    pub after_hand_in: bool,
    /// The triggers that ran the cell and are new to it, by name.
    pub triggers: Vec<String>,
    /// How the session stands with each datum of the cell's context, by name; one not named, as with
    /// [`Stands::Lacks`].
    pub data: BTreeMap<String, Stands>,
    /// What came of taking main in as the run started, by the key of its words in `main.toml`; none where the branch
    /// held main already, or the cell takes none in.
    pub main: Option<String>,
    /// The calls whose results the round is given that its session lacks, each by the key of its words in
    /// `results.toml`, in the order they were made.
    pub results: Vec<String>,
    /// The round is given why what a round of its run returned was sent back, which its session lacks.
    pub sent_back: bool,
}

/// What fills the places of a message filled in a run.
#[derive(Clone, Copy, Debug)]
pub enum Values<'a> {
    /// Each place is left open, as the page shows it.
    Slots,
    /// The places as the round fills them.
    Run(&'a Run),
}

/// What a round fills the places of its message with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    /// Each block the last return in the session handed in, with the version it is kept as.
    pub taken: Vec<(BlockId, Seq)>,
    /// The work's number.
    pub work: String,
    /// The number of the item the work is done for, and its title.
    pub item: String,
    pub item_title: String,
    /// What came of taking main in as the run started; none where the branch held main already.
    pub main: Option<Main>,
    /// The cell's task as it is sent, each setting it names with its value beside it; none where it is sent as
    /// written.
    pub task: Option<String>,
    /// Each datum of the cell's context that has a version, by its name.
    pub data: BTreeMap<BlockId, Datum>,
    /// What happened, by the place a trigger's detail names; a place the engine has no value for is left out with its
    /// detail.
    pub details: BTreeMap<String, String>,
    /// The calls whose results the round is given, in the order they were made.
    pub results: Vec<Result>,
    /// Why what the rounds of the run returned was sent back, each with the number of an earlier round, or none for
    /// the round's own, in their order.
    pub sent_back: Vec<(Option<usize>, Vec<Unmet>)>,
    /// Each version the round's own return would have written over, of a block the cell is not given, which its
    /// session lacks: given with why it was sent back, as a datum given `latest` is.
    pub written_over: Vec<(BlockId, Version)>,
}

/// A datum of the cell's context, as a round fills it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Datum {
    /// Its latest version.
    pub latest: Version,
    /// The version the cell last worked from; none where it worked from none.
    pub last: Option<Seq>,
    /// What changed from the version last worked from to the latest, where it is sent so.
    pub diff: Option<String>,
    /// Of a datum given `all`, each version sent, the first first.
    pub versions: Vec<Version>,
}

/// A version of a datum.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Version {
    pub number: Seq,
    /// Its value, where the datum is a choice.
    pub value: Option<String>,
    pub content: String,
}

/// What came of taking main in as a run started, where the work's branch did not hold it already: `name` is the main
/// branch's name and `main` the commit of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Main {
    TookIn {
        name: String,
        main: String,
        merge: String,
    },
    Conflict {
        name: String,
        main: String,
        files: Vec<String>,
    },
    /// The worktree holds changes not committed to these files; `merging` where they are a merge under way.
    Uncommitted {
        name: String,
        main: String,
        files: Vec<String>,
        merging: bool,
    },
    /// It could not be taken in, for a cause the round is not told.
    Failed {
        name: String,
    },
}

impl Main {
    /// The key of its words in the part `main` of `message.toml`.
    pub fn key(&self) -> &'static str {
        match self {
            Main::TookIn { .. } => "text",
            Main::Conflict { .. } => "conflict",
            Main::Uncommitted { merging: true, .. } => "merging",
            Main::Uncommitted { .. } => "uncommitted",
            Main::Failed { .. } => "failed",
        }
    }
}

/// A call whose result a round is given, with its result; none where it has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Result {
    pub call: Called,
    pub result: Option<String>,
}

/// What a call was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Called {
    /// A tool, run on this.
    Tool { name: String, input: String },
    /// A question put to a person, as it was put.
    Question(String),
    /// The owner's word at a limit.
    AtLimit,
    /// A call the work does not hold.
    Unknown(CallId),
}

impl Called {
    /// The key of its words in `results.toml`.
    pub fn key(&self) -> &'static str {
        match self {
            Called::Tool { input, .. } if input.is_empty() => "tool_alone",
            Called::Tool { .. } => "tool",
            Called::Question(_) => "question",
            Called::AtLimit => "at_limit",
            Called::Unknown(_) => "unknown",
        }
    }
}

/// The keys of the words of the part `main` of `message.toml`, and of those of a call in `results.toml`, in the
/// order the files write them.
pub const MAIN: [&str; 5] = ["text", "conflict", "uncommitted", "merging", "failed"];
pub const CALLS: [&str; 5] = ["tool", "tool_alone", "question", "at_limit", "unknown"];

/// In which of its forms a part is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Form {
    /// As its text is written.
    Text,
    /// What stands in its place where the session holds it already.
    Held,
    /// A datum's content whole, in place of its text, where the session lacks what the text needs.
    Whole,
}

/// A part of the message as one entry sends it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    /// Its `part` in `message.toml`.
    pub part: String,
    /// Of a datum of the context, its name.
    pub block: Option<String>,
    pub form: Form,
    pub pieces: Vec<Piece>,
}

/// A part of the message as the files define it, each of its forms with the places a run fills open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Defined {
    /// Its `part` in `message.toml`.
    pub part: Part,
    /// Of a datum of the context, its name.
    pub block: Option<BlockId>,
    /// The kind the session keeps it in: the part's, or of a datum, the kind the cell gives it in.
    pub kind: Option<InputKind>,
    pub only: Option<Only>,
    /// Its text.
    pub pieces: Vec<Piece>,
    /// What is sent in its place where the session holds it already; nothing where nothing is.
    pub held: Vec<Piece>,
    /// Of a datum, what is sent in place of its text where the session lacks what the text needs.
    pub whole: Vec<Piece>,
    /// Of the triggers, the line of each trigger of the cell, by its name, in the cell's order: the lines of those that
    /// ran it come after `pieces`, and then `after`.
    pub items: Vec<(String, Vec<Piece>)>,
    pub after: Vec<Piece>,
}

/// The message as the files define it for the cell: each part, each datum of its context a part of its own.
pub fn defined(flow: &Compiled, words: &Agents, cell: &CellSpec) -> Vec<Defined> {
    let build = Build {
        flow,
        words,
        cell,
        values: Values::Slots,
        situation: None,
    };
    let mut out = Vec::new();
    for part in &words.message.parts {
        let made = |pieces: Vec<Piece>| Defined {
            part: part.part,
            block: None,
            kind: part.kind,
            only: part.only,
            pieces,
            held: build.held(part),
            whole: Vec::new(),
            items: Vec::new(),
            after: Vec::new(),
        };
        match part.part {
            Part::Context => {
                for given in &cell.given {
                    let datum = build.datum(&given.block, given.kind);
                    out.push(Defined {
                        part: part.part,
                        block: Some(given.block.clone()),
                        kind: Some(given.kind),
                        only: None,
                        pieces: datum.text,
                        held: datum.held,
                        whole: datum.whole,
                        items: Vec::new(),
                        after: Vec::new(),
                    });
                }
            }
            Part::Main if !cell.takes_main && part.only == Some(Only::TakesMain) => {}
            // Each of its texts, by its key.
            Part::Main => {
                let items = MAIN
                    .iter()
                    .map(|key| (key.to_string(), build.main_words(key)))
                    .collect();
                out.push(Defined {
                    items,
                    ..made(Vec::new())
                });
            }
            Part::Results => {
                let items = CALLS
                    .iter()
                    .map(|key| (key.to_string(), build.call_words(key)))
                    .collect();
                out.push(Defined {
                    items,
                    after: build.results_after(),
                    ..made(build.results_heading())
                });
            }
            Part::Triggers => {
                let items = (cell.triggers.iter())
                    .map(|t| (t.name.clone(), build.trigger(part, t)))
                    .collect();
                out.push(Defined {
                    items,
                    after: part.after.as_deref().map_or_else(Vec::new, with_slots),
                    ..made(build.own(part))
                });
            }
            _ => out.push(made(build.text(part).unwrap_or_default())),
        }
    }
    out
}

/// The message one entry in this situation sends, part by part in the order sent, its places filled by these values.
pub fn message(
    flow: &Compiled,
    words: &Agents,
    cell: &CellSpec,
    situation: &Situation,
    values: Values<'_>,
) -> Vec<Sent> {
    let build = Build {
        flow,
        words,
        cell,
        values,
        situation: Some(situation),
    };
    let mut out = Vec::new();
    let sent = |part: &MessagePart, form: Form, pieces: Vec<Piece>| Sent {
        part: part.part.name().to_string(),
        block: None,
        form,
        pieces,
    };
    for part in &words.message.parts {
        let applies = match part.only {
            Some(Only::AfterHandIn) => situation.after_hand_in,
            Some(Only::TakesMain) => cell.takes_main,
            None => true,
        };
        if !applies {
            continue;
        }
        match part.part {
            Part::Context => {
                for given in &cell.given {
                    let name = given.block.as_str();
                    let stands = situation.data.get(name).copied().unwrap_or(Stands::Lacks);
                    if let Some((form, pieces)) = build.datum_as(&given.block, given.kind, stands) {
                        out.push(Sent {
                            part: part.part.name().to_string(),
                            block: Some(name.to_string()),
                            form,
                            pieces,
                        });
                    }
                }
            }
            Part::Triggers => {
                let ran: Vec<_> = (cell.triggers.iter())
                    .filter(|t| situation.triggers.contains(&t.name))
                    .collect();
                if ran.is_empty() {
                    continue;
                }
                let mut pieces = build.own(part);
                for trigger in ran {
                    pieces.extend(build.trigger(part, trigger));
                }
                pieces.extend(part.after.as_deref().map_or_else(Vec::new, with_slots));
                out.push(sent(part, Form::Text, join(pieces)));
            }
            Part::Results if situation.results.is_empty() => {}
            Part::SentBack if !situation.sent_back => {}
            _ if part.kind.is_some() && situation.holds.iter().any(|h| h == part.part.name()) => {
                let held = build.held(part);
                if !held.is_empty() {
                    out.push(sent(part, Form::Held, held));
                }
            }
            _ => {
                if let Some(pieces) = build.text(part) {
                    out.push(sent(part, Form::Text, pieces));
                }
            }
        }
    }
    out
}

/// The text of each part of the message the session keeps, those with a `kind`, by `part`, as these values fill it:
/// what tells a version of it the session holds from another.
pub fn kept(flow: &Compiled, words: &Agents, cell: &CellSpec, values: Values<'_>) -> Vec<(String, String)> {
    let build = Build {
        flow,
        words,
        cell,
        values,
        situation: None,
    };
    (words.message.parts.iter())
        .filter(|p| p.kind.is_some() && p.part != Part::Context)
        .filter_map(|p| Some((p.part.name().to_string(), text::as_written(&build.text(p)?))))
        .collect()
}

/// The message as one text: its parts in order, each without the line breaks it ends with, a blank line between two of
/// them.
pub fn text(sent: &[Sent]) -> String {
    let parts: Vec<String> = (sent.iter())
        .map(|s| text::as_written(&s.pieces).trim_end_matches('\n').to_string())
        .collect();
    let mut text = parts.join("\n\n");
    text.push('\n');
    text
}

/// The forms of a datum's content: its kind's text, what stands in its place where the session holds it, and its
/// content whole where the kind's text needs what the session lacks.
struct DatumForms {
    text: Vec<Piece>,
    held: Vec<Piece>,
    whole: Vec<Piece>,
}

struct Build<'a> {
    flow: &'a Compiled,
    words: &'a Agents,
    cell: &'a CellSpec,
    values: Values<'a>,
    /// The situation of the entry, where one is built; none where the parts are defined.
    situation: Option<&'a Situation>,
}

impl Build<'_> {
    /// The run's values, where the places are filled.
    fn run(&self) -> Option<&Run> {
        match self.values {
            Values::Slots => None,
            Values::Run(run) => Some(run),
        }
    }

    /// The part's own text, with the places the flow fills filled: the cell's title and task, as the run sends it.
    fn own(&self, part: &MessagePart) -> Vec<Piece> {
        let sent = self.run().and_then(|run| run.task.as_deref());
        let task = sent.or(self.cell.task.as_deref()).unwrap_or_default().trim_end();
        let known = [("title", self.cell.title.as_str()), ("task", task)];
        part.text.as_deref().map_or_else(Vec::new, |text| fill(text, &known))
    }

    /// What stands in the part's place where the session holds it, with the places the flow fills filled.
    fn held(&self, part: &MessagePart) -> Vec<Piece> {
        let known = [("title", self.cell.title.as_str())];
        part.held.as_deref().map_or_else(Vec::new, |held| fill(held, &known))
    }

    /// The part as it is sent where it is sent as written, its places filled by the values; none where a run has
    /// nothing to send of it.
    fn text(&self, part: &MessagePart) -> Option<Vec<Piece>> {
        let own = self.own(part);
        let run = self.run();
        Some(match part.part {
            Part::Taken => match run {
                None => own,
                Some(run) => {
                    let each = part.each.as_deref().unwrap_or_default();
                    let items: Vec<String> = (run.taken.iter())
                        .map(|(block, at)| {
                            let number = at.0.to_string();
                            text::as_written(&fill(each, &[("block", block.as_str()), ("number", &number)]))
                        })
                        .collect();
                    let taken = items.join(part.between.as_deref().unwrap_or_default());
                    filled(own, &[("taken", &taken)])
                }
            },
            Part::Work => match run {
                None => own,
                Some(run) => filled(
                    own,
                    &[
                        ("work", &run.work),
                        ("item", &run.item),
                        ("item_title", &run.item_title),
                    ],
                ),
            },
            Part::Main => match run {
                None => self.main_words(self.situation?.main.as_deref()?),
                Some(run) => self.main(run.main.as_ref()?),
            },
            Part::Outputs => self.outputs(),
            Part::Results => self.results(),
            Part::SentBack => self.sent_back(),
            Part::Heading | Part::Task | Part::Context | Part::Triggers => own,
        })
    }

    /// The text of this key of the part `main` of `message.toml`, its places open; nothing where it has none of the
    /// key.
    fn main_words(&self, key: &str) -> Vec<Piece> {
        let Some(part) = self.words.message.parts.iter().find(|p| p.part == Part::Main) else {
            return Vec::new();
        };
        let text = match key {
            "text" => &part.text,
            "conflict" => &part.conflict,
            "uncommitted" => &part.uncommitted,
            "merging" => &part.merging,
            "failed" => &part.failed,
            _ => return Vec::new(),
        };
        text.as_deref()
            .map_or_else(Vec::new, |text| with_slots(text.trim_end()))
    }

    /// What came of taking main in, in one line, in the words of the part `main`: a list in it, the files, with its
    /// `between` between two of them.
    fn main(&self, main: &Main) -> Vec<Piece> {
        let part = self.words.message.parts.iter().find(|p| p.part == Part::Main);
        let between = part.and_then(|p| p.between.as_deref()).unwrap_or_default();
        let places: Vec<(&str, String)> = match main {
            Main::TookIn { name, main, merge } => {
                vec![("name", name.clone()), ("main", main.clone()), ("merge", merge.clone())]
            }
            Main::Conflict { name, main, files } => vec![
                ("name", name.clone()),
                ("main", main.clone()),
                ("files", files.join(between)),
            ],
            Main::Uncommitted { name, main, files, .. } => vec![
                ("name", name.clone()),
                ("main", main.clone()),
                ("changes", files.join(between)),
            ],
            Main::Failed { name } => vec![("name", name.clone())],
        };
        let places: Vec<(&str, &str)> = places.iter().map(|(n, v)| (*n, v.as_str())).collect();
        filled(self.main_words(main.key()), &places)
    }

    /// What the cell writes, with its task: its blocks, each on a line with its marks and its words, as `[outputs]` of
    /// `return.toml` says; that it writes none, where it writes none.
    fn outputs(&self) -> Vec<Piece> {
        let told = &self.words.returns.outputs;
        let place = self.flow.flow.place(&self.cell.id);
        let outputs = place.map_or(&[][..], |p| &self.flow.flow.cells()[p].outputs[..]);
        // A block the cell writes through a field of the return is no block it writes as one.
        let blocks = (outputs.iter()).filter(|b| self.flow.blocks.get(*b).is_none_or(|d| d.field.is_none()));
        let lines: Vec<String> = blocks
            .map(|block| {
                let declared = self.flow.blocks.get(block);
                let values = declared.and_then(|b| b.values.as_ref());
                let choice = values.map_or_else(String::new, |values| {
                    let ticked: Vec<String> = values.iter().map(|v| format!("`{v}`")).collect();
                    told.choice.replace("{values}", &ticked.join(", "))
                });
                let shown = if declared.is_some_and(|b| b.readers.shown) {
                    told.shown.as_str()
                } else {
                    ""
                };
                let must = if self.cell.must_write.contains(block) {
                    told.must_write.as_str()
                } else {
                    ""
                };
                let what = declared.and_then(|d| d.what.as_deref()).unwrap_or_default();
                let places = [
                    ("block", block.as_str()),
                    ("choice", choice.as_str()),
                    ("shown", shown),
                    ("must_write", must),
                    ("what", what),
                ];
                text::as_written(&fill(&told.block, &places))
            })
            .collect();
        let list = if lines.is_empty() {
            told.none.clone()
        } else {
            lines.join("\n")
        };
        written(&[told.heading.clone(), list].join("\n\n"))
    }

    /// What comes before the results of the calls, in the words of `results.toml`.
    fn results_heading(&self) -> Vec<Piece> {
        written(&format!("{}\n\n", self.words.results.heading))
    }

    /// What follows them.
    fn results_after(&self) -> Vec<Piece> {
        written(&format!("\n\n{}", self.words.results.after))
    }

    /// A call of this key with its result, in the words of `results.toml`, the call's places and its result open.
    fn call_words(&self, key: &str) -> Vec<Piece> {
        let words = &self.words.results;
        let head = match key {
            "tool" => &words.tool,
            "tool_alone" => &words.tool_alone,
            "question" => &words.question,
            "at_limit" => &words.at_limit,
            "unknown" => &words.unknown,
            _ => return Vec::new(),
        };
        let mut pieces = with_slots(head);
        pieces.extend(written("\n\n"));
        pieces.push(Piece::Slot { name: "result".into() });
        join(pieces)
    }

    /// The results of the calls the round is given, in the words of `results.toml`; on the page, each call's places
    /// and its result open.
    fn results(&self) -> Vec<Piece> {
        let words = &self.words.results;
        let mut pieces = self.results_heading();
        let calls: Vec<Vec<Piece>> = match self.run() {
            None => (self.situation.into_iter())
                .flat_map(|s| s.results.iter())
                .map(|key| self.call_words(key))
                .collect(),
            Some(run) => (run.results.iter())
                .map(|r| {
                    let places: Vec<(&str, String)> = match &r.call {
                        Called::Tool { name, input } => vec![("name", name.clone()), ("input", input.clone())],
                        Called::Question(question) => vec![("question", question.clone())],
                        Called::AtLimit => Vec::new(),
                        Called::Unknown(call) => vec![("call", call.0.to_string())],
                    };
                    let result = r
                        .result
                        .as_deref()
                        .map(str::trim_end)
                        .unwrap_or(&words.none)
                        .to_string();
                    let places: Vec<(&str, &str)> = (places.iter().map(|(n, v)| (*n, v.as_str())))
                        .chain([("result", result.as_str())])
                        .collect();
                    filled(self.call_words(r.call.key()), &places)
                })
                .collect(),
        };
        for (n, call) in calls.into_iter().enumerate() {
            if n > 0 {
                pieces.extend(written("\n\n"));
            }
            pieces.extend(call);
        }
        pieces.extend(self.results_after());
        join(pieces)
    }

    /// Why what the rounds of the run returned was sent back, in the words of `sent_back.toml`: each earlier round its
    /// session lacks under `earlier`, then the round's own under `heading`, and `again`; on the page, the earlier rounds
    /// and the round's own reasons are each one place.
    fn sent_back(&self) -> Vec<Piece> {
        let words = &self.words.sent_back;
        let slot = |name: &str| Piece::Slot { name: name.into() };
        let Some(run) = self.run() else {
            let mut pieces = vec![slot("earlier")];
            pieces.extend(written(&format!("{}\n\n", words.heading)));
            pieces.push(slot("sent_back"));
            pieces.extend(written(&format!("\n\n{}", words.again)));
            return join(pieces);
        };
        let reasons = |unmet: &[Unmet]| -> String {
            let items: Vec<String> = (unmet.iter())
                .map(|u| words.item.replace("{text}", &self.fault(u)))
                .collect();
            items.join("\n")
        };
        let mut text = String::new();
        for (round, unmet) in &run.sent_back {
            match round {
                Some(round) => {
                    let heading = words.earlier.replace("{round}", &round.to_string());
                    text.push_str(&format!("{heading}\n\n{}\n\n", reasons(unmet)));
                }
                None => {
                    let (heading, again) = (&words.heading, &words.again);
                    text.push_str(&format!("{heading}\n\n{}\n\n", reasons(unmet)));
                    for (block, version) in &run.written_over {
                        let latest = self.datum(block, InputKind::Latest).text;
                        let datum = Datum::default();
                        let written = text::as_written(&self.filled_by(latest, &datum, version));
                        text.push_str(&format!("{written}\n\n"));
                    }
                    text.push_str(again);
                }
            }
        }
        written(&text)
    }

    /// One thing wrong with a return, in the words of `sent_back.toml`, its places filled.
    fn fault(&self, unmet: &Unmet) -> String {
        let words = &self.words.sent_back;
        let f = &words.faults;
        let values = |block: &BlockId| {
            let values = self.flow.blocks.get(block).and_then(|b| b.values.as_ref());
            let ticked: Vec<String> = values.into_iter().flatten().map(|v| format!("`{v}`")).collect();
            ticked.join(&words.between)
        };
        // What the return dealt with is named by the cell's trigger that stands on the block, or on a group of it.
        let trigger = |block: &BlockId| {
            let named = (self.cell.triggers.iter()).find(|t| &t.block == block || t.group.contains(block));
            named.map_or_else(|| block.to_string(), |t| t.name.clone())
        };
        let block = |block: &BlockId| ("block", block.to_string());
        let (text, places): (&str, Vec<(&str, String)>) = match unmet {
            Unmet::AsksNothing => (&f.asks_nothing, vec![]),
            Unmet::NotAnOutput { block: b } => (&f.not_an_output, vec![block(b)]),
            Unmet::WrittenTwice { block: b } => (&f.written_twice, vec![block(b)]),
            Unmet::NoValue { block: b } => (&f.no_value, vec![block(b), ("values", values(b))]),
            Unmet::NotAValue { block: b, value } => (
                &f.not_a_value,
                vec![block(b), ("values", values(b)), ("value", value.clone())],
            ),
            Unmet::NotAChoice { block: b } => (&f.not_a_choice, vec![block(b)]),
            Unmet::NotHandedIn { block: b } => (&f.not_handed_in, vec![block(b)]),
            Unmet::Unreadable { why } => (&f.unreadable, vec![("why", why.clone())]),
            Unmet::NotRunBy { block: b } => (&f.not_run_by, vec![("name", trigger(b))]),
            Unmet::DealtWithTwice { block: b } => (&f.dealt_with_twice, vec![("name", trigger(b))]),
            Unmet::DealtWithNothing => (&f.dealt_with_nothing, vec![]),
            Unmet::WrittenOver { block: b, version } => {
                (&f.written_over, vec![block(b), ("version", version.0.to_string())])
            }
            Unmet::ChoiceChanged { block: b } => (&f.choice_changed, vec![block(b)]),
            Unmet::AsksAndWorks => (&f.asks_and_works, vec![]),
            Unmet::NotATrigger { name } => (&f.not_a_trigger, vec![("name", name.clone())]),
            Unmet::NotATool { name } => (&f.not_a_tool, vec![("name", name.clone())]),
            Unmet::UnchangedNotWritten { block: b } => (&f.unchanged_not_written, vec![block(b)]),
        };
        let places: Vec<(&str, &str)> = places.iter().map(|(name, value)| (*name, value.as_str())).collect();
        text::as_written(&fill(text.trim_end(), &places))
    }

    /// The line of a trigger: its `each`, with what happened where the trigger has a place for it and the engine a
    /// value, and what the cell does where it says.
    fn trigger(&self, part: &MessagePart, trigger: &flow::Trigger) -> Vec<Piece> {
        let each = with_slots(part.each.as_deref().unwrap_or_default());
        let detail = match (&trigger.detail, &part.detail) {
            (Some(place), Some(detail)) => match self.run() {
                None => put(with_slots(detail), "detail", &[Piece::Slot { name: place.clone() }]),
                Some(run) => match run.details.get(place) {
                    Some(value) => fill(detail, &[("detail", value)]),
                    None => Vec::new(),
                },
            },
            _ => Vec::new(),
        };
        // The words of what the cell does are sent as they are: nothing in them is filled.
        let does = match (&trigger.does, &part.does) {
            (Some(words), Some(does)) => put(with_slots(does), "do", &written(words.trim())),
            _ => Vec::new(),
        };
        let line = put(put(each, "detail", &detail), "do", &does);
        filled(line, &[("trigger", &trigger.name)])
    }

    /// The forms of a datum's content as `given.toml` words them for its kind, the places of a run open or filled.
    fn datum(&self, block: &BlockId, kind: InputKind) -> DatumForms {
        let given = &self.words.given;
        let words = self.kind(kind);
        let choice = match self.flow.blocks.get(block) {
            Some(declared) if declared.values.is_some() => given.choice.as_str(),
            _ => "",
        };
        let form = |text: &str| fill(&text.replace("{choice}", choice), &[("block", block.as_str())]);
        let datum = self.run().and_then(|run| run.data.get(block));
        let text = match (kind, datum) {
            (InputKind::All, Some(datum)) => self.versions(&form(&words.text), &datum.versions),
            (_, Some(datum)) => self.filled_by(form(&words.text), datum, &datum.latest),
            (_, None) => form(&words.text),
        };
        let whole = (words.whole.as_deref()).map_or_else(Vec::new, |whole| match datum {
            Some(datum) => self.filled_by(form(whole), datum, &datum.latest),
            None => form(whole),
        });
        let held = match datum {
            Some(datum) => self.filled_by(form(&given.held), datum, &datum.latest),
            None => form(&given.held),
        };
        DatumForms { text, held, whole }
    }

    /// A datum's content as an entry in this situation sends it: none where nothing is sent of it.
    fn datum_as(&self, block: &BlockId, kind: InputKind, stands: Stands) -> Option<(Form, Vec<Piece>)> {
        let forms = self.datum(block, kind);
        let whole = !forms.whole.is_empty();
        match stands {
            Stands::Unwritten | Stands::Holds => None,
            Stands::HoldsLatest => Some((Form::Held, forms.held)),
            Stands::Lacks if whole => Some((Form::Whole, forms.whole)),
            Stands::Lacks | Stands::HoldsLast => Some((Form::Text, forms.text)),
        }
    }

    /// The words of a kind.
    fn kind(&self, kind: InputKind) -> &GivenKind {
        let given = &self.words.given;
        match kind {
            InputKind::Latest => &given.latest,
            InputKind::LastAndLatest => &given.last_and_latest,
            InputKind::All => &given.all,
        }
    }

    /// A text of a datum with the places of this version of it filled.
    fn filled_by(&self, pieces: Vec<Piece>, datum: &Datum, version: &Version) -> Vec<Piece> {
        let number = version.number.0.to_string();
        let last = datum.last.map(|l| l.0.to_string()).unwrap_or_default();
        let places = [
            ("number", number.as_str()),
            ("last_number", last.as_str()),
            ("first_number", number.as_str()),
            ("value", version.value.as_deref().unwrap_or_default()),
            ("diff", datum.diff.as_deref().unwrap_or_default()),
            ("content", version.content.as_str()),
        ];
        filled(pieces, &places)
    }

    /// The text of a datum given `all` with each of these versions: the first as the text says, and in place of
    /// `{each_version}` each later one as the first, as what stands before that place says it, and between two of them
    /// what stands between the first and that place.
    fn versions(&self, pieces: &[Piece], versions: &[Version]) -> Vec<Piece> {
        let each = pieces
            .iter()
            .position(|p| matches!(p, Piece::Slot { name } if name == "each_version"));
        let mut one = each.map_or_else(|| pieces.to_vec(), |at| pieces[..at].to_vec());
        let mut between = String::new();
        if let Some(Piece::Text { text }) = one.last_mut() {
            let kept = text.trim_end().len();
            between = text[kept..].to_string();
            text.truncate(kept);
        }
        let datum = Datum::default();
        let (Some(first), later) = (versions.first(), versions.get(1..).unwrap_or_default()) else {
            return pieces.to_vec();
        };
        let later: Vec<String> = (later.iter())
            .map(|v| text::as_written(&self.filled_by(one.clone(), &datum, v)))
            .collect();
        let filled = self.filled_by(pieces.to_vec(), &datum, first);
        put(filled, "each_version", &written(&later.join(&between)))
    }
}
