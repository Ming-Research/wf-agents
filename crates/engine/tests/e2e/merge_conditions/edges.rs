//! Where the merge gate can go wrong with no error, the work merging all the same: a condition set late, the gate's two
//! kinds of ask side by side, a condition judged before a fix, a start over at the gate, an item merged into another. The
//! reference of each is the design the owner approved: a condition holds the work until the owner judges it, the levels
//! go items, set_aside, approvals, conditions, owner, and a judgment is not bound to a head.

use super::*;

/// An answer to an ask of the owner, its option and its tick as they are given.
fn answer_of(speaker: &str, choice: Option<&str>, text: &str, condition: Option<&str>) -> api::Answer {
    api::Answer {
        text: text.into(),
        speaker: speaker.into(),
        relayed: None,
        why: None,
        reply: None,
        to: None,
        choice: choice.map(str::to_string),
        condition: condition.map(str::to_string),
        images: Vec::new(),
    }
}

/// What the work's open blocker says it waits on, and the ask that stands for it.
fn blocker(e: &Engine, work: i64) -> (String, Option<i64>) {
    e.db.conn()
        .query_row(
            "SELECT what, ask FROM blockers WHERE work = ?1 AND ended IS NULL",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

/// How many asks of the condition were put.
fn asks_of(e: &Engine, condition: i64) -> i64 {
    number(
        e,
        &format!("SELECT count(*) FROM owner_asks WHERE json_extract(body, '$.condition') = {condition}"),
    )
}

/// How many answers went to the engineer as what the owner replied to the gate.
fn relayed_to_the_engineer(e: &Engine, work: i64) -> i64 {
    number(
        e,
        &format!("SELECT count(*) FROM interjections WHERE work = {work} AND text LIKE 'Asked to %'"),
    )
}

/// The rows the gate took, each run of one row once.
fn gate_path(e: &Engine, item: i64) -> Vec<String> {
    let mut gate = transitions(e, item, "M1");
    gate.dedup();
    gate
}

fn merged(e: &Engine, item: i64) -> bool {
    transitions(e, item, "M2").iter().any(|t| t == "merged")
}

// The merge step takes minutes in its full CI, and the owner's tick is taken all that time: a condition set then holds
// the merge, which cannot be undone, and the gate asks of it. The owner's choice then lets the work merge.
#[test]
fn a_condition_set_while_the_merge_waits_for_its_full_ci_stops_the_merge() {
    let s = setup("condition-in-full-ci");
    let mut agents = scripts();
    reporting_each_condition(&mut agents, &config::database(&s.data));
    let (mut e, said, item) = start(&s, agents);
    let at = turns_until(&mut e, &Go::default(), |e| state(e, item).is_some_and(|w| w.1 == "M2"));
    assert!(at, "{}", said.lock().unwrap().join("\n"));
    let work = state(&e, item).unwrap().0;
    s.ci.shut();
    let came = s.ci.came();
    let held = turns_until(&mut e, &Go::default(), |_| s.ci.came() > came);
    assert!(held, "{}", said.lock().unwrap().join("\n"));

    let c = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");
    s.ci.open();
    let settled = turns_until(&mut e, &Go::default(), |e| {
        s.merging.came() > 0 || open_ask(e, work).is_some()
    });
    let log = said.lock().unwrap().join("\n");
    assert!(settled, "{log}");
    assert_eq!(s.merging.came(), 0, "it merged past a condition not judged\n{log}");
    let (ask, body) = open_ask(&e, work).unwrap();
    assert_eq!(body["condition"], json!(c), "{body}");
    assert_eq!(status(&e, c), "open");

    answers(&mut e, ask, "owner", Some("met"), "");
    let done = turns_until(&mut e, &Go::default(), |e| merged(e, item));
    assert!(done, "{}", said.lock().unwrap().join("\n"));
    assert_eq!(status(&e, c), "met");
    e.drain().unwrap();
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect merges an item not started into another, which carries its decisions and its dependencies: the owner's
// condition on it goes too, or the kept item's work merges with no one asked and the page shows it nowhere.
#[test]
fn a_condition_on_an_item_merged_into_another_goes_to_the_item_kept() {
    let s = setup("condition-merged-item");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let merge = json!({ "kind": "merge", "keep": "1", "absorbed": ["2"], "reason": "The same work." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": [merge] })),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let keep = an_item(&mut e);
    let absorbed = a_ready_item(&mut e, "Parse more");
    assert_eq!((keep, absorbed), (1, 2));
    let c = condition_on(&mut e.db, absorbed, "Run it twice before the merge.", "Run it twice.");

    say(&mut e, "Merge the second item into the first.");
    let arch = e.step(None).unwrap().work.unwrap();
    let done = steps_until(&mut e, 80, |e| architect_work_done(e, arch));
    let log = said.lock().unwrap().join("\n");
    assert!(done, "{log}");
    let cancelled = number(&e, "SELECT count(*) FROM items WHERE id = 2 AND cancelled IS NOT NULL");
    assert_eq!(cancelled, 1, "{log}");
    let on = number(&e, &format!("SELECT item FROM item_merge_conditions WHERE id = {c}"));
    assert_eq!(on, keep, "the condition stayed on the item cancelled\n{log}");
    assert_eq!(status(&e, c), "open");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A change the owner approves on an item with a condition: the approval's ask is put as soon as the branch touches its
// path, but the gate waits on the condition first. Words at the gate close the condition's ask, the engine's note goes to
// no one as the owner's reply, and the ask is put again, the closed one taking no answer. The option chosen is not read as
// a reply to the approval, nor "approved" as a judgment: the gate waits on the approval next with the blocker saying so,
// and a condition ticked on that answer is asked with the blocker saying so again, and only then does
// the work merge, with its blocker ended.
#[test]
fn a_condition_is_asked_before_the_approval_and_neither_ask_reads_the_other_s_answer() {
    let s = setup("condition-and-approval");
    owner_approves_src(&s);
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    reporting_each_condition(&mut agents, &config::database(&s.data));
    let (mut e, said, item) = start(&s, agents);
    let first = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");

    let (work, ask, body) = at_the_gate_asking(&mut e, item, &said, None);
    assert_eq!(body["condition"], json!(first), "{body}");

    owner_says(&mut e, work, "A word while it waits.");
    let (_, again, body) = at_the_gate_asking(&mut e, item, &said, Some(ask));
    assert_eq!(body["condition"], json!(first), "{body}");
    assert_eq!(relayed_to_the_engineer(&e, work), 0);
    let closed = api::answer(&mut e.db, ask, &answer_of("owner", Some("met"), "", None));
    assert!(closed.is_err(), "{closed:?}");
    assert_eq!(status(&e, first), "open");

    answers(&mut e, again, "owner", Some("met"), "");
    let (_, approval, body) = at_the_gate_asking(&mut e, item, &said, Some(again));
    assert_eq!(body["condition"], Value::Null, "{body}");
    let (what, held) = blocker(&e, work);
    assert!(what.contains("the owner's approval of the change"), "{what}");
    assert_eq!(held, Some(approval));

    let ticked = answer_of("owner", None, "approved", Some("Show me the output first."));
    api::answer(&mut e.db, approval, &ticked).unwrap();
    let second = number(&e, "SELECT max(id) FROM merge_conditions");
    let (_, last, body) = at_the_gate_asking(&mut e, item, &said, Some(approval));
    assert_eq!(body["condition"], json!(second), "{body}");
    assert_eq!(
        body["asked"][0]["value"]["condition"]["source"]["quote"],
        json!("approved")
    );
    let (what, held) = blocker(&e, work);
    assert!(what.contains(&format!("merge condition C{second}")), "{what}");
    assert_eq!(held, Some(last));

    answers(&mut e, last, "owner", Some("met"), "");
    step_until(&mut e, item, &said, "the merge", |e| merged(e, item));
    assert_eq!(
        gate_path(&e, item),
        [
            "owner_judges",
            "interjection",
            "owner_judges",
            "owner_approves",
            "owner_judges",
            "clear"
        ]
    );
    assert_eq!(relayed_to_the_engineer(&e, work), 0);
    let open = number(
        &e,
        &format!("SELECT count(*) FROM blockers WHERE work = {work} AND ended IS NULL"),
    );
    assert_eq!(open, 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A condition comes while the gate waits for the owner's approval, whose ask was put as soon as the branch touched its path:
// the gate asks of the condition at once, one ask however many times it reads them, and the blocker waits on it. The
// approval's ask stays open, and its answer lets nothing through while the
// condition is open; the owner's judgment then does, with neither ask put again.
#[test]
fn a_condition_set_while_the_approval_waits_is_asked_once_and_holds_the_work_when_the_approval_comes_first() {
    let s = setup("condition-after-approval-asked");
    owner_approves_src(&s);
    let mut agents = scripts();
    reporting_each_condition(&mut agents, &config::database(&s.data));
    let (mut e, said, item) = start(&s, agents);
    let (work, approval, body) = at_the_gate_asking(&mut e, item, &said, None);
    assert_eq!(body["condition"], Value::Null, "{body}");

    let c = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");
    let (_, ask, body) = at_the_gate_asking(&mut e, item, &said, Some(approval));
    assert_eq!(body["condition"], json!(c), "{body}");
    assert_eq!(blocker(&e, work).1, Some(ask));
    for _ in 0..5 {
        e.step(None).unwrap();
    }
    assert_eq!(asks_of(&e, c), 1);

    owner_answers(&mut e, approval, "approved");
    for _ in 0..5 {
        e.step(None).unwrap();
    }
    assert!(transitions(&e, item, "M2").is_empty());
    assert_eq!(open_ask(&e, work).map(|(a, _)| a), Some(ask));
    assert_eq!(status(&e, c), "open");

    answers(&mut e, ask, "owner", Some("met"), "");
    step_until(&mut e, item, &said, "the merge", |e| merged(e, item));
    assert_eq!(gate_path(&e, item), ["owner_approves", "owner_judges", "clear"]);
    let approvals = number(
        &e,
        &format!("SELECT count(*) FROM owner_asks WHERE work = {work} AND json_extract(body, '$.condition') IS NULL"),
    );
    assert_eq!(approvals, 1);
    assert_eq!(asks_of(&e, c), 1);
    assert_eq!(relayed_to_the_engineer(&e, work), 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A judgment is not bound to a head: a fix for the second condition does not bring the first back, which stays met and is
// asked of once; the second is asked again with what the fix reports.
#[test]
fn a_condition_judged_before_a_fix_is_not_asked_again_and_the_one_the_fix_is_for_is() {
    let s = setup("condition-judged-before-fix");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.next", "fix")],
                vec![
                    ("outcome.next.change.what", json!("Show the output.")),
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
                write(wt, "src/lib.txt", "old\nfeature\noutput\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Show the output"]);
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
    agents.insert(
        "T4",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.rows", json!([{ "verdict": "holds", "id": "F1" }]))],
            )
        }),
    );
    reporting_each_condition(&mut agents, &path);
    let (mut e, said, item) = start(&s, agents);
    let first = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");
    let second = condition_on(&mut e.db, item, "Show me the output first.", "Show me.");

    let (_, ask, _) = at_the_gate_asking(&mut e, item, &said, None);
    answers(&mut e, ask, "owner", Some("met"), "");
    let (_, next, body) = at_the_gate_asking(&mut e, item, &said, Some(ask));
    assert_eq!(body["condition"], json!(second), "{body}");
    answers(&mut e, next, "owner", None, "Show the output.");

    let (_, again, body) = at_the_gate_asking(&mut e, item, &said, Some(next));
    assert_eq!(body["condition"], json!(second), "{body}");
    assert!(points(&body).contains("Done in T6."), "{body}");
    assert_eq!(status(&e, first), "met");
    assert_eq!(asks_of(&e, first), 1);

    answers(&mut e, again, "owner", Some("met"), "");
    step_until(&mut e, item, &said, "the merge", |e| merged(e, item));
    assert_eq!(
        gate_path(&e, item),
        ["owner_judges", "condition_answered", "owner_judges", "clear"]
    );
    assert_eq!(asks_of(&e, first), 1);
    assert_eq!(asks_of(&e, second), 2);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A start over at the gate: the ask of the work that ended takes no answer, so no one judges by it, the condition stays
// on the item, and the new work comes to the gate held by it, with a report of its own.
#[test]
fn a_start_over_at_the_gate_closes_its_ask_and_the_condition_holds_the_new_work() {
    let s = setup("condition-start-over");
    let mut agents = scripts();
    let base = agents.remove("I1").unwrap();
    let pushed = AtomicI64::new(0);
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, set) = base(r, n);
            if pick.iter().any(|(p, v)| *p == "outcome" && *v == "tools") {
                pushed.store(r.work, Ordering::SeqCst);
                return (pick, set);
            }
            // The new work goes on from the branch the old one pushed, whose commit the script finds there: it asks for
            // ci on that head itself, as each work must.
            if pushed.swap(r.work, Ordering::SeqCst) != r.work {
                let head = git(&r.worktree, &["rev-parse", "HEAD"]);
                let pr = json!({ "title": "Add the feature", "body": "It adds it." });
                let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"], "pr": pr }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            (pick, set)
        }),
    );
    reporting_each_condition(&mut agents, &config::database(&s.data));
    let (mut e, said, item) = start(&s, agents);
    let c = condition_on(&mut e.db, item, "Run it twice before the merge.", "Run it twice.");
    let (old, ask, _) = at_the_gate_asking(&mut e, item, &said, None);

    api::start_over(&mut e.db, old).unwrap();
    step_until(&mut e, item, &said, "the new work", |e| works_of(e, item).len() == 2);
    let closed = api::answer(&mut e.db, ask, &answer_of("owner", Some("met"), "", None));
    assert!(closed.is_err(), "{closed:?}");
    assert_eq!(status(&e, c), "open");

    let new = works_of(&e, item).into_iter().map(|w| w.0).max().unwrap();
    assert_ne!(new, old);
    step_until(&mut e, item, &said, "the new work's ask at the gate", |e| {
        works_of(e, item).last().is_some_and(|w| w.0 == new && w.1 == "M1") && open_ask(e, new).is_some()
    });
    assert!(transitions_at(&e, new, "M2").is_empty());
    let (again, body) = open_ask(&e, new).unwrap();
    assert_eq!(body["condition"], json!(c), "{body}");
    let round = body["asked"][0]["value"]["condition"]["report"]["round"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(round.starts_with(&format!("R{new}-")), "{body}");

    answers(&mut e, again, "owner", Some("met"), "");
    assert_eq!(status(&e, c), "met");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A tick through the owner's own way in, not a row written by a test: words said in the group while the work implements
// hold it at the gate, whose ask gives those words and the conversation they are in. A second condition ticked while the
// implementation's last round runs was not given to it, so its leaving that one out sends nothing back, and the ask of it
// says the engineer has not reported.
#[test]
fn a_condition_ticked_in_the_group_holds_the_gate_and_one_ticked_while_the_round_runs_is_no_fault_of_it() {
    let s = setup("condition-ticked");
    let path = config::database(&s.data);
    let data = s.data.clone();
    let mut agents = scripts();
    let base = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = base(r, n);
            if pick.iter().any(|(p, v)| *p == "outcome" && *v == "tools") {
                return (pick, set);
            }
            let rows: Vec<Value> = open_numbers(&path)
                .iter()
                .map(|id| json!({ "answer": "done", "id": id, "what": "Done in I1." }))
                .collect();
            let mut db = engine::database::open(&data).unwrap();
            condition_on(&mut db, 1, "Show me the output first.", "Show me.");
            set.push(("outcome.owner_conditions", Value::Array(rows)));
            (pick, set)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    assert_eq!(item, 1);
    step_until(&mut e, item, &said, "the implementation", |e| {
        state(e, item).is_some_and(|(_, at, _)| at == "I1")
    });
    let work = state(&e, item).unwrap().0;
    let words = api::WorkSaid {
        text: "Run it for real.".into(),
        speaker: "owner".into(),
        relayed: None,
        to: Some("engineer".into()),
        question: None,
        design_card: None,
        condition: Some("Run it twice before the merge.".into()),
        images: Vec::new(),
    };
    api::say_to_work(&mut e.db, work, &words).unwrap();
    let first = number(&e, "SELECT max(id) FROM merge_conditions");
    let source = number(
        &e,
        &format!("SELECT source_id FROM merge_conditions WHERE id = {first}"),
    );

    let (_, ask, body) = at_the_gate_asking(&mut e, item, &said, None);
    assert_eq!(body["condition"], json!(first), "{body}");
    let held = &body["asked"][0]["value"]["condition"];
    assert_eq!(held["what"], json!("Run it twice before the merge."), "{body}");
    assert_eq!(held["source"]["from"], json!("interjection"), "{body}");
    assert_eq!(held["source"]["id"], json!(source), "{body}");
    assert_eq!(held["source"]["quote"], json!("Run it for real."), "{body}");
    assert_eq!(held["source"]["said_by"], json!("owner"), "{body}");
    assert_eq!(
        held["source"]["link"],
        json!(format!("/messages?c=work:{work}")),
        "{body}"
    );
    assert!(points(&body).contains("Done in I1."), "{body}");
    assert_eq!(why_sent_back(&e, work, "I1"), Vec::<String>::new());

    answers(&mut e, ask, "owner", Some("met"), "");
    let (_, next, body) = at_the_gate_asking(&mut e, item, &said, Some(ask));
    assert_ne!(body["condition"], json!(first), "{body}");
    assert!(points(&body).contains("has not reported on it yet"), "{body}");
    answers(&mut e, next, "owner", Some("withdrawn"), "");
    step_until(&mut e, item, &said, "the merge", |e| merged(e, item));
    let _ = std::fs::remove_dir_all(&s.root);
}
