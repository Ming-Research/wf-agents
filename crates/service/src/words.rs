//! What stops the loop, and why something asked of it was refused, in plain words for the owner's interface and the
//! command line alike, so that both say a thing the same way.

use std::fmt;

use decide::{End, Outsider, RecordFault};
use driver::{Failed, NotMade, Unchangeable};
use engine::Refused;
use engine::spoken::NoOne;
use store::ItemNumber;

use crate::{Fault, Missing, Refusal};

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Driver(fault) => f.write_str(&driver_fault(fault)),
            Fault::Clock(words) => write!(f, "the clock cannot be read: {words}"),
            Fault::Answer { work, failed } => {
                let failed = failed_words(failed);
                write!(
                    f,
                    "what was said to have come of something to be done for work {}: {failed}",
                    work.0
                )
            }
            Fault::Stray { todo, missing } => {
                let what = match missing {
                    Missing::Work => "a work it does not hold".to_string(),
                    Missing::Flow(flow) => format!("the flow {flow}, which it was not given"),
                    Missing::Cell(cell) => format!("the cell {cell}, which its flow does not have"),
                    Missing::Run(run) => format!("run {}, which never started", run.0),
                    Missing::Given(run) => format!("a round under way of run {} that was given nothing", run.0),
                    Missing::Call(call) => format!("call {}, which was never made", call.0),
                };
                write!(
                    f,
                    "the driver gave something to be done for work {} that names {what}",
                    todo.work.0
                )
            }
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Failed(Failed::Stopped { fault, kept }) => {
                let stopped = sentence(&format!("the engine stopped: {}", driver_fault(fault)));
                let kept = if *kept {
                    "What you asked was kept: do not give it again."
                } else {
                    "What you asked was not kept: give it again once the engine runs."
                };
                write!(f, "{stopped} {kept}")
            }
            Refusal::Failed(failed) => f.write_str(&sentence(&failed_words(failed))),
            Refusal::NotAPersonsCall(call) => {
                write!(
                    f,
                    "Call {call} is a tool's: the tool gives its result, not a person.",
                    call = call.0
                )
            }
            Refusal::NotKept(why) => f.write_str(&sentence(&format!("no account was added: {why}"))),
            Refusal::Closed(why) => f.write_str(&sentence(&format!("no account can be added or taken: {why}"))),
            Refusal::Gone => f.write_str("The engine is not running."),
        }
    }
}

/// Why the driver did not do what it was asked, as a clause.
fn failed_words(failed: &Failed) -> String {
    match failed {
        Failed::NoSuchItem(item) => format!("there is no item #{}", item.0),
        // One change asked is refused in its own words; of several, each refused is named by its place, and none was
        // made.
        Failed::NotChanged { asked: 1, refused } if refused.len() == 1 => why_not_made(&refused[0]),
        Failed::NotChanged { refused, .. } => {
            let said: Vec<String> = (refused.iter())
                .map(|refused| format!("change {}: {}", refused.at + 1, why_not_made(refused)))
                .collect();
            format!("{}; so none of the changes was made", said.join("; "))
        }
        Failed::BlankTitle => blank_title().to_string(),
        Failed::BadTag(tag) => bad_tag(tag),
        Failed::NoSuchWork(work) => format!("there is no work {}", work.0),
        Failed::NoSuchFlow(flow) => format!("there is no flow named {flow}"),
        Failed::Refused(refused) => refused_words(refused),
        Failed::Stopped { fault, .. } => format!("the engine stopped: {}", driver_fault(fault)),
        Failed::AccountName(why) => why.clone(),
        Failed::NameInUse(name) => format!("an account named {name} is added already: give the new one another name"),
        Failed::NoSuchAccount(name) => format!("there is no account named {name}"),
        Failed::RemovingInUse(name) => {
            format!("the agents run under {name}: take another account first, then remove it")
        }
    }
}

/// Why a title of no words is refused, as a clause.
fn blank_title() -> &'static str {
    "a title says what the item is to deliver, and one of no words says nothing: give it words"
}

/// Why a tag is refused, as a clause.
fn bad_tag(tag: &str) -> String {
    format!(
        "{tag:?} is no tag: a tag is a name of 1 to {} characters with no space and no comma",
        driver::changes::TAG_CHARS
    )
}

/// Items by number, in words: `#3`, `#3 and #5`, `#3, #5 and #9`.
fn items(numbers: &[ItemNumber]) -> String {
    let named: Vec<String> = numbers.iter().map(|n| format!("#{}", n.0)).collect();
    match named.as_slice() {
        [] => "no item".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Why a change was refused, as a clause.
fn why_not_made(refused: &NotMade) -> String {
    match &refused.why {
        driver::changes::Refused::NoSuchItem(item) => format!("there is no item #{}", item.0),
        driver::changes::Refused::BlankTitle => blank_title().to_string(),
        driver::changes::Refused::BadTag(tag) => bad_tag(tag),
        driver::changes::Refused::Unchangeable(why) => unchangeable_words(refused.item, why),
    }
}

/// Why an item may not be changed so, as a clause.
fn unchangeable_words(item: ItemNumber, why: &Unchangeable) -> String {
    let item = item.0;
    match why {
        Unchangeable::Ended(End::Merged) => {
            format!("#{item} has merged: an item that has ended takes no change, and is what it ended as")
        }
        Unchangeable::Ended(End::Cancelled) => format!(
            "#{item} is cancelled: an item that has ended takes no change, and is what it ended as; reopen it first"
        ),
        Unchangeable::Started => format!(
            "the work of #{item} has started, and what an item depends on changes only until its work starts: \
             what it depends on is what its work started on. What turns up for it is a new item, which may depend on it"
        ),
        Unchangeable::Circle(way) if way.len() == 1 => format!("#{item} cannot depend on itself"),
        Unchangeable::Circle(way) => {
            let way: Vec<String> = way.iter().map(|on| format!("#{}", on.0)).collect();
            format!(
                "#{item} cannot depend on {}, which depends on it already: {}",
                way[0],
                way.join(" depends on ")
            )
        }
        Unchangeable::NotDependent(on) => format!("#{item} does not depend on #{}", on.0),
        Unchangeable::NotTagged(tag) => format!("#{item} does not carry the tag {tag}"),
        Unchangeable::MergedDependants(merged) => format!(
            "{} merged depending on #{item}, and a merged item depends on nothing that has not ended",
            items(merged)
        ),
        Unchangeable::StartedDependants(started) => {
            let (depend, works, they) = match started.len() {
                1 => ("depends", "its work has", "it has"),
                _ => ("depend", "their works have", "they have"),
            };
            format!(
                "{} {depend} on #{item}, and {works} started: a work goes on from what it depended on as it \
                 started, so #{item} is not reopened while {they} not ended",
                items(started)
            )
        }
        Unchangeable::NotCancelled => format!("#{item} is not cancelled: only a cancelled item is reopened"),
        Unchangeable::LettingGo => {
            format!("the cancel of #{item} is still letting go of what its work held: reopen it once that is done")
        }
    }
}

/// Why what happened cannot have happened to the work, as a clause.
pub fn refused_words(refused: &Refused) -> String {
    match refused {
        Refused::NoSuchBlock(block) => format!("the flow has no block {block}"),
        Refused::NotTheirs {
            block,
            by: Outsider::Person,
        } => format!("people do not write the block {block}"),
        Refused::NotTheirs {
            block,
            by: Outsider::Outside,
        } => format!("nothing outside the work writes the block {block}"),
        Refused::NoValue { block } => format!("the block {block} is a choice, and is written with no value"),
        Refused::NotAValue { block, value } => format!("{value} is not one of the values of the block {block}"),
        Refused::NotAChoice { block } => format!("the block {block} is no choice, and is written with a value"),
        Refused::NoSuchCell(cell) => format!("the flow has no cell {cell}"),
        Refused::NoRoundUnderWay(run) => format!("no round of run {} is under way", run.0),
        Refused::NoSuchRound { run, round } => format!("run {} has no round {round} that began", run.0),
        Refused::NoSuchCall(call) => format!("the work has no call {}", call.0),
        Refused::AlreadyDone(call) => format!("call {} has its result already", call.0),
        Refused::NoSuchLine(line) => format!("the flow has no cell on the line {line}"),
        Refused::NotToRelease => {
            "the work has not ended, or something of it is under way: it has nothing to let go".to_string()
        }
        Refused::NoSuchQuestion(question) => format!("the work has no question {question}"),
        Refused::NoSuchVersion { question, version } => {
            format!("the question {question} has no version {version} that asks it")
        }
        Refused::NotAnOption { question, option } => {
            format!("the version of the question {question} answered has no option {option}")
        }
        Refused::WithoutEnd(decision) => {
            format!("what is decided on the work's records leads to deciding again without end, last {decision:?}")
        }
        Refused::NotCancelled => "the work's item is not cancelled: there is no cancel to reopen".to_string(),
        Refused::NoOneSpokenTo(why) => no_one_words(why),
        Refused::NotInstalled => {
            "the work ended on a version of its flow that is not installed, which decides nothing more of it"
                .to_string()
        }
    }
}

/// Why no cell can take the owner's words, in words: the words go to no other cell.
pub fn no_one_words(why: &NoOne) -> String {
    match why {
        NoOne::Ended => "the work has ended: no cell of it takes words now".to_string(),
        NoOne::NoSuchLine(line) => format!("the flow has no cell on the line {line}"),
        NoOne::NoSuchCell(cell) => format!("the flow has no cell {cell}"),
        NoOne::NoneOfTheLine(line) => format!("no cell of the line {line} has words of its own"),
        NoOne::NotReached(line) => {
            format!("no cell of the line {line} that has words of its own has run, and the work stands at none of them")
        }
        NoOne::NotSpokenTo(cell) => format!("the words would reach {cell}, which has no words of its own"),
        NoOne::Skipped(cell) => format!("the words would reach {cell}, which does not apply now"),
    }
}

/// What is wrong with records that cannot be those of a work, as a clause.
pub fn record_fault_words(fault: &RecordFault) -> String {
    match fault {
        RecordFault::RunTwice(run) => format!("two runs started as run {}", run.0),
        RecordFault::UnknownRun(run) => format!("an entry names run {}, which never started", run.0),
        RecordFault::UnknownCall(call) => format!("a result names call {}, which was never made", call.0),
        RecordFault::FinishedTwice(run) => format!("run {} finished twice", run.0),
        RecordFault::WroteTwice(run) => format!("run {} wrote one block twice as it finished", run.0),
        RecordFault::DoneTwice(call) => format!("call {} has two results", call.0),
        RecordFault::FinishedAndAbandoned(run) => format!("run {} both finished and was abandoned", run.0),
        RecordFault::ReturnedWithoutRound(run) => format!("more rounds of run {} ended than began", run.0),
        RecordFault::PlaceTwice(at) => format!("two entries are at place {}", at.0),
        RecordFault::PutInNoVersion(at) => format!("what was put in at place {} is no version of its block", at.0),
        RecordFault::PutInBeforeWritten(at) => {
            format!(
                "what was put in at place {} was written at that place or after it",
                at.0
            )
        }
        RecordFault::PutInNoRound(at) => format!(
            "what was put in at place {} went to a run with no round under way",
            at.0
        ),
        RecordFault::DealtWithUnknown(run) => {
            format!(
                "run {} says it dealt with a block that neither ran it nor was put in to it",
                run.0
            )
        }
        RecordFault::DealtWithTwice(run) => format!("run {} names one block twice as dealt with", run.0),
        RecordFault::CalledAfterEnd(call) => format!("call {} was made after its run had ended", call.0),
        RecordFault::AnswerNoVersion(at) => format!(
            "the answer at place {} names no version of its question that asks it, written before it",
            at.0
        ),
        RecordFault::WithoutEnd => {
            "what is decided on the work's records leads to deciding again without end, and the \
                                    engine decides nothing more of it"
                .to_string()
        }
        RecordFault::ReopenedNotCancelled(at) => {
            format!("the work was reopened at place {} with no cancel to reopen", at.0)
        }
    }
}

/// What stopped the driver, as a clause.
fn driver_fault(fault: &driver::Fault) -> String {
    use driver::Fault as F;
    match fault {
        F::Store(fault) => format!("the store failed: {}", store_fault_words(fault)),
        F::FlowTwice(flow) => format!("two of the flows are named {flow}"),
        F::NotDependencies { flow, block } => format!(
            "the block {block} of the flow {flow} is written by the source {}, but is no choice of {} and {} that neither people nor a cell write",
            driver::DEPENDENCIES,
            driver::WAITING,
            driver::MERGED,
        ),
        F::DependenciesTwice { flow } => format!(
            "more than one block of the flow {flow} is written by the source {}",
            driver::DEPENDENCIES
        ),
        F::NoSuchFlow { work, flow } => format!("work {} runs on the flow {flow}, which was not given", work.0),
        F::NotOneWork(item) => format!("the store holds item {} without exactly one work", item.0),
        F::Refused { work, refused } => format!(
            "what the driver itself gave the engine's loop for work {} was refused: {}",
            work.0,
            refused_words(refused)
        ),
        F::Outline { version, why } => format!("the outline kept of version {version} of a flow does not read: {why}"),
        F::NoSuchVersion { work, version } => {
            format!(
                "work {} is on version {version} of its flow, which the store does not keep",
                work.0
            )
        }
        F::EarlierIds {
            flow,
            version,
            works,
            why,
        } => {
            let works: Vec<String> = works.iter().map(|work| work.0.to_string()).collect();
            let why = match &**why {
                driver::Misread::Rewritten => format!("it does not begin with the table of version {version}"),
                driver::Misread::TwoAsOne { cells, reads_as } => {
                    let cells: Vec<&str> = cells.iter().map(|cell| cell.as_str()).collect();
                    format!(
                        "it reads the cells {} of version {version} as the one cell {reads_as}",
                        cells.join(", ")
                    )
                }
                driver::Misread::Lost { cell, reads_as } => format!(
                    "it reads the cell {cell} of version {version} as {reads_as}, which the flow neither has nor took out"
                ),
            };
            format!(
                "the table of earlier ids of the flow {flow} would misread what works {} recorded: {why}; mend the table \
                 in the flow",
                works.join(", ")
            )
        }
    }
}

/// Why the store failed, as a clause.
pub fn store_fault_words(fault: &store::Fault) -> String {
    use store::Fault as F;
    match fault {
        F::Database(words) => format!("the database failed: {words}"),
        F::Schema {
            found: Some(found),
            expected,
        } => format!(
            "the database was made with version {found} of the schema, and this program keeps version {expected}"
        ),
        F::Schema { found: None, expected } => {
            format!("the database has tables but no version of the schema, and this program keeps version {expected}")
        }
        F::NoSuchWork(work) => format!("it has no work {}", work.0),
        F::NoSuchDependency(item) => format!("an item was to depend on item {}, which it does not have", item.0),
        F::NoSuchItem(item) => format!("it has no item {}", item.0),
        F::Diverged { table, entry, how } => {
            let how = match how {
                store::Divergence::Changed => "holds something else",
                store::Divergence::Missing => "lacks",
            };
            let entry = match entry {
                store::Entry::At(at) => format!("place {}", at.0),
                store::Entry::Version { block, at } => format!("the version of {block} at place {}", at.0),
                store::Entry::Round { run, round } => format!("round {round} of run {}", run.0),
                store::Entry::Call(call) => format!("call {}", call.0),
            };
            format!("the work given to be saved {how} {entry} in the table {table}, and nothing was saved")
        }
        F::Value { table, message } => format!("a value of the table {table} could not be read or written: {message}"),
    }
}

/// A clause as a sentence: its first letter upper case, and a full stop at its end.
fn sentence(clause: &str) -> String {
    let mut chars = clause.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    //! What the owner and the command line are told where something stopped or was refused: each reason says what it
    //! is about, so that whoever reads it knows which line, cell, question or version it speaks of.

    use std::collections::BTreeSet;

    use decide::CellId;
    use store::WorkNumber;

    use super::*;

    fn says(words: &str, parts: &[&str]) {
        for part in parts {
            assert!(words.contains(part), "{words:?} does not say {part:?}");
        }
    }

    #[test]
    fn each_reason_no_cell_takes_the_owner_s_words_is_said_with_what_it_is_about() {
        let cell = || CellId::from("R1");
        let line = || "planner".to_string();
        let cases: [(NoOne, &[&str]); 7] = [
            (NoOne::Ended, &["ended"]),
            (NoOne::NoSuchLine(line()), &["no cell on the line planner"]),
            (NoOne::NoSuchCell(cell()), &["no cell R1"]),
            (NoOne::NoneOfTheLine(line()), &["planner", "words of its own"]),
            (NoOne::NotReached(line()), &["planner", "has run"]),
            (NoOne::NotSpokenTo(cell()), &["R1", "no words of its own"]),
            (NoOne::Skipped(cell()), &["R1", "does not apply"]),
        ];
        let mut all = BTreeSet::new();
        for (why, parts) in cases {
            let words = no_one_words(&why);
            says(&words, parts);
            // The refusal of the loop says it as its own reason says it.
            assert_eq!(refused_words(&Refused::NoOneSpokenTo(why)), words);
            all.insert(words);
        }
        assert_eq!(all.len(), 7, "two reasons are said alike: {all:?}");
    }

    #[test]
    fn a_question_a_version_or_an_option_the_work_has_not_is_refused_naming_which_and_where_the_reopen_and_the_install_say_why()
     {
        let question = || "check".to_string();
        let cases: [(Refused, &[&str]); 8] = [
            (Refused::NoSuchQuestion(question()), &["no question check"]),
            (
                Refused::NoSuchVersion {
                    question: question(),
                    version: 3,
                },
                &["question check", "no version 3"],
            ),
            (
                Refused::NotAnOption {
                    question: question(),
                    option: 4,
                },
                &["question check", "no option 4"],
            ),
            (Refused::NoSuchLine("planner".to_string()), &["line planner"]),
            (Refused::NoSuchCell(CellId::from("R9")), &["no cell R9"]),
            (Refused::NotCancelled, &["not cancelled", "no cancel to reopen"]),
            (Refused::NotInstalled, &["not installed"]),
            (Refused::NotToRelease, &["has not ended"]),
        ];
        let mut all = BTreeSet::new();
        for (refused, parts) in cases {
            let words = refused_words(&refused);
            says(&words, parts);
            all.insert(words);
        }
        assert_eq!(all.len(), 8, "two refusals are said alike: {all:?}");
    }

    #[test]
    fn what_stopped_the_driver_on_a_flow_version_is_said_with_the_work_the_version_and_the_cells_it_misreads() {
        let flow = || "judged".to_string();
        let words = |fault: driver::Fault| Fault::Driver(fault).to_string();
        says(
            &words(driver::Fault::Outline {
                version: "v1".to_string(),
                why: "a field is lacking".to_string(),
            }),
            &["version v1", "does not read", "a field is lacking"],
        );
        says(
            &words(driver::Fault::NoSuchVersion {
                work: WorkNumber(7),
                version: "v1".to_string(),
            }),
            &["work 7", "version v1", "does not keep"],
        );
        let earlier = |why| {
            words(driver::Fault::EarlierIds {
                flow: flow(),
                version: "v1".to_string(),
                works: vec![WorkNumber(3), WorkNumber(5)],
                why: Box::new(why),
            })
        };
        let rewritten = earlier(driver::Misread::Rewritten);
        says(
            &rewritten,
            &["judged", "works 3, 5", "does not begin with the table of version v1"],
        );
        let two = earlier(driver::Misread::TwoAsOne {
            cells: vec![CellId::from("A"), CellId::from("B")],
            reads_as: CellId::from("C"),
        });
        says(&two, &["cells A, B", "the one cell C"]);
        let lost = earlier(driver::Misread::Lost {
            cell: CellId::from("A"),
            reads_as: CellId::from("D"),
        });
        says(&lost, &["the cell A", "as D", "neither has nor took out"]);
        assert_eq!(
            BTreeSet::from([rewritten, two, lost]).len(),
            3,
            "two ways of misreading are said alike"
        );
    }

    #[test]
    fn a_store_that_refuses_a_save_says_where_the_work_given_differs_from_what_it_holds() {
        let words = |how, entry| {
            store_fault_words(&store::Fault::Diverged {
                table: "on_flow",
                entry,
                how,
            })
        };
        let changed = words(store::Divergence::Changed, store::Entry::At(decide::Seq(12)));
        says(
            &changed,
            &["holds something else", "place 12", "table on_flow", "nothing was saved"],
        );
        let lacking = words(store::Divergence::Missing, store::Entry::At(decide::Seq(12)));
        says(&lacking, &["lacks", "place 12"]);
        let round = words(
            store::Divergence::Changed,
            store::Entry::Round {
                run: decide::RunId(4),
                round: 2,
            },
        );
        says(&round, &["round 2 of run 4"]);
    }
}
