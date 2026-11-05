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
        "R0",
        Box::new(|_: &Request, n: usize| {
            let reports = if n == 1 {
                json!([{ "kind": "improvement", "what": "There is no `jq` on the machine.", "harm_now": false }])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    agents.insert(
        "H1",
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
        "About your side report at cell R0 (\"There is no `jq` on the machine\"): recorded as observation O{id} \
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
        given.contains("From the architect: About your side report at cell R0"),
        "{given}"
    );
    // A word that needs no answer turns nothing: the work is not taken to choose its way on.
    assert!(!path_of(&e, work).contains(&"A4".to_string()), "{log}");
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
        "R0",
        reporting(json!({ "kind": "out_of_scope_defect", "what": "The lexer drops a token.", "harm_now": true })),
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
    assert_eq!(started.cell.as_deref(), Some("Q0"), "{log}");
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert!(path_of(&e, arch).contains(&"H1".to_string()), "{log}");
    assert_eq!(matters(&e), [("report".to_string(), false, true)], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What a round says was wrong with its cell is flow feedback: it stays in the round's return, for the owner and the
// assistant to go through later, and comes to no architect: it is no matter of any queue and starts no work of an
// architect.
#[test]
fn flow_feedback_stays_in_its_round_and_comes_to_no_architect() {
    let s = setup("flow-feedback");
    let mut agents = scripts();
    let saying = |text: &'static str| -> Script {
        Box::new(move |_: &Request, n: usize| {
            let said = if n == 1 { json!(text) } else { json!("none") };
            (Vec::new(), vec![("flow_feedback", said)])
        })
    };
    agents.insert("R0", saying("The problem and the why say the same."));
    agents.insert("R1", saying("No outcome fits a report."));
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..4 {
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "R1").len(), 1, "{log}");
    assert!(architect_works(&e).is_empty(), "{log}");
    assert!(matters(&e).is_empty(), "{log}");
    let kept: Vec<(String, String)> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT cell, output ->> '$.flow_feedback' FROM rounds
                 WHERE cell IN ('R0', 'R1') AND output ->> '$.flow_feedback' <> 'none' ORDER BY work, n",
                )
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(
        kept,
        [
            ("R0".to_string(), "The problem and the why say the same.".to_string()),
            ("R1".to_string(), "No outcome fits a report.".to_string())
        ],
        "{log}"
    );
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
        "R0",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("flow_feedback", json!("Said anyway."))])),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let subject = e.subject(&w).unwrap();
    let r0 = g.cell("R0").unwrap();
    assert!(r0.schema.as_ref().unwrap()["properties"].get("flow_feedback").is_none());
    let req = engine::cell_request(&e, &w, &g, r0, subject.as_ref()).unwrap();
    assert!(
        req.envelope["properties"].get("flow_feedback").is_none(),
        "{}",
        req.envelope
    );
    e.step(Some(work)).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "R0"), [(Some("done".to_string()), None)], "{log}");
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
    let arch = architect_work_at(&mut e, "H1");
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
        "R0",
        Box::new(|_: &Request, n: usize| {
            let r = |what: &str| json!({ "kind": "improvement", "what": what, "harm_now": false });
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
