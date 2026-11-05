//! The reviews and the fixes: their tables, the reviews again, and which side looks again after a fix or a merge
//! of main.

use super::*;

// The advisor is the engineer with a higher model: an item's advisor round runs on the level one above the item's,
// with that level's command line and model.
#[test]
fn the_advisor_runs_one_level_above_the_item() {
    let s = setup("advisor-level");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!("{install}\n[agents.levels.m]\ndesc = \"one level up\"\ncli = \"claude\"\nmodel = \"m-up\"\n"),
    );
    git(&s.data, &["commit", "-q", "-am", "a level above"]);
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                return (vec![("outcome", "ask"), ("outcome.ask", "advisor")], Vec::new());
            }
            implement(r, n - 1)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let advised = |e: &Engine| -> Option<(String, String)> {
        e.db.conn()
            .query_row(
                "SELECT r.model, s.line FROM rounds r JOIN works w ON w.id = r.work JOIN sessions s ON s.id = r.session
                 WHERE w.item = ?1 AND s.line = 'advisor' ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()
    };
    for _ in 0..60 {
        if advised(&e).is_some() {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(advised(&e), Some(("m-up".to_string(), "advisor".to_string())), "{log}");
    let engineer: String =
        e.db.conn()
            .query_row(
                "SELECT r.model FROM rounds r JOIN works w ON w.id = r.work JOIN sessions s ON s.id = r.session
                 WHERE w.item = ?1 AND s.line = 'engineer' ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(engineer, "m", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A value of one branch is the cell as its newest return left it: a conflict the merge queue reported is in the fix
// table only while the newest run of that cell still reports it.
#[test]
fn a_settled_row_leaves_the_fix_table() {
    let s = setup("settled");
    let (mut e, _, item) = start(&s, scripts());
    for _ in 0..3 {
        e.step(None).unwrap();
    }
    let (work, _, _) = state(&e, item).unwrap();
    let conflict = json!({ "result": { "kind": "conflict", "files": ["src/conflicted_here.rs"] } });
    let behind = json!({ "result": { "kind": "behind" } });
    let add = |e: &mut Engine, output: &Value| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, 'M2', '{}', ?3, 'for_engineer', 'c', 1, 2)",
                params![work, next_round(tx, work)?, output.to_string()],
            )?;
            tx.execute("UPDATE works SET at = 'T6' WHERE id = ?1", [work])?;
            Ok(())
        })
        .unwrap();
    };
    add(&mut e, &conflict);
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert!(m.contains("src/conflicted_here.rs"), "{m}");
    add(&mut e, &behind);
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert!(!m.contains("src/conflicted_here.rs"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The re-review after a fix is given the table the fix answered, as the fix was given it: a red check with its run and
// the end of its log, beside the fix's answer to it; before any fix it is given none.
#[test]
fn the_review_after_a_fix_is_given_the_table_the_fix_answered() {
    let s = setup("fix-table");
    let (mut e, _, item) = start(&s, scripts());
    for _ in 0..3 {
        e.step(None).unwrap();
    }
    let (work, _, _) = state(&e, item).unwrap();
    move_to(&mut e, work, "T4");
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert_eq!(given_value(&m, "·engine·fix_table"), None, "{m}");
    let row = json!({ "row": "F1", "from": "T2·result[red]", "kind": "red", "name": "lint", "result": "red",
                      "run": "https://example.com/runs/7", "log_tail": "error: an unused import" });
    let input = json!({ "engine": { "table": [row] } });
    let output = json!({ "outcome": { "rows": [{ "answer": "fixed", "id": "F1", "commit": "abc" }], "head": "abc" },
                         "progress": "p", "tell_owner": [], "side_reports": [], "flow_feedback": "none" });
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, ?2, 'T6', ?3, ?4, 'done', 'c', 1, 2)",
            params![work, next_round(tx, work)?, input.to_string(), output.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    move_to(&mut e, work, "T4");
    let m = engine::next_message(&mut e, work).unwrap().text;
    let table = given_value(&m, "·engine·fix_table").expect(&m);
    assert_eq!(table, json!([row]), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An interjection that comes while the work stands at one of the engine's steps and asks only for a change to the code
// is the way on `fix`: the change is a row of the fix's table, which the fix answers like any other, and the work goes
// on as after any fix, with no new design.
#[test]
fn a_change_to_the_code_an_interjection_asks_is_a_row_of_the_fix() {
    let s = setup("way-on-fix");
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.next", "fix")],
                vec![
                    (
                        "outcome.next.change.what",
                        json!("Halt every work on a red check, with a test."),
                    ),
                    ("outcome.next.change.asked_in", json!("The owner: halt every work.")),
                ],
            )
        }),
    );
    // The fix makes the change and asks for ci, then answers its table's one row.
    agents.insert(
        "T6",
        Box::new(|r: &Request, n: usize| {
            let wt = &r.worktree;
            if n == 1 {
                write(wt, "src/lib.txt", "old\nfeature\nhalt\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Halt every work"]);
                let head = git(wt, &["rev-parse", "HEAD"]);
                let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"] }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            (
                Vec::new(),
                vec![
                    ("outcome.head", json!(head)),
                    (
                        "outcome.rows",
                        json!([{ "answer": "fixed", "id": "F1", "commit": head }]),
                    ),
                ],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "M1");
    let (work, _, _) = state(&e, item).unwrap();
    owner_says(&mut e, work, "Halt every work on a red check; nothing else changes.");
    for _ in 0..12 {
        if ended_at(&e, item, "T6")
            .iter()
            .any(|(t, _)| t.as_deref() == Some("done"))
        {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "A4"), [(Some("fix".to_string()), None)], "{log}");
    let fixes = ended_at(&e, item, "T6");
    assert_eq!(
        fixes.last(),
        Some(&(Some("done".to_string()), None)),
        "{fixes:?}\n{log}"
    );
    let table = message_at(&e, work, "T6", false);
    assert!(
        table.contains("Halt every work on a red check, with a test."),
        "{table}"
    );
    assert!(ended_at(&e, item, "D1").is_empty(), "no new design\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work at the fix whose table to answer has no row runs no round there, which could answer nothing: the engine takes
// the fix's row for it, recorded with why, on to the re-check. In the sixth trial such a work went from the fix to
// choosing the way on, whose carry on led back to the fix, round after round.
#[test]
fn a_fix_with_nothing_to_answer_goes_on_without_a_round() {
    let s = setup("nothing-to-answer");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET at = 'T6' WHERE id = ?1", [work])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(e.advance(work).unwrap(), Advance::Moved);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|x| x.1).as_deref(), Some("T4"), "{log}");
    let (transition, session, why): (Option<String>, Option<i64>, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT transition, session, json_extract(input, '$.engine.nothing_to_answer') FROM rounds
                 WHERE work = ?1 AND cell = 'T6'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(transition.as_deref(), Some("nothing_to_answer"), "{log}");
    assert_eq!(session, None, "{log}");
    assert!(why.is_some_and(|w| w.contains("could answer nothing")), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #16, acceptance A9, A10 and A11 through the engine: a condition the plan says is met by the CI or by reading the
// change, and one that says tests but is met by a job of the CI, are answered by rows of kind ci_job and read_diff. The
// first return names a job no CI run of the work has and a command in no file of the branch: it is sent back once,
// saying both. The second names the job the CI results give and a command the branch has, and is taken; the review is
// given the rows as they were handed in.
#[test]
fn conditions_met_by_a_ci_job_or_a_reading_of_the_change_are_taken_and_a_job_or_command_not_there_is_sent_back() {
    let s = setup("ci-job-conditions");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.can_start.acceptance",
                    json!([
                        { "by": "test", "id": "A1", "condition": "The feature is there." },
                        { "by": "ci", "id": "A2", "condition": "The check job passes." },
                        { "by": "diff", "id": "A3", "condition": "The library says it." }
                    ]),
                )],
            )
        }),
    );
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = implement(r, n);
            for (path, value) in set.iter_mut() {
                if *path == "outcome.conditions" {
                    // The first hand-in of the end names what is not there; the next names what is.
                    let (job, step) = if n == 2 {
                        ("build", "no such command anywhere")
                    } else {
                        ("check", "feature")
                    };
                    *value = json!([
                        { "how": "ci_job", "id": "A1", "jobs": [job], "steps": [step] },
                        { "how": "ci_job", "id": "A2", "jobs": ["check"] },
                        { "how": "read_diff", "id": "A3", "where": "src/lib.txt", "what": "It says feature." }
                    ]);
                }
            }
            (pick, set)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let log = || said.lock().unwrap().join("\n");
    // The work comes to the review, which then runs its first round.
    let at_review = steps_until(&mut e, 80, |e| state(e, item).is_some_and(|(_, at, _)| at == "T2"));
    assert!(at_review, "{:?}\n{}", state(&e, item), log());
    e.step(None).unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    let why = why_sent_back(&e, work, "I1");
    assert_eq!(why.len(), 1, "{why:?}\n{}", log());
    assert!(
        why[0].contains(
            "conditions_cover_acceptance (conditions): the job build is not a check of any CI run of this work; the checks recorded are: check"
        ),
        "{}",
        why[0]
    );
    assert!(
        why[0].contains(
            "conditions_cover_acceptance (conditions): the step no such command anywhere is not in any file of the branch"
        ),
        "{}",
        why[0]
    );
    // The review is given the rows as the second return handed them in.
    let review = message_at(&e, work, "T2", false);
    let rows = given_value(&review, "·I1·conditions").expect(&review);
    assert_eq!(rows[0]["how"], "ci_job", "{rows}");
    assert_eq!(rows[0]["jobs"], json!(["check"]), "{rows}");
    assert_eq!(rows[2]["how"], "read_diff", "{rows}");
    assert_eq!(rows[2]["where"], "src/lib.txt", "{rows}");
    let _ = std::fs::remove_dir_all(&s.root);
}
