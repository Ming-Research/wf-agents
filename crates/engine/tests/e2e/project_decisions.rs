//! The project's decisions in force: the input that gives them as the project's main has them, and the tool that gives
//! the history of one.

use super::*;

/// The decisions project p's main holds in these tests: a README, which is none, and one decision, rewritten once.
const README: &str = "What this directory holds: no decision.\n";
const FIRST: &str = "# One flow\n\nKeep one flow for some projects.\n";
const NOW: &str = "# One flow\n\nKeep one flow for every project.\n\nNot chosen: a flow for each project.\n";

/// Puts the decisions on main, made and then rewritten, each by a commit of its own.
fn decided(s: &Setup) {
    main_moves_on(
        &s.root,
        &[("decisions/README.md", README), ("decisions/one-flow.md", FIRST)],
        "Decide on one flow",
    );
    main_moves_on(
        &s.root,
        &[("decisions/one-flow.md", NOW)],
        "Keep one flow for every project\n\nThe owner chose it for all of them.",
    );
}

/// The flow as `flow/flow.json` has it, with the cells named (each by its graph and its id) given the project's
/// decisions, and those of them that may ask for tools the tool decision_history, where the flow does not give them
/// already: compiled and stored as a version of the flow, which the engine then runs.
fn with_decisions(e: &mut Engine, cells: &[(&str, &str)]) -> i64 {
    let mut f: Value = serde_json::from_str(flow::FLOW).unwrap();
    if f["engine"]["inputs"].get("decisions").is_none() {
        f["engine"]["inputs"]["decisions"] = json!("The project's decisions in force, as its main has them.");
    }
    for (graph, id) in cells {
        let g = f["flows"]
            .as_array_mut()
            .and_then(|gs| gs.iter_mut().find(|g| g["name"] == *graph))
            .unwrap();
        let cell = g["nodes"]
            .as_array_mut()
            .and_then(|ns| ns.iter_mut().find(|n| n["id"] == *id))
            .unwrap();
        let inputs = cell["inputs"].as_array_mut().unwrap();
        let source = json!({ "engine": "decisions" });
        if !inputs
            .iter()
            .any(|x| x["from"].as_array().is_some_and(|f| f.contains(&source)))
        {
            inputs.push(json!({
                "name": "The project's decisions",
                "what": "The decisions in force, each file by its path with its text; none when there are none",
                "from": [source],
                "maybe": true
            }));
        }
        if let Some(tools) = cell["tools"].as_array_mut()
            && !tools.contains(&json!("decision_history"))
        {
            tools.push(json!("decision_history"));
        }
    }
    let cx = flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    };
    let compiled = flow::compile(&f.to_string(), &cx).unwrap_or_else(|x| panic!("{x}"));
    let text = compiled.json();
    let v =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (digest, compiled) VALUES ('with decisions', ?1)",
                [&text],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    e.flow_now = v;
    v
}

// A cell of an engineer's work given the decisions is given none while main's directory holds only its README, and
// then each file of the directory as main has it when the round starts, by its path, the README left out; asking for
// one's history, it is given each version main had, newest first, with its commit's message and its text, in a return
// of the tool cell that fits the flow.
#[test]
fn an_engineer_s_cell_is_given_the_decisions_main_has_and_on_asking_one_s_history() {
    let s = setup("decisions-engineer");
    main_moves_on(&s.root, &[("decisions/README.md", README)], "Say what decisions are");
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                let tools = json!([{ "tool": "decision_history", "path": "decisions/one-flow.md" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            implement(r, n - 1)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    with_decisions(&mut e, &[("engineer", "R1"), ("engineer", "I1")]);
    // Main takes the decisions once research is done, before implementing: the round's fetch brings them.
    step_to(&mut e, item, "D1");
    decided(&s);
    let ran = |e: &Engine| -> Option<String> {
        e.db.conn()
            .query_row(
                "SELECT r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'X1' AND r.transition IS NOT NULL ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| r.get(0),
            )
            .ok()
    };
    for _ in 0..60 {
        if ran(&e).is_some() || state(&e, item).is_some_and(|(_, _, paused)| paused.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let research = message_at(&e, work, "R1", false);
    assert!(!research.contains("engine·decisions"), "{research}");
    let message = message_at(&e, work, "I1", false);
    assert_eq!(
        given_value(&message, "·engine·decisions"),
        Some(json!({ "decisions/one-flow.md": NOW })),
        "{message}"
    );
    // The history: the rewrite, then the first version, each with its commit on main.
    let output: Value = serde_json::from_str(&ran(&e).expect(&log)).unwrap();
    let result = &output["results"][0];
    assert_eq!(result["tool"], "decision_history", "{output}");
    let history = &result["history"];
    assert_eq!(history["verdict"], "found", "{output}");
    let versions = history["versions"].as_array().unwrap();
    let main = |at: &str| git(&s.origin, &["rev-parse", at]);
    let seen: Vec<(&str, &str, &str, &str)> = versions
        .iter()
        .map(|v| {
            (
                v["commit"].as_str().unwrap(),
                v["change"].as_str().unwrap(),
                v["message"].as_str().unwrap(),
                v["text"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                main("main").as_str(),
                "changed",
                "Keep one flow for every project\n\nThe owner chose it for all of them.",
                NOW
            ),
            (main("main~1").as_str(), "added", "Decide on one flow", FIRST),
        ],
        "{output}"
    );
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let schema = g.cell("X1").and_then(|c| c.schema.clone()).unwrap();
    assert!(flow::valid(&schema, &output), "{output}");
    // The attempt is recorded as every tool's is.
    assert_eq!(tool_runs(&e, item, "decision_history").len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cell of a project's architect given the decisions is given them as the project's main has them, the README left
// out, as an engineer's cell is.
#[test]
fn an_architect_s_cell_is_given_the_decisions_main_has() {
    let s = setup("decisions-architect");
    no_reply_wait(&s);
    decided(&s);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    with_decisions(&mut e, &[("architect", "U1")]);
    let arch = architect_work_at(&mut e, "U1");
    let ran = |e: &Engine| -> bool {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'U1'",
                [arch],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0
    };
    for _ in 0..4 {
        if ran(&e) {
            break;
        }
        e.advance(arch).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert!(ran(&e), "{log}");
    let message = message_at(&e, arch, "U1", false);
    assert_eq!(
        given_value(&message, "·engine·decisions"),
        Some(json!({ "decisions/one-flow.md": NOW })),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
