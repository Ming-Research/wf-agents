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

use service::{Bounds, Played};

/// When a play ends at the latest: once it has taken this many things, or once a day of its clock has passed, which
/// passes at once while nothing is under way.
pub const BOUND: Bounds = Bounds {
    steps: 100_000,
    ms: 86_400_000,
};

/// How many lines of items a play prints at most, those that did not take their paths first.
pub const SHOWN: usize = 20;

/// Plays nothing yet, and fails.
pub fn play(data: &Path, dirs: &[PathBuf], script: &Path) -> ExitCode {
    let _ = (dirs, BOUND, SHOWN);
    let played = Played {
        items: Vec::new(),
        steps: 0,
        fault: None,
    };
    print!("{}", said(&played, script, data));
    ExitCode::FAILURE
}

/// Says nothing yet.
fn said(_played: &Played, _script: &Path, _data: &Path) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    use service::{ItemPlayed, Judged};
    use store::WorkNumber;

    // What a play prints is bounded: the items that did not take their paths come first, and those left out are counted
    // with the commands that show them. Microseconds alone; the only test of the bound.
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
        let played = Played {
            items,
            steps: 400,
            fault: None,
        };
        let said = said(&played, Path::new("cases.toml"), Path::new("store"));
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), SHOWN + 1, "{said}");
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
    }
}
