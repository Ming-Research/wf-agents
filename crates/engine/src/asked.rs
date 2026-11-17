//! The questions put to the owner and the owner's answers, as the loop records them (design document
//! `data-and-context`, 30 to 34), and the block the engine writes of how they stand.
//!
//! A cell that may ask has a datum of its own for its questions: the block of the engine's source of answers
//! ([`flow::ANSWER`]) among its outputs ([`questions_of`]). A run's questions become versions of them as the run
//! finishes, each under its name; what the engine itself asks with a round, as the round's end is taken, of the cell
//! the flow names for it (`engine_asks_in`), whichever cell's round it was asked with, or of that round's cell where
//! the flow names none. Each version is also a version of the cell's questions, in the words of their file, written as
//! changing nothing, so that an ask is new to no cell. The owner answers a version of a question with an option, and
//! nothing else: the answer is kept, and is the next version of the same questions, which changes their meaning where
//! it answers the question as last asked with another option than the one the work goes by, and so runs the cells that
//! watch them; one that changes nothing the cell goes by, or that answers an earlier version, is kept as a version
//! that changes nothing. The work goes by the option last taken on the question's latest version, else by the one it
//! recommends; by none while the cell waits for the answer, so that any answer to such a question runs the cell.
//!
//! The block of a flow whose source is [`ASKED`] says whether every question that stands has the owner's answer to its
//! latest version: [`ANSWERED`] where each has, or none stands, [`WAITING`] while one has not. The engine writes it
//! as a question is asked, asked anew or withdrawn and as the owner answers, where it does not say so already, so that
//! a cell that requires it, as the merge's check does, waits while a question stands unanswered.

use decide::questions::questions;
use decide::{Answered, BlockId, CellId, Outsider, Questioned, Records, RunId, Seq, Written};
use flow::Compiled;
use flow::agents::fill;
use work::{Question, Way, Work};

use crate::event::Refused;

/// The source of the block that says whether every question of the work that stands has the owner's answer. Such a
/// block is a choice of [`WAITING`] and [`ANSWERED`].
pub const ASKED: &str = "asked";
/// The value of an [`ASKED`] block while a question stands without the owner's answer to its latest version.
pub const WAITING: &str = "waiting";
/// The value of an [`ASKED`] block while every question that stands has the owner's answer to its latest version.
pub const ANSWERED: &str = "answered";
/// The texts of a cell's questions that the engine fills, one for each kind of version and each part of one: an
/// answer, `text`; a question asked, `asked`, or asked anew, `asked_anew`, with `option`, `recommends`, and `waits` or
/// `goes_on`; a question withdrawn, `withdrawn`, with `why`.
pub const ENTRIES: [&str; 9] = [
    "text",
    "why",
    "asked",
    "asked_anew",
    "option",
    "recommends",
    "waits",
    "goes_on",
    "withdrawn",
];

/// The cell's own questions: the block among its outputs that the engine's source of answers writes, which the cell
/// watches; none where the flow has no such cell or it may not ask.
pub fn questions_of<'f>(flow: &'f Compiled, cell: &CellId) -> Option<&'f BlockId> {
    own(flow, cell, flow::ANSWER)
}

/// The number of the version of the cell's questions that the engine wrote with this entry of the records, a version of
/// a question or an answer to one: the version written at the place right after it, in the same act as the entry, by
/// which an agent and the owner know it (decision a-version-is-numbered-within-its-datum). None where none was written
/// with it, as of a cell with no questions of its own.
pub fn number_of(flow: &Compiled, records: &Records, cell: &CellId, at: Seq) -> Option<u64> {
    let block = questions_of(flow, cell)?;
    records.number([block], Seq(at.0 + 1))
}

/// The block among the outputs of the cell of this id that this source of the engine's writes: a cell's own questions
/// or its own words.
pub(crate) fn own<'f>(flow: &'f Compiled, cell: &CellId, source: &str) -> Option<&'f BlockId> {
    let i = flow.flow.place(cell)?;
    (flow.flow.cells()[i].outputs.iter())
        .find(|b| flow.blocks.get(*b).and_then(|d| d.writers.outside.as_deref()) == Some(source))
}

/// Records each of these questions, asked, asked anew or withdrawn, as a version of it, brought by the run's return,
/// of the run's cell, or by the engine at the end of the run's round, of the cell the flow names for what it asks;
/// each at a place of its own, in the order given, and each also written, as changing nothing, as a version of that
/// cell's questions in the words of their file.
pub(crate) fn record(flow: &Compiled, work: &mut Work, run: RunId, cell: &CellId, engine: bool, asked: &[Question]) {
    let cell = match (engine, &flow.engine_asks_in) {
        (true, Some(named)) => named,
        _ => cell,
    };
    for question in asked {
        // The version it replaces, or withdraws: its latest before this one, by its number.
        let was = (work.records.questions.iter().rev())
            .find(|q| q.name == question.name())
            .and_then(|q| number_of(flow, &work.records, &q.cell, q.at));
        let at = work.tick();
        work.records.questions.push(Questioned {
            name: question.name().to_string(),
            cell: cell.clone(),
            run,
            engine,
            waits: matches!(question, Question::Asked { waits: true, .. }),
            withdrawn: matches!(question, Question::Withdrawn { .. }),
            at,
        });
        if let Some(block) = questions_of(flow, cell) {
            let number = work.records.next_number([block]);
            let content = entry(&flow.blocks[block].writes, question, number, was);
            write(work, block, content, false);
        }
    }
}

/// A version of a question as a version of the cell's questions, in the words of their `writes`: asked, `asked`, or
/// asked anew in place of the version `was`, `asked_anew`, each with `{question}` its name, `{version}` this version,
/// `{ask}` what it asks, `{options}` each option as `option` says it, `{recommends}` as `recommends` says it where it
/// recommends one, and `{waits}` as `waits` says it where its cell waits for the answer, else as `goes_on` does;
/// withdrawn, `withdrawn`, with `{was}` the version withdrawn and `{why}` why, as `why` says it. Each version is told
/// by its number among the versions of the cell's questions.
fn entry(writes: &flow::Texts, question: &Question, version: u64, was: Option<u64>) -> String {
    let text = |name: &str| writes.get(name).unwrap_or_default();
    let version = version.to_string();
    let was = was.map_or_else(String::new, |was| was.to_string());
    match question {
        Question::Asked {
            name,
            waits,
            recommends,
            ask,
            options,
            ..
        } => {
            let options: String = (options.iter().enumerate())
                .map(|(i, way)| {
                    let option = (i + 1).to_string();
                    let places = [
                        ("option", option.as_str()),
                        ("does", way.does.as_str()),
                        ("follows", way.follows.as_str()),
                    ];
                    fill(text("option"), &places)
                })
                .collect();
            let recommends = recommends.map_or_else(String::new, |r| {
                fill(text("recommends"), &[("option", r.to_string().as_str())])
            });
            let waits = text(if *waits { "waits" } else { "goes_on" });
            let asked = if was.is_empty() { "asked" } else { "asked_anew" };
            let places = [
                ("question", name.as_str()),
                ("version", version.as_str()),
                ("was", was.as_str()),
                ("ask", ask.as_str()),
                ("options", options.as_str()),
                ("recommends", recommends.as_str()),
                ("waits", waits),
            ];
            fill(text(asked), &places)
        }
        Question::Withdrawn { name, why } => {
            let why = fill(text("why"), &[("why", why.as_str())]);
            let places = [
                ("question", name.as_str()),
                ("version", version.as_str()),
                ("was", was.as_str()),
                ("why", why.as_str()),
            ];
            fill(text("withdrawn"), &places)
        }
    }
}

/// Writes a version of the block with this content, as the engine's source of answers, changing the block's meaning
/// or not.
fn write(work: &mut Work, block: &BlockId, content: String, matters: bool) {
    let at = work.tick();
    let empty = decide::holds_nothing(None, &content);
    work.contents.insert((block.clone(), at), content);
    work.records.written.push(Written {
        block: block.clone(),
        by: Outsider::Outside,
        matters,
        value: None,
        empty,
        said_to: None,
        at,
    });
}

/// Takes the owner's answer to the version of the question of this number ([`number_of`]): kept, and written as the
/// next version of the questions of the cell whose question it is, in the words of their file, changing their meaning
/// where it answers the question as last asked with another option than the one the work goes by; or why it cannot be
/// one.
pub(crate) fn answer(
    flow: &Compiled,
    work: &mut Work,
    question: String,
    version: u64,
    option: u64,
) -> Result<(), Refused> {
    let r = &work.records;
    if !r.questions.iter().any(|q| q.name == question) {
        return Err(Refused::NoSuchQuestion(question));
    }
    // The latest version of the question of that number: one the engine asks in the cell of the round it asks it with,
    // where the flow names no cell for it, has versions in the questions of more than one cell, each numbered among its
    // own cell's, and the owner answers its latest.
    let numbered = |q: &&Questioned| q.name == question && number_of(flow, r, &q.cell, q.at) == Some(version);
    let Some(asked) = (r.questions.iter()).rfind(numbered).filter(|q| !q.withdrawn) else {
        return Err(Refused::NoSuchVersion { question, version });
    };
    // The place of the version answered, which the records name it by.
    let place = asked.at;
    let way = match work.question(asked) {
        Some(Question::Asked { options, .. }) => (option as usize).checked_sub(1).and_then(|i| options.get(i)),
        _ => None,
    };
    let Some(way) = way.cloned() else {
        return Err(Refused::NotAnOption { question, option });
    };
    let standing = questions(r).get(question.as_str()).copied();
    let latest = standing.is_some_and(|s| s.latest.at == place);
    // The option the work goes by before this answer.
    let goes_by = match standing {
        Some(s) if latest => match (s.answer, work.question(s.latest)) {
            (Some(answer), _) => Some(answer.option),
            (None, _) if s.latest.waits => None,
            (None, Some(Question::Asked { recommends, .. })) => *recommends,
            (None, _) => None,
        },
        _ => None,
    };
    let cell = asked.cell.clone();
    let at = work.tick();
    work.records.answers.push(Answered {
        question: question.clone(),
        version: place,
        option,
        why: None,
        at,
    });
    if let Some(block) = questions_of(flow, &cell) {
        let answered = Said {
            question: &question,
            version,
            option,
            way: &way,
        };
        let content = answered.words(&flow.blocks[block].writes);
        write(work, block, content, latest && goes_by != Some(option));
    }
    Ok(())
}

/// An answer, as the words that say it are filled.
struct Said<'a> {
    question: &'a str,
    /// The number of the version answered.
    version: u64,
    option: u64,
    way: &'a Way,
}

impl Said<'_> {
    /// The words of the block's `writes` for an answer, `text`, with `{question}` the question's name, `{version}` the
    /// version answered, by its number, `{option}` the option's number, and `{does}` and `{follows}` the option's.
    fn words(&self, writes: &flow::Texts) -> String {
        let (version, option) = (self.version.to_string(), self.option.to_string());
        let places = [
            ("question", self.question),
            ("version", version.as_str()),
            ("option", option.as_str()),
            ("does", self.way.does.as_str()),
            ("follows", self.way.follows.as_str()),
        ];
        fill(writes.get("text").unwrap_or_default(), &places)
    }
}

/// Writes the work's block of [`ASKED`], where its flow has one and the block does not say already how the questions
/// stand: its value, and its words as the block's `writes` give them, `answered`, or `waiting` with `{questions}` the
/// names of those without an answer, in the order of their names, joined by `between`. Before it is first written the
/// block says what its default does, with no words.
pub(crate) fn tell(flow: &Compiled, work: &mut Work) {
    let Some((block, declared)) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(ASKED))
    else {
        return;
    };
    let waiting: Vec<&str> = (questions(&work.records).into_iter())
        .filter(|(_, q)| q.unanswered())
        .map(|(name, _)| name)
        .collect();
    let writes = &declared.writes;
    let (value, content) = if waiting.is_empty() {
        (ANSWERED, writes.get(ANSWERED).unwrap_or_default().to_string())
    } else {
        let between = writes.get("between").unwrap_or_default();
        let text = writes.get(WAITING).unwrap_or_default();
        (WAITING, fill(text, &[("questions", &waiting.join(between))]))
    };
    let latest = (work.records.written.iter())
        .filter(|w| &w.block == block)
        .max_by_key(|w| w.at);
    let says = match latest {
        Some(w) => w.value.as_deref() == Some(value) && work.contents.get(&(block.clone(), w.at)) == Some(&content),
        None => declared.default.as_deref() == Some(value),
    };
    if says {
        return;
    }
    let at = work.tick();
    work.contents.insert((block.clone(), at), content);
    work.records.written.push(Written {
        block: block.clone(),
        by: Outsider::Outside,
        matters: true,
        value: Some(value.to_string()),
        empty: false,
        said_to: None,
        at,
    });
}
