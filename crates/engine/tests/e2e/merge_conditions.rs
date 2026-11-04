//! What the owner set to be done before an item's work merges: the merge gate holds the work and asks the owner of each
//! condition still open, and only the owner's own choice of an option judges one; words, and a delegate's answer, go to
//! the engineer, and the gate asks again with what it reports next. The implementation and the fix report on each they
//! are given.

use super::*;

mod edges;

/// A merge condition on the item, recorded as the owner's tick does, resting on the owner's words in a matter that is
/// kept out of the architect's queue; gives its number.
fn condition_on(db: &mut db::Db, item: i64, what: &str, quote: &str) -> i64 {
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO architect_matters (project, kind, detail, what, body, item, at, withdrawn)
             VALUES ('p', 'owner_words', 'said', 'w', ?1, ?2, 1, 'kept out of the queue')",
            params![
                json!({ "text": quote, "where": "item", "speaker": "owner" }).to_string(),
                item
            ],
        )?;
        let matter = tx.last_insert_rowid();
        db::record_merge_condition(
            tx,
            &db::MergeCondition {
                item,
                what,
                source: "matter",
                source_id: matter,
                quote,
                said_by: "owner",
                recorded_by: "owner",
                round: None,
            },
        )
    })
    .unwrap()
}

/// The numbers of the conditions still open, read from the database at `path`.
fn open_numbers(path: &Path) -> Vec<String> {
    let c = db::open_read(path).unwrap();
    let mut st = c
        .prepare("SELECT 'C' || id FROM item_merge_conditions WHERE status = 'open' ORDER BY id")
        .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

/// Has the implementation and the fix report each condition still open as done, in words that say which cell did, as
/// their return does once it hands in its answer.
fn reporting_each_condition(agents: &mut BTreeMap<&'static str, Script>, path: &Path) {
    for cell in ["I1", "T6"] {
        let base = agents
            .remove(cell)
            .unwrap_or_else(|| Box::new(|_: &Request, _: usize| (Vec::new(), Vec::new())));
        let path = path.to_path_buf();
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                let (pick, mut set) = base(r, n);
                if pick.iter().any(|(p, v)| *p == "outcome" && *v == "tools") {
                    return (pick, set);
                }
                let rows: Vec<Value> = open_numbers(&path)
                    .iter()
                    .map(|id| json!({ "answer": "done", "id": id, "what": format!("Done in {cell}.") }))
                    .collect();
                if !rows.is_empty() {
                    set.push(("outcome.owner_conditions", Value::Array(rows)));
                }
                (pick, set)
            }),
        );
    }
}

/// An answer to the gate's ask of a merge condition.
fn answers(e: &mut Engine, ask: i64, speaker: &str, choice: Option<&str>, text: &str) {
    let answer = api::Answer {
        text: text.into(),
        speaker: speaker.into(),
        relayed: None,
        why: None,
        reply: None,
        to: None,
        choice: choice.map(str::to_string),
        condition: None,
        images: Vec::new(),
    };
    api::answer(&mut e.db, ask, &answer).unwrap();
}

/// Where a condition stands, as the view works it out.
fn status(e: &Engine, condition: i64) -> String {
    e.db.conn()
        .query_row(
            "SELECT status FROM item_merge_conditions WHERE id = ?1",
            [condition],
            |r| r.get(0),
        )
        .unwrap()
}

/// The words of what the ask gives the owner to judge with, as the ask's points hold them.
fn points(body: &Value) -> &str {
    body["asked"][0]["value"]["points"][0]["what"]
        .as_str()
        .unwrap_or_default()
}

/// Steps the engine until the item's work stands at the gate with an ask other than `not` open; gives the work, the ask
/// and what it holds.
fn at_the_gate_asking(
    e: &mut Engine,
    item: i64,
    said: &Arc<Mutex<Vec<String>>>,
    not: Option<i64>,
) -> (i64, i64, Value) {
    step_until(e, item, said, "the gate's ask", |e| {
        state(e, item).is_some_and(|(w, at, _)| at == "M1" && open_ask(e, w).is_some_and(|(a, _)| Some(a) != not))
    });
    let (work, _, _) = state(e, item).unwrap();
    let (ask, body) = open_ask(e, work).unwrap();
    (work, ask, body)
}

// Two conditions on an item: the work stops at the merge gate, not going on to the merge queue, on a blocker the owner
// holds that the ask stands for and that names the condition; the gate asks of one at a time, oldest first. The owner's
// own choice judges each, met and withdrawn alike, which the view says, and the work merges with the last.
#[test]
fn the_gate_holds_the_work_until_the_owner_judges_each_condition_one_at_a_time() {
    let s = setup("condition-judged");
    let mut agents = scripts();
    reporting_each_condition(&mut agents, &config::database(&s.data));
    let (mut e, said, item) = start(&s, agents);
    let first = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");
    let second = condition_on(&mut e.db, item, "Show me the output first.", "Show me.");

    let (work, ask, body) = at_the_gate_asking(&mut e, item, &said, None);
    assert_eq!(body["condition"], json!(first), "{body}");
    assert!(points(&body).contains("Done in I1."), "{body}");
    assert_eq!(
        body["asked"][0]["value"]["choice"]["options"][1]["name"],
        json!("withdrawn"),
        "{body}"
    );
    let (holder, kind, held, what): (String, String, i64, String) =
        e.db.conn()
            .query_row(
                "SELECT holder, kind, ask, what FROM blockers WHERE work = ?1 AND ended IS NULL",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
    assert_eq!((holder.as_str(), kind.as_str(), held), ("owner", "ruling", ask));
    assert!(what.contains(&format!("merge condition C{first}")), "{what}");
    for _ in 0..5 {
        e.step(None).unwrap();
    }
    assert!(transitions(&e, item, "M2").is_empty());
    assert_eq!(open_ask(&e, work).map(|(a, _)| a), Some(ask));

    answers(&mut e, ask, "owner", Some("met"), "");
    let (_, next, body) = at_the_gate_asking(&mut e, item, &said, Some(ask));
    assert_eq!(body["condition"], json!(second), "{body}");
    let what: String =
        e.db.conn()
            .query_row(
                "SELECT what FROM blockers WHERE work = ?1 AND ended IS NULL",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(what.contains(&format!("merge condition C{second}")), "{what}");
    assert_eq!(status(&e, first), "met");
    assert_eq!(status(&e, second), "open");

    answers(&mut e, next, "owner", Some("withdrawn"), "Not needed after all.");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    assert_eq!(status(&e, second), "withdrawn");
    let mut gate = transitions(&e, item, "M1");
    gate.dedup();
    assert_eq!(gate, ["owner_judges", "clear"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item with no condition goes through the gate as it did: the row that asks is never taken, and the implementation
// and the fix are not given the conditions, nor asked to report.
#[test]
fn an_item_with_no_condition_goes_through_the_gate_as_before() {
    let s = setup("condition-none");
    let (mut e, said, item) = start(&s, scripts());
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let (work, _, _) = state(&e, item).unwrap();
    let mut gate = transitions(&e, item, "M1");
    gate.dedup();
    assert_eq!(gate, ["clear"]);
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0);
    assert_eq!(
        given_value(&message_at(&e, work, "I1", true), "·engine·merge_conditions"),
        None
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return that leaves out a condition it was given is sent back saying which; once it reports, the work goes on. The
// check is the flow's, on the implementation's return, not only the function under it.
#[test]
fn the_implementation_that_leaves_out_a_condition_it_was_given_is_sent_back() {
    let s = setup("condition-unreported");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let base = agents.remove("I1").unwrap();
    let handed = Arc::new(AtomicUsize::new(0));
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = base(r, n);
            if pick.iter().any(|(p, v)| *p == "outcome" && *v == "tools") {
                return (pick, set);
            }
            // The first answer leaves the field out; the next reports.
            if handed.fetch_add(1, Ordering::SeqCst) > 0 {
                let rows: Vec<Value> = open_numbers(&path)
                    .iter()
                    .map(|id| json!({ "answer": "not_done", "id": id, "why": "The permission check refuses the command." }))
                    .collect();
                set.push(("outcome.owner_conditions", Value::Array(rows)));
            }
            (pick, set)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let c = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");
    let (work, ask, body) = at_the_gate_asking(&mut e, item, &said, None);
    let back = why_sent_back(&e, work, "I1").join("\n");
    assert!(
        back.contains(&format!("merge condition C{c} is not reported on")),
        "{back}"
    );
    assert!(
        points(&body).contains("not done. The permission check refuses the command."),
        "{body}"
    );
    answers(&mut e, ask, "owner", Some("withdrawn"), "");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let _ = std::fs::remove_dir_all(&s.root);
}

// Words with no option judge nothing: they go to the engineer, saying what was asked and who answered, the work chooses
// the way on, and the fix reports what it did, which the second ask holds. A condition recorded after the last fix has no
// report, and its ask says so.
#[test]
fn words_with_no_option_go_to_the_engineer_and_the_gate_asks_again_with_the_new_report() {
    let s = setup("condition-words");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.next", "fix")],
                vec![
                    ("outcome.next.change.what", json!("Run it on the real thing.")),
                    ("outcome.next.change.asked_in", json!("The owner, at the gate.")),
                ],
            )
        }),
    );
    agents.insert(
        "T6",
        Box::new(|r: &Request, n: usize| {
            let wt = &r.worktree;
            if n == 1 {
                write(wt, "src/lib.txt", "old\nfeature\nreal\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Run it for real"]);
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
    // A condition comes while the re-check runs, after the last fix: no return reports on it.
    let late_data = s.data.clone();
    agents.insert(
        "T4",
        Box::new(move |_: &Request, _: usize| {
            let mut db = engine::database::open(&late_data).unwrap();
            condition_on(&mut db, 1, "Show me the output first.", "Show me.");
            (
                Vec::new(),
                vec![("outcome.rows", json!([{ "verdict": "holds", "id": "F1" }]))],
            )
        }),
    );
    reporting_each_condition(&mut agents, &path);
    let (mut e, said, item) = start(&s, agents);
    assert_eq!(item, 1);
    let first = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");

    let (work, ask, body) = at_the_gate_asking(&mut e, item, &said, None);
    assert!(points(&body).contains("Done in I1."), "{body}");
    answers(&mut e, ask, "owner", None, "Run it on the real thing.");

    let (_, again, body) = at_the_gate_asking(&mut e, item, &said, Some(ask));
    assert_eq!(body["condition"], json!(first), "{body}");
    assert!(points(&body).contains("Done in T6."), "{body}");
    assert_eq!(status(&e, first), "open");
    let relayed: String = e
        .db
        .conn()
        .query_row(
            "SELECT text FROM interjections WHERE work = ?1 AND text LIKE 'Asked to judge%' ORDER BY id DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        relayed.ends_with("the owner replied: Run it on the real thing."),
        "{relayed}"
    );
    assert!(relayed.contains(&format!("merge condition C{first}")), "{relayed}");
    answers(&mut e, again, "owner", Some("met"), "");

    let (_, late, body) = at_the_gate_asking(&mut e, item, &said, Some(again));
    assert!(points(&body).contains("has not reported on it yet"), "{body}");
    assert_eq!(body["asked"][0]["value"]["condition"]["report"], Value::Null, "{body}");
    answers(&mut e, late, "owner", Some("met"), "");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let mut gate = transitions(&e, item, "M1");
    gate.dedup();
    assert_eq!(gate, ["owner_judges", "condition_answered", "owner_judges", "clear"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A delegate's answer judges nothing, whichever option it chose: the condition stays open, the work does not merge, and
// the answer goes to the engineer saying the delegate gave it. The owner's own choice then judges it.
#[test]
fn a_delegate_s_choice_judges_no_condition_and_only_the_owner_s_own_does() {
    let s = setup("condition-delegate");
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    reporting_each_condition(&mut agents, &config::database(&s.data));
    let (mut e, said, item) = start(&s, agents);
    let c = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");
    let (work, ask, _) = at_the_gate_asking(&mut e, item, &said, None);
    answers(&mut e, ask, "delegate", Some("met"), "");
    let (_, again, _) = at_the_gate_asking(&mut e, item, &said, Some(ask));
    assert_eq!(status(&e, c), "open");
    assert!(transitions(&e, item, "M2").is_empty());
    let (relayed, declared): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT text, declared FROM interjections WHERE work = ?1 AND text LIKE 'Asked to judge%'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(
        relayed.ends_with("the delegate that speaks for the owner replied: chose met"),
        "{relayed}"
    );
    assert_eq!(declared, "delegate");
    answers(&mut e, again, "owner", Some("met"), "");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    assert_eq!(status(&e, c), "met");
    let _ = std::fs::remove_dir_all(&s.root);
}

// As ask 38 went: the owner answered an ask only "OK", and what it agreed to is in what the ask asked. The architect
// records the condition with a sentence of its own: a quotation the record does not hold is sent back with the whole
// round, one it holds is recorded after the second eye, as any change to an item a work runs on, and the gate's ask gives
// the sentence, the owner's words, who said them and the link to the record.
#[test]
fn the_architect_records_a_condition_from_an_answer_of_only_ok_and_the_gate_asks_with_all_of_it() {
    let s = setup("condition-ok");
    let path = config::database(&s.data);
    let what = "Run claude -p --json-schema twice for real before the merge.";
    let mut agents = scripts();
    let base = agents.remove("R1").unwrap();
    // The research asks twice: the first answer is the words the condition rests on, and the second holds the work back
    // while the architect records it.
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| if n <= 2 { an_ask("owner") } else { base(r, n) }),
    );
    let rests_on = path.clone();
    agents.insert(
        "H8",
        Box::new(move |_: &Request, n: usize| {
            let c = db::open_read(&rests_on).unwrap();
            let asked: i64 = c.query_row("SELECT min(id) FROM owner_asks", [], |r| r.get(0)).unwrap();
            let quote = if n == 1 { "Not said." } else { "OK" };
            let change = json!({ "kind": "add_merge_condition", "item": "1", "what": what,
                                 "source": { "from": "ask", "id": asked, "quote": quote },
                                 "reason": "The owner set it before the merge." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Recorded.")),
                    ("outcome.effects", json!({ "item_changes": [change] })),
                ],
            )
        }),
    );
    reporting_each_condition(&mut agents, &path);
    let (mut e, said, item) = start(&s, agents);
    assert_eq!(item, 1);
    step_until(&mut e, item, &said, "the first ask", |e| {
        state(e, item).is_some_and(|(w, _, _)| open_ask(e, w).is_some())
    });
    let (work, _, _) = state(&e, item).unwrap();
    let (first, _) = open_ask(&e, work).unwrap();
    owner_answers(&mut e, first, "OK");
    step_until(&mut e, item, &said, "the second ask", |e| {
        open_ask(e, work).is_some_and(|(a, _)| a != first)
    });
    let (second, _) = open_ask(&e, work).unwrap();

    db::say_to_architect(
        &mut e.db,
        "p",
        Some(item),
        Some(work),
        "Keep what I agreed to before it merges.",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    let recorded = |e: &Engine| number(e, "SELECT count(*) FROM merge_conditions") == 1;
    assert!(
        steps_until(&mut e, 60, |e| recorded(e)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let handled: i64 =
        e.db.conn()
            .query_row(
                "SELECT recorded_work FROM merge_conditions WHERE recorded_by = 'architect'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let back = why_sent_back(&e, handled, "H8").join("\n");
    assert!(back.contains("is not found as it is in ask"), "{back}");

    owner_answers(&mut e, second, "Go on.");
    let (_, _, body) = at_the_gate_asking(&mut e, item, &said, Some(second));
    let held = &body["asked"][0]["value"]["condition"];
    assert_eq!(held["what"], json!(what), "{body}");
    assert_eq!(held["source"]["quote"], json!("OK"), "{body}");
    assert_eq!(held["source"]["said_by"], json!("owner"), "{body}");
    assert_eq!(held["source"]["from"], json!("ask"), "{body}");
    let link = held["source"]["link"].as_str().unwrap();
    assert!(link.starts_with(&format!("/works/{work}?round=R{work}-")), "{link}");
    let words = points(&body);
    assert!(words.contains(what), "{words}");
    assert!(words.contains("\"OK\", said by the owner"), "{words}");
    assert!(words.contains(link), "{words}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An interjection at the gate closes the ask the owner has not answered, and the engine's note there is no answer of the
// owner's: back at the gate the ask is put again, and nothing goes to the engineer as what the owner replied.
#[test]
fn an_ask_the_engine_closed_at_the_gate_is_put_again_and_not_relayed_as_the_owner_s_reply() {
    let s = setup("gate-closed-ask");
    owner_approves_src(&s);
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    let (work, ask, _) = at_the_gate_asking(&mut e, item, &said, None);
    owner_says(&mut e, work, "A word while it waits.");
    let (_, again, _) = at_the_gate_asking(&mut e, item, &said, Some(ask));
    let relayed: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM interjections WHERE work = ?1 AND text LIKE 'Asked to approve%'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(relayed, 0);
    owner_answers(&mut e, again, "approved");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let mut gate = transitions(&e, item, "M1");
    gate.dedup();
    assert_eq!(gate, ["owner_approves", "interjection", "owner_approves", "clear"]);
    let _ = std::fs::remove_dir_all(&s.root);
}
