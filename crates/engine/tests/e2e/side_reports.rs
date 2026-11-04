//! Side reports and flow feedback, and what the work that handed them in is told.

use super::*;

// A side report that the system lacks something is taken in, and the work that handed it in is told what became of it
// in one line to its engineer: in the work's group, as the architect's word to the engineer, and given to the
// engineer's next round, which needs to answer nothing.
#[test]
fn the_work_that_handed_a_side_report_in_is_told_what_became_of_it() {
    let s = setup("side-report-answered");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let reports = if n == 1 {
                json!([{ "kind": "improvement", "what": "There is no `jq` on the machine.", "urgent": false }])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    agents.insert(
        "H5",
        Box::new(|_: &Request, _: usize| {
            let new = json!({ "kind": "new", "title": "No jq on the machine", "what": "The machine has no jq.",
                              "tags": [], "source": "A side report.", "refs": [], "reason": "Worth keeping." });
            (
                Vec::new(),
                vec![
                    (
                        "outcome.take",
                        json!({ "way": "new_observation", "why": "No item does it." }),
                    ),
                    ("outcome.effects", json!({ "observation_changes": [new] })),
                ],
            )
        }),
    );
    let (mut e, said, _) = start(&s, agents);
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let work = work.unwrap();
    // The work stays where it is while the architect takes the report in.
    pause_by_owner(&mut e, work, "Held while the architect looks.");
    let arch = e.step(Some(work)).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let id: i64 =
        e.db.conn()
            .query_row("SELECT id FROM observations", [], |r| r.get(0))
            .unwrap();
    let line = format!(
        "About your side report at cell R1 (\"There is no `jq` on the machine\"): recorded as observation O{id} \
         \"No jq on the machine\". You need not report it again."
    );
    assert_eq!(lines_told(&e, work), std::slice::from_ref(&line), "{log}");
    let group: Vec<(String, String, String)> = engine::query_json(
        e.db.conn(),
        "SELECT sender, recipient, text FROM conversation_entries WHERE conversation = 'work:' || ?1
           AND source = 'interjection'",
        &[&work],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| {
        let t = |k: &str| r[k].as_str().unwrap().to_string();
        (t("sender"), t("recipient"), t("text"))
    })
    .collect();
    assert_eq!(group, [("architect".into(), "engineer".into(), line)], "{log}");
    let_go_on(&mut e, work);
    let delivered = |e: &Engine| -> Option<i64> {
        e.db.conn()
            .query_row(
                "SELECT delivered_round FROM interjections WHERE work = ?1 AND speaker = 'architect'",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert!(steps_until(&mut e, 20, |e| delivered(e).is_some()), "{log}");
    let input: String =
        e.db.conn()
            .query_row(
                "SELECT input FROM rounds WHERE work = ?1 AND n = ?2",
                params![work, delivered(&e).unwrap()],
                |r| r.get(0),
            )
            .unwrap();
    let given = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap()).to_string();
    assert!(
        given.contains("From the architect: About your side report at cell R1"),
        "{given}"
    );
    // A word that needs no answer turns nothing: the work is not taken to choose its way on.
    assert!(!path_of(&e, work).contains(&"A4".to_string()), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A gap reported a third time is work: a merge that makes an observation no item took in yet reported three times is
// sent back while no change of the batch folds it into an item, and again while that item is not moved ahead; handed in
// folded into a new item moved first, it is taken, and once the batch is made the observation is the item's, the item
// starts first, and the work that reported it is told it was reported again and taken into that item.
#[test]
fn a_gap_reported_a_third_time_is_taken_into_an_item_moved_ahead() {
    let s = setup("gap-recurs");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let reports = if n == 1 {
                json!([{ "kind": "improvement", "what": "There is no `jq` on the machine.", "urgent": false }])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    agents.insert(
        "H5",
        Box::new(|_: &Request, n: usize| {
            let merge = json!({ "kind": "merge", "into": 1, "seen": "", "refs": [], "reason": "The same gap." });
            let mut changes = vec![
                json!({ "kind": "create_item", "key": "jq", "project": "p", "title": "Install jq",
                        "problem": "There is no jq on the machine.", "why": "Works keep meeting it.",
                        "acceptance": ["jq runs."], "level": "l", "deliver": "change", "reason": "It recurs." }),
                json!({ "kind": "fold_observations", "item": "jq", "observations": [1], "reason": "It is the gap." }),
            ];
            if n > 2 {
                changes
                    .push(json!({ "kind": "move", "item": "jq", "to": { "place": "first" }, "reason": "It recurs." }));
            }
            let effects = if n == 1 {
                json!({ "observation_changes": [merge] })
            } else {
                json!({ "observation_changes": [merge], "item_changes": changes })
            };
            (
                Vec::new(),
                vec![
                    ("outcome.take", json!({ "way": "merged", "why": "Reported before." })),
                    ("outcome.effects", effects),
                ],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let ok = |n: i64| json!({ "change": n, "verdict": "ok", "why": "Sound." });
            (Vec::new(), vec![("outcome.review", json!([ok(1), ok(2), ok(3)]))])
        }),
    );
    let (mut e, said, _) = start(&s, agents);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO observations (project, title, what, source, times_reported)
             VALUES ('p', 'No jq', 'The machine has no jq.', 'Two works.', 2)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let work = work.unwrap();
    pause_by_owner(&mut e, work, "Held while the architect looks.");
    let arch = e.step(Some(work)).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H5' AND rejected IS NOT NULL ORDER BY n",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 2, "{rejected:?}\n{log}");
    assert!(
        rejected[0].contains("it makes observation 1 reported 3 times, and an observation reported 3 times is work"),
        "{rejected:?}"
    );
    assert!(
        rejected[1].contains("item jq, which takes it in, is not moved ahead"),
        "{rejected:?}"
    );
    let fix: i64 =
        e.db.conn()
            .query_row("SELECT id FROM items WHERE title = 'Install jq'", [], |r| r.get(0))
            .unwrap();
    let (times, folded): (i64, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT times_reported, folded_into FROM observations WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((times, folded), (3, Some(fix)), "{log}");
    let rank: Option<i64> =
        e.db.conn()
            .query_row("SELECT rank FROM items WHERE id = ?1", [fix], |r| r.get(0))
            .unwrap();
    assert_eq!(rank, Some(1), "{log}");
    assert_eq!(
        lines_told(&e, work),
        [format!(
            "About your side report at cell R1 (\"There is no `jq` on the machine\"): reported once more on \
             observation O1 \"No jq\", 3 times now; taken into new item #{fix} \"Install jq\". You need not report \
             it again."
        )],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each side report is a matter of the kind report in its project's architect's queue, queued once, which starts a work
// of the architect; the queue marks one urgent.
#[test]
fn side_reports_come_to_the_architect_as_reports() {
    let s = setup("side-reports");
    let mut agents = scripts();
    let reporting = |r: Value| -> Script {
        Box::new(move |_: &Request, n: usize| {
            let reports = if n == 1 { json!([r.clone()]) } else { json!([]) };
            (Vec::new(), vec![("side_reports", reports)])
        })
    };
    agents.insert(
        "R1",
        reporting(json!({ "kind": "out_of_scope_defect", "what": "The lexer drops a token.", "urgent": true })),
    );
    let (mut e, said, _) = start(&s, agents);
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(matters(&e), [("report".to_string(), true, false)], "{log}");
    let (detail, urgent): (String, i64) =
        e.db.conn()
            .query_row("SELECT detail, urgent FROM architect_queue", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!((detail.as_str(), urgent), ("out_of_scope_defect", 1));
    // A work of the architect starts for the report, its first step beside the work's own.
    let pass = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let started = pass.jobs.iter().find(|s| s.work != work).expect(&log);
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert!(path_of(&e, arch).contains(&"H4".to_string()), "{log}");
    assert_eq!(matters(&e), [("report".to_string(), false, true)], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What a round says was wrong with its cell is flow feedback: it joins its project's architect's flow feedback, one
// matter of the kind report that holds it by graph and cell, and starts no work of an architect; none adds nothing.
#[test]
fn flow_feedback_waits_for_the_architect_by_cell_and_starts_no_work_of_it() {
    let s = setup("flow-feedback");
    let mut agents = scripts();
    let saying = |text: &'static str| -> Script {
        Box::new(move |_: &Request, n: usize| {
            let said = if n == 1 { json!(text) } else { json!("none") };
            (Vec::new(), vec![("flow_feedback", said)])
        })
    };
    agents.insert("R1", saying("The problem and the why say the same."));
    agents.insert("R2", saying("No outcome fits a report."));
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..4 {
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "R2").len(), 1, "{log}");
    assert!(architect_works(&e).is_empty(), "{log}");
    assert_eq!(matters(&e), [("report".to_string(), true, false)], "{log}");
    let (detail, what, body): (String, String, String) =
        e.db.conn()
            .query_row(
                "SELECT detail, what, body FROM architect_matters WHERE kind = 'report'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(detail, "flow_feedback");
    assert_eq!(what, "Flow feedback of 2 rounds, at cells R1 (engineer), R2 (engineer)");
    let body: Value = serde_json::from_str(&body).unwrap();
    let by_cell = &body["feedback"]["engineer"];
    assert_eq!(by_cell.as_object().unwrap().len(), 2, "{body}");
    assert_eq!(by_cell["R1"][0]["said"], "The problem and the why say the same.");
    assert_eq!(by_cell["R1"][0]["item"], item);
    assert_eq!(by_cell["R2"][0]["said"], "No outcome fits a report.");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Flow feedback one project's architect forwards to another's is marked as flow feedback: it waits in the other's queue
// and starts no work of an architect, as the project's own does.
#[test]
fn flow_feedback_forwarded_to_another_architect_starts_no_work_of_it() {
    let s = setup("forwarded-feedback");
    let q = s.root.join("q-checkout");
    git(
        &s.root,
        &["clone", "-q", &s.origin.to_string_lossy(), &q.to_string_lossy()],
    );
    let p_file = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    let checkout = s.root.join("checkout");
    write(
        &s.data,
        "projects/q.toml",
        &p_file
            .replace("o/p", "o/q")
            .replace(&checkout.display().to_string(), &q.display().to_string()),
    );
    git(&s.data, &["add", "-A"]);
    git(&s.data, &["commit", "-q", "-m", "a second project"]);
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "H5",
        Box::new(|r: &Request, _: usize| {
            if project_of(r) != "p" {
                return (Vec::new(), Vec::new());
            }
            let message = json!({ "project": "q", "text": "Cell R1's problem and why say the same." });
            (
                vec![("outcome.take.way", "forwarded")],
                vec![("outcome.effects", json!({ "messages": [message] }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let feedback =
        json!({ "feedback": { "engineer": { "R1": [{ "said": "The problem and the why say the same." }] } } });
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO dispatch_holds (project, reason, since) VALUES ('q', 'architect', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO architect_matters (project, kind, detail, what, body, at)
             VALUES ('p', 'report', 'flow_feedback', 'Flow feedback of 1 round, at cell R1 (engineer)', ?1, 1)",
            [feedback.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look at the flow feedback too.");
    let p_done = |e: &mut Engine| match architect_works(e).first().copied() {
        Some(w) => architect_work_done(e, w),
        None => false,
    };
    assert!(steps_until(&mut e, 80, p_done), "{}", said.lock().unwrap().join("\n"));
    for _ in 0..5 {
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let started: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT project FROM works WHERE graph = 'architect' AND ended IS NOT 'over before the test'
                     ORDER BY id",
                )
                .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(started, ["p"], "{log}");
    let (detail, from): (String, String) = e
        .db
        .conn()
        .query_row(
            "SELECT detail, body ->> 'from' FROM architect_matters WHERE kind = 'other_architect' AND project = 'q'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((detail.as_str(), from.as_str()), ("flow_feedback", "p"), "{log}");
    let waiting: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM architect_queue WHERE project = 'q'", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(waiting, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A project whose configuration turns flow feedback off is not asked for it: neither the cell's schema nor the command
// line's holds the field, and nothing comes to the architect.
#[test]
fn a_project_with_flow_feedback_off_is_not_asked_for_it() {
    let s = setup("flow-feedback-off");
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &project.replace("[ci]", "flow_feedback = false\n[ci]"),
    );
    git(&s.data, &["commit", "-q", "-am", "no flow feedback"]);
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("flow_feedback", json!("Said anyway."))])),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let subject = e.subject(&w).unwrap();
    let r1 = g.cell("R1").unwrap();
    assert!(r1.schema.as_ref().unwrap()["properties"].get("flow_feedback").is_none());
    let req = engine::cell_request(&e, &w, &g, r1, subject.as_ref()).unwrap();
    assert!(
        req.envelope["properties"].get("flow_feedback").is_none(),
        "{}",
        req.envelope
    );
    e.step(Some(work)).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "R1"), [(Some("done".to_string()), None)], "{log}");
    assert!(matters(&e).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The side reports handed in before the architect ran, which a migration put in one matter, are given by work and
// cell, each text once, its spaces collapsed, with how many times and in which rounds it was handed in, and how the
// work it came from stands; a report that holds nothing, such as the text of an empty list, is left out. In the sixth
// trial the matter gave 65 reports from five works as one list, most handed in again round after round.
#[test]
fn the_side_reports_handed_in_before_are_given_once_by_work_and_cell() {
    let s = setup("reports-once");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let arch = architect_work_at(&mut e, "H4");
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
            params![item, v],
        )?;
        let work = tx.last_insert_rowid();
        let report = |round: i64, cell: &str, what: &str| {
            json!({ "work": work, "round": round, "cell": cell, "index": 0,
                    "report": { "kind": "other", "what": what, "urgent": false } })
        };
        let reports = json!([
            report(2, "R1", "The lexer drops a token."),
            report(3, "R1", " The lexer  drops a token.\n"),
            { "work": work, "round": 4, "cell": "R1", "index": null, "report": "[]" },
            report(5, "D1", "The lexer drops a token."),
        ]);
        let m = db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "report".into(),
                detail: Some("other".into()),
                what: "Look through the 4 side reports handed in before you ran".into(),
                body: json!({ "side_reports": reports }),
                ..db::Matter::default()
            },
        )?;
        record_round(
            tx,
            arch,
            1,
            "Q0",
            &json!({ "queue": { "verdict": "one", "matter": m } }),
            "one",
        )
    })
    .unwrap();
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let given = given_value(&m, "·engine·matter").expect(&m);
    assert_eq!(given["matter"]["body"].get("side_reports"), None, "{given}");
    let groups = given["material"]["side_reports"].as_array().expect(&m).clone();
    let cells: Vec<&str> = groups.iter().map(|g| g["cell"].as_str().unwrap()).collect();
    assert_eq!(cells, ["R1", "D1"], "{given}");
    assert_eq!(groups[0]["work_now"], "it ended: stopped: by hand", "{given}");
    assert_eq!(
        groups[0]["reports"],
        json!([{ "kind": "other", "urgent": false, "what": "The lexer drops a token.", "times": 2, "rounds": [2, 3] }]),
        "{given}"
    );
    assert_eq!(groups[1]["reports"][0]["rounds"], json!([5]), "{given}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A side report a work handed in before, word for word, is not queued again.
#[test]
fn a_side_report_handed_in_again_is_queued_once() {
    let s = setup("report-again");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let r = |what: &str| json!({ "kind": "improvement", "what": what, "urgent": false });
            let reports = if n == 1 {
                json!([
                    r("The parser is slow."),
                    r(" The parser is slow.\n"),
                    r("It needs a machine.")
                ])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    let (mut e, said, _) = start(&s, agents);
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        matters(&e),
        [("report".to_string(), true, false), ("report".to_string(), true, false)],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
