//! What stops the loop, and why something asked of it was refused, in plain words for the owner's interface and the
//! command line alike, so that both say a thing the same way.

use std::fmt;

use decide::run::Unmet;
use decide::{Outsider, RecordFault};
use driver::Failed;
use engine::Refused;

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
            Refusal::Failed(failed) => f.write_str(&sentence(&failed_words(failed))),
            Refusal::NotAPersonsCall(call) => {
                write!(
                    f,
                    "Call {call} is a tool's: the tool gives its result, not a person.",
                    call = call.0
                )
            }
            Refusal::Gone => f.write_str("The engine is not running."),
        }
    }
}

/// Why the driver did not do what it was asked, as a clause.
fn failed_words(failed: &Failed) -> String {
    match failed {
        Failed::NoSuchItem(item) => format!("there is no item {}", item.0),
        Failed::NoSuchWork(work) => format!("there is no work {}", work.0),
        Failed::NoSuchFlow(flow) => format!("there is no flow named {flow}"),
        Failed::Refused(refused) => refused_words(refused),
        Failed::Stopped { fault, .. } => format!("the engine stopped: {}", driver_fault(fault)),
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
        Refused::Unmet(unmet) => unmet_words(unmet),
        Refused::NoSuchCell(cell) => format!("the flow has no cell {cell}"),
        Refused::NoRoundUnderWay(run) => format!("no round of run {} is under way", run.0),
        Refused::NoSuchCall(call) => format!("the work has no call {}", call.0),
        Refused::AlreadyDone(call) => format!("call {} has its result already", call.0),
    }
}

/// Why what was handed in or written does not meet what the flow requires, as a clause.
pub fn unmet_words(unmet: &Unmet) -> String {
    match unmet {
        Unmet::AsksNothing => "it asks for calls, and for none".to_string(),
        Unmet::NotAnOutput { block } => format!("the block {block} is not one the cell may write"),
        Unmet::WrittenTwice { block } => format!("the block {block} is written twice"),
        Unmet::NoValue { block } => format!("the block {block} is a choice, and is written with no value"),
        Unmet::NotAValue { block, value } => format!("{value} is not one of the values of the block {block}"),
        Unmet::NotAChoice { block } => format!("the block {block} is no choice, and is written with a value"),
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
