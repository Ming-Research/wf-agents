//! What stops the loop, and why something asked of it was refused, in plain words for the owner's interface and the
//! command line alike, so that both say a thing the same way.

use std::fmt;

use decide::{End, Outsider, RecordFault};
use driver::{Failed, NotMade, Unchangeable};
use engine::Refused;
use engine::spoken::NoOne;
use store::{Continues, ItemNumber};

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
                    "what was said to have come of something to be done for {work}: {failed}"
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
                    "the driver gave something to be done for {} that names {what}",
                    todo.work
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
        Failed::NoSuchItem(item) => format!("there is no item {item}"),
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
        Failed::BadContinues(continues) => bad_continues(continues),
        Failed::NoStart(flow) => no_start(flow),
        Failed::NoSuchWork(work) => format!("there is no work {work}"),
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

/// Why what an item was to continue is refused, as a clause.
fn bad_continues(continues: &Continues) -> String {
    match continues {
        Continues::Branch(name) => format!(
            "{name:?} is no branch an item may continue: a branch name is not empty, begins with no dash or \
             slash, ends with no dot or slash, has no part between slashes that begins with a dot or ends with \
             `.lock`, and holds no space, control character, `..`, `//`, `@{{` or any of ~ ^ : ? * [ \\"
        ),
        Continues::Pull(number) => format!("{number} is no pull request an item may continue: its number is 1 or more"),
    }
}

/// Why an item whose flow has nothing that starts a work may continue nothing, as a clause.
fn no_start(flow: &str) -> String {
    format!(
        "the flow {flow} has no cell that starts a work, so nothing would read what the item continues: leave it out"
    )
}

/// Items by number, in words: `#3`, `#3 and #5`, `#3, #5 and #9`.
fn items(numbers: &[ItemNumber]) -> String {
    let named: Vec<String> = numbers.iter().map(|n| n.to_string()).collect();
    match named.as_slice() {
        [] => "no item".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Why a change was refused, as a clause.
fn why_not_made(refused: &NotMade) -> String {
    match &refused.why {
        driver::changes::Refused::NoSuchItem(item) => format!("there is no item {item}"),
        driver::changes::Refused::BlankTitle => blank_title().to_string(),
        driver::changes::Refused::BadTag(tag) => bad_tag(tag),
        driver::changes::Refused::BadContinues(continues) => bad_continues(continues),
        driver::changes::Refused::NoStart => format!(
            "the flow of the work of {item} has no cell that starts a work, so nothing would read what the item \
             continues: leave it out",
            item = refused.item
        ),
        driver::changes::Refused::Unchangeable(why) => unchangeable_words(refused.item, why),
    }
}

/// Why an item may not be changed so, as a clause.
fn unchangeable_words(item: ItemNumber, why: &Unchangeable) -> String {
    match why {
        Unchangeable::Ended(End::Merged) => {
            format!("{item} has merged: an item that has ended takes no change, and is what it ended as")
        }
        Unchangeable::Ended(End::Cancelled) => format!(
            "{item} is cancelled: an item that has ended takes no change, and is what it ended as; reopen it first"
        ),
        Unchangeable::Started => format!(
            "the work of {item} has started, and what an item depends on changes only until its work starts: \
             what it depends on is what its work started on. What turns up for it is a new item, which may depend on it"
        ),
        Unchangeable::Placed => format!(
            "the work of {item} has made its branch, and what an item continues changes only until its work's \
             branch is made: the branch was made from it"
        ),
        Unchangeable::Circle(way) if way.len() == 1 => format!("{item} cannot depend on itself"),
        Unchangeable::Circle(way) => {
            let way: Vec<String> = way.iter().map(|on| on.to_string()).collect();
            format!(
                "{item} cannot depend on {}, which depends on it already: {}",
                way[0],
                way.join(" depends on ")
            )
        }
        Unchangeable::NotDependent(on) => format!("{item} does not depend on {on}"),
        Unchangeable::NotTagged(tag) => format!("{item} does not carry the tag {tag}"),
        Unchangeable::MergedDependants(merged) => format!(
            "{} merged depending on {item}, and a merged item depends on nothing that has not ended",
            items(merged)
        ),
        Unchangeable::StartedDependants(started) => {
            let (depend, works, they) = match started.len() {
                1 => ("depends", "its work has", "it has"),
                _ => ("depend", "their works have", "they have"),
            };
            format!(
                "{} {depend} on {item}, and {works} started: a work goes on from what it depended on as it \
                 started, so {item} is not reopened while {they} not ended",
                items(started)
            )
        }
        Unchangeable::NotCancelled => format!("{item} is not cancelled: only a cancelled item is reopened"),
        Unchangeable::LettingGo => {
            format!("the cancel of {item} is still letting go of what its work held: reopen it once that is done")
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
        F::NoSuchFlow { work, flow } => format!("{work} runs on the flow {flow}, which was not given"),
        F::NotOneWork(item) => format!("the store holds {item} without exactly one work"),
        F::Refused { work, refused } => format!(
            "what the driver itself gave the engine's loop for {work} was refused: {}",
            refused_words(refused)
        ),
        F::Outline { version, why } => format!("the outline kept of version {version} of a flow does not read: {why}"),
        F::NoSuchVersion { work, version } => {
            format!("{work} is on version {version} of its flow, which the store does not keep")
        }
        F::EarlierIds {
            flow,
            version,
            works,
            why,
        } => {
            let works: Vec<String> = works.iter().map(|work| work.to_string()).collect();
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
                "the table of earlier ids of the flow {flow} would misread what the works {works} recorded: {why}; mend the \
                 table in the flow",
                works = works.join(", ")
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
        F::NoSuchWork(work) => format!("it has no work {work}"),
        F::NoSuchDependency(item) => format!("an item was to depend on {item}, which it does not have"),
        F::NoSuchItem(item) => format!("it has no item {item}"),
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
    use super::*;

    #[test]
    fn a_change_of_what_an_item_continues_is_refused_once_its_work_made_its_branch_and_says_so() {
        let refused = NotMade {
            at: 0,
            item: ItemNumber(3),
            why: driver::changes::Refused::Unchangeable(Unchangeable::Placed),
        };
        let failed = Failed::NotChanged {
            asked: 1,
            refused: vec![refused],
        };
        assert_eq!(
            failed_words(&failed),
            "the work of #3 has made its branch, and what an item continues changes only until its work's branch is \
             made: the branch was made from it"
        );
    }
}
