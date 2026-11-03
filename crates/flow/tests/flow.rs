use std::collections::{BTreeMap, BTreeSet};

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

/// Whether each graph of the flow compiled to the cells `source` gives it: a compiled graph has a cell for every node
/// of the graph of its name in `source`, and no other. The source is the standard, not what the compiler once
/// produced, and the answer names each cell that is missing or that no node gives.
fn cells_match_nodes(c: &flow::Compiled, source: &str) -> Result<(), String> {
    let v: Value = serde_json::from_str(source).unwrap();
    let mut problems = Vec::new();
    for f in v["flows"].as_array().unwrap() {
        let name = f["name"].as_str().unwrap();
        let nodes: BTreeSet<&str> = f["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        let Some(g) = c.graph(name) else {
            problems.push(format!("{name}: the flow has no compiled graph"));
            continue;
        };
        let cells: BTreeSet<&str> = g.cells.iter().map(|x| x.id.as_str()).collect();
        for id in nodes.difference(&cells) {
            problems.push(format!("{name}: no cell for the node {id}"));
        }
        for id in cells.difference(&nodes) {
            problems.push(format!("{name}: a cell {id} that is no node"));
        }
    }
    for g in &c.graphs {
        if !v["flows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == g.name.as_str())
        {
            problems.push(format!("{}: a compiled graph that is no graph of the flow", g.name));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

// The flow compiles with no finding, and each of its graphs comes out with the cells of its nodes, the engineer's and
// the architect's. The check is shown to fail for a cell the compiler lost and for one it made up, each named.
#[test]
fn each_graph_compiles_to_the_cells_of_its_nodes() {
    let c = compiled();
    cells_match_nodes(&c, FLOW).unwrap_or_else(|e| panic!("{e}"));
    let mut lost = c.clone();
    let gone = lost
        .graphs
        .iter_mut()
        .find(|g| g.name == "engineer")
        .and_then(|g| g.cells.pop())
        .unwrap();
    let e = cells_match_nodes(&lost, FLOW).unwrap_err();
    assert_eq!(e, format!("engineer: no cell for the node {}", gone.id));
    let mut made = c.clone();
    let g = made.graphs.iter_mut().find(|g| g.name == "architect").unwrap();
    let mut extra = g.cells[0].clone();
    extra.id = "ZZ".into();
    g.cells.push(extra);
    let e = cells_match_nodes(&made, FLOW).unwrap_err();
    assert_eq!(e, "architect: a cell ZZ that is no node");
}

/// What of a cell decides whether a work standing at it moves to a new flow version or is taken over, as the engine's
/// `versions::why_not` compares a cell's two versions: its kind and line, its inputs' ids, names and sources, and its
/// rows' names, conditions and targets, each in their order; hashed short, the file of the cell in `tests/moves/`.
fn moves_by(c: &flow::CompiledCell) -> String {
    let inputs: Vec<Value> = c
        .inputs
        .iter()
        .map(|x| serde_json::json!([x.id, x.name, x.from]))
        .collect();
    let rows: Vec<Value> = c
        .rows
        .iter()
        .map(|r| serde_json::json!([r.name, r.conditions, r.to]))
        .collect();
    let text = serde_json::json!([c.kind, c.line, inputs, rows]).to_string();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Every way the files of `tests/moves/` (file name to content) differ from the hashes the compiler gives the cells
/// (cell id to hash), each saying the cell and what to write or delete so that an engineer only follows it.
///
/// Each cell has a file of its own, and not a line of one file, because git's three-way merge takes changes to
/// neighbouring lines for a conflict: two works changing neighbouring cells would meet in a file of one line per cell.
/// A cell added or removed is a file added or removed, which merges. Only two works changing the same cell meet, and
/// that needs the hash worked out again anyway.
fn moves_misses(have: &BTreeMap<String, String>, files: &BTreeMap<String, String>) -> Vec<String> {
    let mut misses = Vec::new();
    for (id, hash) in have {
        match files.get(id) {
            None => misses.push(format!("{id}: no file; write {hash} in crates/flow/tests/moves/{id}")),
            Some(text) if text.trim_end() != hash => {
                misses.push(format!("{id}: write {hash} in crates/flow/tests/moves/{id}"));
            }
            Some(_) => {}
        }
    }
    for id in files.keys().filter(|id| !have.contains_key(*id)) {
        misses.push(format!("{id}: no such cell; delete crates/flow/tests/moves/{id}"));
    }
    misses
}

// The parts of each cell of the engineer's graph that decide whether a work standing at it moves to a new flow version
// are as the file of the cell in `tests/moves/` has them, so that a change of the flow shows in the change's files
// which cells' works a new version takes over when it is installed, at the cost of a work begun again. The comparison
// is shown to fail for a hash that differs, a cell with no file and a file with no cell, each naming the cell.
#[test]
fn the_cells_a_work_moves_by_are_as_their_files_have_them() {
    let c = compiled();
    let have: BTreeMap<String, String> = c
        .graph("engineer")
        .unwrap()
        .cells
        .iter()
        .map(|c| (c.id.clone(), moves_by(c)))
        .collect();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/moves");
    let files: BTreeMap<String, String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let name = e.file_name().into_string().expect("a file name that is text");
            let text = std::fs::read_to_string(e.path()).unwrap_or_else(|x| panic!("{name}: {x}"));
            (name, text)
        })
        .collect();
    let misses = moves_misses(&have, &files);
    assert!(
        misses.is_empty(),
        "cells whose works a new version takes over:\n{}",
        misses.join("\n")
    );

    let (id, hash) = have.iter().next().unwrap();
    let mut changed = files.clone();
    changed.insert(id.clone(), "0000000000000000\n".into());
    assert_eq!(
        moves_misses(&have, &changed),
        [format!("{id}: write {hash} in crates/flow/tests/moves/{id}")]
    );
    let mut missing = files.clone();
    missing.remove(id);
    assert_eq!(
        moves_misses(&have, &missing),
        [format!("{id}: no file; write {hash} in crates/flow/tests/moves/{id}")]
    );
    let mut extra = files.clone();
    extra.insert("ZZ".into(), "0000000000000000\n".into());
    assert_eq!(
        moves_misses(&have, &extra),
        ["ZZ: no such cell; delete crates/flow/tests/moves/ZZ"]
    );
}

// The cells that read the histories and the observations judge what matters in a great many records, so they think:
// the librarian's five cells and the architect's look-up at high effort, the search of observations at medium, and no
// cell of any graph at low, which does not think. A line cannot hold low cells beside others, so that the librarian's
// cell G3 goes with its line.
#[test]
fn the_cells_that_read_histories_and_observations_think() {
    let c = compiled();
    let effort = |graph: &str, cell: &str| {
        let found = c.graph(graph).and_then(|g| g.cell(cell));
        found
            .unwrap_or_else(|| panic!("no cell {cell} in {graph}"))
            .effort
            .clone()
    };
    for (graph, cell) in [
        ("engineer", "G1"),
        ("engineer", "G2"),
        ("engineer", "G4"),
        ("engineer", "G3"),
        ("engineer", "X5"),
        ("architect", "X5"),
    ] {
        assert_eq!(effort(graph, cell).as_deref(), Some("high"), "{graph} {cell}");
    }
    assert_eq!(effort("architect", "X2").as_deref(), Some("medium"));
    for g in &c.graphs {
        for cell in &g.cells {
            assert_ne!(cell.effort.as_deref(), Some("low"), "{} {}", g.name, cell.id);
        }
    }
}

// A stop that comes before the merge is a row of the merge cell, drawn on the chart: it waits there for the engine's
// loop to end the work, taken only at the work's turn in the queue. What depends on the order things happened in, and
// not on a cell, no row can say, so the words the owner and the architect read say it: a cancel or a start over waits
// for the step under way, a merge that came first stands, a cancel closes what a work taken over left, and a resume
// waits for a free slot until a later pause withdraws it.
#[test]
fn a_stop_before_the_merge_is_a_row_and_the_rest_is_said_in_words() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let row = g.cell("M2").unwrap().rows.iter().find(|r| r.name == "stopped").unwrap();
    assert_eq!(row.to, "M2");
    assert!(row.waits && row.counts.is_none());
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let stop = v["engine"]["steps"]["stop_work"].as_str().unwrap();
    assert!(stop.contains("once the step it is in has ended"), "{stop}");
    assert!(stop.contains("A merge that came first stands"), "{stop}");
    assert!(stop.contains("waits for its next start"), "{stop}");
    assert!(
        stop.contains("the cancel removes the worktree and closes the pull request"),
        "{stop}"
    );
    let waits = v["engine"]["steps"]["wait_for_start"].as_str().unwrap();
    assert!(waits.contains("withdraws a resume"), "{waits}");
    for (said, want) in [
        ("Cancel an item; its work ends", "once the step it is in has ended"),
        (
            "Cancel an item; its work ends",
            "whose merge comes first, is not cancelled",
        ),
        ("Let a work you paused go on", "once its machine has a slot free"),
        ("Let a work you paused go on", "later Pause or Step"),
        ("Let a work you paused go on", "the engine's pause for a problem"),
    ] {
        let line = FLOW.lines().find(|l| l.contains(said)).unwrap();
        assert!(line.contains(want), "{line}");
    }
}

// The answers of a cell's list of forms are read from the flow built into the program: the fix has the answer made for
// a fix in no commit, a field that is no list of forms and a cell the flow lacks give none.
#[test]
fn the_answers_of_a_list_of_forms_are_read_from_the_flow() {
    let answers = flow::answers("T6", "rows").unwrap();
    for a in ["fixed", "fixed_outside_commit", "disagree"] {
        assert!(answers.iter().any(|x| x == a), "{answers:?}");
    }
    assert_eq!(flow::answers("T6", "head"), None);
    assert_eq!(flow::answers("T6", "no such field"), None);
    assert_eq!(flow::answers("no such cell", "rows"), None);
}

/// The fix's numbered table.
fn t6_table(c: &flow::Compiled) -> flow::CompiledInput {
    let g = c.graph("engineer").unwrap();
    let t6 = g.cell("T6").unwrap();
    t6.inputs.iter().find(|x| x.numbered.is_some()).unwrap().clone()
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
        .filter(|c| c.rows.iter().any(|r| r.name == "nothing_to_answer"))
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(tables, ["T6"]);
    // Its table reads only what it has not answered, a change to the code the way on asked among it.
    let table = t6_table(&c);
    assert!(table.unanswered, "{table:?}");
    assert!(
        table
            .from
            .contains(&flow::format::Src::Place("A4".into(), "next.change".into()))
    );
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
// and each matter's path, the check of the policy, the writing of a new one, or its draft dropped when the owner keeps
// the policy in force, and its taking effect (`P1`, `P2`, `P12`),
// the plan of what to do next (`N1`), the recording of the decisions in the owner's words, the review of a batch of item
// changes and whether to plan or check again (`B1`, `B5`, `B2`, `B3`, `B4`), the end (`E2`), the owner's ask that holds
// only its matter (`A3`), the tools (`X1`), the observer
// (`X2`) and the wait for the owner's Start after a pause (`X3`). It has no wrap-up, no wait
// for the owner's reply in a talk, and none of the setting of a policy as a stage of its own (its study, its readings by
// new architects, the owner's buttons).
// The owner's words reach the architect only through its queue: no cell of it takes an interjection, and none answers
// one.
#[test]
fn the_architect_graph_has_no_cell_the_design_left_out() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    for gone in [
        "E1", "H6", "H11", "P3", "P4", "P5", "P6", "P7", "P8", "P9", "P10", "P11",
    ] {
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

// Every agent's cell of the architect's line and of its pick but the review of a batch and the observer's search may
// rewrite sections of the notebook in its own answer, beside its effects, the edit made at once and its size checked;
// the understanding and the plan must, each its own sections. Each such cell is given the four sections, each an engine
// input of its own, but the understanding, which writes what the project is now afresh and is given only what is
// unclear, as questions to settle, and the other section; no cell reads the state of things, the notes, the log or the
// last plan, which the notebook took the place of.
#[test]
fn every_cell_of_the_architect_s_line_keeps_the_notebook() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let nodes = v["flows"][1]["nodes"].as_array().unwrap();
    let sections = ["project_now", "plan", "unclear", "other"];
    let mut keep = Vec::new();
    for cell in &g.cells {
        if cell.kind != "agent" || !matches!(cell.line.as_deref(), Some("architect" | "picker")) {
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
            "U1", "P1", "N1", "P2", "Q1", "H1", "H2", "H3", "H4", "H5", "H7", "H8", "H9", "H10", "H12", "B5", "B3",
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

// Item #228: the architect picks side reports or answers about one thing together. The pick names the others to go with
// the matter it picks and sets the group (`group_picked`); the cell that finds what a report is and the cell that takes
// it in each answer for the group in one round, the others in `with`, which they leave out for a matter alone, and
// hold the answer to the group (`group_answered`); every cell that may ask the owner takes the group apart as it asks
// (`dissolve_group`). A `with` that is empty, or lacks a field of an entry, fits no schema.
#[test]
fn a_group_is_picked_by_the_pick_answered_for_by_two_cells_and_taken_apart_by_the_ask() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    assert!(
        g.cell("Q1")
            .unwrap()
            .always
            .iter()
            .any(|a| a.at == "pick" && a.action.as_deref() == Some("group_picked"))
    );
    assert!(
        g.cell("Q1")
            .unwrap()
            .checks
            .iter()
            .any(|x| x.at.as_deref() == Some("pick") && x.check == "picked_pickable")
    );
    for id in ["H4", "H5"] {
        assert!(
            g.cell(id)
                .unwrap()
                .checks
                .iter()
                .any(|x| x.at.as_deref() == Some("with") && x.check == "group_answered"),
            "{id}"
        );
    }
    let asking: Vec<&str> = g
        .cells
        .iter()
        .filter(|x| x.rows.iter().any(|r| r.name == "ask_owner"))
        .map(|x| x.id.as_str())
        .collect();
    assert!(asking.contains(&"H4") && asking.contains(&"H5"), "{asking:?}");
    for id in &asking {
        let row = g.cell(id).unwrap().rows.iter().find(|r| r.name == "ask_owner").unwrap();
        assert!(row.actions.iter().any(|d| d.name() == "dissolve_group"), "{id}");
    }
    // The pick, on a line of its own, tells the owner nothing; the architect's own cells do.
    let ret = |cell: &str, outcome: Value| {
        let mut r = serde_json::json!({ "outcome": outcome, "progress": "p", "flow_feedback": "none" });
        if cell != "Q1" {
            r["tell_owner"] = serde_json::json!([]);
        }
        r
    };
    let takes = |cell: &str, outcome: Value| -> Result<String, String> {
        g.take(cell, &ret(cell, outcome), &Map::new()).map(|r| r.name.clone())
    };
    let refused = |cell: &str, outcome: Value| takes(cell, outcome).unwrap_err().contains("does not fit the schema");
    // The pick names others, or none.
    let pick = serde_json::json!({ "matter": 1, "why": "w" });
    assert_eq!(takes("Q1", serde_json::json!({ "pick": pick })).unwrap(), "picked");
    let group = serde_json::json!({ "matter": 1, "why": "w", "with": [2, 3] });
    assert_eq!(takes("Q1", serde_json::json!({ "pick": group })).unwrap(), "picked");
    let empty = serde_json::json!({ "matter": 1, "why": "w", "with": [] });
    assert!(refused("Q1", serde_json::json!({ "pick": empty })));
    // The finding is the matter picked's, and `with` the others'.
    let finding = serde_json::json!({ "what": "w", "grounds": "g", "sure": "checked" });
    let other = serde_json::json!({ "matter": 2, "what": "w", "grounds": "g", "sure": "likely" });
    assert_eq!(takes("H4", serde_json::json!({ "finding": finding })).unwrap(), "found");
    assert_eq!(
        takes("H4", serde_json::json!({ "finding": finding, "with": [other] })).unwrap(),
        "found"
    );
    let mut lacking = other.clone();
    lacking.as_object_mut().unwrap().remove("sure");
    assert!(refused(
        "H4",
        serde_json::json!({ "finding": finding, "with": [lacking] })
    ));
    assert!(refused("H4", serde_json::json!({ "finding": finding, "with": [] })));
    // The way is the matter picked's, and `with` the others', with the changes that come of each naming it.
    let take = serde_json::json!({ "way": "merged", "why": "w" });
    let way = serde_json::json!({ "matter": 2, "way": "nothing", "why": "w" });
    let change = serde_json::json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "t", "reason": "r",
                                     "matter": 2 });
    assert_eq!(takes("H5", serde_json::json!({ "take": take })).unwrap(), "done");
    let both = serde_json::json!({ "take": take, "with": [way], "effects": { "item_changes": [change] } });
    assert_eq!(takes("H5", both).unwrap(), "done");
    let mut wrong = way.clone();
    wrong["way"] = "somehow".into();
    assert!(refused("H5", serde_json::json!({ "take": take, "with": [wrong] })));
    assert!(refused("H5", serde_json::json!({ "take": take, "with": [] })));
    // A member that needs the owner goes back to the queue (`release_members`, the cell's first action, before the
    // others read the group); the way fits the schema of the entries of `with`, and the check, not the schema, turns it
    // away from the matter picked.
    let always = &g.cell("H5").unwrap().always;
    assert_eq!(
        (always[0].at.as_str(), always[0].action.as_deref()),
        ("with", Some("release_members"))
    );
    let mut back = way.clone();
    back["way"] = "back_for_owner".into();
    assert_eq!(
        takes("H5", serde_json::json!({ "take": take, "with": [back] })).unwrap(),
        "done"
    );
}

// The check of the policy and the plan of what to do next are matters at the head of the queue: a new session's
// understanding queues both and goes on to the queue; the look at the queue takes them after the owner's words, a
// matter back with the owner's reply, the waits for the owner and a matter of two cells going on at its second cell,
// and before the others, without a pick (`head`), and the router sends each to its cell. The check closes it when the
// policy holds, asks the owner in doubt, and goes on to write a new policy by the owner's reply, or by the owner's
// words that asked for it (`write_policy`, which the engine refuses otherwise), through the queue, where the owner's
// words that came meanwhile are taken first; the writing asks the owner to approve it,
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
    assert_eq!(
        q0[..6],
        ["owner_words", "answered", "waits", "listens", "second_cell", "head"]
    );
    assert_eq!(row("Q0", "head").to, "Q2");
    let given = |v: &str| Map::from_iter([("route".to_string(), Value::String(v.into()))]);
    for at in ["P1", "P2", "P12", "N1"] {
        let r = g.take("Q2", &serde_json::json!({}), &given(at)).unwrap();
        assert_eq!(r.to, at);
    }
    assert_eq!(way(&row("P1", "holds")), to("B1", &["close_matter"]));
    // A new policy is written at the second cell of the check, which goes back to the queue between them.
    assert_eq!(way(&row("P1", "write_policy")), to("Q0", &["go_on_at_second_cell"]));
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
    // The owner's words a batch handled have their decisions recorded before its item changes are reviewed: the batch's
    // end goes to the cell that records them, which goes on to the review when a decision or an item change is there to
    // review, and to the rethink, as the end of a batch with none, when none is.
    assert_eq!(way(&row("B1", "decisions")), to("B5", &[]));
    assert_eq!(way(&row("B5", "decisions")), to("B2", &["open_session"]));
    assert_eq!(way(&row("B5", "changes")), to("B2", &["open_session"]));
    assert_eq!(way(&row("B5", "none")), to("B4", &["settle_straight_hold"]));
    assert_eq!(way(&row("B1", "review")), to("B2", &["open_session"]));
    assert_eq!(way(&row("B1", "done")), to("B4", &["settle_straight_hold"]));
    let into = |cell: &str| -> Vec<&str> {
        g.cells
            .iter()
            .filter(|x| x.rows.iter().any(|r| r.to == cell && !r.common))
            .map(|x| x.id.as_str())
            .collect()
    };
    assert_eq!(into("P1"), ["Q2"]);
    assert_eq!(into("N1"), ["Q2"]);
    assert_eq!(into("P2"), ["Q2"]);
    assert_eq!(into("P12"), ["Q2"]);
    // Only the end of a batch goes to the recording of the decisions, and a batch's review comes from it, the end of the
    // batch and the revision.
    assert_eq!(into("B5"), ["B1"]);
    assert_eq!(into("B2"), ["B1", "B5", "B3"]);
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
// ask the owner, whose reply comes back to it; no cell waits for the owner's reply in a talk (`H11`) any longer, but
// while its answer leaves the talk open the look at the queue waits a while for the owner's next words. A row that
// waits in the architect's graph with no setting bounding the wait is refused, since the architect stops for no one.
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
    // While the answer leaves the talk open, the look at the queue waits for the owner's next words, and the batch's
    // review and rethink wait with it.
    let listens = g
        .take(
            "Q0",
            &serde_json::json!({ "queue": { "verdict": "listens", "matter": 3 } }),
            &none,
        )
        .unwrap();
    assert_eq!((listens.to.as_str(), listens.waits), ("Q0", true));
    let talking = Map::from_iter([("next_in_batch".to_string(), Value::String("talking".into()))]);
    assert_eq!(g.take("B1", &serde_json::json!({}), &talking).unwrap().to, "Q0");
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

// A capability report, an agent saying the system lacks what its work needs, is a defect of the system: the ways read
// it from what a question says, with no kind, and unblock the work first, by a way around within the rules, by
// pausing the work until the owner gives what only the owner can, or by an item that fixes it, which the work waits
// for, and then take the gap in as a report is taken in. The work that handed in a side report or a question is told
// what became of it (`answer_reporter`), after the round's effects are made; and an observation a merge makes reported
// three times is work, which the taking in says and the check of the effects holds.
#[test]
fn a_capability_report_unblocks_first_is_answered_and_is_work_once_it_recurs() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let ways = g.cell("H3").unwrap();
    for way in [
        "A capability gap, the system lacking what the work needs",
        "a way around within the rules",
        "by pausing the work until the owner has given it",
        "which the work's item depends on, blocking, and the reply's waits name",
        "then take the gap in as a report is",
    ] {
        assert!(ways.prompt.iter().any(|t| t.contains(way)), "{way}");
    }
    for (cell, at) in [("H3", "effects"), ("H5", "take")] {
        let always: Vec<(String, String)> = g
            .cell(cell)
            .unwrap()
            .always
            .iter()
            .filter_map(|a| Some((a.action.clone()?, a.at.clone())))
            .collect();
        let answer = always.iter().position(|a| a.0 == "answer_reporter").expect(cell);
        let effects = always.iter().position(|a| a.0 == "apply_effects").expect(cell);
        assert!(effects < answer, "{cell}");
        assert_eq!(always[answer].1, at, "{cell}");
    }
    let take = g.cell("H5").unwrap();
    assert!(
        take.prompt
            .iter()
            .any(|t| t.contains("A merge that makes an observation reported three times makes it work"))
    );
    assert!(take.checks.iter().any(|k| k.check == "effects_valid"));
    let v: Value = serde_json::from_str(FLOW).unwrap();
    assert!(
        v["engine"]["checks"]["effects_valid"]
            .as_str()
            .unwrap()
            .contains("reported three times or more is folded into an item by a change of the batch")
    );
}

// An ask carries no kind: the one asked reads what it is about from what it says. An ask's point is what is asked and
// what was checked, with no kind among its fields; a gate asks what its question says, the one about something under
// test, which only a project with something under test reaches, saying what to give; the ways (`H3`) weigh a question by what it says, with no case for a kind of
// question; and no graph lists kinds of question.
#[test]
fn an_ask_carries_no_kind() {
    let c = compiled();
    let engineer = c.graph("engineer").unwrap();
    let forms = &engineer.cell("R1").unwrap().schema.as_ref().unwrap()["properties"]["outcome"]["anyOf"];
    let point = forms
        .as_array()
        .unwrap()
        .iter()
        .find_map(|a| a["properties"]["ask"]["oneOf"][0]["properties"]["points"]["items"]["properties"].as_object())
        .unwrap();
    assert_eq!(point.keys().collect::<Vec<_>>(), ["what", "tried"]);
    let v: Value = serde_json::from_str(FLOW).unwrap();
    for f in v["flows"].as_array().unwrap() {
        assert!(f.get("askKinds").is_none(), "{}", f["name"]);
        for n in f["nodes"].as_array().unwrap() {
            assert!(n.get("about").is_none(), "{}", n["id"]);
        }
    }
    let gap = engineer.cell("D5").unwrap();
    assert!(gap.prompt[0].contains("give the code you want to write, the error it really gives"));
    // Only a project with something under test is asked it: the cell before chooses so by a row of its own.
    let under_test = engineer.cell("D4").unwrap();
    let yes = under_test.rows.iter().find(|r| r.name == "yes").unwrap();
    assert_eq!(yes.to, "D5");
    assert_eq!(
        serde_json::to_value(&yes.conditions).unwrap(),
        serde_json::json!([["inp", "under_test", ["yes"]]])
    );
    let ways = c.graph("architect").unwrap().cell("H3").unwrap();
    assert!(ways.prompt_by.is_none() && ways.inenums.is_empty());
}

// What only the owner can settle reaches the owner through the architect, never as an engineer's ask of the owner: the
// engineer's graph has no owner's cell (`A3`), no row leads there, no cell holds a blocker for the owner but `M1`'s
// ruling, and the architect hands nothing on to the owner (`A2` has no `to_owner`, and its reply hands on only to a
// related project's architect). The engine's rows of a flow anomaly at `A4` raise the task's blocker for the architect
// at `A2`, which is given the anomaly; the ways (`H3`) put a choice of the owner's as an open question of the work's
// design, and an action only the owner can do to the owner by pausing the work. An open question left to the owner they
// pin in the owner's one-to-one by an ask, and answer the engineer by the owner's reply.
#[test]
fn what_only_the_owner_can_settle_goes_through_the_architect() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    assert!(g.cell("A3").is_none());
    assert!(g.cells.iter().all(|c| c.rows.iter().all(|r| r.to != "A3")));
    let held: Vec<&str> = g
        .blockers
        .as_ref()
        .unwrap()
        .cells
        .iter()
        .filter(|b| b.holders.iter().any(|h| h == "owner"))
        .map(|b| b.cell.as_str())
        .collect();
    assert_eq!(held, ["M1"]);
    let asking = g.cell("A2").unwrap();
    assert!(asking.rows.iter().all(|r| r.name != "to_owner"));
    assert!(asking.inputs.iter().any(|x| x.name.as_deref() == Some("Flow anomaly")));
    let way_on = g.cell("A4").unwrap();
    for name in [
        "redesign_in_research",
        "carry_on_unfit",
        "should_wait",
        "nothing_to_wait_for",
        "all_split_but_changed",
    ] {
        let r = way_on.rows.iter().find(|r| r.name == name).unwrap();
        assert_eq!(r.to, "A2", "{name}");
        assert!(r.actions.iter().any(|d| d.name() == "raise_blocker"), "{name}");
    }
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let hand_over = v["shapes"]["architect_reply"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "hand_over")
        .unwrap();
    let to = hand_over["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "to")
        .unwrap();
    let whom: Vec<&str> = to["values"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x[0].as_str())
        .collect();
    assert_eq!(whom, ["architect"]);
    let ways = c.graph("architect").unwrap().cell("H3").unwrap();
    assert!(
        ways.prompt
            .iter()
            .any(|t| t.contains("put to the owner as an open question of its design"))
    );
    assert!(
        ways.prompt
            .iter()
            .any(|t| t.contains("pause the work in effects, saying what the owner must do"))
    );
    // What only the owner decides about an open question the ways pin in the owner's one-to-one, by an ask that only an
    // open question may make; they are given the owner's reply when the matter comes back with it, and the questions
    // already pinned, as every cell that may ask is. The crux (`H2`) asks nothing.
    assert_eq!(ways.rows.iter().find(|r| r.name == "ask_owner").unwrap().to, "A3");
    assert!(
        ways.checks
            .iter()
            .any(|x| x.at.as_deref() == Some("ask") && x.check == "ask_only_on_open_question")
    );
    for input in ["owner_answer", "questions"] {
        assert!(
            ways.inputs.iter().any(|x| {
                x.maybe
                    && x.from
                        .iter()
                        .any(|s| matches!(s, flow::format::Src::Engine(n) if n == input))
            }),
            "{input}"
        );
    }
    let crux = c.graph("architect").unwrap().cell("H2").unwrap();
    assert!(crux.rows.iter().all(|r| r.name != "ask_owner"));
    // What they ask the owner is written for the owner, who reads it with no one to translate it: the description of
    // the question says so, which brings the rule for a field the owner reads to it, and no text of the flow asks for a
    // question with no answer in it, since the architect's recommendation belongs in it.
    let forms = &ways.schema.as_ref().unwrap()["properties"]["outcome"]["anyOf"];
    let to_owner = forms
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["properties"]["ask"]["oneOf"].as_array())
        .flatten()
        .find(|b| b["properties"]["to"]["const"] == "owner")
        .expect("the ways may ask the owner");
    let question = to_owner["properties"]["question"]["description"].as_str().unwrap();
    assert!(question.contains("The owner reads this"), "{question}");
    for no in ["no answer in it", "with no answer"] {
        assert!(!FLOW.contains(no), "the flow asks for a question with {no}");
    }
}

// A matter that comes again and again is not queued while the facts it would carry are those of the last one the
// architect handled, since it would only come to the same end, as the flow says of each: slots free, while the free
// slots and the items ready are the same; what was said in the groups, the owner's words and what the engineers told
// the owner, which every message to the owner brings the architect too, while nothing was said since; and the check of
// the policy asked for with no why of its own, which only a new session's understanding asks for, while the policy in
// force and the constitution are the same. Every other cell that asks for the check gives its why.
#[test]
fn a_matter_that_comes_again_is_not_queued_on_the_same_facts() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let no_why: Vec<&str> = g
        .cells
        .iter()
        .filter(|cell| {
            cell.rows.iter().any(|r| {
                r.actions
                    .iter()
                    .any(|d| d.name() == "queue_check_policy" && d.at().is_none())
            })
        })
        .map(|cell| cell.id.as_str())
        .collect();
    assert_eq!(no_why, ["U1"]);
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let action = |name: &str| v["engine"]["actions"][name].as_str().unwrap().to_string();
    assert!(
        action("queue_slots_free").contains("or the free slots and the items ready are those the last handled held")
    );
    assert!(
        action("queue_group_talk")
            .contains("and what the engineers told the owner, which the architect is to know as well")
    );
    assert!(action("queue_check_policy").contains(
        "One asked for with no why of its own, as a new session's understanding asks for it, is not queued while the \
         policy in force and the constitution are those the last check handled had."
    ));
}

// A part several cells' own answers hold alike, such as the architect's effects, is given once in the forms every
// return shares, and each of those cells' own format refers to it there; a line none of whose cells holds it is not
// given it. A part given once must be in two cells' own answers or more.
#[test]
fn a_part_several_cells_hold_alike_is_given_once() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let straight = flow::formats::cell_format(g, g.cell("H1").unwrap());
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
    let architect = flow::formats::common_format_of(g, &line("H1"));
    assert_eq!(architect.matches(r#""item_changes""#).count(), 1, "{architect}");
    let review = flow::formats::common_format_of(g, &line("B2"));
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
        assert!(flow::formats::common_format(&asked).contains("flow_feedback"));
        assert!(!flow::formats::common_format(&off).contains("flow_feedback"));
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
            "flagged",
            "guarded",
            "cannot_run",
            "prompt_too_long",
            "unparsed"
        ]
    );
    assert_eq!(r.guard.as_ref().map(|x| x.samples), Some(19));
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
                "transient_too_often",
                "transient",
                "flagged",
                "guarded",
                "cannot_run",
                "prompt_too_long",
                "unparsed",
                "cut_off",
                "cannot_step",
                "step_network_too_often",
                "step_network",
                "repeats",
                "work_ended"
            ],
            "{}",
            cell.id
        );
        assert!(shared.iter().all(|x| x.to == "@self" && x.fallback && !x.common));
        // The rows that run again count, and no other does; the row for being taken too often is the pause.
        let counts: Vec<(&str, &str)> = shared
            .iter()
            .filter_map(|x| Some((x.name.as_str(), x.counts.as_deref()?)))
            .collect();
        assert_eq!(
            counts,
            [
                ("sent_back", "sent_back"),
                ("transient", "retries"),
                ("step_network", "retries")
            ]
        );
        let pauses: Vec<&str> = shared.iter().filter(|x| x.pauses).map(|x| x.name.as_str()).collect();
        assert_eq!(
            pauses,
            [
                "sent_back_too_often",
                "transient_too_often",
                "flagged",
                "guarded",
                "prompt_too_long",
                "unparsed",
                "cannot_step",
                "step_network_too_often",
                "repeats"
            ]
        );
        let waits: Vec<&str> = shared.iter().filter(|x| x.waits).map(|x| x.name.as_str()).collect();
        assert_eq!(waits, ["usage_limit", "offline", "cannot_run"]);
        assert_eq!(shared[0].conditions, [flow::format::Atom::SentBack(Some(3))]);
    }
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    v["flows"][0]["rounds"]["failed"]["kinds"][3]["later"] = "A3".into();
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(e.contains("unknown field `later`"), "{e}");
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

// Every agent's cell but the reader of the history's, whose task and rule `history` say all it does, is given the
// general rules and the project's design goal; every engineer's cell and the advisor's are given the engineering standards; no cell has an
// input that only says to read something itself; and the forms every return shares say how a return is written. A rule
// given to a line cannot be named by a cell again.
#[test]
fn the_rules_and_the_shared_inputs_reach_every_agent_cell() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let agents = g.cells.iter().filter(|x| x.kind == "agent");
    for cell in agents.filter(|x| x.line.as_deref() != Some("librarian")) {
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
    implement["rules"] = serde_json::json!(["engineering"]);
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(
        e.contains("a cell names a rule every cell of its line is given: I1"),
        "{e}"
    );
}

// The history of the files a change touches is gathered once, by the reader of the history, between the blind review
// and the table review. The table review is given it and may have more looked up; the blind review is given neither,
// and a re-review reaches the table review by another row, not by the gathering.
#[test]
fn the_history_of_the_changed_files_is_gathered_between_the_two_reviews() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let to = |cell: &str, row: &str| {
        let rows = &g.cell(cell).unwrap().rows;
        rows.iter().find(|r| r.name == row).unwrap().to.clone()
    };
    assert_eq!(to("T3", "done"), "G3");
    assert_eq!(to("G3", "done"), "T4");
    for cell in ["T1", "T2", "T7"] {
        assert_eq!(to(cell, "review_again"), "T4", "{cell}");
    }
    let gather = g.cell("G3").unwrap();
    assert_eq!(
        (gather.line.as_deref(), gather.stage.as_deref()),
        (Some("librarian"), Some("test"))
    );
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let blocks = v["flows"][0]["blocks"].as_array().unwrap();
    let develop = blocks.iter().find(|b| b["name"] == "develop").unwrap();
    assert!(develop["nodes"].as_array().unwrap().iter().any(|n| *n == "G3"));
    let brief = flow::format::Src::Place("G3".into(), "brief".into());
    let table = g.cell("T4").unwrap();
    let input = table.inputs.iter().find(|i| i.from == [brief.clone()]).unwrap();
    assert!(input.maybe, "a re-review may come to it with no gathering run");
    assert!(table.prompt.iter().any(|p| p.contains("records as dropped")));
    let blind = g.cell("T3").unwrap();
    assert!(!blind.inputs.iter().any(|i| i.from.contains(&brief)));
    assert!(!format!("{:?} {:?}", blind.prompt, blind.prompt_by).contains("dropped"));
    // The table review can have more looked up, the blind review cannot.
    let history = serde_json::json!({
        "side_reports": [], "flow_feedback": "none", "outcome": { "history": "Was this approach dropped?" }
    });
    assert_eq!(g.take("T4", &history, &Map::new()).unwrap().name, "history");
    let e = g.take("T3", &history, &Map::new()).unwrap_err();
    assert!(e.contains("does not fit the schema"), "{e}");
}

// What every gatherer of history is told is one rule of the flow, which the reader's line gives to every cell of it and
// the architect's look-up names: the contract is written once, and what a gatherer does with a path is in it.
#[test]
fn what_every_gatherer_is_told_is_one_rule_with_the_check_of_paths() {
    let opening = "You gather the history an agent needs before it works.";
    assert_eq!(FLOW.matches(opening).count(), 1);
    let c = compiled();
    for (graph, id) in [
        ("engineer", "G1"),
        ("engineer", "G2"),
        ("engineer", "G3"),
        ("engineer", "X5"),
        ("architect", "X5"),
    ] {
        let g = c.graph(graph).unwrap();
        let cell = g.cell(id).unwrap();
        let used = cell.rules.iter().find(|r| r.name == "history").unwrap();
        assert_eq!(used.shared, graph == "engineer", "{graph} {id}");
        assert!(!cell.prompt.iter().any(|p| p.starts_with(opening)), "{graph} {id}");
        let rule = g.rules.iter().find(|r| r.name == "history").unwrap();
        assert!(rule.text[0].starts_with(opening), "{graph} {id}");
        assert!(rule.text.iter().any(|t| t.contains("`(gone)`")), "{graph} {id}");
    }
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
        assert_eq!(flow::formats::common_format(g), flow::formats::common_format(h));
        for (x, y) in g.cells.iter().zip(&h.cells) {
            assert_eq!(x.schema, y.schema, "{} {}", g.name, x.id);
            assert_eq!(x.prompt, y.prompt, "{} {}", g.name, x.id);
            assert_eq!(flow::formats::cell_format(g, x), flow::formats::cell_format(h, y));
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
    // The loop below must run: a file with no cell, or a cell with no sample, would pass it having tried nothing.
    assert!(!samples.is_empty(), "no samples");
    for (cell, xs) in &samples {
        assert!(!xs.is_empty(), "no sample of {cell}");
        for s in xs {
            let given: Map<String, Value> = s["given"].as_object().cloned().unwrap_or_default();
            let row = g
                .take(cell, &s["json"], &given)
                .unwrap_or_else(|e| panic!("{cell}: {e}"));
            assert_eq!(row.name, s["row"].as_str().unwrap(), "a sample of {cell}");
        }
    }
}

// The acceptance takes a condition met by a job of the CI or by reading the change, besides tests and measurements, and
// the conditions table takes a row of either kind beside the others; a kind that is none of these, a row of the CI
// naming no job, and a reading naming neither where nor what fit no schema.
#[test]
fn the_acceptance_and_the_conditions_table_take_a_ci_job_and_a_reading_of_the_diff() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let fits = |cell: &str, sample: &Value, place: &str, value: Value| -> Result<String, String> {
        let mut x = sample.clone();
        x["outcome"][place] = value;
        let given: Map<String, Value> = samples[cell][0]["given"].as_object().cloned().unwrap_or_default();
        g.take(cell, &x, &given).map(|r| r.name.clone())
    };
    let design = &samples["D2"][0]["json"];
    let acceptance = |by: &str| {
        serde_json::json!([
            { "by": "test", "id": "A1", "condition": "It works." },
            { "by": by, "id": "A2", "condition": "The page passes its check." }
        ])
    };
    assert_eq!(fits("D2", design, "acceptance", acceptance("ci")).unwrap(), "first");
    assert_eq!(fits("D2", design, "acceptance", acceptance("diff")).unwrap(), "first");
    let e = fits("D2", design, "acceptance", acceptance("by_hand")).unwrap_err();
    assert!(e.contains("does not fit the schema"), "{e}");
    // Of the conditions table, adversarial testing's and the engineer's alike.
    let sample = &samples["T1"][0]["json"];
    let conditions = |rows: Value| fits("T1", sample, "conditions", rows);
    let ci_job = serde_json::json!({ "how": "ci_job", "id": "A1", "jobs": ["lint"], "steps": ["npm run check:ui"] });
    let read_diff =
        serde_json::json!({ "how": "read_diff", "id": "A2", "where": "flow/flow.json", "what": "The rule." });
    assert_eq!(
        conditions(serde_json::json!([ci_job, read_diff])).unwrap(),
        "for_engineer"
    );
    // The steps may be left out, and the whole CI is a job.
    let whole = serde_json::json!({ "how": "ci_job", "id": "A1", "jobs": ["check"] });
    assert_eq!(conditions(serde_json::json!([whole])).unwrap(), "for_engineer");
    for bad in [
        serde_json::json!({ "how": "ci_job", "id": "A1", "jobs": [] }),
        serde_json::json!({ "how": "ci_job", "id": "A1", "steps": ["npm run check:ui"] }),
        serde_json::json!({ "how": "read_diff", "id": "A2", "where": "flow/flow.json" }),
        serde_json::json!({ "how": "read_diff", "id": "A2", "what": "The rule." }),
        serde_json::json!({ "how": "by_hand", "id": "A2" }),
    ] {
        let e = conditions(serde_json::json!([bad.clone()])).unwrap_err();
        assert!(e.contains("does not fit the schema"), "{bad}: {e}");
    }
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
        x["outcome"]["better"].as_object_mut().unwrap().remove("considered");
    });
    assert!(e.ends_with("outcome.better has no considered"), "{e}");
    let e = refused(&|x| x["outcome"] = Value::String(x["outcome"].to_string()));
    assert!(e.ends_with("outcome must be an object, and is a string"), "{e}");
}

// A return that does not fit in several places says every place at once, each on a line of its own after a first line
// that counts them, where a return with one place says it as ever, on the one line: four `surprise` rows with `why`,
// a missing field the answer requires and a value its field may not take are six places. Each of them is reached through
// the alternative the return picks (the answer of `I1`, the kind of a row, the change of a test).
#[test]
fn a_return_that_does_not_fit_in_several_places_says_every_place() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let done = samples["I1"][2]["json"].clone();
    assert_eq!(g.take("I1", &done, &Map::new()).unwrap().name, "done");
    let mut ret = done.clone();
    ret["outcome"].as_object_mut().unwrap().shift_remove("head");
    ret["outcome"]["test_changes"][0]["change"] = "bogus".into();
    ret["outcome"]["deviations"] = Value::Array(
        (0..4)
            .map(|i| serde_json::json!({ "kind": "surprise", "what": format!("W{i}"), "why": "Y" }))
            .collect(),
    );
    let e = g.take("I1", &ret, &Map::new()).unwrap_err();
    let expected = [
        "the return does not fit the schema of I1 in 6 places:",
        "outcome has no head",
        r#"outcome.test_changes[0].change is "bogus", and must be one of "added", "extended", "removed", "skipped", "relaxed""#,
        "outcome.deviations[0] has why, which is not one of its fields: kind, what",
        "outcome.deviations[1] has why, which is not one of its fields: kind, what",
        "outcome.deviations[2] has why, which is not one of its fields: kind, what",
        "outcome.deviations[3] has why, which is not one of its fields: kind, what",
    ];
    assert_eq!(e.lines().collect::<Vec<_>>(), expected, "{e}");
    // One place is told on one line, as before.
    let mut one = done.clone();
    one["outcome"]["deviations"] = serde_json::json!([{ "kind": "surprise", "what": "W", "why": "Y" }]);
    assert_eq!(
        g.take("I1", &one, &Map::new()).unwrap_err(),
        "the return does not fit the schema of I1: outcome.deviations[0] has why, which is not one of its fields: \
         kind, what"
    );
    // A return that fits has no place.
    let schema = g.cell("I1").unwrap().schema.clone().unwrap();
    assert_eq!(flow::misfits(&schema, &done, ""), Vec::<String>::new());
}

// A null a return holds in a field of another form than its own is taken as left out, and the path of each is told; the
// return then takes its row. A null the return's own form requires still does not fit, and nor does a return of nothing
// but nulls.
#[test]
fn a_null_in_a_field_of_another_form_is_left_out_and_told() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let mut brief = samples["G1"][0]["json"].clone();
    brief["outcome"]["ask"] = Value::Null;
    assert!(g.take("G1", &brief, &Map::new()).is_err());
    let schema = g.cell("G1").unwrap().schema.clone().unwrap();
    assert_eq!(flow::without_null_options(&schema, &mut brief), ["outcome.ask"]);
    assert_eq!(g.take("G1", &brief, &Map::new()).unwrap().name, "done");
    let mut only = samples["G1"][0]["json"].clone();
    only["outcome"] = serde_json::json!({ "ask": null });
    assert_eq!(flow::without_null_options(&schema, &mut only), Vec::<String>::new());
    assert!(g.take("G1", &only, &Map::new()).is_err());
}

// The fix has an answer for a row it fixed in no commit, which gives where the fix is and no commit; the ci tool takes a
// pull request's title and description with no tests, and ci_before, which runs the tests it lists, still needs them.
#[test]
fn a_fix_in_no_commit_and_a_description_pushed_with_no_tests_are_taken() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let mut done = samples["T6"][0]["json"].clone();
    done["outcome"]["rows"] = serde_json::json!([
        { "answer": "fixed_outside_commit", "id": "F1", "where": "The pull request description." }
    ]);
    assert_eq!(g.take("T6", &done, &Map::new()).unwrap().name, "done");
    done["outcome"]["rows"][0].as_object_mut().unwrap().remove("where");
    let e = g.take("T6", &done, &Map::new()).unwrap_err();
    assert!(e.contains("does not fit the schema"), "{e}");
    let asks = |tool: Value| {
        serde_json::json!({ "outcome": { "tools": [tool] }, "side_reports": [], "tell_owner": [],
                            "progress": "p", "flow_feedback": "none" })
    };
    let ci = serde_json::json!({ "tool": "ci", "head": "3f9c2e1", "pr": { "title": "t", "body": "b" } });
    assert_eq!(g.take("T6", &asks(ci), &Map::new()).unwrap().name, "tools");
    let before = serde_json::json!({ "tool": "ci_before", "head": "3f9c2e1", "files": ["tests/a.rs"] });
    let e = g.take("T6", &asks(before), &Map::new()).unwrap_err();
    assert!(e.contains("does not fit the schema"), "{e}");
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

// Only the engineer and the architect tell the owner: an agent's cell of a line that helps them (the reader of the
// history, the review, adversarial testing, the advisor, the review of the architect's batch, the search of
// observations and the pick of the architect's next matter) has no `tell_owner` in its return, its description does not
// name one, and nothing is done with it or shown of it there. The helpers of a work
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
    assert_eq!(
        helpers,
        [
            "G1", "G2", "G4", "F2", "X5", "T1", "T3", "G3", "T4", "A1", "Q1", "B2", "X2", "X5"
        ]
    );
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
    assert_eq!((p.at.as_str(), p.recorded["stage"].as_str()), ("G1", "research"));
    let p = take(&p, "done");
    assert_eq!(p.at, "R1");
    // The look-up of the history goes to its reader, and back to the cell that asked.
    let p = take(&p, "history");
    assert_eq!(
        (p.at.as_str(), p.callers.as_slice()),
        ("X5", ["R1".to_string()].as_slice())
    );
    let p = take(&p, "found");
    assert_eq!(
        (p.at.as_str(), p.from.as_deref(), p.callers.len()),
        ("R1", Some("X5"), 0)
    );
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
    assert_eq!(p.at, "R2");
    let wrong = g.cell("A5").unwrap().rows.iter().find(|r| r.name == "back").unwrap();
    assert!(g.step(&p, wrong).is_err());
    // The history of the code is gathered between the research and the design, and what the work learned after it
    // merged, before its end.
    assert_eq!(take(&p, "stands").at, "G2");
    let gathered = g.cell("G2").unwrap().rows.iter().find(|r| r.name == "done").unwrap();
    assert_eq!(gathered.to, "D1");
    let l1 = g.cell("L1").unwrap().rows.iter().find(|r| r.name == "green").unwrap();
    assert_eq!(l1.to, "L2");
    let l2 = g.cell("L2").unwrap();
    assert_eq!(l2.rows.iter().find(|r| r.name == "done").unwrap().to, "E1");
}

/// A cell's return that takes the row `row`, from the samples, given the values of its sample and those of `more`.
fn sampled(cell: &str, row: &str, more: &[(&str, &str)]) -> (Value, Map<String, Value>) {
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let s = samples[cell].iter().find(|s| s["row"] == row).unwrap();
    let mut given = s["given"].as_object().cloned().unwrap_or_default();
    given.extend(more.iter().map(|(k, v)| (k.to_string(), Value::from(*v))));
    (s["json"].clone(), given)
}

/// The cells a work stands at from its start, each step a return and the values the cell is given, which must take a
/// row; the item waiting to start is past.
fn walked(g: &flow::CompiledGraph, steps: &[(Value, Map<String, Value>)]) -> Vec<String> {
    let ready = g.cell("S0").unwrap().rows.iter().find(|r| r.name == "ready").unwrap();
    let mut p = g.step(&g.start_position(), ready).unwrap();
    let mut at = vec![p.at.clone()];
    for (ret, given) in steps {
        let row = g.take(&p.at, ret, given).unwrap_or_else(|e| panic!("{}: {e}", p.at));
        p = g.step(&p, row).unwrap();
        at.push(p.at.clone());
    }
    at
}

/// The engine's choice of what runs next with no adversarial testing, the full CI not yet green on the draft, and the
/// review as `review_round` says.
fn full_ci_next(review_round: &str) -> (Value, Map<String, Value>) {
    let given = [
        ("full_ci", "not_green"),
        ("review_round", review_round),
        ("interjection", "no"),
    ];
    (
        serde_json::json!({ "plan": { "review": "all", "adversarial": "skipped" } }),
        given.iter().map(|(k, v)| (k.to_string(), Value::from(*v))).collect(),
    )
}

/// The full CI on the draft, green, before the review as `review_round` says.
fn full_ci_green(review_round: &str) -> (Value, Map<String, Value>) {
    (
        serde_json::json!({ "result": { "kind": "green", "head": "a41c0d2" } }),
        Map::from_iter([("review_round".to_string(), Value::from(review_round))]),
    )
}

// A work whose item is marked a bug fix or a feature goes the light route: from the research it gathers like fixes,
// plans the fix and has the plan reviewed, then implements, runs the full CI on the draft and goes straight to the
// table review, never standing at the judgment whether it stands, the design and its checks, adversarial testing or the
// blind review. A work on the normal route goes through every one of them, as it did before the light route.
#[test]
fn a_light_work_plans_in_place_of_the_design_and_goes_straight_to_the_table_review() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let light = walked(
        g,
        &[
            sampled("S1", "started", &[]),
            sampled("G1", "done", &[]),
            sampled("R1", "light", &[]),
            sampled("G4", "done", &[]),
            sampled("F1", "planned", &[]),
            sampled("F2", "sound", &[]),
            sampled("I1", "done", &[]),
            full_ci_next("table"),
            full_ci_green("table"),
            sampled("T4", "done", &[("deliver", "change")]),
        ],
    );
    assert_eq!(
        light,
        ["S1", "G1", "R1", "G4", "F1", "F2", "I1", "T7", "T2", "T4", "M1"]
    );
    let mut not_needed = sampled("D11", "needed", &[]);
    not_needed.0["outcome"]["adversarial"]["verdict"] = "not_needed".into();
    let gathered = (sampled("G4", "done", &[]).0, Map::new());
    let normal = walked(
        g,
        &[
            sampled("S1", "started", &[]),
            sampled("G1", "done", &[]),
            sampled("R1", "done", &[]),
            sampled("R2", "stands", &[]),
            sampled("G2", "done", &[]),
            sampled("D1", "done", &[]),
            sampled("D2", "first", &[]),
            sampled("D3", "present_best", &[]),
            sampled("D4", "no", &[]),
            sampled("D6", "sure", &[]),
            sampled("D7", "worth_it", &[]),
            sampled("D8", "yes", &[]),
            sampled("D9", "on_its_own", &[]),
            sampled("D10", "implementation", &[]),
            not_needed,
            sampled("I1", "done", &[]),
            full_ci_next("blind"),
            full_ci_green("blind"),
            sampled("T3", "done", &[]),
            gathered,
            sampled("T4", "done", &[("deliver", "change")]),
        ],
    );
    assert_eq!(
        normal,
        [
            "S1", "G1", "R1", "R2", "G2", "D1", "D2", "D3", "D4", "D6", "D7", "D8", "D9", "D10", "D11", "I1", "T7",
            "T2", "T3", "G3", "T4", "M1"
        ]
    );
    // The plan the review found sound is locked as the design's acceptance is, with the implementer's new session.
    let sound = g.cell("F2").unwrap().rows.iter().find(|r| r.name == "sound").unwrap();
    let does: Vec<&str> = sound.actions.iter().map(|d| d.name()).collect();
    assert_eq!(does, ["open_session", "lock_acceptance"]);
    // The review of the plan and the table review are one line, and the table review starts no session of its own, so
    // it goes on in the session that reviewed the plan; the blind review always starts its own.
    let (f2, t4, t3) = (g.cell("F2").unwrap(), g.cell("T4").unwrap(), g.cell("T3").unwrap());
    assert_eq!(
        (f2.line.as_deref(), t4.line.as_deref()),
        (Some("review"), Some("review"))
    );
    assert!(!t4.new_session && t4.fresh_session.is_empty());
    assert!(t3.new_session);
    for cell in ["T1", "T2", "T7"] {
        let row = g
            .cell(cell)
            .unwrap()
            .rows
            .iter()
            .find(|r| r.name == "review_first")
            .unwrap();
        assert_eq!(row.to, "T3", "{cell}");
        assert!(row.actions.iter().any(|d| d.name() == "open_session"), "{cell}");
    }
}

// A work on the light route turns to the normal route, to be judged whether it stands, when the research, the plan or
// the review of the plan says it needs it, or when the review sends the plan back a third time, and to design when the
// way on after a reply or an interjection is a redesign; each of these rows counts `route_to_normal`. A plan sent back before that goes back to be planned again, and counts as a revision.
#[test]
fn a_light_work_turns_to_the_normal_route_by_rows_that_count_it() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    for (cell, row, more) in [
        ("R1", "to_normal", vec![]),
        ("F1", "to_normal", vec![]),
        ("F2", "to_normal", vec![]),
        ("F2", "revised_out", vec![("revisions_before", "two")]),
    ] {
        let (ret, given) = sampled(cell, row, &more);
        let taken = g.take(cell, &ret, &given).unwrap();
        assert_eq!(
            (taken.name.as_str(), taken.to.as_str(), taken.counts.as_deref()),
            (row, "R2", Some("route_to_normal")),
            "{cell}"
        );
    }
    // The third time the review sends the plan back is told apart from the first two by the engine's value alone.
    let (ret, given) = sampled("F2", "revise", &[]);
    let taken = g.take("F2", &ret, &given).unwrap();
    assert_eq!(
        (taken.name.as_str(), taken.to.as_str(), taken.counts.as_deref()),
        ("revise", "F1", Some("plan_revisions"))
    );
    let (_, two) = sampled("F2", "revised_out", &[]);
    assert_eq!(g.take("F2", &ret, &two).unwrap().name, "revised_out");
    // A work redesigned after a reply or an interjection goes to design either way, and is counted only when it was on
    // the light route.
    let redesign = serde_json::json!({
        "outcome": { "next": { "way": "redesign" } },
        "tell_owner": [], "side_reports": [], "progress": "progress", "flow_feedback": "none",
    });
    for (route, row, counts) in [
        ("light", "redesign_from_light", Some("route_to_normal")),
        ("normal", "redesign", None),
    ] {
        let given = Map::from_iter(
            [
                ("stage", "code"),
                ("split", "not_split"),
                ("must_wait", "no"),
                ("route_taken", route),
            ]
            .map(|(k, v)| (k.to_string(), Value::from(v))),
        );
        let taken = g.take("A4", &redesign, &given).unwrap();
        assert_eq!(
            (taken.name.as_str(), taken.to.as_str(), taken.counts.as_deref()),
            (row, "D1", counts),
            "{route}"
        );
    }
    // The research of a work its item does not mark goes the normal route whatever it says of the route; one on the
    // light route that says nothing of it goes on light, which the engine's check of the research sends back first.
    let (ret, _) = sampled("R1", "light", &[]);
    let normal = Map::from_iter([("route_taken".to_string(), Value::from("normal"))]);
    assert_eq!(g.take("R1", &ret, &normal).unwrap().name, "done");
    let r1 = g.cell("R1").unwrap();
    assert!(
        r1.checks
            .iter()
            .any(|c| c.check == "route_given" && c.at.as_deref() == Some("route"))
    );
}

// The architect's line and the pick of its next matter say their session expires with the CLI's prompt cache, so that
// every cell on them carries that, and no cell of any other line does. A flow version stored before lines could say so
// has no such field in its cells, and reads back with it false.
#[test]
fn only_the_architects_line_expires_with_the_cache() {
    let c = compiled();
    let mut expiring = Vec::new();
    for g in &c.graphs {
        for x in &g.cells {
            if x.cache_expiry {
                assert!(
                    matches!(x.line.as_deref(), Some("architect" | "picker")),
                    "{} {}",
                    g.name,
                    x.id
                );
                expiring.push((g.name.as_str(), x.id.as_str()));
            }
        }
    }
    let architect = c.graph("architect").unwrap();
    let on_line: Vec<&str> = architect
        .cells
        .iter()
        .filter(|x| x.line.as_deref() == Some("architect"))
        .map(|x| x.id.as_str())
        .collect();
    assert!(on_line.contains(&"U1") && on_line.contains(&"N1"), "{on_line:?}");
    assert!(
        on_line.iter().all(|id| expiring.contains(&("architect", *id))),
        "{on_line:?} against {expiring:?}"
    );
    // The architect's other lines (its batch's review, its search) and the engineer's lines go on as they did.
    assert!(!architect.cell("X2").unwrap().cache_expiry);
    assert!(c.graph("engineer").unwrap().cells.iter().all(|x| !x.cache_expiry));

    // What is stored of a flow version from before holds the field for no cell; it reads back false for each.
    let mut stored: Value = serde_json::from_str(&c.json()).unwrap();
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("cache_expiry");
                m.values_mut().for_each(strip);
            }
            Value::Array(xs) => xs.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut stored);
    let old: flow::Compiled = serde_json::from_value(stored).unwrap();
    assert!(old.graphs.iter().all(|g| g.cells.iter().all(|x| !x.cache_expiry)));
}

/// The sample return of a row of a cell of the engineer's graph, and what the row was given.
fn sample(cell: &str, row: &str) -> (Value, Map<String, Value>) {
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let s = samples[cell].iter().find(|s| s["row"] == row).unwrap();
    (s["json"].clone(), s["given"].as_object().cloned().unwrap_or_default())
}

/// A cell of the engineer's graph as `flow/flow.json` writes it.
fn written(id: &str) -> Value {
    let flow: Value = serde_json::from_str(FLOW).unwrap();
    flow["flows"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .unwrap_or_else(|| panic!("no cell {id}"))
        .clone()
}

// Item #217, R231-21: a design that decided by reading the engine's records has an answer of its own, with a field that
// says what it read, beside `not_measured` and `measured`; each answer takes its own field and no other's. And
// R241-28, in the same compile of the flow, which is most of what the test costs: a test is written once, so that the
// forms of implementing and of fixing take it so.
#[test]
fn the_engineer_forms_take_the_answer_from_records_and_a_test_written_once() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    assert_a_test_is_written_once_and_the_forms_have_no_tables_of_it(g);
    let (first, given) = sample("D2", "first");
    assert_eq!(g.take("D2", &first, &given).unwrap().name, "first");
    let measure = |x: Value| {
        let mut ret = first.clone();
        ret["outcome"]["measure"] = x;
        g.take("D2", &ret, &given)
    };
    let read = serde_json::json!({ "verdict": "from_records", "read": "The rounds of the item in the database." });
    assert_eq!(measure(read).unwrap().name, "first");
    let not_measured = serde_json::json!({ "verdict": "not_measured", "why": "Nothing here is slow." });
    assert_eq!(measure(not_measured).unwrap().name, "first");
    // `from_records` has no `why`, and without its own `read` it is not an answer.
    let e = measure(serde_json::json!({ "verdict": "from_records", "why": "x" })).unwrap_err();
    assert!(e.contains("outcome.measure"), "{e}");
}

// Item #217, R241-28: an engineer writes a test once, as a row of the test changes with its kind and its two results,
// or as an `extended` row for a test that was there and tests more; the forms of implementing and of fixing have no
// tables of the same tests beside it.
fn assert_a_test_is_written_once_and_the_forms_have_no_tables_of_it(g: &flow::CompiledGraph) {
    for cell in ["I1", "T6"] {
        let (done, given) = sample(cell, "done");
        let mut once = done.clone();
        once["outcome"]["test_changes"] = serde_json::json!([
            { "change": "extended", "test": "every_sample_takes_its_row", "catches": "c", "kind": "new_behaviour",
              "on_head": "green", "on_base": "red", "time": "1s", "worth": "w" },
            { "change": "added", "test": "a_new_one", "catches": "c", "kind": "old_behaviour",
              "on_head": "green", "on_base": "green", "time": "1s", "worth": "w" },
        ]);
        assert_eq!(g.take(cell, &once, &given).unwrap().name, "done", "{cell}");
        // Both results are left out when the design chose adversarial testing, which runs the tests.
        let mut bare = once.clone();
        for row in bare["outcome"]["test_changes"].as_array_mut().unwrap() {
            let row = row.as_object_mut().unwrap();
            row.shift_remove("on_head");
            row.shift_remove("on_base");
        }
        assert_eq!(g.take(cell, &bare, &given).unwrap().name, "done", "{cell}");
        // A result the form does not have is refused, and so is a table of the old forms.
        let mut wrong = once.clone();
        wrong["outcome"]["test_changes"][0]["on_base"] = "maybe".into();
        assert!(g.take(cell, &wrong, &given).unwrap_err().contains("on_base"), "{cell}");
        for table in ["new_tests", "guard_tests"] {
            let mut old = once.clone();
            old["outcome"][table] = serde_json::json!([]);
            let e = g.take(cell, &old, &given).unwrap_err();
            assert!(e.contains(table), "{cell} {table}: {e}");
        }
    }
}

// Item #217, R232-45: the inputs that give the locked acceptance say how it changes, and the advisor's field says that
// what it holds replaces all of it. The ways: the advisor's `new_acceptance`, and a design after the owner disagreed;
// not an edit of the item's text.
#[test]
fn the_inputs_say_how_the_locked_acceptance_changes_and_the_advisors_field_says_it_replaces_all() {
    for (id, name) in [
        ("D2", "Whether the acceptance is locked"),
        ("I1", "Locked acceptance"),
        ("T6", "Locked acceptance"),
    ] {
        let cell = written(id);
        let input = cell["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == name)
            .unwrap();
        let what = input["what"].as_str().unwrap();
        for said in [
            "new_acceptance, which replaces all of them (a condition it does not list is dropped)",
            "disagreed with a decision of the design or chose another option",
            "Editing the item's text, by the architect or the owner, does not change them",
            "use ask to the advisor",
        ] {
            assert!(what.contains(said), "{id} {name}: {said}\n{what}");
        }
        assert!(!what.contains("acceptance_change"), "{id}: {what}");
    }
    let advisor = written("A1");
    let field = advisor["out"][0]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "new_acceptance")
        .unwrap();
    let desc = field["desc"].as_str().unwrap();
    assert!(
        desc.contains("They replace all the locked conditions") && desc.contains("a condition not listed is dropped"),
        "{desc}"
    );
}
