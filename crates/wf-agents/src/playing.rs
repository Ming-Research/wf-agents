//! `wf-agents play`: plays a script whose agents are all stood in for (design document `testing`, 14), on the engine
//! `wf-agents run --script` runs, made by the same function, in this process and on a clock of its own whose time is
//! paused, so that rounds of seconds play in milliseconds and every act is given at the step its state comes; and says
//! of each item whether its work took the path the script states, or the first step that left it, with what the
//! records hold there. It plays on an empty data directory, which it leaves holding the store, so that a run with the
//! same script serves what it made. A run whose agents are all stood in for is the case this serves: the tests' and
//! the build of the page's states (18).
//!
//! Its clock starts at the time the script names as its start, every other time it names being an offset from it, so
//! that what a play records is the same whatever the day; where the script names none, at the bound of a play's clock
//! before now, so that no time it records is later than now.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use service::{Bounds, Clock, ItemPlayed, Played, SystemClock};
use work::Time;

use crate::running::{self, Read, With};

/// When a play ends at the latest: once it has taken this many things, or once a day of its clock has passed, which
/// passes at once while nothing is under way.
pub const BOUND: Bounds = Bounds {
    steps: 100_000,
    ms: 86_400_000,
};

/// How many lines of items a play prints at most, those that did not take their paths first.
pub const SHOWN: usize = 20;

/// Plays the script on the flows in these directories over the data directory, and says how each item's work went;
/// fails where the script or the data directory is refused, the loop stopped, or any item did not take its path or an
/// act of it was not given or was refused.
pub fn play(data: &Path, dirs: &[PathBuf], script: &Path) -> ExitCode {
    // A play writes its store anew: a data directory that holds anything would hold another store, or another play's.
    let holds = std::fs::read_dir(data).map(|mut entries| entries.next().is_some());
    if holds.unwrap_or(false) {
        eprintln!(
            "{}: the data directory holds files already: a play writes its store into an empty one",
            data.display()
        );
        return ExitCode::FAILURE;
    }
    let with = With::Script(script.to_path_buf());
    let Some((flows, Read::Script(read, beside))) = running::read(data, dirs, &with) else {
        return ExitCode::FAILURE;
    };
    // Held until the play ends, as a run holds it.
    let _held = match running::hold(data) {
        Ok(held) => held,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };
    let start = match SystemClock.now() {
        Ok(now) => Time(now.0.saturating_sub(BOUND.ms)),
        Err(why) => {
            eprintln!("the clock cannot be read: {why}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build();
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("the engine cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    let project = running::script_project(script);
    let open = |clock: Arc<dyn Clock>| {
        let carriers = running::script_carriers((*read).clone(), (*beside).clone());
        running::engine(data, flows.clone(), carriers, clock)
    };
    let played = runtime.block_on(service::play(&read, &flows, &beside, &open, &project, start, BOUND));
    match played {
        Err(faults) => {
            for fault in faults {
                eprintln!("{}: {fault}", script.display());
            }
            ExitCode::FAILURE
        }
        Ok(played) => {
            print!("{}", said(&played, script, data));
            match played.took() {
                true => ExitCode::SUCCESS,
                false => ExitCode::FAILURE,
            }
        }
    }
}

/// What came of the play: a line for each item, those that did not go as the script says first, at most [`SHOWN`] of
/// them, and what was left out with how to see it; a line for each round entry of the script that no round took, at
/// most as many, and how many more; and a line for what stopped the loop.
fn said(played: &Played, script: &Path, data: &Path) -> String {
    let mut items: Vec<(usize, &ItemPlayed)> = played.items.iter().enumerate().collect();
    items.sort_by_key(|(_, item)| item.took());
    let mut said = String::new();
    for (k, item) in items.iter().take(SHOWN) {
        said.push_str(&item.line(*k));
        said.push('\n');
    }
    let more = items.len().saturating_sub(SHOWN);
    if more > 0 {
        let off = (items.iter().skip(SHOWN)).filter(|(_, item)| !item.took()).count();
        said.push_str(&format!(
            "And {more} more items, {off} of which did not take their paths: `wf-agents run --script {} --data {}` \
             serves the store the play left, and `wf-agents list --all` lists them.\n",
            script.display(),
            data.display()
        ));
    }
    for untaken in played.untaken.iter().take(SHOWN) {
        said.push_str(untaken);
        said.push('\n');
    }
    let more = played.untaken.len().saturating_sub(SHOWN);
    if more > 0 {
        said.push_str(&format!(
            "And {more} more round entries of {} that no round took.\n",
            script.display()
        ));
    }
    if let Some(stopped) = played.stopped() {
        said.push_str(&stopped);
        said.push('\n');
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;

    use service::Judged;
    use store::WorkNumber;

    // What a play prints is bounded: the items that did not take their paths come first, those left out are counted
    // with the commands that show them, and the round entries no round took follow. Microseconds alone; the only test
    // of the bound.
    #[test]
    fn a_play_prints_at_most_its_bound_of_items_those_that_did_not_take_their_paths_first() {
        let item = |n: u64, judged: Judged| ItemPlayed {
            work: WorkNumber(n),
            judged,
            unreached: Vec::new(),
            refused: Vec::new(),
        };
        let left = Judged::Left {
            step: 3,
            said: Some("{ runs = \"build\" }".to_string()),
            records: "{ runs = \"review\" }".to_string(),
        };
        let mut items: Vec<ItemPlayed> = (1..=30).map(|n| item(n, Judged::Took)).collect();
        items[25] = item(26, left.clone());
        items[27] = item(28, left);
        let untaken = "flows.trial.rounds[5]: no round of the play took it, round 1 of the cell review's run 3";
        let played = Played {
            items,
            untaken: vec![untaken.to_string()],
            steps: 400,
            fault: None,
        };
        let said = said(&played, Path::new("cases.toml"), Path::new("store"));
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), SHOWN + 2, "{said}");
        assert!(
            lines[0].starts_with("items[25], W26: left its path at step 3"),
            "{said}"
        );
        assert!(
            lines[1].starts_with("items[27], W28: left its path at step 3"),
            "{said}"
        );
        assert_eq!(lines[2], "items[0], W1: took its path");
        assert_eq!(
            lines[SHOWN],
            "And 10 more items, 0 of which did not take their paths: `wf-agents run --script cases.toml --data store` \
             serves the store the play left, and `wf-agents list --all` lists them."
        );
        assert_eq!(lines[SHOWN + 1], untaken);
    }
}
