//! A work that changes the engine's own checks of returns names them (`checks_changed`): what only those checks find in
//! its returns is set aside, not sent back, and the reviews judge each before the work may merge; no work of a project
//! other than the one the engine is built from can name one.

use super::*;

/// The check the tests' work says it changes: the one of the conditions met, which a return breaks alone by naming a
/// test no run of ci reported.
const CHANGED: &str = "conditions_cover_acceptance";

/// Makes project p the one the engine is built from, by the repository its configuration names: the engine's own,
/// which the fake GitHub names only in what it reports.
fn the_engine_s_own(s: &Setup) {
    let path = s.data.join("projects/p.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let own = env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/");
    write(
        &s.data,
        "projects/p.toml",
        &text.replacen("repo = \"o/p\"", &format!("repo = \"{own}\""), 1),
    );
    git(&s.data, &["commit", "-q", "-am", "the engine's own project"]);
}

/// The declaration of the tests' work: it changes the check of the conditions met.
fn declaration() -> Value {
    json!([{ "check": CHANGED, "why": "The branch reads the tests a condition names from the runs of ci_before too." }])
}

/// Conditions that only the check the work changes refuses: the one condition, met by a test no run of ci reported.
fn conditions_with_a_test_never_run() -> Value {
    json!([{ "how": "tested", "id": "A1", "tests": ["never_run"] }])
}

/// The rounds of the item's work at a cell that ended, in order: the row each took, what it was sent back with and
/// what it set aside, each as the round keeps it.
fn ended_with(e: &Engine, item: i64, cell: &str) -> Vec<(Option<String>, Option<Value>, Option<Value>)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT r.transition, r.rejected, r.set_aside FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = ?2 AND r.ended_at IS NOT NULL ORDER BY r.work, r.n",
            )
            .unwrap();
    let json = |t: Option<String>| t.map(|t| serde_json::from_str::<Value>(&t).unwrap());
    st.query_map(params![item, cell], |r| {
        Ok((r.get(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
    })
    .unwrap()
    .map(|x| {
        let (took, rejected, aside) = x.unwrap();
        (took, json(rejected), json(aside))
    })
    .collect()
}

/// What the item's work was given and handed back at its last round at a cell that took a row.
fn last_taken(e: &Engine, item: i64, cell: &str) -> (Value, Value) {
    let (input, output): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT r.input, r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = ?2 AND r.transition IS NOT NULL ORDER BY r.work DESC, r.n DESC LIMIT 1",
                params![item, cell],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    (serde_json::from_str(&input).unwrap(), serde_json::from_str(&output).unwrap())
}

/// The checks of the problems a return was sent back with, as `rounds.rejected` keeps them.
fn checks_of(rejected: &Value) -> Vec<String> {
    rejected["problems"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["check"].as_str().map(str::to_string))
        .collect()
}

// In the project the engine is built from, a work names the check it changes at the implementation; adversarial
// testing then hands in a return only that check refuses. The return is taken, its problem kept on its round as S1;
// no review judged it, so the merge gate sends it to the fix, before it asks the owner to approve the change: the fix
// answers it old_rule and the re-check holds the answer. Only then does the gate ask the owner, and the work merges.
#[test]
fn a_finding_of_a_check_the_work_changes_is_judged_before_the_owner_approves_and_the_work_merges() {
    let s = setup("declared-check");
    the_engine_s_own(&s);
    owner_approves_src(&s);
    let mut agents = scripts();
    agents.insert("I1", adding("I1", |_| vec![("outcome.checks_changed", declaration())]));
    agents.insert("T3", adversarial());
    agents.insert(
        "T1",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.wrote", "none")],
                vec![("outcome.conditions", conditions_with_a_test_never_run())],
            )
        }),
    );
    agents.insert(
        "T6",
        Box::new(|r: &Request, _: usize| {
            let head = git(&r.worktree, &["rev-parse", "HEAD"]);
            let rows = json!([{ "answer": "old_rule", "id": "F1", "why": "The branch's check reads the run that named it." }]);
            (Vec::new(), vec![("outcome.rows", rows), ("outcome.head", json!(head))])
        }),
    );
    agents.insert(
        "T4",
        Box::new(|_: &Request, _: usize| {
            (Vec::new(), vec![("outcome.rows", json!([{ "verdict": "holds", "id": "F1" }]))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the gate's ask", |e| {
        state(e, item).is_some_and(|(w, at, _)| at == "M1" && open_ask(e, w).is_some())
    });
    let log = said.lock().unwrap().join("\n");
    // Adversarial testing's return was taken, and what the check found kept on its round, numbered in the work.
    let tested = ended_with(&e, item, "T1");
    assert_eq!(tested.len(), 1, "{tested:?}\n{log}");
    let (took, rejected, aside) = &tested[0];
    assert_eq!((took.as_deref(), rejected), (Some("clear"), &None), "{log}");
    let aside = aside.clone().unwrap_or_default();
    assert_eq!(aside.as_array().map(Vec::len), Some(1), "{aside}");
    assert_eq!(
        (&aside[0]["id"], &aside[0]["check"], &aside[0]["field"]),
        (&json!("S1"), &json!(CHANGED), &json!("conditions")),
        "{aside}"
    );
    // The fix was given it as the one row of its table, from the gate, and answered it as the old rule's; the
    // re-check held the answer.
    let (given, fixed) = last_taken(&e, item, "T6");
    let table = &given["engine"]["table"];
    assert_eq!(
        (&table[0]["row"], &table[0]["from"], &table[0]["id"], table.as_array().map(Vec::len)),
        (&json!("F1"), &json!("M1·set_aside"), &json!("S1"), Some(1)),
        "{table}"
    );
    assert_eq!(fixed["outcome"]["rows"][0]["answer"], "old_rule", "{fixed}");
    let (_, rechecked) = last_taken(&e, item, "T4");
    assert_eq!(rechecked["outcome"]["rows"][0]["verdict"], "holds", "{rechecked}");
    let (work, _, _) = state(&e, item).unwrap();
    let (ask, _) = open_ask(&e, work).unwrap();
    owner_answers(&mut e, ask, "approved");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let mut gate = transitions(&e, item, "M1");
    gate.dedup();
    assert_eq!(gate, ["set_aside", "owner_approves", "clear"]);
    let sent_back: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work WHERE w.item = ?1 AND r.rejected IS NOT NULL",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(sent_back, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A project other than the one the engine is built from cannot change the engine's checks: an implementation that
// names one is sent back, saying so; handed in again without the declaration, it is taken.
#[test]
fn only_the_engine_s_own_project_can_say_its_change_changes_the_engine_s_checks() {
    let s = setup("declared-elsewhere");
    let mut agents = scripts();
    let implement = scripts().remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = implement(r, n);
            if n == 2 {
                set.push(("outcome.checks_changed", declaration()));
            }
            (pick, set)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the implementation taken", |e| {
        transitions(e, item, "I1").iter().any(|t| t == "done")
    });
    let rounds = ended_with(&e, item, "I1");
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rounds.len(), 3, "{rounds:?}\n{log}");
    let rejected = rounds[1].1.clone().unwrap_or_default();
    assert_eq!(checks_of(&rejected), ["checks_changed_known"], "{rejected}");
    let text = db::sent_back_text(&rejected).unwrap_or_default();
    assert!(
        text.contains("this project's changes cannot change the engine's checks"),
        "{text}"
    );
    assert_eq!((rounds[2].0.as_deref(), &rounds[2].2), (Some("done"), &None), "{rounds:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return that breaks the check the work changes and another is sent back with the other's problems only, and sets
// nothing aside; handed in again breaking only the one the work changes, it is taken with that set aside. The details
// review, given it, is sent back while it does not answer it; it answers it real, with a finding of its own that
// carries it, and the work goes to the fix.
#[test]
fn a_return_that_breaks_another_check_too_is_sent_back_with_its_problems_only_and_the_review_judges_what_was_set_aside() {
    let s = setup("declared-and-not");
    the_engine_s_own(&s);
    let mut agents = scripts();
    let implement = scripts().remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = implement(r, n);
            if n >= 2 {
                set.push(("outcome.checks_changed", declaration()));
                set.push(("outcome.conditions", conditions_with_a_test_never_run()));
            }
            // The second round names a head that is not the branch's, which a check the work does not change refuses.
            if n == 2 {
                set.push(("outcome.head", json!("0000000000000000000000000000000000000000")));
            }
            (pick, set)
        }),
    );
    agents.insert(
        "T3",
        Box::new(|_: &Request, n: usize| {
            let pick = vec![("outcome.silent", "no"), ("outcome.must_fix", if n == 1 { "no" } else { "yes" })];
            if n == 1 {
                return (pick, Vec::new());
            }
            let finding = json!([{ "fid": "D1", "where": "crates/engine/src/checks.rs", "what": "The test is never run.",
                                   "impact": "The condition is not met.", "basis": "S1", "fix": "Run the test." }]);
            let answer = json!([{ "answer": "real", "id": "S1", "finding": "D1" }]);
            (pick, vec![("outcome.findings", finding), ("outcome.set_aside", answer)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the details review's fix", |e| {
        transitions(e, item, "T3").iter().any(|t| t == "fix")
    });
    let log = said.lock().unwrap().join("\n");
    let implemented = ended_with(&e, item, "I1");
    assert_eq!(implemented.len(), 3, "{implemented:?}\n{log}");
    // Sent back with the problems of the check the work does not change only, and nothing set aside.
    let (_, rejected, aside) = &implemented[1];
    let rejected = rejected.clone().unwrap_or_default();
    let checks = checks_of(&rejected);
    assert!(checks.iter().any(|c| c == "head_pushed_and_green"), "{rejected}");
    assert!(!checks.iter().any(|c| c == CHANGED), "{rejected}");
    let text = db::sent_back_text(&rejected).unwrap_or_default();
    assert!(!text.contains(CHANGED), "{text}");
    assert_eq!(aside, &None, "{log}");
    // Handed in again, it is taken with the one problem set aside.
    let (took, _, aside) = &implemented[2];
    assert_eq!(took.as_deref(), Some("done"), "{log}");
    let aside = aside.clone().unwrap_or_default();
    assert_eq!(
        (aside.as_array().map(Vec::len), &aside[0]["id"], &aside[0]["check"]),
        (Some(1), &json!("S1"), &json!(CHANGED)),
        "{aside}"
    );
    // The details review is held to answering it, then answers it real with its own finding, and the work goes to the
    // fix.
    let reviewed = ended_with(&e, item, "T3");
    assert_eq!(reviewed.len(), 2, "{reviewed:?}\n{log}");
    let first = reviewed[0].1.clone().unwrap_or_default();
    assert_eq!(checks_of(&first), ["set_aside_answered"], "{first}");
    let text = db::sent_back_text(&first).unwrap_or_default();
    assert!(text.contains("finding set aside S1 is not answered"), "{text}");
    assert_eq!(reviewed[1].0.as_deref(), Some("fix"), "{reviewed:?}");
    let (_, judged) = last_taken(&e, item, "T3");
    assert_eq!(judged["outcome"]["findings"][0]["fid"], "D1", "{judged}");
    let _ = std::fs::remove_dir_all(&s.root);
}
