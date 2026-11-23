//! What a work waits on the owner for, defined once (principle one-count-of-what-waits): the page's count, each item's
//! mark, the cards the owner acts on, the browser's notifications, the stretches of a work's bar the owner held it, and
//! what `wf-agents wait` says rests for a person are all read from [`waits`].
//!
//! A work waits on the owner for: a question put to them that holds it, at its cell or at a precondition that only
//! their answers make hold, as the check before the merge does; a cell at its limit, or its run; a
//! precondition the owner makes hold by writing a block, as a review's verdict for them to rule on; a pause the engine
//! made; and the agents' command line where it cannot run, which only the owner mends. Who can make a precondition
//! hold is what the flow declares of each block, as the deciding code reads it ([`Maker::Owner`]). Nothing else is the
//! owner's to end: a fault, a stalled work, the owner's own pause, a slot, a source outside the work, the project's
//! turn, the wait every work shares for any other cause, which the engine lifts by itself, and the pause of every
//! work, which is one thing wherever it holds and is said and lifted where it is set, not work by work.
//!
//! Each is said in a sentence in the words beside the flows ([`flow::agents::OwnerWords`]), of the flow the work runs
//! on, with what the owner does that ends it.

use decide::questions::{self, Standing};
use decide::run::Limit;
use decide::{
    BlockId, CallId, CellId, Condition, Decision, End, Holder, Maker, Pauser, Records, Seq, TestKind, Waited,
};
use flow::Kind;
use flow::agents::{OwnerWords, fill, sentence};
use work::Call;

use crate::Kept;

/// The key of the engine's pause.
pub const PAUSE: &str = "pause";
/// The key of a cell, or its run, at its limit.
pub const LIMIT: &str = "limit";
/// The key of the agents' command line that cannot run.
pub const AGENTS: &str = "agents";

/// One thing a work waits on the owner for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
    /// Names it within its work, the same each time it is read while it stands: the address a notification opens and
    /// the card the owner acts on are found by it.
    pub key: String,
    /// Why the work waits on the owner for it, in a sentence: for a question, what it asks.
    pub why: String,
    /// What the owner does that ends it.
    pub ends: Ends,
}

/// What the owner does that ends a wait on them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ends {
    /// Answers the latest version of the work's question of this name, by one of its options: the version by its number
    /// among the versions of its cell's questions, none where it has none, as a question of a cell with no questions of
    /// its own.
    Answer { question: String, version: Option<u64> },
    /// Lets the run that reached its limit go on, as the result of this call.
    GoOn { call: CallId },
    /// Lets the cell go on past its limit.
    LetGo { cell: CellId },
    /// Writes the block with this value, which makes the precondition hold; none where the block's values do not make
    /// that plain, and the owner says to the cell what is to be done.
    Write { block: BlockId, value: Option<String> },
    /// Lets the work go on, lifting the engine's pause.
    Resume,
    /// Does what this says, which no control of the page does; the engine sees it done by itself.
    Outside { what: String },
}

/// What holds every work at once at a moment: who set the pause of everything, while it holds, which no work waits on
/// the owner for and which whoever reads a work's stretches reads; and what the agents' command line said, where it
/// cannot run and their wait holds their rounds, which the owner mends.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shared<'a> {
    pub everything: Option<Pauser>,
    pub cannot_run: Option<&'a str>,
}

/// Everything the work, on these records and so decided, waits on the owner for, in the order it waits for them: the
/// engine's pause, then what it waits for at its cell, then the agents' command line. `held` is what holds back the
/// round it would begin next, where anything does: the pause of everything is none of the work's own waits, and is
/// read here only to tell whether the wait every work shares holds a round the work would otherwise begin.
pub fn waits(
    kept: &Kept<'_>,
    records: &Records,
    decision: &Decision,
    held: Option<Holder>,
    shared: Shared<'_>,
) -> Vec<Waiting> {
    let words = &kept.flow.waits.owner;
    let mut waits = Vec::new();
    // A round under way goes on under a pause, and a run that answers the owner's words begins under it: the pause
    // holds the work once they are done.
    if let Decision::Fault(_) | Decision::Busy { .. } | Decision::Abandon { .. } = decision {
        return waits;
    }
    let pause_holds = !matches!(decision, Decision::Run { .. });
    // A work that ended is paused by the engine only where letting go of what it held failed after it ended.
    let ended = match decision {
        Decision::Ended(end) => Some(ended_at(records, *end)),
        _ => None,
    };
    if pause_holds
        && let Some(pause) = decide::pause(records).filter(|pause| pause.by == Pauser::Engine)
        && ended.is_none_or(|at| at.is_some_and(|at| pause.at > at))
    {
        let why = match &pause.why {
            Some(why) => fill(&words.paused, &[("why", &sentence(why))]),
            None => words.no_reason.clone(),
        };
        push(&mut waits, PAUSE.to_string(), why, Ends::Resume);
    }
    if ended.is_some() {
        return waits;
    }
    let agent = |cell: &CellId| (kept.flow.cells.iter()).any(|spec| spec.id == *cell && spec.kind == Kind::Agent);
    // The cell whose agent's next round the wait every work shares holds, where it holds one: one that would run or go
    // on, its own pause and the pause of everything not holding it, or that waits for a slot alone.
    let shared_holds = match decision {
        Decision::Run { cell, .. } => Some(cell),
        Decision::GoOn { cell, .. } if !matches!(held, Some(Holder::Pause | Holder::Everything)) => Some(cell),
        Decision::Wait { at, on } if on.as_slice() == [Waited::Slot] => Some(at),
        _ => None,
    };
    if let Decision::Wait { at, on } = decision {
        for waited in on {
            at_cell(kept, records, words, at, waited, &mut waits);
        }
    }
    if let (Some(cell), Some(why)) = (shared_holds, shared.cannot_run)
        && agent(cell)
    {
        let what = Ends::Outside {
            what: words.mend.clone(),
        };
        push(
            &mut waits,
            AGENTS.to_string(),
            fill(&words.agents, &[("why", &sentence(why))]),
            what,
        );
    }
    waits
}

/// The place at which the work ended: its merge's finish, or its cancel; none where the records do not say.
pub(crate) fn ended_at(records: &Records, end: End) -> Option<Seq> {
    match end {
        End::Merged => records.finished.iter().map(|finished| finished.at).max(),
        End::Cancelled => records.cancelled(),
    }
}

/// What the work waits on the owner for of this, which it waits for at this cell.
fn at_cell(
    kept: &Kept<'_>,
    records: &Records,
    words: &OwnerWords,
    at: &CellId,
    waited: &Waited,
    waits: &mut Vec<Waiting>,
) {
    let cell = at.as_str();
    match waited {
        Waited::Call { call, .. } => {
            if let Some(Call::AtLimit(limit)) = kept.work.calls.get(call) {
                let why = match limit {
                    Limit::Returns { sent_back } => {
                        let sent_back = sent_back.to_string();
                        fill(&words.returns, &[("cell", cell), ("sent_back", sent_back.as_str())])
                    }
                    Limit::Calls { made } => {
                        let made = made.to_string();
                        fill(&words.made, &[("cell", cell), ("made", made.as_str())])
                    }
                };
                push(waits, LIMIT.to_string(), why, Ends::GoOn { call: *call });
            }
        }
        Waited::Precondition { on: key, by } if by.contains(&Maker::Owner) => {
            for block in decide::keeping(&kept.flow.flow, records, at, key) {
                let Some(outsiders) = kept.flow.flow.outsiders(&block) else {
                    continue;
                };
                if outsiders.people {
                    let requires = key_words(key);
                    let places = [("block", block.as_str()), ("cell", cell), ("key", requires.as_str())];
                    let ends = Ends::Write {
                        value: holding(kept, key, &block),
                        block: block.clone(),
                    };
                    push(waits, format!("write-{block}"), fill(&words.ruling, &places), ends);
                } else if outsiders.by_owner {
                    // What its source writes changes as the owner answers the work's questions: each that stands
                    // without the owner's answer to its latest version, which the cell waits for.
                    for (name, standing) in questions::questions(records) {
                        if standing.unanswered() {
                            question(kept, name, &standing, waits);
                        }
                    }
                }
            }
        }
        Waited::Limit { runs } => {
            let runs = runs.to_string();
            let why = fill(&words.limit, &[("cell", cell), ("runs", runs.as_str())]);
            push(waits, LIMIT.to_string(), why, Ends::LetGo { cell: at.clone() });
        }
        Waited::Calls { runs } => {
            let runs = runs.to_string();
            let why = fill(&words.calls, &[("cell", cell), ("runs", runs.as_str())]);
            push(waits, LIMIT.to_string(), why, Ends::LetGo { cell: at.clone() });
        }
        Waited::Answer { question: name } => {
            if let Some(standing) = questions::questions(records).get(name.as_str()) {
                question(kept, name, standing, waits);
            }
        }
        Waited::Precondition { .. } | Waited::Slot | Waited::Outside { .. } => {}
    }
}

/// The wait for the owner's answer to the latest version of a question of the work, said by what it asks.
fn question(kept: &Kept<'_>, name: &str, standing: &Standing<'_>, waits: &mut Vec<Waiting>) {
    let why = match kept.work.question(standing.latest) {
        Some(work::Question::Asked { ask, .. }) => ask.clone(),
        _ => name.to_string(),
    };
    let latest = standing.latest;
    let ends = Ends::Answer {
        question: name.to_string(),
        version: kept.question_version(&latest.cell, latest.at),
    };
    let key = format!("answer-{name}");
    if !waits.iter().any(|w| w.key == key) {
        waits.push(Waiting { key, why, ends });
    }
}

/// Adds the wait, where none of its key is there already: a question the work waits for both at its cell and at the
/// merge is one.
fn push(waits: &mut Vec<Waiting>, key: String, why: String, ends: Ends) {
    if !waits.iter().any(|w| w.key == key) {
        waits.push(Waiting { key, why, ends });
    }
}

/// A key of a cell's `requires` as the cell's file writes it: `review_ok = yes`, `test = settled`.
fn key_words(key: &Condition) -> String {
    match key {
        Condition::Is { block, value } => format!("{block} = {value}"),
        Condition::Test(name) => format!("test = {name}"),
    }
}

/// The value of the block that makes the key hold, where the block's values make it plain: the key's own value, or the
/// named test's where it holds when a block has it; where it holds when none has it, the block's only other value.
/// None where that is not plain, as of a test of content, which looks for no value.
fn holding(kept: &Kept<'_>, key: &Condition, block: &BlockId) -> Option<String> {
    let values: &[String] = (kept.flow.blocks.get(block))
        .and_then(|b| b.values.as_deref())
        .unwrap_or_default();
    let has = |value: &str| values.iter().any(|v| v == value).then(|| value.to_string());
    match key {
        Condition::Is { value, .. } => has(value),
        Condition::Test(name) => {
            let test = kept.flow.flow.tests().get(name)?;
            let is = test.is.as_deref()?;
            match test.kind {
                TestKind::All | TestKind::Any => has(is),
                TestKind::None | TestKind::NoneStanding => {
                    let others: Vec<&String> = values.iter().filter(|v| v.as_str() != is).collect();
                    match others.as_slice() {
                        [other] => Some((*other).clone()),
                        _ => None,
                    }
                }
                TestKind::AnyContent => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Each kind the one definition names is counted, with its key, its reason in the words beside the flows and the
    //! act that ends it; each kind that is not the owner's to end is not. The decisions are given by hand, so that each
    //! kind is tried on its own, whatever records would bring it about.

    use std::collections::BTreeMap;

    use decide::{Outsider, Pause, Questioned, Returned, RunId, Who, Written};
    use flow::{Compiled, Has, Source, compile};
    use store::{First, Item, ItemNumber, WorkHead, WorkNumber};
    use work::{Back, Question, Said, Time, Work};

    use super::*;

    /// A writer, which writes a text and a verdict that people may write too, and the merge, which requires the verdict
    /// to be `yes` and the questions to have the owner's answers: what the engine writes of them only the owner's
    /// answers change. With the words beside the repository's flows.
    fn flow() -> Compiled {
        let text = r#"
            name = "small"
            order = ["write", "merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.text]
            [blocks.verdict]
            values = ["yes", "no"]
            people = true
            [blocks.asked]
            values = ["waiting", "answered"]
            default = "answered"
            outside = "asked"
            changed_by = "owner"
            [blocks.merged]
            shown = true
        "#;
        let write = r#"
            title = "Write"
            kind = "agent"
            effort = "high"
            line = "main"
            task = "Write the text the request asks for."
            watches = ["request"]
            outputs = ["text", "verdict"]

            [kinds]
            request = "last_and_latest"
        "#;
        let merge = r#"
            title = "Merge"
            kind = "engine"
            act = "merge"
            requires = { verdict = "yes", asked = "answered" }
            watches = ["text"]
            outputs = ["merged"]
        "#;
        let source = Source {
            flow: text.to_string(),
            cells: [("write", write), ("merge", merge)]
                .iter()
                .map(|(id, text)| (id.to_string(), text.to_string()))
                .collect(),
            files: [(
                flow::waits_file(),
                include_str!("../../../flow/agents/waits.toml").to_string(),
            )]
            .into(),
            ..Source::default()
        };
        let has = Has {
            sources: ["asked".to_string()].into(),
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    fn head() -> WorkHead {
        WorkHead {
            number: WorkNumber(12),
            item: ItemNumber(3),
            flow: "small".to_string(),
            added: Time(0),
        }
    }

    fn item() -> Item {
        let first = First {
            title: "Greet".to_string(),
            level: None,
            project: None,
            depends_on: Vec::new(),
            tags: Vec::new(),
        };
        Item::new(ItemNumber(3), Time(0), first, Vec::new())
    }

    /// What the work on these records, so decided, waits on the owner for, nothing holding every work.
    fn of(work: &Work, decision: &Decision) -> Vec<Waiting> {
        of_with(work, decision, None, Shared::default())
    }

    fn of_with(work: &Work, decision: &Decision, held: Option<Holder>, shared: Shared<'_>) -> Vec<Waiting> {
        let (flow, head, item) = (flow(), head(), item());
        let on = crate::On::installed_flow(&flow);
        let kept = Kept {
            head: &head,
            item: &item,
            flow: &flow,
            on: &on,
            passed: &[],
            work,
        };
        waits(&kept, &work.records, decision, held, shared)
    }

    fn at(cell: &str, on: Vec<Waited>) -> Decision {
        Decision::Wait { at: cell.into(), on }
    }

    fn going(cell: &str) -> Decision {
        Decision::GoOn {
            cell: cell.into(),
            run: RunId(1),
        }
    }

    /// The work's first run asked these questions, by their names and whether its cell waits for each, each asking
    /// `Which way for NAME?`; the owner answered those named.
    fn asking(asked: &[(&str, bool)], answered: &[&str]) -> Work {
        let mut work = Work::default();
        work.records.returned.push(Returned {
            run: RunId(1),
            at: Seq(2),
        });
        let questions = (asked.iter())
            .map(|(name, waits)| Question::Asked {
                name: name.to_string(),
                waits: *waits,
                tree: None,
                recommends: Some(1),
                ask: format!("Which way for {name}?"),
                options: Vec::new(),
                about: None,
                reasons: BTreeMap::new(),
            })
            .collect();
        let said = Said {
            questions,
            observations: Vec::new(),
        };
        work.back.insert(
            (RunId(1), 1),
            Back::Returned {
                dealt_with: Vec::new(),
                handed: Vec::new(),
                asks: Vec::new(),
                found: Vec::new(),
                said,
                engine: Vec::new(),
                waits_for: None,
                read: Vec::new(),
            },
        );
        for (i, (name, waits)) in asked.iter().enumerate() {
            work.records.questions.push(Questioned {
                name: name.to_string(),
                cell: "write".into(),
                run: RunId(1),
                engine: false,
                waits: *waits,
                withdrawn: false,
                at: Seq(3 + i as u64),
            });
        }
        for name in answered {
            let version = (work.records.questions.iter()).find(|q| q.name == *name).map(|q| q.at);
            work.records.answers.push(decide::Answered {
                question: name.to_string(),
                version: version.expect("asked"),
                option: 1,
                why: None,
                at: Seq(20),
            });
        }
        work
    }

    /// A version of the block, written from outside the cells with this value.
    fn written(work: &mut Work, block: &str, by: Outsider, value: &str, at: u64) {
        work.records.written.push(Written {
            block: block.into(),
            by,
            matters: true,
            value: Some(value.to_string()),
            empty: false,
            said_to: None,
            at: Seq(at),
        });
    }

    fn paused(work: &mut Work, by: Pauser, why: Option<&str>) {
        work.records.pauses.push(Pause {
            paused: true,
            at: Seq(30),
            by,
            why: why.map(str::to_string),
        });
    }

    fn keys(waits: &[Waiting]) -> Vec<&str> {
        waits.iter().map(|wait| wait.key.as_str()).collect()
    }

    #[test]
    fn a_question_the_work_waits_for_at_its_cell_is_the_owners_said_by_what_it_asks() {
        let work = asking(&[("way", true)], &[]);
        let decision = at(
            "write",
            vec![Waited::Answer {
                question: "way".to_string(),
            }],
        );
        // The cell has no questions of its own in this flow, so no version of them names the question.
        let ends = Ends::Answer {
            question: "way".to_string(),
            version: None,
        };
        assert_eq!(
            of(&work, &decision),
            [Waiting {
                key: "answer-way".to_string(),
                why: "Which way for way?".to_string(),
                ends,
            }]
        );
    }

    #[test]
    fn the_merge_waiting_for_the_owners_answers_waits_for_each_question_that_stands_unanswered() {
        let mut work = asking(&[("a", false), ("b", false), ("c", false)], &["c"]);
        written(&mut work, "asked", Outsider::Outside, "waiting", 10);
        let key = Condition::Is {
            block: "asked".into(),
            value: "answered".to_string(),
        };
        // Who can make it hold is what the flow declares of the block: the owner, whose answers alone change it.
        let decision = at(
            "merge",
            vec![Waited::Precondition {
                on: key,
                by: vec![Maker::Owner],
            }],
        );
        let waits = of(&work, &decision);
        assert_eq!(keys(&waits), ["answer-a", "answer-b"]);
        // Each is said by what it asks, and nothing else is said of why the merge waits for it.
        assert_eq!(waits[1].why, "Which way for b?");
    }

    #[test]
    fn a_block_the_owner_may_write_that_keeps_a_precondition_is_a_ruling_with_the_value_that_makes_it_hold() {
        let mut work = Work::default();
        written(&mut work, "verdict", Outsider::Person, "no", 4);
        let key = Condition::Is {
            block: "verdict".into(),
            value: "yes".to_string(),
        };
        let by = vec![Maker::Owner, Maker::Cell { cell: "write".into() }];
        let decision = at("merge", vec![Waited::Precondition { on: key.clone(), by }]);
        let ends = Ends::Write {
            block: "verdict".into(),
            value: Some("yes".to_string()),
        };
        assert_eq!(
            of(&work, &decision),
            [Waiting {
                key: "write-verdict".to_string(),
                why: "`verdict` keeps `verdict = yes` from holding.".to_string(),
                ends,
            }]
        );
        // One only cells can make hold is not the owner's.
        let by = vec![Maker::Cell { cell: "write".into() }];
        assert_eq!(
            of(&work, &at("merge", vec![Waited::Precondition { on: key, by }])),
            Vec::<Waiting>::new()
        );
    }

    #[test]
    fn a_cell_and_a_run_at_their_limits_are_the_owners_with_what_lets_them_go_on() {
        let mut work = Work::default();
        work.calls
            .insert(CallId(9), Call::AtLimit(Limit::Returns { sent_back: 2 }));
        let at_limit = Waited::Call {
            call: CallId(9),
            on: Some(Who::Owner),
        };
        let run = of(&work, &at("write", vec![at_limit]));
        assert_eq!(keys(&run), [LIMIT]);
        // Each said beside the cell, which it does not name.
        assert_eq!(
            run[0].why,
            "The cell's run had 2 returns sent back, which is its limit."
        );
        assert_eq!(run[0].ends, Ends::GoOn { call: CallId(9) });
        let cell = of(&work, &at("write", vec![Waited::Limit { runs: 3 }]));
        assert_eq!(
            cell[0].why,
            "The cell ran 3 times in a row for what a cell after it wrote, which is its limit."
        );
        assert_eq!(cell[0].ends, Ends::LetGo { cell: "write".into() });
        let calls = of(&work, &at("write", vec![Waited::Calls { runs: 3 }]));
        assert_eq!(keys(&calls), [LIMIT]);
    }

    #[test]
    fn the_engines_pause_is_the_owners_with_its_reason_and_the_owners_own_is_not() {
        let mut work = Work::default();
        paused(&mut work, Pauser::Engine, Some("It spent more than it may."));
        let held = Decision::Paused { cell: "write".into() };
        let waits = of(&work, &held);
        assert_eq!(keys(&waits), [PAUSE]);
        assert_eq!(waits[0].why, "The engine paused the work. It spent more than it may.");
        assert_eq!(waits[0].ends, Ends::Resume);
        // Its round under way goes on, and the pause holds the work once it ends.
        let busy = Decision::Busy {
            cell: "write".into(),
            run: RunId(1),
        };
        assert_eq!(of(&work, &busy), Vec::<Waiting>::new());
        let mut work = Work::default();
        paused(&mut work, Pauser::Owner, None);
        assert_eq!(of(&work, &held), Vec::<Waiting>::new());
    }

    #[test]
    fn the_pause_of_every_work_is_no_wait_of_a_works_own() {
        // The pause of everything is one thing, said and lifted where it is set: a work it holds waits on the owner
        // for nothing of it, whoever set it, so that one pause of five works is one thing waiting and not five, and
        // no work carries the button that lifts it over every work.
        let work = Work::default();
        let by = |pauser: Pauser| Shared {
            everything: Some(pauser),
            cannot_run: None,
        };
        for pauser in [Pauser::Engine, Pauser::Owner] {
            let held = of_with(&work, &going("write"), Some(Holder::Everything), by(pauser));
            assert_eq!(held, [], "{pauser:?}");
            let waits_for_a_slot = of_with(&work, &at("write", vec![Waited::Slot]), None, by(pauser));
            assert_eq!(waits_for_a_slot, [], "{pauser:?}");
        }
    }

    #[test]
    fn the_agents_command_line_that_cannot_run_is_the_owners_and_says_what_to_do() {
        let work = Work::default();
        let cannot = Shared {
            everything: None,
            cannot_run: Some("No account is in use: choose one on the Settings view."),
        };
        let waits = of_with(&work, &going("write"), None, cannot);
        assert_eq!(keys(&waits), [AGENTS]);
        assert_eq!(
            waits[0].why,
            "The agents' command line cannot run. No account is in use: choose one on the Settings view."
        );
        let mend = "Choose an account on the Settings view, add its token again, or install the command line. The \
                    engine lets the work go on once its check finds that the command line runs.";
        assert_eq!(waits[0].ends, Ends::Outside { what: mend.to_string() });
        assert_eq!(
            keys(&of_with(&work, &at("write", vec![Waited::Slot]), None, cannot)),
            [AGENTS]
        );
        // It holds only an agent's round; any other cause of the wait every work shares the engine lifts by itself.
        assert_eq!(of_with(&work, &going("merge"), None, cannot), Vec::<Waiting>::new());
        assert_eq!(
            of_with(&work, &going("write"), None, Shared::default()),
            Vec::<Waiting>::new()
        );
    }

    #[test]
    fn a_fault_a_stall_a_slot_and_a_source_outside_the_work_are_not_the_owners() {
        let work = Work::default();
        let stalled = Decision::Stalled {
            at: "merge".into(),
            on: Vec::new(),
            by: Vec::new(),
        };
        let outside = Waited::Outside {
            source: "main".to_string(),
            run: RunId(1),
            block: "text".into(),
        };
        for decision in [
            Decision::Fault(decide::RecordFault::WithoutEnd),
            stalled,
            at("write", vec![Waited::Slot]),
            at("merge", vec![outside]),
        ] {
            assert_eq!(of(&work, &decision), Vec::<Waiting>::new(), "{decision:?}");
        }
    }
}
