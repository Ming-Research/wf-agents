//! The questions put to the owner and the owner's answers, as the loop records them (design document
//! `data-and-context`, 32 to 36), and the block the engine writes of how they stand.
//!
//! A run's questions become versions of them as the run finishes, each under its name; what the engine itself asks
//! with a round, as the round's end is taken. The owner answers a version of a question: the answer is kept, it is said
//! in the data the asking cell's agent reads for its trigger of an answer (`answers`, written as the owner's), and,
//! where it takes another option than the one the work goes by, it gives the block of that trigger ([`flow::ANSWER`]) a
//! new version, which runs the cell again. The work goes by the option last taken on
//! the question's latest version, else by the one it recommends; by none while the cell waits for the answer, so that
//! any answer to such a question runs the cell. An answer to an earlier version is kept and answers nothing.
//!
//! The block of a flow whose source is [`ASKED`] says whether every question that stands has the owner's answer to its
//! latest version: [`ANSWERED`] where each has, or none stands, [`WAITING`] while one has not. The engine writes it
//! as a question is asked, asked anew or withdrawn and as the owner answers, where it does not say so already, so that
//! a cell that requires it, as the merge's check does, waits while a question stands unanswered.

use decide::questions::questions;
use decide::{Answered, CellId, Outsider, Questioned, RunId, Written};
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

/// Records each of these questions, asked, asked anew or withdrawn, as a version of it of the cell, brought by the
/// run's return, or by the engine at the end of the run's round; each at a place of its own, in the order given.
pub(crate) fn record(work: &mut Work, run: RunId, cell: &CellId, engine: bool, asked: &[Question]) {
    for question in asked {
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
    }
}

/// Takes the owner's answer to the version of the question: kept, and, where it is to the question's latest version,
/// said in the words of the data the asking cell's agent reads for its trigger of an answer ([`said`]), and, where it
/// takes another option than the one the work goes by, the cell's block of that trigger written; or why it cannot be
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
    let cell = asked.cell.clone();
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

/// Says the answer where the asking cell has a trigger of an answer: as the owner's entry, in its words, in each datum
/// its agent reads for the trigger; and, where the answer `turns` to the cell, as a new version of the trigger's own
/// block, which runs the cell, in that block's words, its agent reading it where the cell is given it.
fn said(flow: &Compiled, work: &mut Work, cell: &CellId, answered: &Said, turns: bool) {
    let Some(spec) = flow.cells.iter().find(|spec| spec.id == *cell) else {
        return;
    };
    let trigger = (spec.triggers.iter())
        .find(|t| flow.blocks.get(&t.block).and_then(|b| b.writers.outside.as_deref()) == Some(flow::ANSWER));
    let Some(trigger) = trigger else {
        return;
    };
    for block in trigger.read.iter().filter(|b| **b != trigger.block) {
        let Some(declared) = flow.blocks.get(block).filter(|b| b.writers.people) else {
            continue;
        };
        let at = work.tick();
        work.contents
            .insert((block.clone(), at), answered.words(&declared.writes));
        work.records.written.push(Written {
            block: block.clone(),
            by: Outsider::Person,
            matters: true,
            value: None,
            at,
        });
    }
    if turns {
        let block = &trigger.block;
        let at = work.tick();
        work.contents
            .insert((block.clone(), at), answered.words(&flow.blocks[block].writes));
        work.records.written.push(Written {
            block: block.clone(),
            by: Outsider::Outside,
            matters: true,
            value: None,
            at,
        });
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
        at,
    });
}
