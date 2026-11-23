//! `wf-agents repair-costs`: corrects once the costs recorded before a round's cost was what its results say it spent
//! (crate `carry`, `repair`), while no engine runs on the data directory. It is removed once the owner has run it on the
//! installation's data directory.

use std::path::Path;
use std::process::ExitCode;

use store::Store;

pub fn run(data: &Path, limit: usize) -> ExitCode {
    // Held until the process ends: an engine that runs would write to the store meanwhile.
    let _held = match crate::running::hold(data) {
        Ok(held) => held,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };
    let mut store = match Store::open(data.join(store::FILE)) {
        Ok(store) => store,
        Err(fault) => {
            let words = service::store_fault_words(&fault);
            eprintln!(
                "{}: the store cannot be opened: {words}",
                data.join(store::FILE).display()
            );
            return ExitCode::FAILURE;
        }
    };
    match carry::repair::repair(data, &mut store, limit) {
        Ok(said) => {
            said.iter().for_each(|line| println!("{line}"));
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
