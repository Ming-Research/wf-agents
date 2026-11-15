//! The questions put to the owner and the owner's answers, as the loop records them (design document
//! `data-and-context`, 32 to 36), and the block the engine writes of how they stand.
//!
//! A run's questions become versions of them as the run finishes, each under its name; what the engine itself asks
//! with a round, as the round's end is taken. Each version is an entry of the record of questions and answers that the
//! cell its answer goes to has its agent read for its trigger of an answer (`q_and_a` in the engineer's flow), which
//! the engine's source of answers writes ([`flow::ANSWER`]), in the words of that record's file: so that an answer
//! is never given without its question. The owner answers a version of a question: the answer is kept, it is an entry
//! of the same record, and, where it takes another option than the one the work goes by, it gives the block of that
//! trigger a new version, which runs the cell again. The record runs no cell. The answering cell is the asking one, for a question of a run's; for one of
//! the engine's, which is of the work and not of what a round did, the last cell of the walk that has a trigger of an
//! answer, the one nearest the merge that can act on it, whichever cell's round it was asked with. The work goes by
//! the option last taken on the question's latest version, else by the one it recommends; by none while the cell waits
//! for the answer, so that any answer to such a question runs the cell. An answer to an earlier version is kept and
//! answers nothing.
//!
//! The block of a flow whose source is [`ASKED`] says whether every question that stands has the owner's answer to its
//! latest version: [`ANSWERED`] where each has, or none stands, [`WAITING`] while one has not. The engine writes it
//! as a question is asked, asked anew or withdrawn and as the owner answers, where it does not say so already, so that
//! a cell that requires it, as the merge's check does, waits while a question stands unanswered.

use decide::questions::questions;
use decide::{Answered, BlockId, CellId, Outsider, Questioned, RunId, Seq, Written};
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
/// The texts of a record of the questions and the answers that the engine fills, one for each kind of entry and each
/// part of one: an answer, `text` with `why`; a question asked, `asked`, or asked anew, `asked_anew`, with `option`,
/// `recommends`, and `waits` or `goes_on`; a question withdrawn, `withdrawn`, with `why`.
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

/// Records each of these questions, asked, asked anew or withdrawn, as a version of it of the cell, brought by the
/// run's return, or by the engine at the end of the run's round; each at a place of its own, in the order given, and
/// each written as an entry of the record its answer is said in ([`records`]), in the words of the record's file.
pub(crate) fn record(flow: &Compiled, work: &mut Work, run: RunId, cell: &CellId, engine: bool, asked: &[Question]) {
    for question in asked {
        // The version it replaces, or withdraws: its latest before this one.
        let was = (work.records.questions.iter().rev())
            .find(|q| q.name == question.name())
            .map(|q| q.at);
        let at = work.tick();
        let version = Questioned {
            name: question.name().to_string(),
            cell: cell.clone(),
            run,
            engine,
            waits: matches!(question, Question::Asked { waits: true, .. }),
            withdrawn: matches!(question, Question::Withdrawn { .. }),
            at,
        };
        // Kept first, so that each entry has a place after it.
        let to = answering(flow, &version);
        work.records.questions.push(version);
        for block in records(flow, &to) {
            let content = entry(&flow.blocks[&block].writes, question, at, was);
            write(work, &block, content);
        }
    }
}

/// The records of the work's questions and the owner's answers the cell's agent reads for its trigger of an answer,
/// which the engine's source of answers writes ([`flow::Trigger::records`]); none where it has no such trigger.
fn records(flow: &Compiled, cell: &CellId) -> Vec<BlockId> {
    let Some(spec) = flow.cells.iter().find(|spec| spec.id == *cell) else {
        return Vec::new();
    };
    let mut out: Vec<BlockId> = Vec::new();
    for trigger in spec
        .triggers
        .iter()
        .filter(|t| t.of_source(flow, flow::ANSWER).is_some())
    {
        for block in trigger.records(flow, flow::ANSWER) {
            if !out.contains(block) {
                out.push(block.clone());
            }
        }
    }
    out
}

/// A version of a question as an entry of the record, in the words of its `writes`: asked, `asked`, or asked anew in
/// place of the version `was`, `asked_anew`, each with `{question}` its name, `{version}` this version, `{ask}` what it
/// asks, `{options}` each option as `option` says it, `{recommends}` as `recommends` says it where it recommends one,
/// and `{waits}` as `waits` says it where its cell waits for the answer, else as `goes_on` does; withdrawn, `withdrawn`,
/// with `{was}` the version withdrawn and `{why}` why, as `why` says it.
fn entry(writes: &flow::Texts, question: &Question, version: Seq, was: Option<Seq>) -> String {
    let text = |name: &str| writes.get(name).unwrap_or_default();
    let version = version.0.to_string();
    let was = was.map_or_else(String::new, |was| was.0.to_string());
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

/// Writes a version of the block with this content, as the engine's source of answers.
fn write(work: &mut Work, block: &BlockId, content: String) {
    let at = work.tick();
    work.contents.insert((block.clone(), at), content);
    work.records.written.push(Written {
        block: block.clone(),
        by: Outsider::Outside,
        matters: true,
        value: None,
        said_to: None,
        at,
    });
}

/// Takes the owner's answer to the version of the question: kept, and, where it is to the question's latest version,
/// said in the words of the data the answering cell's agent reads for its trigger of an answer ([`said`]), and, where
/// it takes another option than the one the work goes by, the cell's block of that trigger written; or why it cannot be
/// one.
pub(crate) fn answer(
    flow: &Compiled,
    work: &mut Work,
    question: String,
    version: decide::Seq,
    option: u64,
    why: Option<String>,
) -> Result<(), Refused> {
    let r = &work.records;
    if !r.questions.iter().any(|q| q.name == question) {
        return Err(Refused::NoSuchQuestion(question));
    }
    let Some(asked) = (r.questions.iter()).find(|q| q.name == question && q.at == version && !q.withdrawn) else {
        return Err(Refused::NoSuchVersion { question, version });
    };
    let way = match work.question(asked) {
        Some(Question::Asked { options, .. }) => (option as usize).checked_sub(1).and_then(|i| options.get(i)),
        _ => None,
    };
    let Some(way) = way.cloned() else {
        return Err(Refused::NotAnOption { question, option });
    };
    let standing = questions(r).get(question.as_str()).copied();
    let latest = standing.is_some_and(|s| s.latest.at == version);
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
    let cell = answering(flow, asked);
    let at = work.tick();
    work.records.answers.push(Answered {
        question: question.clone(),
        version,
        option,
        why: why.clone(),
        at,
    });
    if latest {
        let answered = Said {
            question: &question,
            version,
            option,
            way: &way,
            why: why.as_deref(),
        };
        said(flow, work, &cell, &answered, goes_by != Some(option));
    }
    Ok(())
}

/// The cell an answer to this version is said to, and the owner's words about it: the asking cell, for a question a
/// run asked; for one the engine asked, the last cell of the walk that has a trigger of an answer, and the asking cell
/// where none has.
pub(crate) fn answering(flow: &Compiled, asked: &Questioned) -> CellId {
    if !asked.engine {
        return asked.cell.clone();
    }
    (flow.cells.iter().rev())
        .find(|spec| (spec.triggers.iter()).any(|t| answer_block(flow, &t.block)))
        .map_or(&asked.cell, |spec| &spec.id)
        .clone()
}

/// Whether the block is one the engine writes as the owner answers, a trigger's of an answer.
fn answer_block(flow: &Compiled, block: &decide::BlockId) -> bool {
    flow.blocks.get(block).and_then(|b| b.writers.outside.as_deref()) == Some(flow::ANSWER)
}

/// An answer, as the words that say it are filled.
struct Said<'a> {
    question: &'a str,
    version: decide::Seq,
    option: u64,
    way: &'a Way,
    why: Option<&'a str>,
}

impl Said<'_> {
    /// The words of the block's `writes` for an answer, `text`, with `{question}` the question's name, `{version}` the
    /// version answered, `{option}` the option's number, `{does}` and `{follows}` the option's, and `{why}` the reason
    /// as `why` says it where the owner gave one, else nothing.
    fn words(&self, writes: &flow::Texts) -> String {
        let why = match self.why {
            Some(why) => fill(writes.get("why").unwrap_or_default(), &[("why", why)]),
            None => String::new(),
        };
        let (version, option) = (self.version.0.to_string(), self.option.to_string());
        let places = [
            ("question", self.question),
            ("version", version.as_str()),
            ("option", option.as_str()),
            ("does", self.way.does.as_str()),
            ("follows", self.way.follows.as_str()),
            ("why", why.as_str()),
        ];
        fill(writes.get("text").unwrap_or_default(), &places)
    }
}

/// Says the answer where the answering cell has a trigger of an answer: as an entry, in its words, of each record of
/// questions and answers its agent reads for the trigger ([`records`]); and, where the answer `turns` to the cell, as
/// a new version of the trigger's own block, which runs the cell and no other, in that block's words.
fn said(flow: &Compiled, work: &mut Work, cell: &CellId, answered: &Said, turns: bool) {
    for block in records(flow, cell) {
        let content = answered.words(&flow.blocks[&block].writes);
        write(work, &block, content);
    }
    let Some(spec) = flow.cells.iter().find(|spec| spec.id == *cell) else {
        return;
    };
    let trigger = spec.triggers.iter().find(|t| answer_block(flow, &t.block));
    if let (Some(trigger), true) = (trigger, turns) {
        let content = answered.words(&flow.blocks[&trigger.block].writes);
        write(work, &trigger.block, content);
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
        said_to: None,
        at,
    });
}
