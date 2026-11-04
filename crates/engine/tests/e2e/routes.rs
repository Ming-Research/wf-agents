//! An item's way from the queue to main: the normal route, the light route and the design stage.

use super::*;

#[test]
fn an_item_goes_from_the_queue_to_main() {
    let s = setup("main");
    let (mut e, said, item) = start(&s, scripts());
    let mut done = false;
    for _ in 0..200 {
        e.tick().unwrap();
        if let Some((id, _, paused)) = &state(&e, item) {
            assert!(
                paused.is_none(),
                "the work paused: {paused:?}\n{}",
                said.lock().unwrap().join("\n")
            );
            if e.advance(*id).unwrap() == Advance::Stopped {
                done = true;
                break;
            }
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert!(done, "the work did not end:\n{log}");
    let (at, pr): (String, Option<i64>) =
        e.db.conn()
            .query_row("SELECT at, pr FROM works WHERE item = ?1", [item], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(at, "E1", "{log}");
    assert_eq!(pr, Some(1));
    // Done at its end, the work let go of what it held: no session goes on, and its pull request, merged, is not
    // closed.
    let going: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM sessions s JOIN works w ON w.id = s.work WHERE w.item = ?1 AND s.ended IS NULL",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(going, 0, "{log}");
    let closing: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM outbox WHERE kind = 'close_pr'", [], |r| r.get(0))
            .unwrap();
    assert_eq!(closing, 0, "{log}");
    let main = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
    );
    assert_eq!(main, "old\nfeature");
    let cells: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT cell, transition FROM rounds WHERE work = (SELECT id FROM works WHERE item = ?1) ORDER BY n",
        &[&item],
        1000,
    )
    .unwrap()
    .iter()
    .map(|r| {
        format!(
            "{}:{}",
            r["cell"].as_str().unwrap_or_default(),
            r["transition"].as_str().unwrap_or("-")
        )
    })
    .collect();
    for c in [
        "S1:started",
        "I1:tools",
        "X1:ran",
        "T2:review_first",
        "T3:done",
        "M2:merged",
        "L1:green",
        "E1:-",
    ] {
        assert!(cells.iter().any(|x| x == c), "no round {c}: {cells:?}\n{log}");
    }
    let rejected: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds WHERE rejected IS NOT NULL", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(rejected, 0, "{log}");
    // The table review reads the worktree only, and has no column for a check the engine answered itself because
    // the project lacks what it needs (here a gap in what is under test, and a dependency on another project).
    let input_of = |cell: &str| -> Value {
        let input: String =
            e.db.conn()
                .query_row(
                    "SELECT input FROM rounds WHERE cell = ?1 ORDER BY work, n LIMIT 1",
                    [cell],
                    |r| r.get(0),
                )
                .unwrap();
        db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap())
    };
    let columns = input_of("T4")["engine"]["columns"].to_string();
    assert!(!columns.contains("a gap in what is under test"), "{columns}");
    assert!(!columns.contains("cross-project dependency"), "{columns}");
    assert!(columns.contains("unsure facts"), "{columns}");
    // The table review is given its columns under their address, which a later review of its session refers to or
    // is given what changed in.
    let review = input_of("T4");
    let m = review["message"].as_str().unwrap_or_default();
    assert!(
        m.contains("## Review table: answer each column by its number\n\n### columns:T4\n\n1. "),
        "{m}"
    );
    // The first review has the columns whole in its message, and is not told again which to answer.
    assert!(!m.contains(COLUMNS_TO_ANSWER), "{m}");
    // The review line's session is given the worktree once, at its first round, the blind review, and the CI on the
    // work as one object, whose entries a later review is given as they change: the table review of the same session
    // is not given the whole object again.
    let blind = input_of("T3");
    let message = blind["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("you only read it, and change and commit nothing there"),
        "{message}"
    );
    let ci = given_value(message, "·engine·work_ci").expect(message);
    assert!(ci.is_object() && ci.get("fast_ci").is_some(), "{ci}");
    assert!(given_value(m, "·engine·work_ci").is_none(), "{m}");
    // The review is given what it reviews by commits the worktree has: main as fetched, the commit the branch was cut
    // from, the head under review and the command that reads the change between them.
    let base = given_value(message, "·engine·review_base").expect(message);
    let fork = base["fork_point"].as_str().unwrap_or_default();
    assert_eq!(base["main"], "origin/main", "{base}");
    assert!(
        fork.len() == 40 && base["head"].as_str().is_some_and(|h| h.len() == 40),
        "{base}"
    );
    assert_eq!(
        base["change"],
        format!("git diff {fork}..{}", base["head"].as_str().unwrap())
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Runs the engine until the work on the item ends, failing if it pauses or does not end; gives the work.
fn ran_to_its_end(e: &mut Engine, item: i64, said: &Arc<Mutex<Vec<String>>>) -> i64 {
    for _ in 0..300 {
        e.tick().unwrap();
        if let Some((id, _, paused)) = &state(e, item) {
            assert!(
                paused.is_none(),
                "the work paused: {paused:?}\n{}",
                said.lock().unwrap().join("\n")
            );
            if e.advance(*id).unwrap() == Advance::Stopped {
                return *id;
            }
        }
    }
    panic!("the work did not end:\n{}", said.lock().unwrap().join("\n"));
}

/// The rows a work's rounds took, each as `cell:row`, in order.
fn rows_said(e: &Engine, work: i64) -> Vec<String> {
    rows_taken(e, work)
        .into_iter()
        .filter_map(|(cell, row)| Some(format!("{cell}:{}", row?)))
        .collect()
}

/// The scripts of a bug fix on the light route. The research says the work goes the light route, at its second round:
/// its first says nothing of the route. The plan has one condition. Implementing fixes the bug with a regression test,
/// asking for ci on the head and ci_before on the test, then hands in both results of the test; the table review
/// answers its columns, and nothing of adversarial testing, which the light route has none of.
fn light_scripts() -> BTreeMap<&'static str, Script> {
    let mut s = scripts();
    s.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let route = if n == 1 {
                Vec::new()
            } else {
                vec![("outcome.route", "light")]
            };
            (route, Vec::new())
        }),
    );
    s.insert(
        "F1",
        Box::new(|_: &Request, _: usize| {
            let plan = json!({
                "verdict": "planned", "cause": "The parser drops the last line.", "change": "Keep the last line.",
                "tests": "regression: red before the fix, green after it.",
                "acceptance": [{ "by": "test", "id": "A1", "condition": "The last line is kept." }],
            });
            (Vec::new(), vec![("outcome.plan", plan)])
        }),
    );
    s.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            let done = git(wt, &["log", "--format=%s"]).lines().any(|l| l == "Fix the bug");
            if !done {
                write(wt, "src/lib.txt", "old\nfixed\n");
                write(wt, &format!("tests/{REGRESSION}.txt"), &format!("{REGRESSION}\n"));
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Fix the bug"]);
                let head = git(wt, &["rev-parse", "HEAD"]);
                let tools = json!([
                    { "tool": "ci", "head": head, "tests": [REGRESSION],
                      "pr": { "title": "Fix the bug", "body": "It fixes it." } },
                    { "tool": "ci_before", "head": head, "files": [format!("tests/{REGRESSION}.txt")],
                      "tests": [REGRESSION] },
                ]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            // The regression test is written once, in the test changes, with its two results.
            let added = json!([{ "change": "added", "test": REGRESSION, "catches": "The last line dropped.",
                                 "kind": "new_behaviour", "on_head": "green", "on_base": "red",
                                 "time": "1 ms", "worth": "No other test reads the last line." }]);
            (
                Vec::new(),
                vec![
                    ("outcome.head", json!(head)),
                    ("outcome.test_changes", added),
                    (
                        "outcome.conditions",
                        json!([{ "how": "tested", "id": "A1", "tests": [REGRESSION] }]),
                    ),
                ],
            )
        }),
    );
    s.insert("T4", answers_columns(|_| Vec::new()));
    s
}

// A bug fix goes the light route from its start to main. Its research, sent back once for saying nothing of the route
// its item is marked for, says it goes light; like fixes are gathered, the plan is made and found sound, which locks the
// plan's acceptance, and the fix is implemented with a regression test that ci_before shows red before the change and ci
// green after it. The full CI runs on the draft, and the table review, which goes on in the session that reviewed the
// plan, is given both results of the test. No round of the judgment whether it stands, the design, its checks,
// adversarial testing or the blind review runs.
#[test]
fn a_bug_fix_goes_the_light_route_from_its_start_to_main() {
    let s = setup("light-route");
    let (mut e, said, item) = start(&s, light_scripts());
    marked(&mut e, item, "bugfix");
    let work = ran_to_its_end(&mut e, item, &said);
    let log = said.lock().unwrap().join("\n");
    let rows = rows_said(&e, work);
    let wanted = [
        "R1:light",
        "G4:done",
        "F1:planned",
        "F2:sound",
        "I1:done",
        "T7:full_ci",
        "T2:review_again",
        "T4:done",
        "M2:merged",
    ];
    let mut at = rows.iter();
    for w in wanted {
        assert!(at.any(|r| r == w), "no {w} in order: {rows:?}\n{log}");
    }
    let ran: Vec<String> = path_of(&e, work);
    for skipped in [
        "R2", "G2", "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11", "T1", "T3", "G3",
    ] {
        assert!(!ran.iter().any(|c| c == skipped), "{skipped} ran: {ran:?}");
    }
    assert_eq!(counted(&e, work, "route_to_normal"), 0, "{rows:?}");
    assert_eq!(route_of(&e, work), "light");
    // The research that said nothing of the route was sent back for it, once.
    let back = why_sent_back(&e, work, "R1");
    assert_eq!(back.len(), 1, "{back:?}");
    assert!(
        back[0].contains("the work goes the light route: say in route whether"),
        "{back:?}"
    );
    // The plan found sound locked its acceptance, from the plan's round.
    let (acceptance, locked_at): (String, i64) =
        e.db.conn()
            .query_row(
                "SELECT acceptance, acceptance_round FROM items WHERE id = ?1",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(acceptance.contains("The last line is kept."), "{acceptance}");
    let planned: i64 =
        e.db.conn()
            .query_row(
                "SELECT n FROM rounds WHERE work = ?1 AND cell = 'F1' AND transition = 'planned'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(locked_at, planned);
    // ci_before ran the regression test on the code before the change, which was red.
    let before: String =
        e.db.conn()
            .query_row(
                "SELECT result FROM tool_runs WHERE work = ?1 AND tool = 'ci_before'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(before.contains("\"result\":\"red\""), "{before}");
    // The table review is given the test's results before and after the change, in the session of the plan's review.
    let review = message_at(&e, work, "T4", false);
    let results = given_value(&review, "·engine·new_tests").expect(&review);
    assert_eq!(
        results,
        json!([{ "name": REGRESSION, "by": "engineer", "on_head": "green", "on_base": "red" }])
    );
    assert_eq!(session_of(&e, work, "T4"), session_of(&e, work, "F2"));
    let main = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
    );
    assert_eq!(main, "old\nfixed");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The review of the plan sends it back twice, and each time the work plans it again; the third time it still sends it
// back, the work turns to the normal route, counted once. It is then judged to stand, designed and checked, and its first
// review is the blind review, in a session of its own and not the one that reviewed the plan, which the table review goes
// on in.
#[test]
fn a_plan_sent_back_a_third_time_turns_the_work_to_the_normal_route() {
    let s = setup("light-to-normal");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (vec![("outcome.route", "light")], Vec::new())),
    );
    agents.insert(
        "F2",
        Box::new(|_: &Request, _: usize| (vec![("outcome.review", "revise")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    marked(&mut e, item, "feature");
    let work = ran_to_its_end(&mut e, item, &said);
    let rows = rows_said(&e, work);
    let wanted = [
        "R1:light",
        "G4:done",
        "F1:planned",
        "F2:revise",
        "F1:planned",
        "F2:revise",
        "F1:planned",
        "F2:revised_out",
        "R2:stands",
        "D2:first",
        "D11:not_needed",
        "I1:done",
        "T2:review_first",
        "T3:done",
        "G3:done",
        "T4:done",
        "M2:merged",
    ];
    let mut at = rows.iter();
    for w in wanted {
        assert!(at.any(|r| r == w), "no {w} in order: {rows:?}");
    }
    assert_eq!(counted(&e, work, "plan_revisions"), 2, "{rows:?}");
    assert_eq!(counted(&e, work, "route_to_normal"), 1, "{rows:?}");
    // The design's acceptance is the one locked, not the last plan's.
    let acceptance: String =
        e.db.conn()
            .query_row("SELECT acceptance FROM items WHERE id = ?1", [item], |r| r.get(0))
            .unwrap();
    assert!(acceptance.contains("The feature is there."), "{acceptance}");
    let (plan, blind, table) = (
        session_of(&e, work, "F2"),
        session_of(&e, work, "T3"),
        session_of(&e, work, "T4"),
    );
    assert_ne!(plan, blind);
    assert_eq!(blind, table);
    assert_eq!(route_of(&e, work), "turned");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The route the work goes, as the view the engine, the owner's page and the architect's board read it.
fn route_of(e: &Engine, work: i64) -> String {
    e.db.conn()
        .query_row("SELECT route FROM work_routes WHERE work = ?1", [work], |r| r.get(0))
        .unwrap()
}

// A bug fix on the light route that the owner's word sends back to design after it was implemented turns to the normal
// route by the way on's own row, counted as any turn is, and is designed, checked and blind reviewed. Its table review
// has the columns of what ran: the plan its review found sound and the plan's tests, and no judgment that the work
// stands, which never ran.
#[test]
fn a_bug_fix_redesigned_from_the_light_route_turns_to_the_normal_route() {
    let s = setup("light-redesign");
    let mut agents = light_scripts();
    // Turned to the normal route, its table review judges its design's choice of no adversarial testing, as any first
    // review of such a design does.
    agents.insert("T4", answers_columns(|_| vec![("outcome.no_adversarial", "stands")]));
    agents.insert(
        "A4",
        Box::new(|_: &Request, n: usize| {
            let next = if n == 1 { "redesign" } else { "carry_on" };
            (vec![("outcome.next", next)], Vec::new())
        }),
    );
    // The design hands in the acceptance the plan locked, word for word.
    agents.insert(
        "D2",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.measure", "not_measured")],
                vec![(
                    "outcome.acceptance",
                    json!([{ "by": "test", "id": "A1", "condition": "The last line is kept." }]),
                )],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    marked(&mut e, item, "bugfix");
    step_to(&mut e, item, "T7");
    let (work, _, _) = state(&e, item).unwrap();
    assert_eq!(route_of(&e, work), "light");
    owner_says(&mut e, work, "Design it after all: the parser's interface changes.");
    let ended = ran_to_its_end(&mut e, item, &said);
    assert_eq!(ended, work);
    let rows = rows_said(&e, work);
    let wanted = [
        "R1:light",
        "F2:sound",
        "I1:done",
        "A4:redesign_from_light",
        "D2:first",
        "T2:review_first",
        "T3:done",
        "T4:done",
        "M2:merged",
    ];
    let mut at = rows.iter();
    for w in wanted {
        assert!(at.any(|r| r == w), "no {w} in order: {rows:?}");
    }
    assert_eq!(counted(&e, work, "route_to_normal"), 1, "{rows:?}");
    assert_eq!(route_of(&e, work), "turned");
    let review = message_at(&e, work, "T4", false);
    assert!(review.contains("The plan the review found sound"), "{review}");
    assert!(review.contains("The plan's tests"), "{review}");
    assert!(
        !review.contains("The research's judgment that the work stands"),
        "{review}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work past its research keeps the route it took, as the design and the items' table say: an item that went the
// normal route and is marked a bug fix once its research answered still has its work judged whether it stands, designed
// and checked, and its first review is the blind review; nothing of the light route runs, and no turn is counted.
#[test]
fn an_item_marked_a_bug_fix_after_its_research_keeps_its_work_on_the_normal_route() {
    let s = setup("marked-late");
    let (mut e, said, item) = start(&s, scripts());
    let mut researched = None;
    for _ in 0..100 {
        e.tick().unwrap();
        if let Some((id, _, paused)) = &state(&e, item) {
            assert!(paused.is_none(), "the work paused: {paused:?}");
            if rows_said(&e, *id).iter().any(|r| r == "R1:done") {
                researched = Some(*id);
                break;
            }
            e.advance(*id).unwrap();
        }
    }
    let work = researched.expect("the research did not answer");
    // The item is marked while the work is still well before its review.
    let before = path_of(&e, work);
    assert!(!before.iter().any(|c| c == "T7"), "{before:?}");
    marked(&mut e, item, "bugfix");
    let ended = ran_to_its_end(&mut e, item, &said);
    assert_eq!(ended, work);
    let rows = rows_said(&e, work);
    let wanted = [
        "R1:done",
        "R2:stands",
        "D2:first",
        "D11:not_needed",
        "I1:done",
        "T2:review_first",
        "T3:done",
        "G3:done",
        "T4:done",
        "M2:merged",
    ];
    let mut at = rows.iter();
    for w in wanted {
        assert!(at.any(|r| r == w), "no {w} in order: {rows:?}");
    }
    let ran = path_of(&e, work);
    for light in ["G4", "F1", "F2"] {
        assert!(!ran.iter().any(|c| c == light), "{light} ran: {ran:?}");
    }
    assert_eq!(counted(&e, work, "route_to_normal"), 0, "{rows:?}");
    // The work goes the normal route as its page and the board show it, whatever the item's mark says now.
    assert_eq!(route_of(&e, work), "normal");
    // The table review is asked of the judgment that the work stands, and not of a plan.
    let review = message_at(&e, work, "T4", false);
    assert!(
        review.contains("The research's judgment that the work stands"),
        "{review}"
    );
    assert!(!review.contains("The plan the review found sound"), "{review}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Which way the design stage goes is the engine's cells' rows, never a switch hidden in a gate: a project with nothing
// under test and no other project goes past those gates (`D4` `no`, `D8` `no`), each a round of the engine's own; the
// cells on the ways not taken never run.
#[test]
fn the_design_stage_goes_its_way_by_the_engines_rows() {
    let s = setup("design-way");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "I1");
    let log = said.lock().unwrap().join("\n");
    // An item with no decision has none its design departs from: the wait for a ruling lets the work go at once.
    for (cell, row) in [("D4", "no"), ("D8", "no"), ("D12", "clear")] {
        assert_eq!(
            ended_at(&e, item, cell),
            [(Some(row.to_string()), None)],
            "{cell}\n{log}"
        );
    }
    for cell in ["D5", "D9"] {
        assert!(ended_at(&e, item, cell).is_empty(), "{cell}\n{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// On a redesign, a check after design is given what the design changed since it last answered, sentence by sentence,
// with its answer then; the first time it judges the whole design and is given no change.
#[test]
fn a_check_after_a_redesign_is_given_what_the_design_changed() {
    let s = setup("gates-change");
    let mut agents = scripts();
    agents.insert(
        "D2",
        Box::new(|_: &Request, n: usize| {
            let solution = if n == 1 {
                "Read the input. Parse it into tokens."
            } else {
                "Read the input. Parse it into tokens. Keep the tokens' positions."
            };
            (
                vec![("outcome.measure", "not_measured")],
                vec![
                    (
                        "outcome.acceptance",
                        json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }]),
                    ),
                    ("outcome.design.solution", json!(solution)),
                    (
                        "outcome.design.where",
                        json!("The parser module and its tests, and nothing else of the program."),
                    ),
                ],
            )
        }),
    );
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "redesign")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T7");
    let (work, _, _) = state(&e, item).unwrap();
    owner_says(&mut e, work, "Keep the tokens' positions too.");
    for _ in 0..30 {
        if ended_at(&e, item, "D6").len() == 2 {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "D6").len(), 2, "{log}");
    let first = message_at(&e, work, "D6", false);
    assert!(!first.contains("engine·design_change"), "{first}");
    let again = message_at(&e, work, "D6", true);
    assert!(again.contains("+Keep the tokens' positions."), "{again}");
    assert!(again.contains("your_last_answer"), "{again}");
    assert!(!again.contains("-Read the input."), "{again}");
    let _ = std::fs::remove_dir_all(&s.root);
}
