//! The architect's notebook, and the sections reading the project and its plan hand in.

use super::*;

// A round of reading the project that asks for tools writes nothing in the notebook: until the cell answers, the
// project's notebook is empty, as at its architect's first base, and the cell's next round is referred to the task it
// was given.
// The cell's own inputs that have nothing are listed as none; the tool results, which every cell may have, only once
// there are some.
#[test]
fn a_round_of_the_understanding_that_asks_for_tools_changes_neither_the_notebook_nor_the_task() {
    let s = setup("understand-tools");
    let mut agents = scripts();
    agents.insert(
        "U1",
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
                .any(|c| c.0 == "U1" && c.1.as_deref() == Some(row))
        }
    };
    let states = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM notebook WHERE project = 'p'", [], |r| r.get(0))
            .unwrap()
    };
    assert!(
        steps_until(&mut e, 10, took("tools")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(states(&e), 0, "{}", said.lock().unwrap().join("\n"));
    assert!(
        steps_until(&mut e, 10, took("to_plan")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(states(&e), 2);
    let (first, second) = (message_at(&e, arch, "U1", false), message_at(&e, arch, "U1", true));
    // The first round is given the task of a planning; the round after its tools is told it holds it.
    let task = "What to do next is planned in this session next";
    assert!(first.contains(task), "{first}");
    assert!(!second.contains(task), "{second}");
    assert!(
        second.contains("task:U1·rebuild_for=plan, given to you before in R"),
        "{second}"
    );
    let w = e.work(arch).unwrap();
    let g = e.graph_of(&w).unwrap();
    let numbered: Vec<&flow::CompiledInput> = g
        .cell("U1")
        .unwrap()
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == flow::format::Src::Itself))
        .collect();
    let number = |of: &dyn Fn(&flow::CompiledInput) -> bool| numbered.iter().position(|x| of(x)).unwrap() + 1;
    let before = number(&|x| x.from == [flow::format::Src::Engine("notebook_unclear".into())]);
    let tools = number(&|x| x.name.as_deref() == Some("Tool results"));
    let listed = |m: &str, n: usize| work_from(m).into_iter().find(|l| l.starts_with(&format!("{n}. ")));
    assert_eq!(listed(&first, before), Some(format!("{before}. none")), "{first}");
    assert_eq!(listed(&first, tools), None, "{first}");
    assert!(listed(&second, tools).is_some_and(|l| l.contains("·X1·")), "{second}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The understanding rewrites the notebook's sections on what the project is now and on what is unclear, and the plan
// its plan: a return of the understanding that rewrites only one of its two, and one of the plan that leaves the plan
// as it was, are sent back saying which section to write; handed in again whole, they are the notebook's.
#[test]
fn a_return_of_the_understanding_or_the_plan_without_its_sections_is_sent_back() {
    let s = setup("notebook-written");
    let mut agents = scripts();
    agents.insert(
        "U1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edits = json!([{ "section": "project_now", "text": "One line of work: the parser." }]);
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    agents.insert(
        "N1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edits = json!([{ "section": "other", "text": "Nothing else." }]);
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let understanding = why_sent_back(&e, arch, "U1");
    assert_eq!(understanding.len(), 1, "{understanding:?}\n{log}");
    assert!(
        understanding[0].contains("rewrite the notebook's section unclear whole in notebook"),
        "{understanding:?}"
    );
    let plan = why_sent_back(&e, arch, "N1");
    assert_eq!(plan.len(), 1, "{plan:?}\n{log}");
    assert!(
        plan[0].contains("rewrite the notebook's section plan whole in notebook"),
        "{plan:?}"
    );
    let sections: Vec<(String, String)> = notebook(&e).into_iter().map(|x| (x.0, x.2)).collect();
    assert_eq!(
        sections,
        [
            ("plan".to_string(), "N1".to_string()),
            ("project_now".to_string(), "U1".to_string()),
            ("unclear".to_string(), "U1".to_string())
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A section of the notebook holds 4,000 characters at most and the four 12,000 in all, the sections a round leaves as
// they are counted as they stand, and a return names each section once: a return past them is sent back saying which
// section and how many characters, and nothing of it is written.
#[test]
fn a_notebook_past_its_size_is_sent_back_saying_which_section() {
    let s = setup("notebook-size");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let long = |k: usize| "x".repeat(k);
            let edits = match n {
                1 => json!([{ "section": "plan", "text": long(4_001) }, { "section": "plan", "text": "Short." }]),
                2 => json!([
                    { "section": "project_now", "text": "Now." },
                    { "section": "plan", "text": long(4_000) },
                    { "section": "unclear", "text": long(4_000) },
                    { "section": "other", "text": long(4_000) }
                ]),
                _ => json!([{ "section": "other", "text": "Kept." }]),
            };
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let back = why_sent_back(&e, arch, "H8");
    assert_eq!(back.len(), 2, "{back:?}\n{log}");
    assert!(
        back[0].contains("the notebook's section plan is 4001 characters, and a section holds 4000 at most"),
        "{back:?}"
    );
    assert!(
        back[0].contains("the notebook's section plan is rewritten twice"),
        "{back:?}"
    );
    let now = "Now.".chars().count();
    assert!(
        back[1].contains(&format!(
            "the notebook would be {} characters in all, and it holds 12000 at most",
            12_000 + now
        )),
        "{back:?}"
    );
    let kept: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM notebook_edits WHERE project = 'p' AND length(text) >= 4000",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(kept, 0);
    assert!(
        notebook(&e).contains(&("other".to_string(), "Kept.".to_string(), "H8".to_string())),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A section of the notebook a cell rewrites is the notebook's latest version of it at once, every version kept; each
// section is an input of its own of the architect's cells, so that a session is given a section that changed whole and
// one it holds already only by reference. The answer to the owner's words, with no base yet, keeps something in the
// other section, which reading the project for the base is given whole; the check of the policy, in a session forked
// from the base, holds every section the base was given, and is referred to each.
#[test]
fn the_notebook_is_written_where_it_is_learned_and_given_whole_once_changed() {
    let s = setup("notebook-given");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let edits = json!([{ "section": "other", "text": "The owner looks at the parser first." }]);
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the parser first.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    assert_eq!(path.iter().filter(|c| *c == "N1").count(), 1, "{path:?}\n{log}");
    assert_eq!(
        notebook(&e),
        [
            (
                "other".to_string(),
                "The owner looks at the parser first.".to_string(),
                "H8".to_string()
            ),
            ("plan".to_string(), "The plan at round 1.".to_string(), "N1".to_string()),
            (
                "project_now".to_string(),
                "The project_now at round 1.".to_string(),
                "U1".to_string()
            ),
            (
                "unclear".to_string(),
                "The unclear at round 1.".to_string(),
                "U1".to_string()
            )
        ],
        "{log}"
    );
    // The answer to the owner's words came before anything was in the notebook.
    let answer = input_at(&e, arch, "H8", false);
    assert_eq!(given_ending(&answer, "·engine·notebook_project_now"), None, "{answer}");
    assert_eq!(given_ending(&answer, "·engine·notebook_other"), None, "{answer}");
    // Reading the project is given the other section whole.
    let read = input_at(&e, arch, "U1", false);
    assert_eq!(given_ending(&read, "·engine·notebook_other"), Some(true), "{read}");
    assert!(
        read["message"]
            .as_str()
            .unwrap()
            .contains("The owner looks at the parser first."),
        "{read}"
    );
    // The check of the policy, forked from the base, is referred to every section the base was given, and given whole
    // the plan, which the base wrote and was never given.
    let check = input_at(&e, arch, "P1", false);
    for (section, whole) in [("project_now", false), ("unclear", false), ("plan", true), ("other", false)] {
        assert_eq!(
            given_ending(&check, &format!("·engine·notebook_{section}")),
            Some(whole),
            "{section}: {check}"
        );
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// Reading the project anew for a new base reads it afresh: it is not given the notebook's section on what the project
// is now, which it writes anew, and is given what is unclear, as questions to settle.
#[test]
fn reading_the_project_anew_is_given_what_is_unclear_and_not_what_the_project_is_now() {
    let s = setup("notebook-new-base");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, first)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // Slots free with nothing of the project to start call for a planning, which reads the project anew.
    e.db.write(Actor::Engine, |tx| {
        db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "slots_free".into(),
                what: "1 slot is free, and no item of p is ready to start".into(),
                body: json!({ "free": 1, "ready": [] }),
                ..db::Matter::default()
            },
        )
        .map(|_| ())
    })
    .unwrap();
    let second = e.step(None).unwrap().work.unwrap();
    assert_ne!(second, first);
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, second)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(path_of(&e, second).iter().filter(|c| *c == "U1").count(), 1, "{log}");
    let again = input_at(&e, second, "U1", false);
    assert_eq!(given_ending(&again, "·engine·notebook_project_now"), None, "{again}");
    assert_eq!(given_ending(&again, "·engine·notebook_unclear"), Some(true), "{again}");
    let m = again["message"].as_str().unwrap();
    assert!(m.contains("The unclear at round 1."), "{m}");
    assert!(!m.contains("The project_now at round 1."), "{m}");
    let now = notebook(&e).into_iter().find(|x| x.0 == "project_now").unwrap();
    assert_eq!(now.1, "The project_now at round 2.");
    let _ = std::fs::remove_dir_all(&s.root);
}
