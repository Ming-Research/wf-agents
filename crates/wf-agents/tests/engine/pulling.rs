//! `wf-agents pulls`: the pull requests the host of a project says are open, asked through the command line, the route
//! and the carrier of the engine's acts, which here is the script's host.

use crate::{Engine, Place, SCRIPT};

// What the script's host says is open comes back a line each, whole and with no work, since no work of the script keeps
// a pull request; what is asked for from a number on leaves out the lower, and a project the engine has none of is refused
// in the engine's words. One whole run of the command line over the engine, for the seam between the command, the route
// and the carrier, which no test of one of them sees.
#[test]
fn pulls_gives_the_pull_requests_the_projects_host_says_are_open_and_that_no_work_keeps() {
    let script = format!(
        "{SCRIPT}\n[[pulls]]\nproject = \"script\"\nnumber = 12\ntitle = \"Greet the user\"\ndraft = true\nupdated = 0\n\n\
         [[pulls]]\nproject = \"script\"\nnumber = 14\ntitle = \"Greet them all\"\nupdated = 0\n"
    );
    let place = Place::new(&script);
    let engine = Engine::start(&place);
    let said = engine.says(&["pulls"]);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines.len(), 2, "{said}");
    assert!(
        lines[0].starts_with("script pull request 12 \"Greet the user\" (draft), updated ")
            && lines[0].ends_with(" ago, https://host.example/pull/12: no work keeps it."),
        "{said}"
    );
    assert!(
        lines[1].starts_with("script pull request 14 \"Greet them all\", updated ")
            && lines[1].ends_with(" ago, https://host.example/pull/14: no work keeps it."),
        "{said}"
    );
    let from = engine.says(&["pulls", "--from", "13"]);
    assert_eq!(from.lines().count(), 1, "{from}");
    assert!(from.starts_with("script pull request 14 "), "{from}");
    assert_eq!(
        engine.says(&["pulls", "--from", "15"]),
        "No pull request is open numbered 15 or more.\n"
    );
    assert_eq!(
        engine.refuses(&["pulls", "--project", "nowhere"]),
        "There is no project \"nowhere\"; there is script.\n"
    );
}
