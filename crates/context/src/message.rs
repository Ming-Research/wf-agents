//! The message a cell's agent is sent as the engine enters the cell, built from the flow's files alone: its parts in
//! the order `message.toml` gives them, each datum of the cell's context as `given.toml` words it for the kind the cell
//! gives it in. The code holds no word an agent is sent, only how the words are put together.
//!
//! What is sent depends on the situation the entry is in ([`Situation`]): which of the parts the session keeps it
//! holds, whether the last return in it handed in blocks, what the cell watches that is new to it, and how the session stands with
//! each datum. The context manager works the situation out from the records (`crate::manager`); the page picks one.
//! The places a run fills ([`Values`]) are filled by the engine, and left open for the page, so that what the page
//! shows for a situation is what the engine sends in it.

use std::collections::BTreeMap;

use decide::run::Unmet;
use decide::{BlockId, CallId, Seq};
use flow::agents::{Agents, GivenKind, MessagePart, Only, Part};
use flow::system::Settings;
use flow::{CellSpec, Compiled, Given, InputKind, Kind, Listed};
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
    /// The session holds its latest version, and the cell never worked from it: all of it is new to the cell.
    HoldsNew,
    /// The session holds its latest version, and nothing of it is new to the cell since it last worked from it: nothing
    /// is sent of it.
    Holds,
    /// The session holds no version of a datum whose versions the carrier compares itself, as the design tree, given
    /// `last_and_latest`: its latest is sent whole, saying so (design document `data-and-context`, 23).
    Unread,
}

impl Stands {
    /// Its name, in the words the files use.
    pub fn name(self) -> &'static str {
        match self {
            Stands::Unwritten => "unwritten",
            Stands::Lacks => "lacks",
            Stands::HoldsLast => "holds_last",
            Stands::HoldsLatest => "holds_latest",
            Stands::HoldsNew => "holds_new",
            Stands::Holds => "holds",
            Stands::Unread => "unread",
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
    /// What the cell watches that ran it, new to it, each datum or group by its name.
    pub new: Vec<String>,
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

/// What fills the places of a message filled in a run. Either way, each setting that the cell's task, what the cell
/// does when a datum it watches is new, or the words of what the cell returns name is given with the value beside its
/// name, as the system prompt gives a setting.
#[derive(Clone, Copy, Debug)]
pub enum Values<'a> {
    /// Each place is left open, as the page shows it, and each setting given as these settings give it.
    Slots(&'a Settings),
    /// The places as the round fills them.
    Run(&'a Run),
}

/// What a round fills the places of its message with. A version is told by its number among the versions of its datum,
/// never by its place in the records (decision a-version-is-numbered-within-its-datum).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    /// Each block the last return in the session handed in, with the number of the version it is kept as.
    pub taken: Vec<(BlockId, u64)>,
    /// The work's number.
    pub work: String,
    /// The number of the item the work is done for, and its title.
    pub item: String,
    pub item_title: String,
    /// What came of taking main in as the run started; none where the branch held main already.
    pub main: Option<Main>,
    /// The settings the flow's words may name, with their values.
    pub named: Settings,
    /// Each datum of the cell's context that has a version, by its name.
    pub data: BTreeMap<BlockId, Datum>,
    /// Of each datum or group the cell watches that is new to it, by its name, the version the cell last worked from
    /// and the versions that changed its meaning (design document `data-and-context`, 10 and 11).
    pub new: BTreeMap<String, NewVersions>,
    /// The calls whose results the round is given, in the order they were made.
    pub results: Vec<Result>,
    /// Why what the rounds of the run returned was sent back, each with the number of an earlier round, or none for
    /// the round's own, in their order.
    pub sent_back: Vec<(Option<usize>, Vec<Unmet>)>,
    /// What the return the round's own reasons are for holds itself stands for nothing of what the round gives: it
    /// asked for calls, or it could not be read, so the round gives it whole (`sent_back.toml`, `again_whole`).
    pub whole: bool,
    /// The number of each version a reason for sending back names, by its block and the place it was written at.
    pub numbers: BTreeMap<(BlockId, Seq), u64>,
    /// Each version the round's own return would have written over, of a block the cell is not given, which its
    /// session lacks: given with why it was sent back, as a datum given `latest` is.
    pub written_over: Vec<(BlockId, Version)>,
}

/// A datum of the cell's context, as a round fills it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Datum {
    /// Its latest version.
    pub latest: Version,
    /// The number of the version the cell last worked from; none where it worked from none.
    pub last: Option<u64>,
    /// What changed from the version last worked from to the latest, where it is sent so.
    pub diff: Option<String>,
    /// Of a datum given `all`, each version sent, the first first.
    pub versions: Vec<Version>,
}

/// The versions a round is told of a datum or group new to it, by their numbers: of a group, among the versions of the
/// group, each place that wrote any of its blocks being one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewVersions {
    /// The version the cell last worked from: the latest it consumed, wrote itself or the owner ruled on; none where
    /// it worked from none.
    pub last: Option<u64>,
    /// Each version new to the cell, which changed the meaning, the earliest first.
    pub changed: Vec<u64>,
}

/// A version of a datum.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Version {
    /// Its number among the versions of its datum, the first being 1.
    pub number: u64,
    /// Its value, where the datum is a choice.
    pub value: Option<String>,
    pub content: String,
    /// Who wrote it, as the round is told it.
    pub by: By,
}

/// Who wrote a version, as a round is told it (design document `data-and-context`, 29): the line of the cell whose run
/// wrote it, or the owner; nobody where an act of the engine wrote it, or the outside with no round. A round is never
/// told the cell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum By {
    Line(String),
    Owner,
    #[default]
    Nobody,
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
            Called::AtLimit => "at_limit",
            Called::Unknown(_) => "unknown",
        }
    }
}

/// The keys of the words of the part `main` of `message.toml`, and of those of a call in `results.toml`, in the
/// order the files write them.
pub const MAIN: [&str; 5] = ["text", "conflict", "uncommitted", "merging", "failed"];
pub const CALLS: [&str; 4] = ["tool", "tool_alone", "at_limit", "unknown"];

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
    /// Of a datum, what is sent in its place where the session holds it and the cell never worked from it.
    pub held_new: Vec<Piece>,
    /// Of a datum, what is sent in place of its text where the session lacks what the text needs.
    pub whole: Vec<Piece>,
    /// Of what is new to the cell, the line of each datum or group it watches, by its name, in the cell's order: the
    /// lines of those new to it come after `pieces`, and then `after`.
    pub items: Vec<(String, Vec<Piece>)>,
    pub after: Vec<Piece>,
}

/// What a round of the cell is given, in the order the `context` part names (design document `data-and-context`, 14,
/// 15, 21): each datum the definition gives it, by where it gives it, each list in its own order; and, under
/// `outputs`, each block an agent's cell writes that nothing else gives it, as `latest`, so that the cell brings its
/// outputs up to date from their latest versions, whoever wrote them. Such a block is sent only where the session lacks
/// its latest version.
pub fn given(flow: &Compiled, words: &Agents, cell: &CellSpec) -> Vec<Given> {
    let mut given = cell.given.clone();
    if cell.kind == Kind::Agent {
        let place = flow.flow.place(&cell.id);
        for block in place.map_or(&[][..], |p| &flow.flow.cells()[p].outputs[..]) {
            if !given.iter().any(|g| &g.block == block) {
                given.push(Given {
                    block: block.clone(),
                    kind: InputKind::Latest,
                    list: Listed::Outputs,
                });
            }
        }
    }
    let context = words.message.parts.iter().find(|p| p.part == Part::Context);
    let order = context.and_then(|p| p.order.as_deref()).unwrap_or_default();
    given.sort_by_key(|g| order.iter().position(|l| *l == g.list));
    given
}

/// The message as the files define it for the cell: each part, each datum of its context a part of its own.
pub fn defined(flow: &Compiled, words: &Agents, cell: &CellSpec) -> Vec<Defined> {
    // The page shows the words as the files write them, each setting by its name alone.
    let as_written = Settings::default();
    let build = Build {
        flow,
        words,
        cell,
        values: Values::Slots(&as_written),
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
            held_new: Vec::new(),
            whole: Vec::new(),
            items: Vec::new(),
            after: Vec::new(),
        };
        match part.part {
            Part::Context => {
                for given in &self::given(flow, words, cell) {
                    let datum = build.datum(&given.block, given.kind);
                    // What the cell writes and is given by nothing else is sent only where the session lacks it.
                    let output = given.list == Listed::Outputs;
                    let held_as = |held: Vec<Piece>| if output { Vec::new() } else { held };
                    out.push(Defined {
                        part: part.part,
                        block: Some(given.block.clone()),
                        kind: Some(given.kind),
                        only: None,
                        pieces: datum.text,
                        held: held_as(datum.held),
                        held_new: held_as(datum.held_new),
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
            Part::New => {
                let items = (cell.watches.iter())
                    .map(|w| (w.name.clone(), build.watched(part, w)))
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
                for given in &self::given(flow, words, cell) {
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
            Part::New => {
                let new: Vec<_> = (cell.watches.iter())
                    .filter(|w| situation.new.contains(&w.name))
                    .collect();
                if new.is_empty() {
                    continue;
                }
                let mut pieces = build.own(part);
                for watched in new {
                    pieces.extend(build.watched(part, watched));
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

/// The forms of a datum's content: its kind's text, what stands in its place where the session holds it, as the cell
/// worked from an earlier version or from none, and its content whole where the kind's text needs what the session
/// lacks.
struct DatumForms {
    text: Vec<Piece>,
    held: Vec<Piece>,
    held_new: Vec<Piece>,
    whole: Vec<Piece>,
    unread: Vec<Piece>,
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
            Values::Slots(_) => None,
            Values::Run(run) => Some(run),
        }
    }

    /// The text with each setting it names given its value beside its name; as it is where a setting has none, which
    /// the engine refuses to start with.
    fn named(&self, text: &str) -> String {
        let settings = match self.values {
            Values::Slots(settings) => settings,
            Values::Run(run) => &run.named,
        };
        settings.named_in(text).unwrap_or_else(|_| text.to_string())
    }

    /// The part's own text, with the places the flow fills filled: the cell's title and task.
    fn own(&self, part: &MessagePart) -> Vec<Piece> {
        let task = self.named(self.cell.task.as_deref().unwrap_or_default().trim_end());
        let known = [("title", self.cell.title.as_str()), ("task", task.as_str())];
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
                        .map(|(block, number)| {
                            let number = number.to_string();
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
            Part::Heading | Part::Task | Part::Context | Part::New => own,
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
    /// `return.toml` says; after the list its `owner_reads`, once, where a block of it is one the owner reads, and its
    /// `every_time`, once, where a block of it is one the cell must hand in every time; then what the cell is told of
    /// writing each block whose file says it, each a paragraph, in the order of the list, with what its file says
    /// after it where the cell has such a block; that it writes none, where it writes none.
    fn outputs(&self) -> Vec<Piece> {
        let told = &self.words.returns.outputs;
        let place = self.flow.flow.place(&self.cell.id);
        let outputs = place.map_or(&[][..], |p| &self.flow.flow.cells()[p].outputs[..]);
        let lines: Vec<String> = (outputs.iter())
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
        let shown = (outputs.iter()).any(|block| self.flow.blocks.get(block).is_some_and(|b| b.readers.shown));
        let reads = shown.then(|| told.owner_reads.trim().to_string());
        let marked = (outputs.iter()).any(|block| self.cell.must_write.contains(block));
        let every_time = marked.then(|| told.every_time.trim().to_string());
        let words = (outputs.iter())
            .filter_map(|block| self.flow.blocks.get(block))
            .flat_map(|block| {
                let after = block.words_if_marked.as_deref().filter(|_| marked);
                (block.words.as_deref().into_iter()).chain(after)
            })
            .map(|words| words.trim().to_string());
        let parts: Vec<String> = [told.heading.clone(), list]
            .into_iter()
            .chain(reads)
            .chain(every_time)
            .chain(words)
            .collect();
        written(&self.named(&parts.join("\n\n")))
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
                    let again = if run.whole { &words.again_whole } else { &words.again };
                    let heading = &words.heading;
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
        // What the return dealt with is named as the cell watches it: the datum, or a group it is of.
        let watched = |block: &BlockId| {
            let named = (self.cell.watches.iter()).find(|w| w.name == block.as_str() || w.blocks.contains(block));
            named.map_or_else(|| block.to_string(), |w| w.name.clone())
        };
        let block = |block: &BlockId| ("block", block.to_string());
        let places: Vec<(&str, String)> = match unmet {
            Unmet::AsksNothing => vec![],
            Unmet::NotAnOutput { block: b } => vec![block(b)],
            Unmet::WrittenTwice { block: b } => vec![block(b)],
            Unmet::NoValue { block: b } => vec![block(b), ("values", values(b))],
            Unmet::NotAValue { block: b, value } => {
                vec![block(b), ("values", values(b)), ("value", value.clone())]
            }
            Unmet::NotAChoice { block: b } => vec![block(b)],
            Unmet::NotHandedIn { block: b } => vec![block(b)],
            Unmet::NothingBut {
                block: b,
                value,
                besides,
            } => {
                let ticked: Vec<String> = besides.iter().map(|b| format!("`{b}`")).collect();
                let besides = ticked.join(&words.between);
                vec![block(b), ("value", value.clone()), ("besides", besides)]
            }
            Unmet::Unreadable { why } => vec![("why", why.clone())],
            Unmet::NotRunBy { block: b } => vec![("name", watched(b))],
            Unmet::DealtWithTwice { block: b } => vec![("name", watched(b))],
            Unmet::DealtWithNothing => vec![],
            Unmet::WrittenOver { block: b, version } => {
                let numbers = self.run().map(|run| &run.numbers);
                let number = numbers.and_then(|n| n.get(&(b.clone(), *version)));
                vec![block(b), ("version", number.map_or_else(String::new, u64::to_string))]
            }
            Unmet::ChoiceChanged { block: b } => vec![block(b)],
            Unmet::AsksAndWorks => vec![],
            Unmet::NotWatched { name } => vec![("name", name.clone())],
            Unmet::NotATool { name } => vec![("name", name.clone())],
            Unmet::UnchangedNotWritten { block: b } => vec![block(b)],
            Unmet::QuestionTaken { name } => vec![("name", name.clone())],
            Unmet::NoSuchQuestion { name } => vec![("name", name.clone())],
            Unmet::QuestionTwice { name } => vec![("name", name.clone())],
            Unmet::NotAnOption { name } => vec![("name", name.clone())],
            Unmet::NotAnswerable { name } => vec![("name", name.clone())],
            Unmet::NotAnswered { name } => vec![("name", name.clone())],
            Unmet::Unexplained { path } => vec![("path", path.clone())],
            Unmet::NotATest { path } => vec![("path", path.clone())],
            Unmet::Uncommitted { changes } => vec![("changes", changes.clone())],
            Unmet::Merging { changes } => vec![("changes", changes.clone())],
            Unmet::TreeUnchanged { name, .. } => vec![("name", name.clone())],
            Unmet::NotAnEntry { line, .. } => vec![("line", line.clone())],
            Unmet::TreeForm { lines } => vec![("faults", lines.join("\n"))],
            Unmet::Unanswered { block } => vec![("block", block.to_string())],
            Unmet::WordOwed { name, watched } => {
                let ticked: Vec<String> = watched.iter().map(|b| format!("`{b}`")).collect();
                let watched = ticked.join(&words.between);
                vec![("name", name.to_string()), ("watched", watched)]
            }
            Unmet::NoPoint { block: b, value, name } => {
                vec![block(b), ("value", value.clone()), ("name", name.to_string())]
            }
            Unmet::WouldStall { blocks } => {
                let ticked: Vec<String> = blocks.iter().map(|b| format!("`{b}`")).collect();
                vec![("blocks", ticked.join(&words.between))]
            }
        };
        // The sentence is the file's for the key the page shows (`Unmet::key`); every key has one (a test holds it).
        let text = f
            .text(unmet.key())
            .expect("every key of a reason a return is sent back for has its sentence");
        let places: Vec<(&str, &str)> = places.iter().map(|(name, value)| (*name, value.as_str())).collect();
        text::as_written(&fill(text.trim_end(), &places))
    }

    /// The line of a datum or group the cell watches, new to it: its `each`, with its versions, the one the cell last
    /// worked from, where it worked from one, and each that changed its meaning, where one did (design document
    /// `data-and-context`, 10 and 11), and what the cell does where it says. The page is shown the versions as one
    /// place left open, which a run fills with the words of either.
    fn watched(&self, part: &MessagePart, watched: &flow::Watched) -> Vec<Piece> {
        let each = with_slots(part.each.as_deref().unwrap_or_default());
        let words = |text: &Option<String>| text.as_deref().unwrap_or_default().to_string();
        let versions = match self.run() {
            None => vec![Piece::Slot {
                name: "versions".to_string(),
            }],
            Some(run) => match run.new.get(&watched.name) {
                Some(new) => {
                    let mut pieces = match new.last {
                        Some(last) => fill(&words(&part.since), &[("last", &last.to_string())]),
                        None => Vec::new(),
                    };
                    let numbers: Vec<String> = new.changed.iter().map(u64::to_string).collect();
                    let changed = match new.changed.len() {
                        0 => None,
                        1 => Some(&part.changed_one),
                        _ => Some(&part.changed_many),
                    };
                    if let Some(changed) = changed {
                        let numbers = numbers.join(&words(&part.between));
                        pieces.extend(fill(&words(changed), &[("changed", &numbers)]));
                    }
                    join(pieces)
                }
                None => Vec::new(),
            },
        };
        // The words of what the cell does are sent as they are: nothing in them is filled.
        let does = match (&watched.does, &part.does) {
            (Some(words), Some(does)) => put(with_slots(does), "do", &written(&self.named(words.trim()))),
            _ => Vec::new(),
        };
        let line = put(put(each, "versions", &versions), "do", &does);
        filled(line, &[("datum", &watched.name)])
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
        let whole_of = |text: Option<&str>| {
            text.map_or_else(Vec::new, |whole| match datum {
                Some(datum) => self.filled_by(form(whole), datum, &datum.latest),
                None => form(whole),
            })
        };
        let (whole, unread) = (whole_of(words.whole.as_deref()), whole_of(words.unread.as_deref()));
        let held_as = |text: &str| match datum {
            Some(datum) => self.filled_by(form(text), datum, &datum.latest),
            None => form(text),
        };
        DatumForms {
            text,
            held: held_as(&given.held),
            held_new: held_as(&given.held_new),
            whole,
            unread,
        }
    }

    /// A datum's content as an entry in this situation sends it: none where nothing is sent of it.
    fn datum_as(&self, block: &BlockId, kind: InputKind, stands: Stands) -> Option<(Form, Vec<Piece>)> {
        let forms = self.datum(block, kind);
        let whole = !forms.whole.is_empty();
        match stands {
            Stands::Unwritten | Stands::Holds => None,
            Stands::HoldsLatest => Some((Form::Held, forms.held)),
            Stands::HoldsNew => Some((Form::Held, forms.held_new)),
            Stands::Lacks if whole => Some((Form::Whole, forms.whole)),
            Stands::Unread if !forms.unread.is_empty() => Some((Form::Whole, forms.unread)),
            Stands::Unread if whole => Some((Form::Whole, forms.whole)),
            Stands::Unread => Some((Form::Text, forms.text)),
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

    /// A text of a datum with the places of this version of it filled: its content and what changed in it, each set
    /// apart from the message's own words in a fence, so that no heading or list in what an agent wrote reads as the
    /// message's own structure (principle agent-writing-is-data), the lines at its end left out; a content with nothing
    /// in it stays empty.
    fn filled_by(&self, pieces: Vec<Piece>, datum: &Datum, version: &Version) -> Vec<Piece> {
        let apart = |said: &str| match said.trim_end_matches('\n') {
            "" => String::new(),
            said => text::fenced(said),
        };
        let diff = apart(datum.diff.as_deref().unwrap_or_default());
        let content = apart(&version.content);
        let number = version.number.to_string();
        let last = datum.last.map(|l| l.to_string()).unwrap_or_default();
        let given = &self.words.given;
        let by = match &version.by {
            By::Line(line) => given.by_line.replace("{line}", line),
            By::Owner => given.by_owner.clone(),
            By::Nobody => String::new(),
        };
        let places = [
            ("by", by.as_str()),
            ("number", number.as_str()),
            ("last_number", last.as_str()),
            ("first_number", number.as_str()),
            ("value", version.value.as_deref().unwrap_or_default()),
            ("diff", diff.as_str()),
            ("content", content.as_str()),
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
