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

#[cfg(test)]
mod tests {
    use super::*;

    // A data directory an engine holds is refused before the store is opened: no store file is made in it. A few
    // milliseconds, in a temporary directory; no other test runs the command, and without the lock taken first the
    // command would open the store, copy it and correct it under the engine.
    #[test]
    fn a_data_directory_an_engine_holds_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let _engine = crate::running::hold(data.path()).unwrap();
        assert_eq!(
            format!("{:?}", run(data.path(), 10)),
            format!("{:?}", ExitCode::FAILURE)
        );
        assert!(!data.path().join(store::FILE).exists(), "the store was not opened");
    }
}
