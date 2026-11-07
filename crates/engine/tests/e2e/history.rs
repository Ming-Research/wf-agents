//! The history gathered for a line and looked up by its reader.

use super::*;

// The background is gathered before the research, in a round of the reader's line at the lowest level, and given to the
// research.
#[test]
fn the_history_is_gathered_by_the_reader_of_the_line_librarian() {
    let s = setup("history");
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.lessons",
                    json!([{ "lesson": "The problem's history.", "address": "#1" }]),
                )],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(
        message_at(&e, work, "R1", false).contains("The problem's history."),
        "{log}"
    );
    let session = |cell: &str| session_of(&e, work, cell);
    assert_ne!(session("R0"), session("R1"));
    let line: String =
        e.db.conn()
            .query_row("SELECT line FROM sessions WHERE id = ?1", [session("R0")], |r| r.get(0))
            .unwrap();
    assert_eq!(line, "librarian");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The gatherer is given for git the directory its own round runs in, where the permission check lets git run, and the
// project's main as git has it there, not the project's checkout (#250).
#[test]
fn the_gatherer_s_git_runs_in_its_round_s_directory() {
    let s = setup("history-wal");
    let ran_in: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let mut agents = scripts();
    let seen = ran_in.clone();
    agents.insert(
        "R0",
        Box::new(move |r: &Request, _: usize| {
            *seen.lock().unwrap() = Some(r.worktree.clone());
            (
                Vec::new(),
                vec![(
                    "outcome.lessons",
                    json!([{ "lesson": "The problem's history.", "address": "#1" }]),
                )],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "R1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    let sources = given_value(&message_at(&e, work, "R0", false), "·engine·history_sources").expect(&log);
    let ran_in = ran_in.lock().unwrap().clone().expect(&log);
    assert_eq!(sources["git"]["directory"].as_str(), ran_in.to_str(), "{sources}");
    assert!(sources.get("checkout").is_none(), "{sources}");
    // Git runs there: the main it names is a commit of the repository the directory is a worktree of, the project's.
    let main = sources["git"]["main"].as_str().unwrap();
    assert_eq!(main, "origin/main", "{sources}");
    assert_eq!(
        git(&ran_in, &["rev-parse", "--verify", &format!("{main}^{{commit}}")]),
        git(&s.origin, &["rev-parse", "main"]),
        "{sources}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
