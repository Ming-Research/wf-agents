//! The architect's notebook: its two parts, which a base writes, and the lines matters add to them.

use super::*;

// A round of the plan that asks for tools writes nothing in the notebook: until the cell answers, the plan is not there,
// and the cell's next round is referred to the inputs it was given. The tool results, which every cell may have, are
// listed only when there are some.
#[test]
fn a_round_of_the_plan_that_asks_for_tools_changes_nothing_in_the_notebook() {
    let s = setup("plan-tools");
    let mut agents = scripts();
    agents.insert(
        "B2",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                let tools = json!([{ "tool": "query", "sql": "SELECT id FROM items" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    let took = |row: &'static str| {
        move |e: &mut Engine| {
            cells_of(e, arch)
                .iter()
                .any(|c| c.0 == "B2" && c.1.as_deref() == Some(row))
        }
    };
    assert!(
        steps_until(&mut e, 10, took("tools")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let understanding = (
        "project_now".to_string(),
        "The understanding at round 1.".to_string(),
        "B1".to_string(),
    );
    assert_eq!(
        notebook(&e),
        std::slice::from_ref(&understanding),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert!(
        steps_until(&mut e, 10, took("planned")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(
        notebook(&e),
        [
            ("plan".to_string(), "The plan at round 2.".to_string(), "B2".to_string()),
            understanding
        ]
    );
    let (first, second) = (message_at(&e, arch, "B2", false), message_at(&e, arch, "B2", true));
    let w = e.work(arch).unwrap();
    let g = e.graph_of(&w).unwrap();
    let numbered: Vec<&flow::CompiledInput> = g
        .cell("B2")
        .unwrap()
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == flow::format::Src::Itself))
        .collect();
    let tools = numbered
        .iter()
        .position(|x| x.name.as_deref() == Some("Tool results"))
        .unwrap()
        + 1;
    let listed = |m: &str, n: usize| work_from(m).into_iter().find(|l| l.starts_with(&format!("{n}. ")));
    assert_eq!(listed(&first, tools), None, "{first}");
    assert!(listed(&second, tools).is_some_and(|l| l.contains("·X1·")), "{second}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A matter does not rewrite the notebook: the lines it adds go with its record, once it is handled, and a later matter,
// forked from the same base, reads them in what came since it. The parts stay as the base wrote them.
#[test]
fn what_a_matter_adds_is_read_by_the_next_matter_and_the_parts_stay() {
    let s = setup("notebook-lines");
    let mut agents = scripts();
    agents.insert(
        "H1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let lines = json!({ "plan": "The parser comes first.", "understanding": "The parser moved." });
            (
                Vec::new(),
                vec![("progress", json!("Noted the parser.")), ("outcome.notebook", lines)],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the parser first.");
    say(&mut e, "And then?");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        notebook(&e),
        [
            ("plan".to_string(), "The plan at round 1.".to_string(), "B2".to_string()),
            (
                "project_now".to_string(),
                "The understanding at round 1.".to_string(),
                "B1".to_string()
            ),
        ],
        "{log}"
    );
    let later = input_at(&e, arch, "H1", true);
    assert_eq!(given_ending(&later, "·engine·appended"), Some(true), "{later}");
    let came = later["message"].as_str().unwrap();
    assert!(came.contains("Noted the parser."), "{came}");
    assert!(came.contains("It adds to the plan: The parser comes first."), "{came}");
    assert!(
        came.contains("It adds to the understanding: The parser moved."),
        "{came}"
    );
    // What the owner said after the base was built reads there too; what was said before it, the base read.
    assert!(came.contains("architect to owner"), "{came}");
    assert!(!came.contains("Look at the parser first."), "{came}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What the architect asks the owner and what the owner answers come after the base as a matter's record does: the matter
// taken next reads the question, and once the owner answered, the next run takes the matter that asked in a new fork of
// the base, whose session is not the one that asked, and reads both there.
#[test]
fn what_is_asked_of_the_owner_and_answered_is_read_after_the_base() {
    let s = setup("notebook-asked");
    let mut agents = scripts();
    agents.insert(
        "H1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                asks_the_owner("The parser first? QUESTIONWORDS")
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the parser.");
    say(&mut e, "And then?");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, first)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let next = message_at(&e, first, "H1", true);
    assert!(
        next.contains("architect to owner: The parser first? QUESTIONWORDS"),
        "{next}"
    );
    let ask: i64 =
        e.db.conn()
            .query_row("SELECT id FROM owner_asks WHERE answer IS NULL", [], |r| r.get(0))
            .unwrap();
    api::reply_to_question(&mut e.db, "p", ask, "Yes. ANSWERWORDS", &[], db::Speaker::Owner).unwrap();
    let again = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, again)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let base = base_session(&e, first);
    assert_eq!(
        sessions_of(&e, again),
        [("matter".to_string(), "fork".to_string(), Some(base), false, true)],
        "{log}"
    );
    let answered = message_at(&e, again, "H1", false);
    assert!(answered.contains("The parser first? QUESTIONWORDS"), "{answered}");
    assert!(answered.contains("The owner answered: Yes. ANSWERWORDS"), "{answered}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A new base reads the project afresh: its understanding is given neither the understanding before, nor the plan, nor
// what matters added, only what the owner said; its plan is given the plan before and all that came since, and hands in
// the whole plan. What came since the base is what matters forked from it read: the new base clears it.
#[test]
fn a_new_base_writes_its_understanding_from_nothing_and_its_plan_takes_in_what_came() {
    let s = setup("notebook-new-base");
    let mut agents = scripts();
    agents.insert(
        "H1",
        Box::new(|_: &Request, _: usize| {
            let lines = json!({ "plan": "The parser comes first.", "understanding": "The parser moved." });
            (Vec::new(), vec![("outcome.notebook", lines)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the parser. FIRSTWORDS");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, first)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    a_new_policy_takes_effect(&mut e);
    say(&mut e, "Look again. SECONDWORDS");
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, second)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, second),
        ["N0", "B1", "B2", "N0", "N2", "H1", "C1", "N0", "R1"],
        "{log}"
    );
    let understanding = message_at(&e, second, "B1", false);
    assert!(
        !understanding.contains("The understanding at round 1."),
        "{understanding}"
    );
    assert!(!understanding.contains("The plan at round 1."), "{understanding}");
    assert!(!understanding.contains("The parser moved."), "{understanding}");
    assert!(understanding.contains("SECONDWORDS"), "{understanding}");
    let plan = message_at(&e, second, "B2", false);
    assert!(plan.contains("The plan at round 1."), "{plan}");
    assert!(plan.contains("It adds to the plan: The parser comes first."), "{plan}");
    assert!(
        plan.contains("It adds to the understanding: The parser moved."),
        "{plan}"
    );
    assert_eq!(
        notebook(&e),
        [
            ("plan".to_string(), "The plan at round 2.".to_string(), "B2".to_string()),
            (
                "project_now".to_string(),
                "The understanding at round 2.".to_string(),
                "B1".to_string()
            ),
        ],
        "{log}"
    );
    let answer = message_at(&e, second, "H1", false);
    assert!(
        !answer.contains("It adds to the plan: The parser comes first."),
        "{answer}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
