//! The architect's notebook, and the sections its understanding and its plan hand in.

use super::*;

// A round of the understanding that asks for tools writes nothing in the notebook: until the cell answers, the project's
// notebook is empty, as at its architect's first work, and the cell's next round is still told it is the first work.
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
        steps_until(&mut e, 10, took("understood")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(states(&e), 2);
    let (first, second) = (message_at(&e, arch, "U1", false), message_at(&e, arch, "U1", true));
    // The first round is given the task of a first work; the round after its tools is told it holds it.
    assert!(first.contains("This is your first work on the project"), "{first}");
    assert!(!second.contains("This is your first work on the project"), "{second}");
    assert!(
        second.contains("task:U1·entry=first, given to you before in R"),
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
    // What the project is now, as the understanding wrote it, counts as it stands.
    let now = "The project_now at round 1.".chars().count();
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
// section is an input of its own of the architect's cells, so that the next cell's session is given a section that
// changed whole and one that did not only by reference. The answer to the owner's words keeps something in the other
// section, which the check of the policy after it is given whole while it is referred to what the project is now; the
// plan, made again after a batch, is given the plan it made before, and the plan is given whole to the first cell of
// the batch's end that reads it.
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
    agents.insert(
        "B4",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                (vec![("outcome.rethink", "re_plan")], Vec::new())
            } else {
                (Vec::new(), Vec::new())
            }
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
    assert_eq!(path.iter().filter(|c| *c == "N1").count(), 2, "{path:?}\n{log}");
    assert_eq!(
        notebook(&e),
        [
            (
                "other".to_string(),
                "The owner looks at the parser first.".to_string(),
                "H8".to_string()
            ),
            ("plan".to_string(), "The plan at round 2.".to_string(), "N1".to_string()),
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
    let plans: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM notebook_edits WHERE project = 'p' AND section = 'plan'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(plans, 2);
    // The understanding wrote what the project is now: the answer to the owner's words, the next cell, is given it
    // whole, and has nothing in the other section yet.
    let answer = input_at(&e, arch, "H8", false);
    assert_eq!(
        given_ending(&answer, "·engine·notebook_project_now"),
        Some(true),
        "{answer}"
    );
    assert_eq!(given_ending(&answer, "·engine·notebook_other"), None, "{answer}");
    // The check of the policy after it is referred to what did not change and given the other section whole.
    let check = input_at(&e, arch, "P1", false);
    assert_eq!(
        given_ending(&check, "·engine·notebook_project_now"),
        Some(false),
        "{check}"
    );
    assert_eq!(given_ending(&check, "·engine·notebook_unclear"), Some(false), "{check}");
    assert_eq!(given_ending(&check, "·engine·notebook_other"), Some(true), "{check}");
    assert!(
        check["message"]
            .as_str()
            .unwrap()
            .contains("The owner looks at the parser first."),
        "{check}"
    );
    // The plan made again is given the plan before, which the first cell after the batch's end that reads it, the
    // recording of the decisions, was given whole, and the rethink after that only by reference.
    let (first, again) = (input_at(&e, arch, "N1", false), input_at(&e, arch, "N1", true));
    assert_eq!(given_ending(&first, "·engine·notebook_plan"), None, "{first}");
    let record = input_at(&e, arch, "B5", false);
    assert_eq!(given_ending(&record, "·engine·notebook_plan"), Some(true), "{record}");
    assert!(
        record["message"].as_str().unwrap().contains("The plan at round 1."),
        "{record}"
    );
    let rethink = input_at(&e, arch, "B4", false);
    assert_eq!(
        given_ending(&rethink, "·engine·notebook_plan"),
        Some(false),
        "{rethink}"
    );
    assert_eq!(given_ending(&again, "·engine·notebook_plan"), Some(false), "{again}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The understanding of a new session understands the project afresh: it is not given the notebook's section on what
// the project is now, which it writes anew, and is given what is unclear, as questions to settle.
#[test]
fn a_new_session_s_understanding_is_given_what_is_unclear_and_not_what_the_project_is_now() {
    let s = setup("notebook-new-session");
    understanding_each_session(&s);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| e.work(arch).unwrap().pos.at == "H8"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    e.sessions().ask_new(api::Fresh::Architect("p".into()));
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(path_of(&e, arch).iter().filter(|c| *c == "U1").count(), 2, "{log}");
    let again = input_at(&e, arch, "U1", true);
    assert_eq!(given_ending(&again, "·engine·notebook_project_now"), None, "{again}");
    assert_eq!(given_ending(&again, "·engine·notebook_unclear"), Some(true), "{again}");
    let m = again["message"].as_str().unwrap();
    assert!(m.contains("The unclear at round 1."), "{m}");
    assert!(!m.contains("The project_now at round 1."), "{m}");
    let now = notebook(&e).into_iter().find(|x| x.0 == "project_now").unwrap();
    assert_eq!(now.1, "The project_now at round 2.");
    let _ = std::fs::remove_dir_all(&s.root);
}
