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
        Ok((
            r.get(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
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
    (
        serde_json::from_str(&input).unwrap(),
        serde_json::from_str(&output).unwrap(),
    )
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
            let rows =
                json!([{ "answer": "old_rule", "id": "F1", "why": "The branch's check reads the run that named it." }]);
            (Vec::new(), vec![("outcome.rows", rows), ("outcome.head", json!(head))])
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
        (
            &table[0]["row"],
            &table[0]["from"],
            &table[0]["id"],
            table.as_array().map(Vec::len)
        ),
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
    let sent_back: i64 = e
        .db
        .conn()
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
    assert_eq!(
        (rounds[2].0.as_deref(), &rounds[2].2),
        (Some("done"), &None),
        "{rounds:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return that breaks the check the work changes and another is sent back with the other's problems only, and sets
// nothing aside; handed in again breaking only the one the work changes, it is taken with that set aside. The details
// review, given it, is sent back while it does not answer it; it answers it real, with a finding of its own that
// carries it, and the work goes to the fix.
#[test]
fn a_return_that_breaks_another_check_too_is_sent_back_with_its_problems_only_and_the_review_judges_what_was_set_aside()
{
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

/// A declaration of the check the tests' work changes and of `other` besides.
fn declaring_too(other: &str) -> Value {
    json!([declaration()[0].clone(), { "check": other, "why": "The branch changes this check as well." }])
}

/// The numbers of the findings set aside in a list, as a review's `set_aside` answers them, the merge gate returns them
/// or a round keeps them.
fn ids_in(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x["id"].as_str().map(str::to_string))
        .collect()
}

/// What the merge gate returned the last time it took a row for the item's work.
fn gate_return(e: &Engine, item: i64) -> Value {
    last_taken(e, item, "M1").1
}

/// What the merge gate did the first time it judged the item's work, past waiting for items: the row it took, and the
/// findings set aside before it that it let through, those neither answered in the `set_aside` of a details review's
/// or a re-check's return the engine took before it, nor among those it returned for the fix to answer.
fn left_unjudged_at_the_gate(e: &Engine, item: i64) -> (String, Vec<String>) {
    let (work, _, _) = state(e, item).unwrap();
    let conn = e.db.conn();
    let (gate, took, output): (i64, String, String) = conn
        .query_row(
            "SELECT n, transition, output FROM rounds WHERE work = ?1 AND cell = 'M1' AND transition IS NOT NULL
             AND transition != 'wait' ORDER BY n LIMIT 1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let mut st = conn
        .prepare("SELECT id FROM work_set_aside WHERE work = ?1 AND round < ?2 ORDER BY round, position")
        .unwrap();
    let recorded: Vec<String> = st
        .query_map(params![work, gate], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let mut st = conn
        .prepare(
            "SELECT output FROM rounds WHERE work = ?1 AND cell IN ('T3', 'T4') AND transition IS NOT NULL AND n < ?2",
        )
        .unwrap();
    let mut judged: BTreeSet<String> = st
        .query_map(params![work, gate], |r| r.get::<_, String>(0))
        .unwrap()
        .flat_map(|o| ids_in(&serde_json::from_str::<Value>(&o.unwrap()).unwrap()["outcome"]["set_aside"]))
        .collect();
    judged.extend(ids_in(&serde_json::from_str::<Value>(&output).unwrap()["set_aside"]));
    (took, recorded.into_iter().filter(|id| !judged.contains(id)).collect())
}

// The details review's own return can hold a finding set aside, when the work says it changes a check of that cell.
// The review that handed it in has not judged it: no review came after it, so the merge gate must send it to the fix
// and the re-check rather than let the work through. The finding set aside earlier, which the review answered, is
// judged, and the numbers go on across the rounds: S1 at the implementation, S2 at the review.
#[test]
fn a_finding_set_aside_in_the_details_review_s_own_return_waits_for_a_review_after_it() {
    let s = setup("declared-review-own");
    the_engine_s_own(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        adding("I1", |n| {
            if n < 2 {
                return Vec::new();
            }
            vec![
                ("outcome.checks_changed", declaring_too("must_fix_fits_findings")),
                ("outcome.conditions", conditions_with_a_test_never_run()),
            ]
        }),
    );
    // The review finds a problem but says nothing must be fixed, which only must_fix_fits_findings refuses.
    agents.insert(
        "T3",
        Box::new(|_: &Request, _: usize| {
            let finding = json!([{ "fid": "D1", "where": "src/lib.txt", "what": "The feature is not described.",
                                   "impact": "A reader cannot tell what it does.", "basis": "The design.",
                                   "fix": "Describe it." }]);
            let answer = json!([{ "answer": "old_rule", "id": "S1", "why": "The branch reads ci_before's runs too." }]);
            (
                vec![("outcome.silent", "no"), ("outcome.must_fix", "no")],
                vec![("outcome.findings", finding), ("outcome.set_aside", answer)],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the merge gate's judgment", |e| {
        transitions(e, item, "M1").iter().any(|t| t != "wait")
    });
    let log = said.lock().unwrap().join("\n");
    let implemented = ended_with(&e, item, "I1");
    let aside = implemented.last().and_then(|r| r.2.clone()).unwrap_or_default();
    assert_eq!(
        (ids_in(&aside), &aside[0]["check"]),
        (vec!["S1".to_string()], &json!(CHANGED)),
        "{implemented:?}\n{log}"
    );
    let reviewed = ended_with(&e, item, "T3");
    assert_eq!(reviewed.len(), 1, "{reviewed:?}\n{log}");
    let (took, _, aside) = &reviewed[0];
    let aside = aside.clone().unwrap_or_default();
    assert_eq!(
        (took.as_deref(), ids_in(&aside), &aside[0]["check"]),
        (Some("clear"), vec!["S2".to_string()], &json!("must_fix_fits_findings")),
        "{log}"
    );
    let (gate, left) = left_unjudged_at_the_gate(&e, item);
    assert_eq!((gate.as_str(), left), ("set_aside", Vec::<String>::new()), "{log}");
    // The gate hands on the review's own finding alone, saying the cell whose return it was found in.
    let sent = gate_return(&e, item);
    assert_eq!(
        (ids_in(&sent["set_aside"]), &sent["set_aside"][0]["cell"]),
        (vec!["S2".to_string()], &json!("T3")),
        "{sent}\n{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A finding set aside in the re-check's own return, when the work says it changes the re-check's check, has no review
// after it: the merge gate sends it to the fix and the re-check once more, saying it came from the re-check, and lets the
// work through only once a return of the re-check sets none aside.
#[test]
fn a_finding_set_aside_in_the_re_check_s_own_return_goes_round_until_a_return_of_the_re_check_sets_none_aside() {
    let s = setup("declared-recheck-own");
    the_engine_s_own(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        adding("I1", |_| {
            vec![("outcome.checks_changed", declaring_too("previous_findings_answered"))]
        }),
    );
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
            let rows = json!([{ "answer": "old_rule", "id": "F1", "why": "The branch's check reads the run." }]);
            (Vec::new(), vec![("outcome.rows", rows), ("outcome.head", json!(head))])
        }),
    );
    // The first return of the re-check leaves the row out, which only the check the work says it changes refuses.
    agents.insert(
        "T4",
        Box::new(|_: &Request, n: usize| {
            let rows = if n == 1 {
                json!([])
            } else {
                json!([{ "verdict": "holds", "id": "F1" }])
            };
            (Vec::new(), vec![("outcome.rows", rows)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        transitions(&e, item, "M1"),
        ["set_aside", "set_aside", "clear"],
        "{log}"
    );
    let rechecked = ended_with(&e, item, "T4");
    let aside = rechecked[0].2.clone().unwrap_or_default();
    assert_eq!(
        (ids_in(&aside), &aside[0]["check"]),
        (vec!["S2".to_string()], &json!("previous_findings_answered")),
        "{rechecked:?}\n{log}"
    );
    assert_eq!(rechecked[1].2, None, "{rechecked:?}\n{log}");
    let (given, _) = last_taken(&e, item, "T6");
    let table = &given["engine"]["table"];
    assert_eq!(
        (&table[0]["id"], &table[0]["cell"], table.as_array().map(Vec::len)),
        (&json!("S2"), &json!("T4"), Some(1)),
        "{table}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The check that holds the reviews to answering each finding set aside is what makes a finding judged; a work that
// says it changes that check too must not get its findings through the merge gate with no review answering them.
// Whether the declaration is refused or the gate sends them to the fix, every finding set aside before the gate is
// answered by a review or sent on by the gate.
#[test]
fn saying_the_work_changes_the_check_of_the_reviews_answers_lets_no_finding_set_aside_through_unanswered() {
    let s = setup("declared-answers");
    the_engine_s_own(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        adding("I1", |n| {
            let declared = match n {
                0 | 1 => return Vec::new(),
                2 => declaring_too("set_aside_answered"),
                _ => declaration(),
            };
            vec![
                ("outcome.checks_changed", declared),
                ("outcome.conditions", conditions_with_a_test_never_run()),
            ]
        }),
    );
    // The review answers nothing set aside the first time, and the implementation's finding when it is sent back.
    agents.insert(
        "T3",
        Box::new(|_: &Request, n: usize| {
            let pick = vec![("outcome.silent", "no"), ("outcome.must_fix", "no")];
            if n == 1 {
                return (pick, Vec::new());
            }
            let answer = json!([{ "answer": "old_rule", "id": "S1", "why": "The branch reads ci_before's runs too." }]);
            (pick, vec![("outcome.set_aside", answer)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the merge gate's judgment", |e| {
        transitions(e, item, "M1").iter().any(|t| t != "wait")
    });
    let log = said.lock().unwrap().join("\n");
    let (gate, left) = left_unjudged_at_the_gate(&e, item);
    assert_eq!(left, Vec::<String>::new(), "the gate took {gate}\n{log}");
    // The declaration of that check is refused, and the review held to answering.
    let implemented = ended_with(&e, item, "I1");
    let refused = implemented[1].1.clone().unwrap_or_default();
    assert!(
        checks_of(&refused).iter().any(|c| c == "checks_changed_known"),
        "{refused}"
    );
    assert!(
        db::sent_back_text(&refused)
            .unwrap_or_default()
            .contains("set_aside_answered"),
        "{refused}"
    );
    let reviewed = ended_with(&e, item, "T3");
    assert_eq!(
        checks_of(&reviewed[0].1.clone().unwrap_or_default()),
        ["set_aside_answered"],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A declaration its own check refuses holds nothing in the return that makes it: what the check it names finds there
// is sent back with the refusal, and nothing is set aside. A fix's declaration adds to the implementation's and holds
// for the fix's own return: what it finds there is set aside and given to the re-check, which is not given the finding
// the fix answered as a row of its table, and answers only the other. Once it has, the gate lets the work through.
#[test]
fn a_refused_declaration_holds_nothing_and_a_fix_s_declaration_holds_for_its_own_return_which_the_re_check_judges() {
    let s = setup("declared-refused-and-fixed");
    the_engine_s_own(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        adding("I1", |n| {
            let declared = match n {
                0 | 1 => return Vec::new(),
                2 => declaring_too("no_such_check"),
                _ => declaration(),
            };
            vec![
                ("outcome.checks_changed", declared),
                ("outcome.conditions", conditions_with_a_test_never_run()),
            ]
        }),
    );
    agents.insert(
        "T3",
        Box::new(|_: &Request, _: usize| {
            let answer = json!([{ "answer": "old_rule", "id": "S1", "why": "The branch reads ci_before's runs too." }]);
            (
                vec![("outcome.silent", "yes"), ("outcome.must_fix", "no")],
                vec![("outcome.set_aside", answer)],
            )
        }),
    );
    agents.insert(
        "T1",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.wrote", "none")],
                vec![("outcome.conditions", conditions_with_a_test_never_run())],
            )
        }),
    );
    // The fix answers its one row, and a row there is not, which only table_answered refuses, a check it names.
    agents.insert(
        "T6",
        Box::new(|r: &Request, _: usize| {
            let head = git(&r.worktree, &["rev-parse", "HEAD"]);
            let rows = json!([
                { "answer": "old_rule", "id": "F1", "why": "The branch's check reads the run that named it." },
                { "answer": "old_rule", "id": "F9", "why": "The branch's check reads the run that named it." }
            ]);
            let declared = json!([{ "check": "table_answered", "why": "The branch lets a fix answer more rows." }]);
            (
                Vec::new(),
                vec![
                    ("outcome.rows", rows),
                    ("outcome.head", json!(head)),
                    ("outcome.checks_changed", declared),
                ],
            )
        }),
    );
    agents.insert(
        "T4",
        Box::new(|_: &Request, _: usize| {
            let rows = json!([{ "verdict": "holds", "id": "F1" }, { "verdict": "holds", "id": "F9" }]);
            let answer =
                json!([{ "answer": "old_rule", "id": "S3", "why": "The branch lets a fix answer more rows." }]);
            (Vec::new(), vec![("outcome.rows", rows), ("outcome.set_aside", answer)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let log = said.lock().unwrap().join("\n");
    // The refused declaration: sent back with the refusal and the problem of the check it named, nothing set aside.
    let implemented = ended_with(&e, item, "I1");
    assert_eq!(implemented.len(), 3, "{implemented:?}\n{log}");
    let (_, rejected, aside) = &implemented[1];
    let rejected = rejected.clone().unwrap_or_default();
    let checks = checks_of(&rejected);
    assert!(
        checks.iter().any(|c| c == "checks_changed_known") && checks.iter().any(|c| c == CHANGED),
        "{rejected}"
    );
    assert!(
        db::sent_back_text(&rejected)
            .unwrap_or_default()
            .contains("no_such_check"),
        "{rejected}"
    );
    assert_eq!(aside, &None, "{log}");
    // Each cell's finding set aside, numbered on across the rounds, by the check that found it.
    let first = |cell: &str| {
        let rounds = ended_with(&e, item, cell);
        let aside = rounds.iter().find_map(|r| r.2.clone()).unwrap_or_default();
        (
            ids_in(&aside),
            aside[0]["check"].as_str().unwrap_or_default().to_string(),
        )
    };
    assert_eq!(first("I1"), (vec!["S1".to_string()], CHANGED.to_string()), "{log}");
    assert_eq!(first("T1"), (vec!["S2".to_string()], CHANGED.to_string()), "{log}");
    assert_eq!(
        first("T6"),
        (vec!["S3".to_string()], "table_answered".to_string()),
        "{log}"
    );
    // The re-check was given S3 alone: answering it alone, it was never sent back.
    let rechecked = ended_with(&e, item, "T4");
    assert!(
        !rechecked.is_empty() && rechecked.iter().all(|r| r.1.is_none()),
        "{rechecked:?}\n{log}"
    );
    let mut gate = transitions(&e, item, "M1");
    gate.dedup();
    assert_eq!(gate, ["set_aside", "clear"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
