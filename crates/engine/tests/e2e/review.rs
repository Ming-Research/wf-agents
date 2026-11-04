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

// A value of one branch is the cell as its newest return left it: a conflict the full CI reported is in the fix
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
    let green = json!({ "result": { "kind": "green", "head": "abc" } });
    let add = |e: &mut Engine, output: &Value| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, 'T2', '{}', ?3, 'next', 'c', 1, 2)",
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
    add(&mut e, &green);
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

// Item #216: a table review in the session of an earlier one is not given the columns of its table whole, but referred
// to, or given as what changed in them, and it answers every column whatever the change touched. The message says by
// number which columns the round answers, so that a review that reads nothing but the message answers them all and
// `columns_answered` takes it: R233-59 answered 9 of 19 columns and R238-64 8 of 27, each told only "given to you
// before". The review here reads nothing else (`answers_columns`).
#[test]
fn a_re_review_in_the_same_session_is_told_every_column_to_answer() {
    let s = setup("columns-line");
    let mut scripts = scripts();
    // The first review judges the skipping of adversarial testing; a re-review does not.
    scripts.insert(
        "T4",
        answers_columns(|n| {
            if n == 1 {
                vec![("outcome.no_adversarial", "stands")]
            } else {
                Vec::new()
            }
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    let reviews = |e: &Engine| -> Vec<Value> {
        agent_rounds(e, item)
            .into_iter()
            .filter(|r| r.1 == "T4")
            .map(|r| r.2)
            .collect()
    };
    for _ in 0..60 {
        if !reviews(&e).is_empty() {
            break;
        }
        e.step(None).unwrap();
    }
    let log = || said.lock().unwrap().join("\n");
    let message = |round: &Value| round["message"].as_str().unwrap_or_default().to_string();
    let first = reviews(&e);
    assert_eq!(first.len(), 1, "{}", log());
    // The first review is given its columns whole, and is not told which to answer.
    let m = message(&first[0]);
    let all = columns(&m);
    assert!(all.len() >= 3, "{all:?}\n{m}");
    assert!(!m.contains(COLUMNS_TO_ANSWER), "{m}");
    let (work, _, _) = state(&e, item).unwrap();
    let sent_back = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'T4' AND rejected IS NOT NULL",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(sent_back(&e), 0, "{}", log());
    // Another round of the table review, as the work comes back to it.
    let again = |e: &mut Engine| -> Value {
        move_to(e, work, "T4");
        e.step(None).unwrap();
        reviews(e).last().unwrap().clone()
    };
    let insert = |e: &mut Engine, cell: &str, input: &Value, output: &Value, row: &str| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'c', 1, 2)",
                params![
                    work,
                    next_round(tx, work)?,
                    cell,
                    input.to_string(),
                    output.to_string(),
                    row
                ],
            )?;
            Ok(())
        })
        .unwrap();
    };
    let returned =
        |outcome: Value| json!({ "outcome": outcome, "progress": "p", "side_reports": [], "flow_feedback": "none" });

    // The columns are what they were: the message refers to them, and lists every one.
    let second = again(&mut e);
    let m = message(&second);
    assert!(second["call"]["session"]["resumed"].is_string(), "{}", second["call"]);
    assert!(m.contains("columns:T4, given to you before in R"), "{m}");
    assert!(m.contains(&format!("{COLUMNS_TO_ANSWER}{}.", all.join(", "))), "{m}");
    assert_eq!(columns(&m), all, "{m}");
    assert_eq!(sent_back(&e), 0, "{}", log());

    // A deviation of the implementation adds a column: the message gives what changed, and lists every column, the
    // new one among them.
    let deviation = returned(
        json!({ "deviations": [{ "kind": "workaround", "what": "Used the other way.",
                                                      "why": "It was needed." }], "test_changes": [] }),
    );
    insert(&mut e, "I1", &json!({}), &deviation, "done");
    let third = again(&mut e);
    let m = message(&third);
    assert!(m.contains("What changed since"), "{m}");
    let added = m
        .lines()
        .find(|l| l.starts_with('+') && l.contains("The workaround \"Used the other way.\""))
        .unwrap_or_else(|| panic!("no added column for the deviation: {m}"));
    let new_id = added[1..].split_once(". ").unwrap().0.to_string();
    let listed = columns(&m);
    assert!(m.contains(COLUMNS_TO_ANSWER), "{m}");
    assert!(listed.contains(&new_id), "{new_id} not in {listed:?}\n{m}");
    assert!(listed.len() >= all.len(), "{listed:?} {all:?}");
    // The review that answered those columns only was taken: `columns_answered` wants exactly the engine's columns.
    assert_eq!(sent_back(&e), 0, "{}", log());

    // A re-review of disputes has only the column the last review could not verify, and says so, while that number
    // stands for the words it had when the review that could not verify it was given the columns, which its round
    // records.
    let unverified = returned(json!({ "columns": [{ "id": "4", "verdict": "cannot_verify",
                                                    "reason": "Nothing was measured." }] }));
    let recorded = json!({ "engine": { "columns": third["engine"]["columns"] } });
    insert(&mut e, "T4", &recorded, &unverified, "done");
    insert(
        &mut e,
        "T7",
        &json!({}),
        &json!({ "plan": { "review": "disputes", "adversarial": "skipped" } }),
        "review_first",
    );
    let fourth = again(&mut e);
    let m = message(&fourth);
    assert!(m.contains(&format!("{COLUMNS_TO_ANSWER}4.")), "{m}");
    assert_eq!(columns(&m), ["4"], "{m}");
    assert!(!m.contains(COLUMNS_CHANGED), "{m}");
    assert_eq!(sent_back(&e), 0, "{}", log());

    // A review whose record has other words for the number it could not verify, as when a column came in before it,
    // is not pointed at another column by that number: the round lists every column, and says why.
    let mut then = third["engine"]["columns"].clone();
    assert_eq!(then[3][0], "4", "{then}");
    then[3][1] = json!("A column that was the fourth when the last review was given the columns.");
    insert(
        &mut e,
        "T4",
        &json!({ "engine": { "columns": then } }),
        &unverified,
        "done",
    );
    let fifth = again(&mut e);
    let m = message(&fifth);
    assert!(m.contains(COLUMNS_CHANGED), "{m}");
    assert_eq!(columns(&m), listed, "{m}");
    assert_eq!(sent_back(&e), 0, "{}", log());

    // A review whose record keeps no columns (a round of a flow version before the record had them) cannot say what
    // its numbers stood for, and is told the same.
    insert(&mut e, "T4", &json!({}), &unverified, "done");
    let sixth = again(&mut e);
    let m = message(&sixth);
    assert!(m.contains(COLUMNS_CHANGED), "{m}");
    assert_eq!(columns(&m), listed, "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #216, acceptance A1, A2 and A3 through the engine, not through the functions the check is made of: a table
// review that answers a column that is none of them, one column twice and leaves two out is sent back once, with a
// line for each of the three kinds and every column of a kind in its line; one that then leaves out a single column is
// sent back with the one line it always had, and the third return is taken. The reference is the item's own example
// (R246-46/47: columns 4 and 5 left out, said at once) and the text of the single miss before the change.
//
// Item #256, acceptance A1: the record of each return sent back keeps each problem with the check and the field. The
// first has three problems of one check and one field. The second, besides leaving out a column, cites a file that is
// not in the worktree: it fails two checks at once, and is recorded with exactly two problems, one for each check, in
// the order of the checks in the flow, and the text under `check` is their lines, one to a line, in that order.
#[test]
fn a_review_with_columns_unknown_twice_and_left_out_is_sent_back_once_saying_all_of_them() {
    let s = setup("columns-misses");
    let mut scripts = scripts();
    let listed: Arc<Mutex<Vec<String>>> = Arc::default();
    let kept = listed.clone();
    scripts.insert(
        "T4",
        Box::new(move |r: &Request, n: usize| {
            let mut all = kept.lock().unwrap();
            if all.is_empty() {
                *all = columns(&r.prompt);
            }
            let ids: Vec<String> = match n {
                // An unknown column, the first column twice, and the last two columns left out.
                1 => {
                    let mut v = all[..all.len() - 2].to_vec();
                    v.push("99".to_string());
                    v.push(all[0].clone());
                    v
                }
                // Only the last column left out.
                2 => all[..all.len() - 1].to_vec(),
                _ => all.clone(),
            };
            // The second return also cites, in its first column, a file that is not in the worktree.
            let cols: Vec<Value> = ids
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let files = if n == 2 && i == 0 {
                        vec!["no/such/file.rs"]
                    } else {
                        Vec::new()
                    };
                    json!({ "id": id, "verdict": "pass", "evidence": { "files": files, "ran": [], "nodes": [] } })
                })
                .collect();
            (
                vec![("outcome.no_adversarial", "stands")],
                vec![("outcome.columns", json!(cols))],
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    let taken = |e: &mut Engine| -> bool {
        state(e, item).is_some_and(|(work, _, _)| {
            e.db.conn()
                .query_row(
                    "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'T4' AND transition IS NOT NULL",
                    [work],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap()
                > 0
        })
    };
    assert!(steps_until(&mut e, 80, taken), "{}", said.lock().unwrap().join("\n"));
    let (work, _, _) = state(&e, item).unwrap();
    let all = listed.lock().unwrap().clone();
    assert!(all.len() >= 3, "{all:?}");
    let why = why_sent_back(&e, work, "T4");
    assert_eq!(why.len(), 2, "{why:?}\n{}", said.lock().unwrap().join("\n"));
    let lines = |text: &str| text.matches("columns_answered (columns.id): ").count();
    // The first return: one line for each kind, and nothing else said.
    assert_eq!(lines(&why[0]), 3, "{}", why[0]);
    assert!(
        why[0].contains("columns_answered (columns.id): there is no column 99"),
        "{}",
        why[0]
    );
    assert!(
        why[0].contains(&format!(
            "columns_answered (columns.id): column {} is answered twice",
            all[0]
        )),
        "{}",
        why[0]
    );
    let n = all.len();
    assert!(
        why[0].contains(&format!(
            "columns_answered (columns.id): columns {}, {} are not answered",
            all[n - 2],
            all[n - 1]
        )),
        "{}",
        why[0]
    );
    // The second return left out one column: the line of a single miss, as before the change.
    assert_eq!(lines(&why[1]), 1, "{}", why[1]);
    assert!(
        why[1].contains(&format!(
            "columns_answered (columns.id): column {} is not answered",
            all[n - 1]
        )),
        "{}",
        why[1]
    );
    // What the column keeps of each return: the text, then the problems.
    let stored: Vec<Value> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'T4' AND rejected IS NOT NULL ORDER BY n",
        &[&work],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].clone())
    .collect();
    assert_eq!(stored.len(), 2);
    for r in &stored {
        assert_eq!(
            r.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["check", "problems"],
            "{r}"
        );
        // The text is the lines of the problems, `check (field): text`, one to a line, in their order.
        let lines: Vec<String> = r["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                format!(
                    "{} ({}): {}",
                    p["check"].as_str().unwrap(),
                    p["field"].as_str().unwrap(),
                    p["text"].as_str().unwrap()
                )
            })
            .collect();
        assert_eq!(r["check"].as_str().unwrap(), lines.join("\n"), "{r}");
    }
    // The first: three problems of one check and one field, each its own entry.
    let first = stored[0]["problems"].as_array().unwrap();
    assert_eq!(first.len(), 3, "{}", stored[0]);
    assert!(
        first
            .iter()
            .all(|p| p["check"] == "columns_answered" && p["field"] == "columns.id"),
        "{}",
        stored[0]
    );
    // The second: exactly two problems, one of each check, with the field each check looks at.
    let second = stored[1]["problems"].as_array().unwrap();
    assert_eq!(second.len(), 2, "{}", stored[1]);
    assert_eq!(
        second[0],
        json!({
            "check": "columns_answered",
            "field": "columns.id",
            "text": format!("column {} is not answered", all[n - 1])
        })
    );
    assert_eq!(
        (&second[1]["check"], &second[1]["field"]),
        (&json!("evidence_holds"), &json!("columns[pass].evidence"))
    );
    assert!(
        second[1]["text"]
            .as_str()
            .unwrap()
            .starts_with("no/such/file.rs is not in the worktree"),
        "{}",
        stored[1]
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The judgment whether a work may enter the merge queue reads the newest review, as the fix table does: a column the
// first review could not verify sends the work to the fix; the re-review that looks only at disputes is given that
// column again, and only it; and once it hands in no table, the first review's column no longer sends the work to a
// fix with nothing to answer, as in the sixth trial.
#[test]
fn the_merge_judgment_reads_the_newest_review_as_the_fix_does() {
    let s = setup("newest-review");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..3 {
        e.step(None).unwrap();
    }
    let (work, _, _) = state(&e, item).unwrap();
    let add = |e: &mut Engine, cell: &str, input: &Value, output: &Value, row: &str, at: &str| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'c', 1, 2)",
                params![
                    work,
                    next_round(tx, work)?,
                    cell,
                    input.to_string(),
                    output.to_string(),
                    row
                ],
            )?;
            tx.execute("UPDATE works SET at = ?2 WHERE id = ?1", params![work, at])?;
            Ok(())
        })
        .unwrap();
    };
    let took = |e: &Engine| -> String {
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'M1' ORDER BY n DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    let first = returned(json!({ "columns": [
        { "id": "1", "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } },
        { "id": "4", "verdict": "cannot_verify", "reason": "Nothing was measured." },
    ] }));
    // The first review's round records the columns it was given, as a real round does; a disputes-only review is
    // narrowed by them.
    let given = {
        let w = e.work(work).unwrap();
        let g = e.graph_of(&w).unwrap();
        let subject = e.subject(&w).unwrap();
        engine::message::columns(&e, &w, &g, g.cell("T4").unwrap(), subject.as_ref()).columns
    };
    assert!(given.len() >= 4, "{given:?}");
    add(
        &mut e,
        "T4",
        &json!({ "engine": { "columns": given } }),
        &first,
        "done",
        "M1",
    );
    e.step_work(work).unwrap();
    assert_eq!(took(&e), "fix", "{}", said.lock().unwrap().join("\n"));
    let disputes = json!({ "plan": { "review": "disputes", "adversarial": "skipped" } });
    add(&mut e, "T7", &json!({}), &disputes, "review_first", "T4");
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert_eq!(columns(&m), ["4"], "{m}");
    let again = returned(json!({ "previous": [{ "fid": "F1", "verdict": "withdraw" }] }));
    add(&mut e, "T4", &json!({}), &again, "done", "M1");
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(!took(&e).starts_with("fix"), "{}\n{log}", took(&e));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A fix that is in no commit, such as the pull request's description, answers the review's finding fixed_outside_commit,
// and the review then looks at that finding again, and only it. In the fourth trial a work whose description lacked two
// files turned from the merge judgment to the fix, which had no row to answer, to choosing what runs next, which
// chose nothing, ninety times and more: the answer left the change as it was, and nothing sent the review back.
#[test]
fn a_fix_in_no_commit_sends_the_review_back_to_the_finding_it_answered() {
    let s = setup("fix-no-commit");
    let mut agents = scripts();
    agents.insert(
        "T4",
        review_with_a_finding(vec![json!([{ "fid": "R1", "verdict": "closed" }])]),
    );
    let answered = "The pull request description now names the two test files; pushed with ci.";
    agents.insert(
        "T6",
        Box::new(move |r: &Request, n: usize| {
            let head = git(&r.worktree, &["rev-parse", "HEAD"]);
            if n == 1 {
                // The description is corrected with ci and no tests: there is nothing to run.
                let pr = json!({ "title": "Add the feature", "body": "It adds the feature and its test." });
                let tools = json!([{ "tool": "ci", "head": head, "pr": pr }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let row = json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": answered }]);
            (Vec::new(), vec![("outcome.head", json!(head)), ("outcome.rows", row)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the review clearing the work", |e| {
        transitions(e, item, "M1").iter().any(|t| t == "clear")
    });
    let (work, _, _) = state(&e, item).unwrap();
    let m1 = transitions(&e, item, "M1");
    assert_eq!(m1.iter().filter(|t| *t == "fix").count(), 1, "{m1:?}");
    let t6 = transitions(&e, item, "T6");
    assert!(!t6.iter().any(|t| t == "nothing_to_answer"), "{t6:?}");
    assert_eq!(transitions(&e, item, "T4"), ["done", "done"]);
    let plans: Vec<String> = e
        .db
        .conn()
        .prepare("SELECT json_extract(output, '$.plan.review') FROM rounds WHERE work = ?1 AND cell = 'T7' ORDER BY n")
        .unwrap()
        .query_map([work], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert!(plans.iter().any(|p| p == "disputes"), "{plans:?}");
    // The re-review is given the row and where the fix is.
    let m = message_at(&e, work, "T4", true);
    assert!(m.contains("fixed_outside_commit") && m.contains(answered), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A work whose review raises one finding, R1, run through the review's later answers and the fix's rows (see
/// [`review_with_a_finding`] and [`fix_answering`]) until the review clears it. When the second round of the fix has
/// answered, `second` replaces the rows it handed in, as a fix that asked the advisor would have answered them. The
/// review looks at the finding again after every fix, and no round of the fix has an empty table.
fn finding_is_cleared(name: &str, review: Vec<Value>, fix: Vec<Value>, second: Option<Value>) {
    let s = setup(name);
    let mut agents = scripts();
    let fixes = review.len();
    agents.insert("T4", review_with_a_finding(review));
    agents.insert("T6", fix_answering(fix));
    let (mut e, said, item) = start(&s, agents);
    let done =
        |e: &Engine, cell: &str, n: usize| transitions(e, item, cell).iter().filter(|t| *t == "done").count() == n;
    if let Some(rows) = second {
        step_until(&mut e, item, &said, "the second fix answering", |e| done(e, "T6", 2));
        let (work, _, _) = state(&e, item).unwrap();
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET output = json_set(output, '$.outcome.rows', json(?2))
                 WHERE work = ?1 AND cell = 'T6' AND n = (SELECT max(n) FROM rounds WHERE work = ?1 AND cell = 'T6'
                                                          AND transition = 'done')",
                params![work, rows.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
    }
    step_until(&mut e, item, &said, "the review clearing the finding", |e| {
        transitions(e, item, "M1").iter().any(|t| t == "clear")
    });
    let t6 = transitions(&e, item, "T6");
    assert!(!t6.iter().any(|t| t == "nothing_to_answer"), "{t6:?}");
    assert_eq!(transitions(&e, item, "T4").len(), fixes + 1);
    let m1 = transitions(&e, item, "M1");
    assert_eq!(m1.iter().filter(|t| *t == "fix").count(), fixes, "{m1:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The review insisted on a finding, the engineer asked the advisor and answered advisor_ruled, which changes nothing in
// the code: the review looks at the finding again, withdraws it by the ruling, and the work goes on.
#[test]
fn a_ruling_that_changes_no_code_sends_the_review_back_to_the_finding() {
    finding_is_cleared(
        "ruled-no-code",
        vec![
            json!([{ "fid": "R1", "verdict": "insist", "reason": "It is still wrong.", "basis": { "kind": "judgment" } }]),
            json!([{ "fid": "R1", "verdict": "withdraw" }]),
        ],
        vec![
            json!([{ "answer": "disagree", "id": "F1", "reason": "It is right as it is.", "basis": { "kind": "judgment" } }]),
            json!([{ "answer": "cannot_verify", "id": "F1", "reason": "Replaced below." }]),
        ],
        Some(
            json!([{ "answer": "advisor_ruled", "id": "F1", "ruling": "The advisor ruled that no change is needed." }]),
        ),
    );
}

// The review said the finding was not fixed, and the engineer answers fixed again with the head it had, no new commit:
// the review looks at it again, not the merge judgment, the fix and the choice of what runs next in turn.
#[test]
fn an_answer_fixed_again_with_no_new_commit_sends_the_review_back_to_the_finding() {
    finding_is_cleared(
        "fixed-again-no-commit",
        vec![
            json!([{ "fid": "R1", "verdict": "not_fixed", "reason": "It is not." }]),
            json!([{ "fid": "R1", "verdict": "closed" }]),
        ],
        vec![
            json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": "The pull request description." }]),
            json!([{ "answer": "fixed", "id": "F1", "commit": "HEAD" }]),
        ],
        None,
    );
}

// Whatever the fix answers to a row a side raised, and the code does not change, that side looks at it again: every
// answer the flow gives is tried, so an answer added later cannot be left out. The review looks at the rows it raised and
// adversarial testing, which has run in this work, stays skipped; adversarial testing looks at the rows it raised, and
// the review goes with it as it does when adversarial testing runs; a row another cell raised, as a red check of the full
// CI, sends neither anywhere.
#[test]
fn every_answer_to_a_row_a_side_raised_makes_that_side_look_again() {
    let s = setup("every-answer");
    let mut agents = scripts();
    agents.insert("T4", review_with_a_finding(Vec::new()));
    agents.insert(
        "T6",
        fix_answering(vec![
            json!([{ "answer": "cannot_verify", "id": "F1", "reason": "Replaced below." }]),
        ]),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the fix taking the finding", |e| {
        state(e, item).is_some_and(|(_, at, _)| at == "T6")
    });
    let (work, _, _) = state(&e, item).unwrap();
    // Adversarial testing has run on the change the review saw: a round of it, before the fix.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             SELECT ?1, (SELECT max(n) + 1 FROM rounds WHERE work = ?1), 'T1',
                    json_object('engine', json(json_extract(input, '$.engine'))), '{\"outcome\":{}}', 'done', 'c', 1, 2
             FROM rounds WHERE work = ?1 AND cell = 'T4' ORDER BY n DESC LIMIT 1",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    step_until(&mut e, item, &said, "the fix answering", |e| {
        transitions(e, item, "T6").iter().any(|t| t == "done")
    });
    let plan = |e: &mut Engine, rows: Value| -> (String, String) {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET output = json_set(output, '$.outcome.rows', json(?2)) WHERE work = ?1 AND cell = 'T6'",
                params![work, rows.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
        move_to(e, work, "T7");
        e.step_work(work).unwrap();
        e.db.conn()
            .query_row(
                "SELECT json_extract(output, '$.plan.review'), json_extract(output, '$.plan.adversarial') FROM rounds
                 WHERE work = ?1 AND cell = 'T7' ORDER BY n DESC LIMIT 1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    };
    let answers = flow::answers("T6", "rows").unwrap();
    assert!(answers.iter().any(|a| a == "fixed_outside_commit"), "{answers:?}");
    let raised_by = |e: &mut Engine, cell: &str| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.table[0].from', ?2)
                 WHERE work = ?1 AND cell = 'T6'",
                params![work, cell],
            )?;
            Ok(())
        })
        .unwrap();
    };
    let expect = |e: &mut Engine, review: &str, adversarial: &str| {
        for answer in &answers {
            let got = plan(e, json!([{ "answer": answer, "id": "F1" }]));
            assert_eq!(got, (review.to_string(), adversarial.to_string()), "{answer}");
        }
    };
    // The row is the review's finding.
    expect(&mut e, "disputes", "skipped");
    // The row is a test adversarial testing wrote that went wrong.
    raised_by(&mut e, "T1·new_tests");
    expect(&mut e, "changed", "disputes");
    // The row is a red check of the full CI.
    raised_by(&mut e, "T2·result[red]");
    expect(&mut e, "none", "skipped");
    // Adversarial testing, sent back to a row it raised, is given the fix's table with that row and the answer to it,
    // whatever the answer was: the row's number alone would say nothing of which test it is.
    raised_by(&mut e, "T1·new_tests");
    let where_it_is = "Only in the pull request description, pushed with ci.";
    let fixed = json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": where_it_is }]);
    assert_eq!(plan(&mut e, fixed).1, "disputes");
    e.step_work(work).unwrap();
    let m = message_at(&e, work, "T1", true);
    assert!(
        m.contains("It says too little.") && m.contains("fixed_outside_commit") && m.contains(where_it_is),
        "{m}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Records a round of `cell` that was given `input` and handed in `output`, and leaves the work at `at`, where the row
/// `transition` of the cell leads.
fn seed_round(e: &mut Engine, work: i64, cell: &str, input: &Value, output: &Value, transition: &str, at: &str) {
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'c', 1, 2)",
            params![
                work,
                next_round(tx, work)?,
                cell,
                input.to_string(),
                output.to_string(),
                transition
            ],
        )?;
        tx.execute("UPDATE works SET at = ?2 WHERE id = ?1", params![work, at])?;
        Ok(())
    })
    .unwrap();
}

/// A return of an agent's cell with `outcome`, which tells the owner nothing.
fn returned(outcome: Value) -> Value {
    json!({ "outcome": outcome, "progress": "p", "side_reports": [], "flow_feedback": "none", "tell_owner": [] })
}

/// The input of a round of the review that looked at the change as the blind review left it: the head the work stood
/// at when the blind review ended, as a review that sees it again while no commit follows would record, as when a fix
/// changes only the PR description.
fn input_of_the_change_reviewed(e: &Engine, work: i64) -> Value {
    let head: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(input, '$.engine.head_after') FROM rounds WHERE work = ?1 AND cell = 'T3'
             ORDER BY n DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    json!({ "engine": { "head": head, "head_after": head } })
}

/// What the latest round of choosing what runs next compared for each side that had answered, as its input records it.
fn compared(e: &Engine, work: i64) -> Value {
    let text: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT json_extract(input, '$.engine.compared') FROM rounds
                 WHERE work = ?1 AND cell = 'T7' ORDER BY n DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    serde_json::from_str(&text.expect("the choice recorded nothing it compared")).unwrap()
}

// A fix that answers a row the review left itself (a finding, a finding it did not accept as fixed, or one it insisted
// on) with no commit after it, as when it changes only the PR description or rests on the advisor's ruling, sends the
// work back to that review, whatever the answer: only the review can close the row. Without it, the merge judgment
// sends the work to a fix that has no row to answer, and the work goes round the merge judgment, the fix and the
// choice of what runs next with no end, as in the sixth trial. Each of the three ways back to the review is taken
// once, then the review closes its row and the work goes on.
#[test]
fn a_fix_that_answered_the_reviews_row_without_a_commit_goes_back_to_the_review() {
    let s = setup("fix-no-commit");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let reviewed = input_of_the_change_reviewed(&e, work);
    let log = || said.lock().unwrap().join("\n");
    // The review hands in its return and the merge judgment reads it.
    let review = |e: &mut Engine, outcome: Value| {
        seed_round(e, work, "T4", &reviewed, &returned(outcome), "done", "M1");
        e.step_work(work).unwrap();
    };
    // The fix answers the one row it was given, which came from `from`, and the work stands where the choice of what
    // runs next is made.
    let fix = |e: &mut Engine, row: Value, answer: Value| {
        let input = json!({ "engine": { "table": [row] } });
        let output = returned(json!({ "rows": [answer], "head": "abc" }));
        seed_round(e, work, "T6", &input, &output, "done", "T7");
    };
    // The choice of what runs next, and where the work goes after it.
    let choose = |e: &mut Engine| {
        e.step_work(work).unwrap();
        (planned(e, work), state(e, item).unwrap().1)
    };
    let disputes = (("disputes".to_string(), "skipped".to_string()), "T4".to_string());
    let finding = json!({ "id": "1", "verdict": "findings", "evidence": { "files": [], "ran": [], "nodes": [] },
        "findings": [{ "fid": "R1", "where": "PR description", "what": "It omits the ruling.", "fix": "Add it." }] });

    // A finding of a column, answered fixed: the PR description was changed.
    review(&mut e, json!({ "columns": [finding] }));
    assert_eq!(judged(&e, work), "fix", "{}", log());
    let row = json!({ "row": "F1", "from": "T4·columns[findings]", "column": "1", "fid": "R1",
                      "where": "PR description", "what": "It omits the ruling.", "fix": "Add it." });
    fix(&mut e, row, json!({ "id": "F1", "answer": "fixed", "commit": "abc" }));
    assert_eq!(choose(&mut e), disputes, "{}", log());
    // The review closes it: the merge judgment sends the work to no fix, and the same answer is not read again.
    review(&mut e, json!({ "previous": [{ "fid": "R1", "verdict": "closed" }] }));
    assert!(!judged(&e, work).starts_with("fix"), "{}\n{}", judged(&e, work), log());
    move_to(&mut e, work, "T7");
    assert_eq!(choose(&mut e).0.0, "none", "{}", log());

    // The finding not accepted as fixed, answered fixed again.
    review(
        &mut e,
        json!({ "previous": [{ "fid": "R1", "verdict": "not_fixed", "reason": "The ruling is still missing." }] }),
    );
    assert_eq!(judged(&e, work), "fix", "{}", log());
    let message = engine::next_message(&mut e, work).unwrap().text;
    assert!(message.contains("The ruling is still missing."), "{message}");
    let row = json!({ "row": "F1", "from": "T4·previous[not_fixed]", "fid": "R1", "verdict": "not_fixed",
                      "reason": "The ruling is still missing." });
    fix(&mut e, row, json!({ "id": "F1", "answer": "fixed", "commit": "abc" }));
    assert_eq!(choose(&mut e), disputes, "{}", log());
    review(&mut e, json!({ "previous": [{ "fid": "R1", "verdict": "closed" }] }));
    assert!(!judged(&e, work).starts_with("fix"), "{}\n{}", judged(&e, work), log());

    // The finding the review insisted on, answered advisor_ruled: the review withdraws it by the ruling.
    review(
        &mut e,
        json!({ "previous": [{ "fid": "R1", "verdict": "insist", "reason": "The ruling is still missing.",
                               "basis": { "kind": "read", "check": "the PR description" } }] }),
    );
    assert_eq!(judged(&e, work), "fix", "{}", log());
    let row = json!({ "row": "F1", "from": "T4·previous[insist]", "fid": "R1", "verdict": "insist",
                      "reason": "The ruling is still missing." });
    fix(
        &mut e,
        row,
        json!({ "id": "F1", "answer": "advisor_ruled", "ruling": "No change is needed." }),
    );
    assert_eq!(choose(&mut e), disputes, "{}", log());
    review(&mut e, json!({ "previous": [{ "fid": "R1", "verdict": "withdraw" }] }));
    assert!(!judged(&e, work).starts_with("fix"), "{}\n{}", judged(&e, work), log());
    let _ = std::fs::remove_dir_all(&s.root);
}

// A fix that answers a row another cell left (a red check of the CI, say) with no commit after it is no reason to
// review again: the review has no row of its own to close, and what the fix changed is what the checks look at.
#[test]
fn a_fix_that_answered_a_row_of_another_cell_without_a_commit_is_not_reviewed_again() {
    let s = setup("fix-no-commit-other");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let pass = json!({ "id": "1", "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } });
    let reviewed = input_of_the_change_reviewed(&e, work);
    seed_round(
        &mut e,
        work,
        "T4",
        &reviewed,
        &returned(json!({ "columns": [pass] })),
        "done",
        "M1",
    );
    let row = json!({ "row": "F1", "from": "T2·result[red]", "kind": "red", "name": "lint", "result": "red",
                      "run": "https://example.com/runs/7" });
    let input = json!({ "engine": { "table": [row] } });
    let output = returned(json!({ "rows": [{ "id": "F1", "answer": "fixed", "commit": "abc" }], "head": "abc" }));
    seed_round(&mut e, work, "T6", &input, &output, "done", "T7");
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(planned(&e, work), ("none".to_string(), "skipped".to_string()), "{log}");
    // Only the side that has answered has anything recorded to compare.
    let kept = compared(&e, work);
    assert_eq!(
        kept.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["review"],
        "{kept}"
    );
    assert_eq!(kept["review"]["own_lines"], "same", "{kept}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Records adversarial testing and the review as having answered at the head the work stands at in `wt`, as sides that
/// have looked at it, and gives that head.
fn both_sides_looked(e: &mut Engine, work: i64, wt: &Path) -> String {
    let head = git(wt, &["rev-parse", "HEAD"]);
    let left = json!({ "engine": { "head": head, "head_after": head } });
    let pass = json!({ "id": "1", "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } });
    seed_round(e, work, "T1", &left, &json!({ "outcome": {} }), "done", "T4");
    seed_round(
        e,
        work,
        "T4",
        &left,
        &returned(json!({ "columns": [pass] })),
        "done",
        "T7",
    );
    head
}

/// Chooses what runs next for the work, and gives the plan for the review and for adversarial testing.
fn choose_next(e: &mut Engine, work: i64) -> (String, String) {
    move_to(e, work, "T7");
    e.step_work(work).unwrap();
    planned(e, work)
}

/// The plan for the review and for adversarial testing, as [`planned`] gives it.
fn plan_of(review: &str, adversarial: &str) -> (String, String) {
    (review.to_string(), adversarial.to_string())
}

// After both sides have looked, a merge of main is no reason for either to look again when the lines the work itself
// adds and removes are as they were, though main changed the file the work changed too; it is when the conflict was
// resolved by writing another line of the work's own. The round records what it compared, for each side.
#[test]
fn a_merge_of_main_sends_the_checks_back_only_if_it_changed_the_works_own_lines() {
    let s = setup("merge-own-lines");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    let head = both_sides_looked(&mut e, work, &wt);
    // Main adds a line above the file the work wrote, which merges without a conflict.
    main_moves_on(&s.root, &[("src/lib.txt", "first\nold\n")], "Main adds a line above");
    merge_main(&wt);
    let merged = git(&wt, &["rev-parse", "HEAD"]);
    assert_ne!(merged, head);
    assert_eq!(
        choose_next(&mut e, work),
        plan_of("none", "skipped"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let same = json!({ "since": head, "now": merged, "own_lines": "same" });
    assert_eq!(compared(&e, work), json!({ "adversarial": same, "review": same }));

    // Main writes the line after the work's own; the conflict is resolved by writing a third.
    main_moves_on(
        &s.root,
        &[("src/lib.txt", "first\nold\nmain's feature\n")],
        "Main writes the line too",
    );
    git(&wt, &["fetch", "-q", "origin", "main"]);
    let merging = Command::new("git")
        .arg("-C")
        .arg(&wt)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "merge",
            "-q",
            "--no-edit",
            "origin/main",
        ])
        .output()
        .unwrap();
    assert!(!merging.status.success(), "the merge was expected to conflict");
    write(&wt, "src/lib.txt", "first\nold\nresolved\n");
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "Merge main"]);
    let resolved = git(&wt, &["rev-parse", "HEAD"]);
    assert_eq!(
        choose_next(&mut e, work),
        plan_of("changed", "changed"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let differs = json!({ "since": head, "now": resolved, "own_lines": "differs" });
    assert_eq!(compared(&e, work), json!({ "adversarial": differs, "review": differs }));
    let _ = std::fs::remove_dir_all(&s.root);
}

// Adversarial testing commits the tests it writes, so the head its round ended at holds them: a fix that adds no commit
// and brings in no main changes nothing since it looked. It is not sent back as changed; it is sent back to the rows it
// raised that the fix answered.
#[test]
fn what_adversarial_testing_committed_itself_is_not_a_change_since_it_looked() {
    let s = setup("own-commit");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    let began = git(&wt, &["rev-parse", "HEAD"]);
    write(&wt, "tests/adversarial.txt", "A test.\n");
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "Add a test of adversarial testing"]);
    let ended = git(&wt, &["rev-parse", "HEAD"]);
    // The round of adversarial testing began at one head and ended at the next; the review looked at the second.
    let adversarial = json!({ "engine": { "head": began, "head_after": ended } });
    seed_round(
        &mut e,
        work,
        "T1",
        &adversarial,
        &json!({ "outcome": {} }),
        "done",
        "T4",
    );
    let pass = json!({ "id": "1", "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } });
    let review = json!({ "engine": { "head": ended, "head_after": ended } });
    seed_round(
        &mut e,
        work,
        "T4",
        &review,
        &returned(json!({ "columns": [pass] })),
        "done",
        "T7",
    );
    let log = || said.lock().unwrap().join("\n");
    assert_eq!(choose_next(&mut e, work), plan_of("none", "skipped"), "{}", log());
    let same = json!({ "since": ended, "now": ended, "own_lines": "same" });
    assert_eq!(compared(&e, work), json!({ "adversarial": same, "review": same }));

    // The fix answers a row of a test adversarial testing wrote, in no commit.
    let row = json!({ "row": "F1", "from": "T1·new_tests", "name": "t" });
    let input = json!({ "engine": { "table": [row] } });
    let rows = json!([{ "id": "F1", "answer": "fixed_outside_commit", "where": "The PR description." }]);
    seed_round(
        &mut e,
        work,
        "T6",
        &input,
        &returned(json!({ "rows": rows, "head": ended })),
        "done",
        "T7",
    );
    assert_eq!(choose_next(&mut e, work), plan_of("changed", "disputes"), "{}", log());
    // The review is sent back because adversarial testing runs again, not because the work's own lines changed: the
    // record says what was compared, which is the same for both sides.
    assert_eq!(compared(&e, work), json!({ "adversarial": same, "review": same }));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A side whose head cannot be compared with the work as it is, because the repository has no such commit, the commit
// shares no history with main, or the round recorded none, is not taken to have seen the work as it is: both sides look
// again, and the round records why they could not be compared.
#[test]
fn a_side_whose_head_cannot_be_compared_looks_again() {
    let s = setup("cannot-compare");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    let head = both_sides_looked(&mut e, work, &wt);
    assert_eq!(
        choose_next(&mut e, work),
        plan_of("none", "skipped"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let tree = git(&wt, &["rev-parse", "HEAD^{tree}"]);
    let unrelated = git(&wt, &["commit-tree", &tree, "-m", "Shares no history with main"]);
    let missing = "0123456789abcdef0123456789abcdef01234567";
    let pass = json!({ "id": "1", "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } });
    for (case, since) in [
        ("a commit the repository does not have", Some(missing.to_string())),
        ("a commit that shares no history with main", Some(unrelated)),
        ("a round that recorded no head", None),
    ] {
        let left = match &since {
            Some(h) => json!({ "engine": { "head": h, "head_after": h } }),
            None => json!({ "engine": {} }),
        };
        seed_round(&mut e, work, "T1", &left, &json!({ "outcome": {} }), "done", "T4");
        seed_round(
            &mut e,
            work,
            "T4",
            &left,
            &returned(json!({ "columns": [pass.clone()] })),
            "done",
            "T7",
        );
        assert_eq!(
            choose_next(&mut e, work),
            plan_of("changed", "changed"),
            "{case}\n{}",
            said.lock().unwrap().join("\n")
        );
        let kept = compared(&e, work);
        for side in ["adversarial", "review"] {
            assert_eq!(kept[side]["since"], json!(since), "{case}: {kept}");
            assert_eq!(kept[side]["now"], head, "{case}: {kept}");
            let verdict = kept[side]["own_lines"].as_str().unwrap();
            assert!(verdict.starts_with("cannot_compare: "), "{case}: {kept}");
        }
    }
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
    step_to(&mut e, item, "T7");
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
    assert!(ended_at(&e, item, "D1").len() == 1, "no new design\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work at the fix whose table to answer has no row runs no round there, which could answer nothing: the engine takes
// the fix's row for it, recorded with why, on to choose what runs next. In the sixth trial such a work went from the
// fix to choosing the way on, whose carry on led back to the fix, round after round.
#[test]
fn a_fix_with_nothing_to_answer_goes_on_without_a_round() {
    let s = setup("nothing-to-answer");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{}", said.lock().unwrap().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET at = 'T6' WHERE id = ?1", [work])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(e.advance(work).unwrap(), Advance::Moved);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|x| x.1).as_deref(), Some("T7"), "{log}");
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

/// The name of the latest round of the work at a cell, `R<work>-<n>`.
fn latest_round(e: &Engine, work: i64, cell: &str) -> String {
    let n: i64 =
        e.db.conn()
            .query_row(
                "SELECT max(n) FROM rounds WHERE work = ?1 AND cell = ?2",
                params![work, cell],
                |r| r.get(0),
            )
            .unwrap();
    format!("R{work}-{n}")
}

/// A commit in a worktree that writes one file, and its id.
fn commit_file(wt: &Path, file: &str, text: &str, message: &str) -> String {
    write(wt, file, text);
    git(wt, &["add", "-A"]);
    git(wt, &["commit", "-q", "-m", message]);
    git(wt, &["rev-parse", "HEAD"])
}

// Item #16, acceptance A1 to A6 through the engine: the blind review is told whether the branch took main in (an empty
// list when it never did) and given the CI on the work, with the plain statement that it has `gh` alone; a re-review is
// told the merges of main since the head its last review looked at; adversarial testing's re-check is told the commits
// since the head its last round ended at, a merge among them marked, the files the work itself changed in them, and the
// rounds of the fix in between, none at its first round; and the table review has a column for each test and workaround
// of the implementation, of the fixes and of adversarial testing, a test once however many returns name it.
#[test]
fn the_reviews_and_a_re_check_are_told_what_the_branch_took_in_and_did_since() {
    let s = setup("branch-since");
    let (mut e, _said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);

    // The blind review: the branch never took main in, said as an empty list; it is given the CI on the work and told
    // its only tool is `gh`, so that it does not look for the CI in the database.
    let blind = message_at(&e, work, "T3", false);
    let base = given_value(&blind, "·engine·review_base").expect(&blind);
    assert_eq!(base["main_merges"], json!([]), "{base}");
    assert!(base.get("last_reviewed").is_none(), "{base}");
    let ci = given_value(&blind, "·engine·work_ci").expect(&blind);
    assert!(ci.is_object() && ci.get("fast_ci").is_some(), "{ci}");
    assert!(blind.contains("Your only tool is `gh`, with no `query`"), "{blind}");

    // A table review was given the change at the head the work has now; the branch took main in since: no merge yet.
    let head0 = git(&wt, &["rev-parse", "HEAD"]);
    let reviewed = json!({ "engine": { "head": head0, "columns": [["1", "one"], ["2", "two"]] } });
    seed_round(
        &mut e,
        work,
        "T4",
        &reviewed,
        &returned(json!({ "columns": [] })),
        "done",
        "T4",
    );
    let review_base = |e: &mut Engine| -> Value {
        let m = engine::next_message(e, work).unwrap().text;
        given_value(&m, "·engine·review_base").expect(&m)
    };
    let before = review_base(&mut e);
    assert_eq!(before["last_reviewed"], json!(head0), "{before}");
    assert_eq!(before["main_merges"], json!([]), "{before}");
    assert_eq!(before["merges_since_last_review"], json!([]), "{before}");

    // The work's own commit, then main's change taken in by a merge, then the work's second commit.
    let own = commit_file(&wt, "src/own.txt", "own\n", "The work's own change");
    main_moves_on(&s.root, &[("src/main-only.txt", "main\n")], "Main's change");
    merge_main(&wt);
    let merge = git(&wt, &["rev-parse", "HEAD"]);
    let second = commit_file(&wt, "src/own2.txt", "own two\n", "The work's second change");
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET pushed_head = ?2 WHERE id = ?1", params![work, second])?;
        Ok(())
    })
    .unwrap();
    let after = review_base(&mut e);
    let commits_of = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|m| m["commit"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(after["head"], json!(second), "{after}");
    assert_eq!(
        commits_of(&after["main_merges"]),
        std::slice::from_ref(&merge),
        "{after}"
    );
    assert_eq!(
        commits_of(&after["merges_since_last_review"]),
        std::slice::from_ref(&merge),
        "{after}"
    );

    // Adversarial testing's first round is told nothing of a last one.
    let plan = json!({ "plan": { "review": "changed", "adversarial": "changed" } });
    seed_round(&mut e, work, "T7", &json!({}), &plan, "adversarial_changed", "T1");
    let first = engine::next_message(&mut e, work).unwrap().text;
    assert!(given_value(&first, "·engine·since_adversarial").is_none(), "{first}");

    // A round that wrote no test ended at the head it began at; a fix answered after it. The re-check is told every
    // commit since, the merge marked, the files the work itself changed without what main brought, and the fix's round.
    let none = returned(
        json!({ "wrote": { "verdict": "none" }, "directions": ["d"], "new_tests": [],
                                 "guard_tests": [], "conditions": [] }),
    );
    seed_round(
        &mut e,
        work,
        "T1",
        &json!({ "engine": { "head": head0 } }),
        &none,
        "for_engineer",
        "T6",
    );
    let fix = returned(json!({
        "rows": [], "head": second,
        "deviations": [
            { "kind": "workaround", "what": "Used the fix's way.", "why": "It was needed." },
            { "kind": "surprise", "what": "A small thing." }
        ],
        "test_changes": [
            { "change": "added", "test": "fix_test", "catches": "c", "kind": "new_behaviour", "time": "1s", "worth": "w" },
            { "change": "relaxed", "test": "old_test", "reason": "A technical one." }
        ]
    }));
    seed_round(&mut e, work, "T6", &json!({}), &fix, "done", "T7");
    let fixed_in = latest_round(&e, work, "T6");
    seed_round(&mut e, work, "T7", &json!({}), &plan, "adversarial_changed", "T1");
    let m = engine::next_message(&mut e, work).unwrap().text;
    let since = given_value(&m, "·engine·since_adversarial").expect(&m);
    assert_eq!(since["since"], json!(head0), "{since}");
    let listed: Vec<(String, bool)> = since["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["commit"].as_str().unwrap().to_string(), c["merge"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        listed,
        [(second.clone(), false), (merge.clone(), true), (own.clone(), false)],
        "{since}"
    );
    assert_eq!(since["own_files"], json!(["src/own.txt", "src/own2.txt"]), "{since}");
    assert_eq!(since["fix_rounds"], json!([fixed_in]), "{since}");

    // A round that wrote tests ended at the last commit it wrote them in: the commits after that one only, and no fix
    // since.
    let wrote = returned(json!({
        "wrote": { "verdict": "wrote", "head": own, "tests": [
            { "name": "adv_test", "checks": "c", "oracle": "o", "time": "1s", "worth": "w" },
            { "name": "fix_test", "checks": "c", "oracle": "o", "time": "1s", "worth": "w" }
        ] },
        "directions": ["d"], "new_tests": [], "guard_tests": [], "conditions": []
    }));
    seed_round(
        &mut e,
        work,
        "T1",
        &json!({ "engine": { "head": head0 } }),
        &wrote,
        "for_engineer",
        "T7",
    );
    seed_round(&mut e, work, "T7", &json!({}), &plan, "adversarial_changed", "T1");
    let m = engine::next_message(&mut e, work).unwrap().text;
    let since = given_value(&m, "·engine·since_adversarial").expect(&m);
    assert_eq!(since["since"], json!(own), "{since}");
    assert_eq!(since["commits"].as_array().unwrap().len(), 2, "{since}");
    assert_eq!(since["own_files"], json!(["src/own2.txt"]), "{since}");
    assert_eq!(since["fix_rounds"], json!([]), "{since}");

    // The table review has a column for each test and workaround, whoever reported it, a test named twice once, and
    // the tests adversarial testing wrote say so.
    move_to(&mut e, work, "T4");
    let table = engine::next_message(&mut e, work).unwrap().text;
    let count = |needle: &str| table.matches(needle).count();
    assert_eq!(count("The new test fix_test: take the other side"), 1, "{table}");
    assert_eq!(
        count("The new test adv_test, written by adversarial testing: take the other side"),
        1,
        "{table}"
    );
    assert_eq!(count("fix_test, written by adversarial testing"), 0, "{table}");
    assert_eq!(
        count("The workaround \"Used the fix's way.\": does its reason hold, and did the design change with it?"),
        1,
        "{table}"
    );
    assert_eq!(count("The surprise \"A small thing.\""), 0, "{table}");
    assert_eq!(
        count("The test old_test, relaxed: compared with before, is there a technical reason?"),
        1,
        "{table}"
    );
    // Appended in the order the returns came: the fix's test before the one adversarial testing wrote.
    assert!(
        table.find("The new test fix_test").unwrap() < table.find("The new test adv_test").unwrap(),
        "{table}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #16, acceptance A9, A10 and A11 through the engine: a condition the design says is met by the CI or by reading
// the change, and one that says tests but is met by a job of the CI, are answered by rows of kind ci_job and read_diff.
// The first return names a job no CI run of the work has and a command in no file of the branch: it is sent back once,
// saying both. The second names the job the CI results give and a command the branch has, and is taken; the table review
// is given the rows as they were handed in, and told how to judge them.
#[test]
fn conditions_met_by_a_ci_job_or_a_reading_of_the_change_are_taken_and_a_job_or_command_not_there_is_sent_back() {
    let s = setup("ci-job-conditions");
    let mut agents = scripts();
    agents.insert(
        "D2",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.measure", "not_measured")],
                vec![(
                    "outcome.acceptance",
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
    // The work comes to the table review, which then runs its first round.
    let at_review = steps_until(&mut e, 80, |e| state(e, item).is_some_and(|(_, at, _)| at == "T4"));
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
    // The table review is given the rows as the second return handed them in, and how to judge a CI job and a reading.
    let review = message_at(&e, work, "T4", false);
    let rows = given_value(&review, "·I1·conditions").expect(&review);
    assert_eq!(rows[0]["how"], "ci_job", "{rows}");
    assert_eq!(rows[0]["jobs"], json!(["check"]), "{rows}");
    assert_eq!(rows[2]["how"], "read_diff", "{rows}");
    assert_eq!(rows[2]["where"], "src/lib.txt", "{rows}");
    assert!(
        review.contains("If the table meets it with a CI job (`ci_job`), read that job's result on this head, and check that each command in its `steps` runs in the job it names"),
        "{review}"
    );
    assert!(
        review.contains("For either, if a test could prove the condition instead, that is a finding."),
        "{review}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
