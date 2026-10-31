use std::collections::BTreeMap;

use serde_json::{Map, Value};

const FLOW: &str = include_str!("../../../flow/flow.json");

fn context() -> flow::Context {
    flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    }
}

fn compiled() -> flow::Compiled {
    flow::compile(FLOW, &context()).unwrap_or_else(|e| panic!("{e}"))
}

// The engineer's graph compiles with no finding, and comes out as the generator the flow was designed with worked
// it out: as many cells, rows, states and places.
#[test]
fn the_engineer_graph_compiles_as_its_reference_has_it() {
    let c = compiled();
    let r: Value = serde_json::from_str(include_str!("reference.json")).unwrap();
    let s = &c.stats[0];
    let want = |k: &str| r[k].as_u64().unwrap() as usize;
    assert_eq!(
        (s.cells, s.rows, s.states, s.places),
        (want("cells"), want("rows"), want("states"), want("places"))
    );
}

// The fix, the one cell with a table it must answer, has a way on when the table has no row, which the engine takes in
// place of its round: on to choose what runs next, where its answer goes.
#[test]
fn a_table_to_answer_with_no_row_goes_on_past_the_round() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let tables: Vec<&str> = g
        .cells
        .iter()
        .filter(|c| c.inputs.iter().any(|x| x.numbered.is_some() && !x.maybe))
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(tables, ["T6"]);
    let t6 = g.cell("T6").unwrap();
    let row = t6.rows.iter().find(|r| r.name == "nothing_to_answer").unwrap();
    assert_eq!(row.to, "T7");
    assert_eq!(
        serde_json::to_value(&row.conditions).unwrap(),
        serde_json::json!([["engine", "nothing_to_answer"]])
    );
    assert!(!row.common && !row.fallback && row.counts.is_none());
    assert_eq!(t6.rows.iter().find(|r| r.name == "done").unwrap().to, "T7");
}

// The architect's graph compiles with no finding, as the design the owner approved: where it waits for a work to start
// and the start of its work, which only a new session understands the project at (`S0`, `S1`, `U1`), the queue (`Q0`)
// and each matter's path, the check of the policy, the writing of a new one and its taking effect (`P1`, `P2`, `P12`),
// the plan of what to do next (`N1`), the review of a batch of item changes and whether to plan or check again (`B1`,
// `B2`, `B3`, `B4`), the end (`E2`), the owner's ask that holds only its matter (`A3`), the tools (`X1`), the observer
// (`X2`) and the wait for the owner's Start after a pause (`X3`): as many cells, rows and states. No wrap-up, no wait
// for the owner's reply in a talk, and none of the setting of a policy as a stage of its own (its study, its readings by
// new architects, the owner's buttons).
// The owner's words reach the architect only through its queue: no cell of it takes an interjection, and none answers
// one.
#[test]
fn the_architect_graph_compiles_as_its_reference_has_it() {
    let c = compiled();
    let s = c.stats.iter().find(|s| s.graph == "architect").unwrap();
    assert_eq!((s.cells, s.rows, s.states), (30, 120, 164));
    let g = c.graph("architect").unwrap();
    for gone in ["E1", "H11", "P3", "P4", "P5", "P6", "P7", "P8", "P9", "P10", "P11"] {
        assert!(g.cell(gone).is_none(), "{gone}");
    }
    // Only a new session understands the project: no row but the start's goes to the understanding.
    let into = |cell: &str| -> Vec<&str> {
        g.cells
            .iter()
            .filter(|x| x.rows.iter().any(|r| r.to == cell && !r.common))
            .map(|x| x.id.as_str())
            .collect()
    };
    assert_eq!(into("U1"), ["S1"]);
    assert!(g.cell("X4").is_none());
    assert!(
        g.cells
            .iter()
            .all(|x| x.rows.iter().all(|r| !r.interject && r.name != "answers"))
    );
    assert!(c.graph("policy").is_none());
}

// Every agent's cell of the architect's line but the review of a batch and the observer's search may rewrite sections
// of the notebook in its own answer, beside its effects, the edit made at once and its size checked; the understanding
// and the plan must, each its own sections. Each such cell is given the four sections, each an engine input of its own,
// but the understanding, which writes what the project is now afresh and is given only what is unclear, as questions
// to settle, and the other section; no cell reads the state of things, the notes, the log or the last plan, which the
// notebook took the place of.
#[test]
fn every_cell_of_the_architect_s_line_keeps_the_notebook() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let nodes = v["flows"][1]["nodes"].as_array().unwrap();
    let sections = ["project_now", "plan", "unclear", "other"];
    let mut keep = Vec::new();
    for cell in &g.cells {
        if cell.kind != "agent" || cell.line.as_deref() != Some("architect") {
            continue;
        }
        keep.push(cell.id.as_str());
        assert!(
            cell.always
                .iter()
                .any(|a| a.at == "notebook" && a.action.as_deref() == Some("edit_notebook")),
            "{}",
            cell.id
        );
        assert!(
            cell.checks
                .iter()
                .any(|x| x.at.as_deref() == Some("notebook") && x.check == "notebook_fits"),
            "{}",
            cell.id
        );
        let given: Vec<&str> = sections
            .iter()
            .copied()
            .filter(|s| {
                let name = format!("notebook_{s}");
                cell.inputs
                    .iter()
                    .any(|x| x.from == [flow::format::Src::Engine(name.clone())])
            })
            .collect();
        let want: &[&str] = if cell.id == "U1" { &sections[2..] } else { &sections };
        assert_eq!(given, want, "{}", cell.id);
        let node = nodes.iter().find(|n| n["id"] == cell.id.as_str()).unwrap();
        let field = node["out"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["$ref"] == "notebook")
            .unwrap();
        let must = ["U1", "N1"].contains(&cell.id.as_str());
        assert_eq!(field.get("optional") == Some(&Value::Bool(false)), must, "{}", cell.id);
        for gone in [
            "architect_state",
            "architect_notes",
            "architect_log",
            "architect_last_fit",
        ] {
            assert!(
                cell.inputs.iter().all(|x| x
                    .from
                    .iter()
                    .all(|s| !matches!(s, flow::format::Src::Db { relation, .. } if relation == gone))),
                "{} {gone}",
                cell.id
            );
        }
    }
    assert_eq!(
        keep,
        [
            "U1", "P1", "N1", "P2", "Q1", "H1", "H2", "H3", "H4", "H5", "H6", "H7", "H8", "H9", "H10", "H12", "B3",
            "B4"
        ]
    );
    for (cell, check) in [("U1", "understanding_written"), ("N1", "plan_written")] {
        assert!(
            g.cell(cell)
                .unwrap()
                .checks
                .iter()
                .any(|x| x.at.as_deref() == Some("notebook") && x.check == check),
            "{cell}"
        );
    }
    for cell in ["B2", "X2"] {
        assert!(
            g.cell(cell).unwrap().always.iter().all(|a| a.at != "notebook"),
            "{cell}"
        );
    }
    assert_eq!(g.cell("N1").unwrap().title, "The plan");
}

// The check of the policy and the plan of what to do next are matters at the head of the queue: a new session's
// understanding queues both and goes on to the queue; the look at the queue takes them after the owner's words, a
// matter back with the owner's reply and the wait for a question just put, and before the others, without a pick
// (`head`), and the router sends each to its cell. The check closes it when the policy holds, asks the owner in doubt,
// and goes on to write a new policy by the owner's reply, or by the owner's words that asked for it (`write_policy`,
// which the engine refuses otherwise); the writing asks the owner to approve it,
// which parks the check there, and the router sends it back to the writing with any other reply, and with approved to
// where it takes effect, which queues the plan and closes the check. The plan closes its matter. Whoever wants a check
// or a plan queues it: no row of a batch's rethink, a talk or the taking effect goes to the check or the plan. An
// owner's cell of the architect's graph with no row that goes on without the answer is refused, since the architect
// never stops for the owner.
#[test]
fn the_check_of_the_policy_and_the_plan_are_matters_at_the_head_of_the_queue() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let row = |cell: &str, name: &str| {
        g.cell(cell)
            .unwrap()
            .rows
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{cell} {name}"))
            .clone()
    };
    let does = |r: &flow::CompiledRow| r.actions.iter().map(|d| d.name().to_string()).collect::<Vec<_>>();
    let way = |r: &flow::CompiledRow| (r.to.clone(), does(r));
    let to = |to: &str, then: &[&str]| (to.to_string(), then.iter().map(|x| x.to_string()).collect::<Vec<_>>());
    assert_eq!(
        way(&row("U1", "understood")),
        to("Q0", &["queue_check_policy", "queue_make_plan"])
    );
    let q0: Vec<&str> = g.cell("Q0").unwrap().rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(q0[..4], ["owner_words", "answered", "waits", "head"]);
    assert_eq!(row("Q0", "head").to, "Q2");
    let given = |v: &str| Map::from_iter([("route".to_string(), Value::String(v.into()))]);
    for at in ["P1", "P2", "P12", "N1"] {
        let r = g.take("Q2", &serde_json::json!({}), &given(at)).unwrap();
        assert_eq!(r.to, at);
    }
    assert_eq!(way(&row("P1", "holds")), to("B1", &["close_matter"]));
    assert_eq!(way(&row("P1", "write_policy")), to("P2", &[]));
    assert_eq!(row("P1", "ask_owner").to, "A3");
    // It writes a new policy only once the owner said yes, by the reply or by the words that asked for the check.
    assert!(
        g.cell("P1")
            .unwrap()
            .checks
            .iter()
            .any(|x| x.at.as_deref() == Some("verdict") && x.check == "asked_before_writing")
    );
    // The new measures a draft's signals need are queued once it takes effect, from the draft approved.
    assert_eq!(way(&row("P2", "written")), to("B1", &["ask_approval"]));
    assert_eq!(
        way(&row("P12", "next")),
        to("B1", &["queue_measures", "queue_make_plan", "close_matter"])
    );
    for name in ["fits", "some_to_change", "set_straight"] {
        let r = row("N1", name);
        assert_eq!(
            (r.to.as_str(), does(&r).last().map(String::as_str)),
            ("B1", Some("close_matter"))
        );
    }
    assert_eq!(way(&row("B4", "go_on")), to("Q0", &["queue_group_talk"]));
    assert_eq!(
        way(&row("B4", "re_plan")),
        to("Q0", &["queue_make_plan", "queue_group_talk"])
    );
    assert_eq!(
        way(&row("B4", "check_policy")),
        to("Q0", &["queue_check_policy", "queue_group_talk"])
    );
    // A batch with nothing to rethink goes back to the queue with no round of the rethink, still at a batch's end.
    assert_eq!(
        way(&row("B1", "nothing")),
        to("Q0", &["settle_straight_hold", "queue_group_talk"])
    );
    let into = |cell: &str| -> Vec<&str> {
        g.cells
            .iter()
            .filter(|x| x.rows.iter().any(|r| r.to == cell && !r.common))
            .map(|x| x.id.as_str())
            .collect()
    };
    assert_eq!(into("P1"), ["Q2"]);
    assert_eq!(into("N1"), ["Q2"]);
    assert_eq!(into("P2"), ["P1", "Q2"]);
    assert_eq!(into("P12"), ["Q2"]);
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let ask = v["flows"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "A3")
        .unwrap();
    // Back to the cell that asked with the answer, and no row that goes on without it.
    ask["cases"][0].as_object_mut().unwrap().remove("drops");
    ask["cases"][0]["to"] = Value::String("@asker".into());
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(
        e.contains("the architect's own graph stops: A3 goes on only with the owner's answer"),
        "{e}"
    );
}

// The owner's words are answered in one round, as the owner decided: the look at the queue takes them first, without a
// pick (`Q0`); the cell of the owner's words (`H8`) replies and takes one way on, back to the batch (`B1`): done; the
// plan of what to do next made again, or the policy checked, each a matter it queues at the head of the queue. It may
// ask the owner, whose reply comes back to it; no cell waits for the owner's reply in a talk (`H11`) any longer. A row
// that waits in the architect's graph with no setting bounding the wait is refused, since the architect stops for no
// one.
#[test]
fn the_owner_s_words_are_answered_in_one_round() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let row = |cell: &str, name: &str| {
        g.cell(cell)
            .unwrap()
            .rows
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{cell} {name}"))
            .clone()
    };
    let does = |r: &flow::CompiledRow| r.actions.iter().map(|d| d.name().to_string()).collect::<Vec<_>>();
    let none = Map::new();
    let first = g
        .take(
            "Q0",
            &serde_json::json!({ "queue": { "verdict": "owner_words", "matter": 3 } }),
            &none,
        )
        .unwrap();
    assert_eq!((first.name.as_str(), first.to.as_str()), ("owner_words", "Q2"));
    // The owner's words come first, then a matter back with the owner's reply; a question just put waits for the
    // reply, no longer than the installation's wait.
    let q0: Vec<&str> = g.cell("Q0").unwrap().rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(q0[..3], ["owner_words", "answered", "waits"]);
    let wait = g
        .take(
            "Q0",
            &serde_json::json!({ "queue": { "verdict": "waits", "question": 4 } }),
            &none,
        )
        .unwrap();
    assert_eq!((wait.to.as_str(), wait.waits), ("Q0", true));
    assert!(g.cell("H11").is_none());
    for (name, to, then) in [
        ("done", "B1", vec!["close_matter"]),
        ("re_plan", "B1", vec!["queue_make_plan", "close_matter"]),
        ("new_policy", "B1", vec!["queue_check_policy", "close_matter"]),
    ] {
        let r = row("H8", name);
        assert_eq!(
            (r.to.as_str(), does(&r)),
            (to, then.iter().map(|x| x.to_string()).collect()),
            "{name}"
        );
    }
    assert_eq!(row("H8", "ask_owner").to, "A3");
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let look = v["flows"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "Q0")
        .unwrap();
    let waits = look["cases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["name"] == "waits")
        .unwrap();
    waits.as_object_mut().unwrap().remove("until");
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(e.contains("the architect's own graph stops: Q0 waits"), "{e}");
}

// What was said in the works' groups, other than to the architect, is a matter of its own (`group_talk`), which the
// cell the architect waits at to start queues (`S0`'s `queue_group_talk`), and each row out of the rethink that ends
// each batch while a work runs (`B4`), and the router sends to its cell (`H12`): it
// may do nothing more, interject in its effects, plan again or check the policy, each a matter it queues at the head of
// the queue, back to the batch; the pick is given it no longer.
#[test]
fn what_was_said_in_the_groups_is_a_matter_of_its_own() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    assert!(g.cell("S0").unwrap().actions.iter().any(|a| a == "queue_group_talk"));
    let given = Map::from_iter([("route".to_string(), Value::String("H12".into()))]);
    assert_eq!(g.take("Q2", &serde_json::json!({}), &given).unwrap().to, "H12");
    let does = |name: &str| {
        let r = g
            .cell("H12")
            .unwrap()
            .rows
            .iter()
            .find(|r| r.name == name)
            .unwrap()
            .clone();
        (
            r.to.clone(),
            r.actions.iter().map(|d| d.name().to_string()).collect::<Vec<_>>(),
        )
    };
    for (name, then) in [
        ("done", vec!["close_matter"]),
        ("re_plan", vec!["queue_make_plan", "close_matter"]),
        ("new_policy", vec!["queue_check_policy", "close_matter"]),
    ] {
        assert_eq!(
            does(name),
            ("B1".to_string(), then.iter().map(|x| x.to_string()).collect()),
            "{name}"
        );
    }
    let reads = |cell: &str| {
        g.cell(cell).unwrap().inputs.iter().any(|x| {
            x.from
                .iter()
                .any(|s| matches!(s, flow::format::Src::Engine(n) if n == "group_talk"))
        })
    };
    assert!(reads("H12"));
    assert!(!reads("Q1"));
    // Each row from the rethink every batch ends at back to the queue's look queues it too, for the work that runs.
    for name in ["go_on", "re_plan", "check_policy"] {
        let r = g.cell("B4").unwrap().rows.iter().find(|r| r.name == name).unwrap();
        assert_eq!(r.to, "Q0", "{name}");
        assert!(r.actions.iter().any(|d| d.name() == "queue_group_talk"), "{name}");
    }
}

// A part several cells' own answers hold alike, such as the architect's effects, is given once in the forms every
// return shares, and each of those cells' own format refers to it there; a line none of whose cells holds it is not
// given it. A part given once must be in two cells' own answers or more.
#[test]
fn a_part_several_cells_hold_alike_is_given_once() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let straight = flow::formats::cell_format(g, g.cell("H1").unwrap(), &[]);
    assert!(
        straight.contains(r#""effects":{"$ref":"format:common#/$defs/effects"}"#),
        "{straight}"
    );
    assert!(!straight.contains("item_changes"), "{straight}");
    let line = |id: &str| -> Vec<&flow::CompiledCell> {
        let line = &g.cell(id).unwrap().line;
        g.cells
            .iter()
            .filter(|x| x.kind == "agent" && &x.line == line)
            .collect()
    };
    let architect = flow::formats::common_format_of(g, &line("H1"), &[]);
    assert_eq!(architect.matches(r#""item_changes""#).count(), 1, "{architect}");
    let review = flow::formats::common_format_of(g, &line("B2"), &[]);
    assert!(!review.contains(r#""item_changes""#), "{review}");
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    v["flows"][1]["agentCommon"]["returns"]["once"] = serde_json::json!(["settled"]);
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(
        e.contains("a field given once is in fewer than two cells' own answers: settled"),
        "{e}"
    );
}

// The query tool lists every table and view agents read by its name and all its columns, whole, and what it holds by
// the first line of its definition's comment, cut at a word to 80 characters: enough to tell which to read in full.
#[test]
fn the_query_tool_lists_every_table_whole_and_what_it_holds_in_short() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let schema = g.cell("U1").unwrap().schema.clone().unwrap();
    let desc = schema["properties"][&g.within]["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| {
            f["properties"]["tools"]["items"]["oneOf"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .find(|t| t["properties"]["tool"]["const"] == "query")
        .and_then(|t| t["properties"]["tool"]["description"].as_str())
        .unwrap()
        .to_string();
    let index = db::index_of(db::SCHEMA).unwrap();
    assert!(index.iter().any(|(_, _, first)| first.chars().count() > 80));
    for (name, columns, first) in index {
        let head = format!("\n- {name} ({}): ", columns.join(", "));
        let at = desc.find(&head).unwrap_or_else(|| panic!("{head}\n{desc}"));
        let line = desc[at + head.len()..].lines().next().unwrap();
        if first.chars().count() <= 80 {
            assert_eq!(line, first);
        } else {
            assert!(line.ends_with('…') && line.chars().count() <= 80, "{line}");
            assert!(first.starts_with(line.trim_end_matches('…')), "{line}");
        }
    }
}

// Flow feedback is a field of every agent's return that only a project asking for it is asked for: the graph as a
// project that asks for it is asked it is the graph as compiled, and the graph as one that does not is asked it holds
// the field in no agent's schema, does nothing at it, has no role read it and gives no format of it.
#[test]
fn a_field_a_project_does_not_ask_for_is_taken_out_of_every_agent_cell() {
    let c = compiled();
    for name in ["engineer", "architect"] {
        let g = c.graph(name).unwrap();
        assert_eq!(
            g.every_needs.get("flow_feedback").map(String::as_str),
            Some("flow_feedback")
        );
        let asked = g.clone().as_asked(|need| need == "flow_feedback");
        assert_eq!(serde_json::to_value(&asked).unwrap(), serde_json::to_value(g).unwrap());
        let off = g.clone().as_asked(|_| false);
        assert!(off.every_needs.is_empty());
        let field = serde_json::json!("flow_feedback");
        let agents = g.cells.iter().zip(&off.cells).filter(|(x, _)| x.kind == "agent");
        for (on, cell) in agents {
            let s = on.schema.as_ref().unwrap();
            assert!(s["properties"].get("flow_feedback").is_some(), "{}", on.id);
            assert!(s["required"].as_array().unwrap().contains(&field), "{}", on.id);
            assert!(on.always.iter().any(|a| a.at == "flow_feedback"), "{}", on.id);
            let s = cell.schema.as_ref().unwrap();
            assert!(s["properties"].get("flow_feedback").is_none(), "{}", cell.id);
            assert!(!s["required"].as_array().unwrap().contains(&field), "{}", cell.id);
            assert!(cell.always.iter().all(|a| a.at != "flow_feedback"), "{}", cell.id);
            assert!(cell.read_by.iter().all(|x| x.at != "flow_feedback"), "{}", cell.id);
        }
        assert!(flow::formats::common_format(&asked, &[]).contains("flow_feedback"));
        assert!(!flow::formats::common_format(&off, &[]).contains("flow_feedback"));
    }
}

// The rounds section reaches the compiled flow with every kind of failed run, and gives every agent's cell, and only
// an agent's, a shared row back to itself for a return sent back, for so many in a row, and for each kind; a row of
// the flow's own cannot switch on what only those rows do, nor take one of their names.
#[test]
fn the_rounds_section_gives_every_agent_cell_its_shared_rows() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let r = g.rounds.clone().expect("the rounds section");
    assert_eq!(r.sent_back.pause_after, 3);
    let kinds: Vec<&str> = r.failed.kinds.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "usage_limit",
            "offline",
            "transient",
            "cannot_run",
            "prompt_too_long",
            "unparsed"
        ]
    );
    for cell in &g.cells {
        // The fix's own row for a table with no row is an engine's case too, but of that cell alone.
        let shared: Vec<&flow::CompiledRow> = cell
            .rows
            .iter()
            .filter(|x| x.conditions.iter().any(|a| a.about_no_return()))
            .filter(|x| x.name != flow::format::NOTHING_TO_ANSWER)
            .collect();
        if cell.kind != "agent" {
            assert!(shared.is_empty(), "{}", cell.id);
            continue;
        }
        let names: Vec<&str> = shared.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "sent_back_too_often",
                "sent_back",
                "usage_limit",
                "offline",
                "transient",
                "cannot_run",
                "prompt_too_long",
                "unparsed",
                "cut_off",
                "cannot_step",
                "repeats",
                "work_ended"
            ],
            "{}",
            cell.id
        );
        assert!(
            shared
                .iter()
                .all(|x| x.to == "@self" && x.fallback && !x.common && x.counts.is_none())
        );
        let pauses: Vec<&str> = shared.iter().filter(|x| x.pauses).map(|x| x.name.as_str()).collect();
        assert_eq!(
            pauses,
            [
                "sent_back_too_often",
                "prompt_too_long",
                "unparsed",
                "cannot_step",
                "repeats"
            ]
        );
        let waits: Vec<&str> = shared.iter().filter(|x| x.waits).map(|x| x.name.as_str()).collect();
        assert_eq!(waits, ["usage_limit", "offline", "cannot_run"]);
        assert_eq!(shared[0].conditions, [flow::format::Atom::SentBack(Some(3))]);
    }
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    v["flows"][0]["rounds"]["failed"]["kinds"][3]["then"] = "A3".into();
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(e.contains("unknown field `then`"), "{e}");
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    v["flows"][0]["rounds"]["failed"]["kinds"][1]["name"] = "pause".into();
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(
        e.contains("has the name of a shared row the rounds section gives: pause"),
        "{e}"
    );
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    v["flows"][0]["agentCommon"]["rows"][1]["if"] = serde_json::json!([["sent_back"]]);
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(
        e.contains("a row switches on a return sent back or a failed run"),
        "{e}"
    );
}

// Every agent's cell is given the general rules and the project's design goal; every engineer's cell and the
// advisor's are given the engineering standards; no cell has an input that only says to read something itself; and
// the forms every return shares say how a return is written. A rule given to a line cannot be named by a cell again.
#[test]
fn the_rules_and_the_shared_inputs_reach_every_agent_cell() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    for cell in g.cells.iter().filter(|x| x.kind == "agent") {
        let rules: Vec<(&str, bool)> = cell.rules.iter().map(|r| (r.name.as_str(), r.shared)).collect();
        assert_eq!(rules[0], ("general", true), "{}", cell.id);
        let engineer = matches!(cell.line.as_deref(), Some("engineer" | "advisor"));
        assert_eq!(rules.contains(&("engineering", true)), engineer, "{}", cell.id);
        assert!(
            cell.inputs
                .iter()
                .any(|i| i.from == [flow::format::Src::Config("design_goal".into())]),
            "{}",
            cell.id
        );
        assert!(
            !cell.inputs.iter().any(|i| i.from == [flow::format::Src::Itself]),
            "{}",
            cell.id
        );
    }
    assert!(g.write.iter().any(|l| l.starts_with("Only your return is read")));
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let cells = v["flows"][0]["nodes"].as_array_mut().unwrap();
    let implement = cells.iter_mut().find(|n| n["id"] == "I1").unwrap();
    implement["rules"] = serde_json::json!(["design_tree_writing", "engineering"]);
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(
        e.contains("a cell names a rule every cell of its line is given: I1"),
        "{e}"
    );
}

// A field the flow lets a return leave out that the agent wrote as null is taken as left out, and the return fits; a
// null in a field the return must hold still does not fit.
#[test]
fn a_null_in_a_field_that_may_be_left_out_is_left_out() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let s = g.cell("H7").unwrap().schema.clone().unwrap();
    let mut ret = serde_json::json!({
        "outcome": {
            "fix": { "way": "nothing", "why": "It passed." },
            "reply": { "text": "Go on.", "why": "It passed.", "waits": null, "children": null },
            "effects": null
        },
        "tell_owner": [],
        "progress": "Nothing to fix.",
        "flow_feedback": "none"
    });
    assert!(!flow::valid(&s, &ret));
    flow::without_null_options(&s, &mut ret);
    assert!(flow::valid(&s, &ret), "{ret}");
    assert_eq!(
        ret["outcome"],
        serde_json::json!({ "fix": { "way": "nothing", "why": "It passed." },
                            "reply": { "text": "Go on.", "why": "It passed." } })
    );
    let mut bad = ret.clone();
    bad["outcome"]["reply"]["text"] = Value::Null;
    flow::without_null_options(&s, &mut bad);
    assert!(!flow::valid(&s, &bad), "{bad}");
}

// What the engine stores reads back as it was, with the same digest each time.
#[test]
fn the_compiled_flow_reads_back_with_a_stable_digest() {
    let c = compiled();
    let back: flow::Compiled = serde_json::from_str(&c.json()).unwrap();
    assert_eq!(back.json(), c.json());
    assert_eq!(compiled().digest(), c.digest());
    assert_eq!(c.digest().len(), 64);
}

// The owner's page heads each field of a return with its title and each input with its name, and no agent is sent
// either, so that agents' prompts and their cache stay as they were: the flow with every title, and the name of every
// input but those every agent's cell has, written otherwise compiles to the same flow but for the titles and those
// names, and so gives every agent the same return schemas, formats, lists of inputs and prompts. The names of the
// inputs every agent's cell has are the agents' too, and stay.
#[test]
fn no_agent_is_sent_a_title_or_the_name_of_an_input_only_the_owner_reads() {
    fn retitle(v: &mut Value) {
        match v {
            Value::Object(o) => {
                let field = o.contains_key("name") && (o.contains_key("type") || o.contains_key("$ref"));
                let keys = ["answerTitle", "whyTitle"].into_iter().chain(field.then_some("title"));
                for k in keys {
                    if let Some(Value::String(t)) = o.get_mut(k) {
                        t.push_str(", written otherwise");
                    }
                }
                o.values_mut().for_each(retitle);
            }
            Value::Array(xs) => xs.iter_mut().for_each(retitle),
            _ => {}
        }
    }
    // The compiled flow without what only the owner's page reads.
    fn agents_part(c: &flow::Compiled) -> Value {
        let mut v = serde_json::to_value(c).unwrap();
        for g in v["graphs"].as_array_mut().unwrap() {
            for cell in g["cells"].as_array_mut().unwrap() {
                cell.as_object_mut().unwrap().remove("titles");
                for x in cell["inputs"].as_array_mut().unwrap() {
                    if x.get("common") != Some(&Value::Bool(true)) {
                        x.as_object_mut().unwrap().remove("name");
                    }
                }
            }
        }
        v
    }
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    retitle(&mut v);
    for g in v["flows"].as_array_mut().unwrap() {
        for n in g["nodes"].as_array_mut().unwrap() {
            for x in n.get_mut("inputs").and_then(Value::as_array_mut).into_iter().flatten() {
                if let Some(Value::String(name)) = x.get_mut("name") {
                    name.push_str(", written otherwise");
                }
            }
        }
    }
    let other = flow::compile(&v.to_string(), &context()).unwrap_or_else(|e| panic!("{e}"));
    let c = compiled();
    assert!(
        agents_part(&c) == agents_part(&other),
        "a title or an input's name reached what agents are sent"
    );
    for (g, h) in c.graphs.iter().zip(&other.graphs) {
        assert_eq!(
            flow::formats::common_format(g, &[]),
            flow::formats::common_format(h, &[])
        );
        for (x, y) in g.cells.iter().zip(&h.cells) {
            assert_eq!(x.schema, y.schema, "{} {}", g.name, x.id);
            assert_eq!(x.prompt, y.prompt, "{} {}", g.name, x.id);
            assert_eq!(
                flow::formats::cell_format(g, x, &[]),
                flow::formats::cell_format(h, y, &[])
            );
            assert_eq!(
                flow::formats::cell_inputs(x),
                flow::formats::cell_inputs(y),
                "{} {}",
                g.name,
                x.id
            );
            assert!(x.titles.keys().eq(y.titles.keys()), "{} {}", g.name, x.id);
            assert!(
                x.titles.values().zip(y.titles.values()).all(|(a, b)| a != b),
                "{} {}",
                g.name,
                x.id
            );
        }
    }
    // Every field with a name has a title: those of R1's own answer, in the places the page finds them.
    let r1 = c.graph("engineer").unwrap().cell("R1").unwrap();
    assert_eq!(
        r1.titles["understanding.vs_item"],
        "Where this differs from the item as written"
    );
    assert_eq!(
        r1.titles["understanding.grounds[inferred].confirm"],
        "How it will be confirmed"
    );
    assert_eq!(r1.titles["progress"], "Progress note");
}

// Every sample return fits its cell's schema and takes the row it was written for.
#[test]
fn every_sample_takes_its_row() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let mut n = 0;
    for (cell, xs) in &samples {
        for s in xs {
            let given: Map<String, Value> = s["given"].as_object().cloned().unwrap_or_default();
            let row = g
                .take(cell, &s["json"], &given)
                .unwrap_or_else(|e| panic!("{cell}: {e}"));
            assert_eq!(row.name, s["row"].as_str().unwrap(), "a sample of {cell}");
            n += 1;
        }
    }
    assert_eq!(n, 33);
}

// A return with its form outside the field it sits in fits no schema; a recorded value changes the row taken.
#[test]
fn a_sample_that_does_not_fit_or_takes_another_row_is_found() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let s = &samples["A1"][0]["json"];
    let mut flat = s["outcome"].as_object().unwrap().clone();
    flat.insert("side_reports".into(), Value::Array(Vec::new()));
    flat.insert("progress".into(), "x".into());
    let e = g.take("A1", &Value::Object(flat), &Map::new()).unwrap_err();
    assert!(e.contains("does not fit the schema"), "{e}");
    let s = &samples["T7"][0];
    let mut given = s["given"].as_object().unwrap().clone();
    given.insert("interjection".into(), "yes".into());
    assert_eq!(g.take("T7", &s["json"], &given).unwrap().name, "interjection");
}

// A return that does not fit says where and how, following the alternative it picks.
#[test]
fn a_return_that_does_not_fit_says_where() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let sample = &samples["D3"][0]["json"];
    let refused = |edit: &dyn Fn(&mut Value)| {
        let mut x = sample.clone();
        edit(&mut x);
        g.take("D3", &x, &Map::new()).unwrap_err()
    };
    let e = refused(&|x| x["outcome"]["better"]["verdict"] = "maybe".into());
    assert!(
        e.ends_with(r#"outcome.better.verdict is "maybe", and must be one of "found", "present_best""#),
        "{e}"
    );
    let e = refused(&|x| {
        x["outcome"]["better"].as_object_mut().unwrap().remove("why");
    });
    assert!(e.ends_with("outcome.better has no why"), "{e}");
    let e = refused(&|x| x["outcome"] = Value::String(x["outcome"].to_string()));
    assert!(e.ends_with("outcome must be an object, and is a string"), "{e}");
}

// An agent's return schema holds its forms one level down, and a field a form leaves out is not required.
#[test]
fn an_agent_schema_holds_its_forms_in_outcome() {
    let c = compiled();
    let s = c.graph("engineer").unwrap().cell("I1").unwrap().schema.clone().unwrap();
    assert!(s["properties"]["outcome"]["anyOf"].is_array());
    let required: Vec<&str> = s["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(required, ["tell_owner", "side_reports", "flow_feedback", "outcome"]);
    // The short fields every return holds come first, and the field the forms are under last.
    let order: Vec<&String> = s["properties"].as_object().unwrap().keys().collect();
    assert_eq!(
        order,
        ["tell_owner", "progress", "side_reports", "flow_feedback", "outcome"]
    );
}

// Only the engineer and the architect tell the owner: an agent's cell of a line that helps them (the review, adversarial
// testing, the advisor, the review of the architect's batch and the search of observations) has no `tell_owner` in its
// return, its description does not name one, and nothing is done with it or shown of it there. The helpers of a work
// still hand in side reports, which the architect reads.
#[test]
fn only_the_engineer_and_the_architect_tell_the_owner() {
    let c = compiled();
    let mut helpers = Vec::new();
    for g in &c.graphs {
        for cell in g.cells.iter().filter(|x| x.kind == "agent") {
            let tells = matches!(cell.line.as_deref(), Some("engineer" | "architect"));
            let s = cell.schema.as_ref().unwrap();
            let id = &cell.id;
            assert_eq!(s["properties"].get("tell_owner").is_some(), tells, "{id}");
            assert_eq!(
                s["description"].as_str().unwrap().contains("`tell_owner`"),
                tells,
                "{id}"
            );
            assert_eq!(cell.always.iter().any(|a| a.at == "tell_owner"), tells, "{id}");
            assert_eq!(cell.shown.iter().any(|p| p == "tell_owner"), tells, "{id}");
            if !tells {
                helpers.push(id.as_str());
                let reports = s["properties"].get("side_reports").is_some();
                assert_eq!(reports, g.name == "engineer", "{id}");
            }
        }
    }
    assert_eq!(helpers, ["T1", "T3", "T4", "T5", "A1", "B2", "X2"]);
}

// The architect tells the owner what the engineer does and no more: what only the owner can do (starting a work the
// owner paused among it), a risk that cannot be undone, or a boundary crossed; never a guess, nor what it changed in
// the project, which the page shows. Nothing in its rule or its cells' prompts asks it to tell the owner anything else:
// a defect of the system goes in its flow feedback.
#[test]
fn the_architect_tells_the_owner_three_things_only() {
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let graph = |name: &str| {
        v["flows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["name"] == name)
            .unwrap()
    };
    let desc = |name: &str| {
        let out = graph(name)["agentCommon"]["out"].as_array().unwrap();
        let tell = out.iter().find(|o| o["name"] == "tell_owner").unwrap();
        tell["desc"].as_str().unwrap().to_string()
    };
    let (architect, engineer) = (desc("architect"), desc("engineer"));
    let three = "Messages for the owner, a sentence or two each, of three things only: what only the owner can do";
    assert!(architect.starts_with(three) && engineer.starts_with(three));
    for part in [
        "starting a work the owner paused",
        "a risk that cannot be undone",
        "a boundary crossed",
        "Never a guess or what you have not checked",
        "What you changed in the project (items, observations, the plan, the policy) is not told",
    ] {
        assert!(architect.contains(part), "{part}");
    }
    // Every text the architect is given to act by: its rule, and its cells' prompts and the cases of each.
    let mut texts: Vec<String> = Vec::new();
    let rule = v["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "architect")
        .unwrap();
    texts.extend(
        rule["text"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_string()),
    );
    for cell in graph("architect")["nodes"].as_array().unwrap() {
        for t in cell["prompt"].as_array().into_iter().flatten() {
            texts.push(t.as_str().unwrap().to_string());
        }
        for case in cell["promptBy"]["cases"].as_object().into_iter().flatten() {
            for t in case.1.as_array().unwrap() {
                texts.push(t.as_str().unwrap().to_string());
            }
        }
    }
    for t in &texts {
        let asks = t.contains("tell_owner") || t.replace("tell the owner nothing", "").contains("tell the owner");
        assert!(!asks, "{t}");
    }
    assert!(
        texts
            .iter()
            .any(|t| t.contains("say in flow_feedback where it is and what it cost"))
    );
}

// A work moves as the rows say: into the tool cell and back to the agent that called it, out to the advisor and,
// through choosing the way on, back to where it stopped.
#[test]
fn a_work_moves_by_its_rows() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let take = |p: &flow::Position, name: &str| {
        let row = g.cell(&p.at).unwrap().rows.iter().find(|r| r.name == name).unwrap();
        g.step(p, row).unwrap()
    };
    let p = g.start_position();
    assert_eq!(p.at, "S0");
    let p = take(&p, "ready");
    assert_eq!(p.at, "S1");
    let p = take(&p, "started");
    assert_eq!((p.at.as_str(), p.recorded["stage"].as_str()), ("R1", "research"));
    let p = take(&p, "tools");
    assert_eq!(
        (p.at.as_str(), p.callers.as_slice()),
        ("X1", ["R1".to_string()].as_slice())
    );
    let p = take(&p, "ran");
    assert_eq!(
        (p.at.as_str(), p.from.as_deref(), p.callers.len()),
        ("R1", Some("X1"), 0)
    );
    let p = take(&p, "ask_advisor");
    assert_eq!(
        (p.at.as_str(), p.stopped.as_deref(), p.pointer("@caller")),
        ("A1", Some("R1"), Some("R1"))
    );
    let p = take(&p, "answered");
    assert_eq!(
        (p.at.as_str(), p.stopped.as_deref(), p.from.as_deref()),
        ("A4", Some("R1"), Some("A1"))
    );
    let p = take(&p, "carry_on");
    assert_eq!(p.at, "A5");
    let p = take(&p, "back");
    assert_eq!((p.at.as_str(), p.stopped.as_deref()), ("R1", None));
    let p = take(&p, "done");
    assert_eq!((p.at.as_str(), p.recorded["rethought"].as_str()), ("R2", "no"));
    let wrong = g.cell("A5").unwrap().rows.iter().find(|r| r.name == "back").unwrap();
    assert!(g.step(&p, wrong).is_err());
}
