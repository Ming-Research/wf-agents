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

/// What of a cell decides whether a work that has begun it moves to a new flow version where it stands or goes back to
/// do it again, as the engine's `versions::plan` compares a cell's two versions: its kind and line, and the ids, names
/// and sources of the inputs of its own, in their order (those every agent's cell has, and the cell's rows, a work
/// under way takes on as they change); hashed short, the file of the cell in `tests/moves/`.
fn moves_by(c: &flow::CompiledCell) -> String {
    let inputs: Vec<Value> = c
        .inputs
        .iter()
        .filter(|x| !x.common)
        .map(|x| serde_json::json!([x.id, x.name, x.from]))
        .collect();
    let text = serde_json::json!([c.kind, c.line, inputs]).to_string();
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

// The parts of each cell of the engineer's graph that decide whether a work that has begun it moves to a new flow
// version are as the file of the cell in `tests/moves/` has them, so that a change of the flow shows in the change's
// files which cells a work under way goes back to through cell V1 when the version is installed, at the cost of the
// cell done again. The comparison is shown to fail for a hash that differs, a cell with no file and a file with no
// cell, each naming the cell.
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
        "cells a work under way at goes back to do again:\n{}",
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

// The name of a project's main branch is that project's setting (`main`), so the flow writes it `origin/<main>`: a command
// the flow gives with `origin/main` written out names no ref in a project whose main branch has another name.
#[test]
fn the_flow_writes_a_projects_main_branch_by_no_name_of_its_own() {
    assert!(FLOW.contains("origin/<main>"));
    let written: Vec<&str> = FLOW.lines().filter(|l| l.contains("origin/main")).collect();
    assert!(written.is_empty(), "{written:?}");
}

// The cells that read the histories and the observations judge what matters in a great many records, so they think:
// the librarian's two cells and the architect's look-up at high effort, the search of observations at medium, and no
// cell of any graph at low, which does not think. A line cannot hold low cells beside others.
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
    for (graph, cell) in [("engineer", "R0"), ("engineer", "X5"), ("architect", "X5")] {
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
// place of its round: on to the re-check, where its answer goes.
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
    assert_eq!(row.to, "T4");
    assert_eq!(
        serde_json::to_value(&row.conditions).unwrap(),
        serde_json::json!([["engine", "nothing_to_answer"]])
    );
    assert!(!row.common && !row.fallback && row.counts.is_none());
    assert_eq!(t6.rows.iter().find(|r| r.name == "done").unwrap().to, "T4");
}

// The architect's graph compiles with no finding, as the design the owner approved: where it waits for a work to start
// (`S0`), what comes next in the engine's fixed order and where the matter goes (`Q0`, `Q2`), the base read anew and
// planned in a new session (`U1`, `N1`), each matter's path in a session forked from the base, main's CI red (`L1`,
// `L2`), the check of the policy, the writing of a new one and its taking effect (`P1`, `P2`, `P12`), the second eye
// (`G1`, `G2`), the end (`E2`), the owner's ask that holds only its matter (`A3`), the tools (`X1`), the observer
// (`X2`), the wait for the owner's Start after a pause (`X3`) and the bringing of a work to a new flow version (`V1`).
// It has no pick of the next matter, no batch, its review, its revision, the recording of its decisions or the rethink
// after it, no wait for the owner's reply, no cell of slots free or of what was said in the groups apart from the
// planning and what changed, and none of the setting of a policy as a stage of its own. The owner's words reach the
// architect only through its queue: no cell of it takes an interjection, and none answers one.
#[test]
fn the_architect_graph_has_no_cell_the_design_left_out() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    for gone in [
        "E1", "H6", "H10", "H11", "H12", "P3", "P4", "P5", "P6", "P7", "P8", "P9", "P10", "P11", "Q1", "S1", "B1",
        "B2", "B3", "B4", "B5",
    ] {
        assert!(g.cell(gone).is_none(), "{gone}");
    }
    // Only the look at the queue, when a planning or a rebuild of the base is due, reads the project anew.
    let into = |cell: &str| -> Vec<&str> {
        g.cells
            .iter()
            .filter(|x| x.rows.iter().any(|r| r.to == cell && !r.common))
            .map(|x| x.id.as_str())
            .collect()
    };
    assert_eq!(into("U1"), ["Q0"]);
    assert!(g.cell("X4").is_none());
    assert!(
        g.cells
            .iter()
            .all(|x| x.rows.iter().all(|r| !r.interject && r.name != "answers"))
    );
    assert!(c.graph("policy").is_none());
}

// Every agent's cell of the base and of a matter's session, but the second eye and the observer's search, may rewrite
// sections of the notebook in its own answer, beside its effects, the edit made at once and its size checked; reading
// the project and the plan must, each its own sections. Each such cell is given the four sections, each an engine
// input of its own, but reading the project, which writes what the project is now afresh and is given the plan to say
// whether it holds, what is unclear, as questions to settle, and the other section; no cell reads the state of things,
// the notes, the log or the last plan, which the notebook took the place of.
#[test]
fn every_cell_of_the_architect_s_line_keeps_the_notebook() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let nodes = v["flows"][1]["nodes"].as_array().unwrap();
    let sections = ["project_now", "plan", "unclear", "other"];
    let mut keep = Vec::new();
    for cell in &g.cells {
        if cell.kind != "agent" || !matches!(cell.line.as_deref(), Some("architect" | "matter")) {
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
        let want: &[&str] = if cell.id == "U1" { &sections[1..] } else { &sections };
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
    // No total of the graph's cells is held here; the cells named are those other tests read the notebook of.
    for id in ["U1", "N1", "H3", "L2"] {
        assert!(keep.contains(&id), "{id} is not among {keep:?}");
    }
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
    for cell in ["G2", "X2"] {
        assert!(
            g.cell(cell).unwrap().always.iter().all(|a| a.at != "notebook"),
            "{cell}"
        );
    }
    assert_eq!(g.cell("N1").unwrap().title, "The plan");
}

// Each matter is handled in a session forked from the base: the lines of a matter's session and of the second eye fork
// from the base's line, which forks from none, and the compiler gives each of their cells the line it forks from; a line
// that forks from itself, from a line its graph has not, or from a line that forks itself is refused.
#[test]
fn a_matter_s_session_and_the_second_eye_fork_from_the_base() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    for (cell, of) in [
        ("H3", Some("architect")),
        ("L2", Some("architect")),
        ("G2", Some("architect")),
        ("U1", None),
        ("N1", None),
        ("X2", None),
    ] {
        assert_eq!(g.cell(cell).unwrap().fork_of.as_deref(), of, "{cell}");
    }
    assert!(
        !g.cell("N1").unwrap().cache_expiry,
        "a cold cache is no reason to read the project again"
    );
    let line = |v: &mut Value, name: &str| -> usize {
        v["flows"][1]["sessions"]["lines"]
            .as_array()
            .unwrap()
            .iter()
            .position(|l| l["name"] == name)
            .unwrap()
    };
    for (to, said) in [
        (
            "matter",
            "a line forks from no other line of its graph that forks from none: matter forks matter",
        ),
        (
            "nowhere",
            "a line forks from no other line of its graph that forks from none: matter forks nowhere",
        ),
        (
            "second_eye",
            "a line forks from no other line of its graph that forks from none: matter forks second_eye",
        ),
    ] {
        let mut v: Value = serde_json::from_str(FLOW).unwrap();
        let i = line(&mut v, "matter");
        v["flows"][1]["sessions"]["lines"][i]["forks"] = Value::String(to.into());
        let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
        assert!(e.contains(said), "{e}");
    }
}

// What comes next is the engine's to take, in a fixed order and by fact (`Q0`): the owner's words or reply, main's CI
// red, a matter a work waits on, a planning or a rebuild of the base, what the plan queued, what waited for the planning,
// or nothing, which ends the work. A planning and a rebuild read the project anew in a new session each time, which
// plans when a planning is due or the plan no longer holds; every plan closes what it answers. No row of the graph waits
// but at its start: a question to the owner parks only its matter, and a row that waits elsewhere is refused.
#[test]
fn what_comes_next_is_taken_in_a_fixed_order_and_the_base_is_read_anew() {
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
    let next = |verdict: &str, more: Value| {
        let mut out = serde_json::json!({ "next": { "verdict": verdict } });
        if let Value::Object(o) = more {
            for (k, v) in o {
                out["next"][k] = v;
            }
        }
        g.take("Q0", &out, &Map::new()).unwrap().clone()
    };
    for verdict in ["owner", "main_red", "waiting", "planned", "gathered"] {
        assert_eq!(next(verdict, serde_json::json!({ "matter": 3 })).to, "Q2", "{verdict}");
    }
    for (verdict, recorded) in [("plan", "plan"), ("rebuild", "rebuild")] {
        let r = next(verdict, serde_json::json!({ "why": "w" }));
        assert_eq!(r.to, "U1", "{verdict}");
        assert!(does(&r).iter().any(|d| d == "fresh_session"), "{verdict}");
        assert_eq!(
            r.sets.get("rebuild_for").map(String::as_str),
            Some(recorded),
            "{verdict}"
        );
    }
    assert_eq!(next("none", Value::Null).to, "E2");
    let given = |v: &str| Map::from_iter([("rebuild_for".to_string(), Value::String(v.into()))]);
    let u1 = |verdict: &str, for_: &str| {
        let ret = serde_json::json!({
            "outcome": { "notebook": [{ "section": "project_now", "text": "t" }, { "section": "unclear", "text": "u" }],
                         "plan": { "verdict": verdict, "why": "w" } },
            "progress": "p", "tell_owner": [], "flow_feedback": "none",
        });
        g.take("U1", &ret, &given(for_)).unwrap().clone()
    };
    assert_eq!(u1("holds", "plan").to, "N1");
    assert_eq!(u1("wrong", "plan").to, "N1");
    assert_eq!(u1("holds", "rebuild").to, "Q0");
    assert_eq!(u1("wrong", "rebuild").to, "N1");
    for name in ["fits", "some_to_change", "set_straight"] {
        let r = row("N1", name);
        assert_eq!(r.to, "Q0", "{name}");
        assert!(does(&r).iter().any(|d| d == "close_planning"), "{name}");
    }
    // The policy is a matter too, in a fork of the base: its check closes through the second eye's look at what it
    // handed in, writes a new policy in the same session, and the draft parks the matter for the owner's approval.
    let route = |v: &str| Map::from_iter([("route".to_string(), Value::String(v.into()))]);
    for at in ["P1", "P2", "P12", "L1", "L2", "H3", "H5"] {
        assert_eq!(g.take("Q2", &serde_json::json!({}), &route(at)).unwrap().to, at);
    }
    assert_eq!(row("P1", "holds").to, "G1");
    assert_eq!(row("P1", "write_policy").to, "P2");
    assert_eq!(
        (row("P2", "written").to, does(&row("P2", "written"))),
        ("Q0".to_string(), vec!["ask_approval".to_string()])
    );
    assert_eq!(
        does(&row("P12", "next")),
        ["queue_measures".to_string(), "close_matter".to_string()]
    );
    assert_eq!(row("H2", "found").to, "H3");
    assert_eq!(row("H4", "found").to, "H5");
    let held = row("A3", "held");
    assert!(held.drops && held.to == "Q0");
    // A row every agent's cell has, as the wait for the command line's usage limit, is the engine's, not the graph's.
    let agents: Vec<&flow::CompiledCell> = g.cells.iter().filter(|c| c.kind == "agent").collect();
    let shared = |name: &str| agents.iter().all(|c| c.rows.iter().any(|r| r.name == name && r.waits));
    for cell in &g.cells {
        for r in cell.rows.iter().filter(|r| r.waits && !shared(&r.name)) {
            assert!(cell.id == "S0" || cell.id == "X3", "{} {}", cell.id, r.name);
        }
    }
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let look = v["flows"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "Q0")
        .unwrap();
    let none = look["cases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["name"] == "none")
        .unwrap();
    none["waits"] = Value::Bool(true);
    none["to"] = Value::String("Q0".into());
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(e.contains("the architect's own graph stops: Q0 none waits"), "{e}");
}

// Each cell that ends a matter leaves its line of the decision log and its two answers (`closing`), which the engine
// keeps and checks, and goes to the second eye's gate (`G1`): with no change of a kind the second eye checks, the
// matter closes; with one, the second eye forked from the same base looks (`G2`), and what it finds sound is made and
// the matter closes, or the changes go back to the cell that handed them in, which is given what it found.
#[test]
fn each_matter_ends_with_its_log_line_and_the_second_eye_where_a_change_can_lose_something() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    for id in ["H1", "H3", "H5", "H7", "H8", "H9", "L2", "P1"] {
        let cell = g.cell(id).unwrap();
        assert!(
            cell.always
                .iter()
                .any(|a| a.at == "closing" && a.action.as_deref() == Some("log_matter")),
            "{id}"
        );
        assert!(
            cell.checks
                .iter()
                .any(|x| x.at.as_deref() == Some("closing") && x.check == "closing_given"),
            "{id}"
        );
        assert!(cell.rows.iter().any(|r| r.to == "G1" && !r.common), "{id}");
        assert!(
            cell.inputs
                .iter()
                .any(|x| x.from == [flow::format::Src::Place("G2".into(), "check".into())]),
            "{id} is not given what the second eye found"
        );
        let schema = cell.schema.as_ref().unwrap().to_string();
        assert!(schema.contains("\"closing\""), "{id}");
    }
    for id in ["H2", "H4"] {
        assert!(g.cell(id).unwrap().always.iter().all(|a| a.at != "closing"), "{id}");
    }
    let gate = |v: &str| Map::from_iter([("second_eye".to_string(), Value::String(v.into()))]);
    let none = g.take("G1", &serde_json::json!({}), &gate("none")).unwrap();
    assert_eq!(none.to, "Q0");
    assert!(none.actions.iter().any(|d| d.name() == "close_matter"));
    assert_eq!(g.take("G1", &serde_json::json!({}), &gate("needed")).unwrap().to, "G2");
    let eye = |verdict: &str| {
        let check = if verdict == "sound" {
            serde_json::json!({ "verdict": "sound", "why": "w" })
        } else {
            serde_json::json!({ "verdict": "problems", "problems": [{ "change": 1, "kind": "loses", "what": "w",
                                "fact": "f", "fix": "x" }] })
        };
        let ret = serde_json::json!({ "outcome": { "check": check }, "progress": "p", "flow_feedback": "none" });
        g.take("G2", &ret, &Map::new()).unwrap().clone()
    };
    let sound = eye("sound");
    assert_eq!(sound.to, "Q0");
    assert_eq!(
        sound.actions.iter().map(|d| d.name()).collect::<Vec<_>>(),
        ["apply_checked", "close_matter"]
    );
    let problems = eye("problems");
    assert_eq!(problems.to, "Q2");
    // What it found goes back as its return is taken, before the way back is read: done at what it found, not by the
    // row, so that only a return that found problems does it.
    assert!(problems.actions.is_empty());
    assert!(
        g.cell("G2")
            .unwrap()
            .always
            .iter()
            .any(|a| a.action.as_deref() == Some("return_checked") && a.at == "check[problems].problems")
    );
    // The second eye runs on the architect's level as the matter's session does, and never asks, searches or looks up
    // the history.
    let g2 = g.cell("G2").unwrap();
    assert!(
        g2.rows
            .iter()
            .all(|r| !["ask_owner", "search", "history"].contains(&r.name.as_str()))
    );
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
            .contains("reported three times or more is folded into an item by a change of the round")
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
    let ways = c.graph("architect").unwrap().cell("H3").unwrap();
    assert!(ways.prompt_by.is_none() && ways.inenums.is_empty());
}

// What the owner alone wants of a work its engineer asks the owner, at the owner's cell (`A3`); what only the owner can
// do or decide besides reaches the owner through the architect. The owner holds a work's blocker there, at the review of
// the answers the work asked for while it went on, at the merge gate (`M1`), and nowhere else; the
// architect hands nothing on to the owner (`A2` has no `to_owner`, and its reply hands on only to a related project's
// architect). The engine's rows of a flow anomaly at `A4` raise the task's blocker for the architect at `A2`, which is
// given the anomaly; the ways (`H3`) put a choice of the owner's as an open question of the work's design, and an
// action only the owner can do to the owner by pausing the work. An open question left to the owner they pin in the
// owner's one-to-one by an ask, and answer the engineer by the owner's reply.
#[test]
fn what_only_the_owner_wants_is_asked_of_the_owner_and_the_rest_goes_through_the_architect() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let owner = g.cell("A3").unwrap();
    assert_eq!(
        (owner.kind.as_str(), owner.step.as_deref()),
        ("owner", Some("ask_owner"))
    );
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let engineer = v["flows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "engineer")
        .unwrap();
    let role = engineer["asking"]["roles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "owner")
        .unwrap();
    assert_eq!(role["node"], "A3");
    assert!(
        role["desc"]
            .as_str()
            .unwrap()
            .contains("what the owner wants of this work")
    );
    let held: Vec<&str> = g
        .blockers
        .as_ref()
        .unwrap()
        .cells
        .iter()
        .filter(|b| b.holders.iter().any(|h| h == "owner"))
        .map(|b| b.cell.as_str())
        .collect();
    assert_eq!(held, ["A3", "M1"]);
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
// slots and the items ready are the same; and the check of the policy asked for with no why of its own, which only
// reading the project anew asks for, while the policy in force and the constitution are the same. Every other cell that
// asks for the check gives its why.
#[test]
fn a_matter_that_comes_again_is_not_queued_on_the_same_facts() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    // A cell asks with a why of its own by a place that holds one, a `why`; reading the project asks at its plan's
    // verdict, which holds none.
    let no_why: Vec<&str> = g
        .cells
        .iter()
        .filter(|cell| {
            let by_row = cell.rows.iter().any(|r| {
                r.actions
                    .iter()
                    .any(|d| d.name() == "queue_check_policy" && !d.at().is_some_and(|at| at.ends_with("why")))
            });
            let always = cell
                .always
                .iter()
                .any(|a| a.action.as_deref() == Some("queue_check_policy") && !a.at.ends_with("why"));
            by_row || always
        })
        .map(|cell| cell.id.as_str())
        .collect();
    assert_eq!(no_why, ["U1"]);
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let action = |name: &str| v["engine"]["actions"][name].as_str().unwrap().to_string();
    assert!(
        action("queue_slots_free").contains("or the free slots and the items ready are those the last handled held")
    );
    assert!(action("queue_check_policy").contains(
        "One asked for with no why of its own, as reading the project anew asks for it, is not queued while the policy \
         in force and the constitution are those the last check handled had."
    ));
}

// Item #239: the owner can hold a project's new starts, and the architect reads the holds. Every cell of the
// architect that can hand in a hold or a release among its effects is given the board, where the holds are listed,
// so that a cell added later which hands in effects without the board is found here; and the texts say who holds,
// who releases which hold and when the engine queues no matter of free slots.
#[test]
fn every_cell_that_can_hold_or_release_new_starts_is_given_the_holds_and_the_texts_say_who_releases_which() {
    let c = compiled();
    let g = c.graph("architect").unwrap();
    let effects = r#""effects":{"$ref":"format:common#/$defs/effects"}"#;
    let gives_effects =
        |cell: &flow::CompiledCell| cell.kind == "agent" && flow::formats::cell_format(g, cell).contains(effects);
    let handing_effects: Vec<&str> = g
        .cells
        .iter()
        .filter(|cell| gives_effects(cell))
        .map(|cell| cell.id.as_str())
        .collect();
    // No total of the graph's cells is held here: a work that adds a cell handing in effects would rewrite it. The
    // two cells named are the ones other tests here read the effects of, so the filter is shown to find cells at all.
    for id in ["H1", "H3"] {
        assert!(
            handing_effects.contains(&id),
            "{id} is not found among {handing_effects:?}"
        );
    }
    for id in handing_effects {
        let cell = g.cell(id).unwrap();
        let board = cell
            .inputs
            .iter()
            .any(|i| serde_json::to_string(&i.from).unwrap().contains(r#""engine":"board""#));
        assert!(board, "{id} is given no board, where the architect reads the holds");
    }
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let text = |path: &[&str]| -> String {
        path.iter()
            .fold(&v, |at, k| &at[*k])
            .as_str()
            .unwrap_or_else(|| panic!("{path:?}"))
            .to_string()
    };
    assert!(text(&["engine", "inputs", "board"]).contains("`starts_held`"));
    assert!(text(&["engine", "actions", "queue_slots_free"]).contains("the owner has paused every work"));
    assert!(text(&["engine", "stores", "holds", "owner"]).contains("releases any hold"));
    let held = &v["engine"]["stores"]["holds"]["inputs"];
    assert!(held.as_array().unwrap().iter().any(|i| i == "board"), "{held}");
    // The row of the cell where a new work starts that a hold keeps waiting says who releases which hold.
    let start = c.graph("engineer").unwrap().cell("S0").unwrap();
    let row = start.rows.iter().find(|r| r.name == "project_held").unwrap();
    let note = row.note.as_deref().unwrap();
    assert!(
        note.contains("the owner's, by the owner") && note.contains("by the architect or the owner"),
        "{note}"
    );
    assert!(!note.contains("sets a policy"), "{note}");
    // The architect's release is said to leave the owner's hold alone.
    let release = &v["shapes"]["arch_action"];
    assert!(
        release.to_string().contains("stays until the owner releases it"),
        "{release}"
    );
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
    let second_eye = flow::formats::common_format_of(g, &line("G2"));
    assert!(!second_eye.contains(r#""item_changes""#), "{second_eye}");
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
            // What a return carried from an earlier version must be loses the field alike.
            for (was, is) in on.carried.iter().zip(&cell.carried) {
                assert!(was["properties"].get("flow_feedback").is_some(), "{}", on.id);
                assert!(is["properties"].get("flow_feedback").is_none(), "{}", cell.id);
                assert!(!is["required"].as_array().unwrap().contains(&field), "{}", cell.id);
            }
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
    // Each return sent back of the engineer's work is the project's architect's to look at, and three in a row pause
    // the work; the architect's own work has none above it, and pauses for the owner after three in a row all the same.
    assert_eq!((r.sent_back.pause_after, r.sent_back.look), (3, true));
    let architect = c
        .graph("architect")
        .unwrap()
        .rounds
        .clone()
        .expect("the architect's rounds section");
    assert_eq!((architect.sent_back.pause_after, architect.sent_back.look), (3, false));
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
                "work_ended",
                "steps_without_agent"
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
                "repeats",
                "steps_without_agent"
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
        let engineer = matches!(cell.line.as_deref(), Some("engineer" | "designer" | "advisor"));
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

// What every gatherer of history is told is one rule of the flow, which the reader's line gives to every cell of it and
// the architect's look-up names: the contract is written once, and what a gatherer does with a path is in it.
#[test]
fn what_every_gatherer_is_told_is_one_rule_with_the_check_of_paths() {
    let opening = "You gather the history an agent needs before it works.";
    assert_eq!(FLOW.matches(opening).count(), 1);
    let c = compiled();
    for (graph, id) in [("engineer", "R0"), ("engineer", "X5"), ("architect", "X5")] {
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
            "effects": null,
            "closing": { "log": "Nothing to fix.", "plan": "no", "plan_why": "w", "base": "holds", "base_why": "w" }
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
                            "reply": { "text": "Go on.", "why": "It passed." },
                            "closing": { "log": "Nothing to fix.", "plan": "no", "plan_why": "w", "base": "holds",
                                         "base_why": "w" } })
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

// Item #270: adversarial testing goes on, past the engineer's tests, when a guard test cannot be built before the change
// (the review judges it), and goes back to the engineer only when one fails there. The route reads the result alone: a
// guard test of the adversarial tester that cannot be built is stopped earlier, by the check `test_kinds_match`.
#[test]
fn a_guard_test_that_cannot_be_built_before_the_change_does_not_send_testing_back() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let clear = samples["T1"]
        .iter()
        .find(|s| s["row"] == "clear")
        .expect("a sample of the row clear");
    let given: Map<String, Value> = clear["given"].as_object().cloned().unwrap_or_default();
    let row_with = |by: &str, base: &str| {
        let mut x = clear["json"].clone();
        x["outcome"]["guard_tests"][0]["by"] = by.into();
        x["outcome"]["guard_tests"][0]["on_base"] = base.into();
        g.take("T1", &x, &given).unwrap().name.clone()
    };
    for by in ["engineer", "adversarial"] {
        assert_eq!(row_with(by, "green"), "clear", "{by}");
        assert_eq!(row_with(by, "cannot_run"), "clear", "{by}");
        assert_eq!(row_with(by, "red"), "for_engineer", "{by}");
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
        let at = place.split('.').fold(&mut x["outcome"], |v, k| &mut v[k]);
        *at = value;
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
    assert_eq!(
        fits("D2", design, "design.acceptance", acceptance("ci")).unwrap(),
        "done"
    );
    assert_eq!(
        fits("D2", design, "design.acceptance", acceptance("diff")).unwrap(),
        "done"
    );
    let e = fits("D2", design, "design.acceptance", acceptance("by_hand")).unwrap_err();
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
    let s = &samples["M2"][0];
    let mut given = s["given"].as_object().unwrap().clone();
    given.insert("turn".into(), "behind".into());
    assert_eq!(g.take("M2", &s["json"], &given).unwrap().name, "behind");
}

// A return that does not fit says where and how, following the alternative it picks.
#[test]
fn a_return_that_does_not_fit_says_where() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let sample = &samples["T3"][0]["json"];
    let refused = |edit: &dyn Fn(&mut Value)| {
        let mut x = sample.clone();
        edit(&mut x);
        g.take("T3", &x, &Map::new()).unwrap_err()
    };
    let e = refused(&|x| x["outcome"]["silent"]["verdict"] = "maybe".into());
    assert!(
        e.ends_with(r#"outcome.silent.verdict is "maybe", and must be one of "no", "yes""#),
        "{e}"
    );
    let e = refused(&|x| {
        x["outcome"]["silent"].as_object_mut().unwrap().remove("why");
    });
    assert!(e.ends_with("outcome.silent has no why"), "{e}");
    let e = refused(&|x| x["outcome"] = Value::String(x["outcome"].to_string()));
    assert!(e.ends_with("outcome must be an object, and is a string"), "{e}");
}

// A field may name what a return handed in on an earlier flow version held there (`earlier`): absent, or another shape,
// which may itself be left out; a field inside a list's items as well. What such a return is held to when a work moves
// (`carried`) takes it as well as the field now; what an agent is held to is the schema as it would be without the
// marks, which refuses a return of the earlier shape, saying where. A cell no field of which names one has no `carried`.
#[test]
fn what_a_field_was_earlier_is_carried_and_never_handed_in_now() {
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let note = serde_json::json!({ "name": "note", "title": "Note", "type": "text", "desc": "A note on the change" });
    let i1 = v["flows"][0]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "I1")
        .unwrap();
    i1["out"].as_array_mut().unwrap().push(note);
    // A field nobody reads is refused; the owner reads this one on the item.
    i1["shown"].as_array_mut().unwrap().push("note".into());
    let plain = flow::compile(&v.to_string(), &context()).unwrap_or_else(|e| panic!("{e}"));
    let out = v["flows"][0]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "I1")
        .unwrap()["out"]
        .as_array_mut()
        .unwrap();
    let written = out.iter_mut().find(|f| f["name"] == "decisions_written").unwrap();
    written["earlier"] = serde_json::json!({ "type": "text", "optional": true });
    let file = written["item"]["fields"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["name"] == "file")
        .unwrap();
    file["earlier"] = "absent".into();
    let note = out.iter_mut().find(|f| f["name"] == "note").unwrap();
    note["earlier"] = "absent".into();
    let marked = flow::compile(&v.to_string(), &context()).unwrap_or_else(|e| panic!("{e}"));
    let (was, is) = (
        plain.graph("engineer").unwrap().cell("I1").unwrap(),
        marked.graph("engineer").unwrap().cell("I1").unwrap(),
    );
    assert_eq!(is.schema, was.schema);
    assert!(was.carried.is_none());
    let carried = is.carried.as_ref().unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let mut now = samples["I1"].iter().find(|s| s["row"] == "done").unwrap()["json"].clone();
    now["outcome"]["note"] = "Two of them are alike.".into();
    let mut no_file = now.clone();
    for row in no_file["outcome"]["decisions_written"].as_array_mut().unwrap() {
        row.as_object_mut().unwrap().shift_remove("file");
    }
    let mut text = now.clone();
    text["outcome"]["decisions_written"] = "One decision, written in a text.".into();
    text["outcome"].as_object_mut().unwrap().shift_remove("note");
    let mut neither = text.clone();
    neither["outcome"]
        .as_object_mut()
        .unwrap()
        .shift_remove("decisions_written");
    assert!(flow::valid(carried, &now));
    for old in [&no_file, &text, &neither] {
        assert!(
            flow::valid(carried, old),
            "{old}: {:?}",
            flow::misfits(carried, old, "")
        );
        assert!(!flow::valid(is.schema.as_ref().unwrap(), old), "{old}");
    }
    let g = marked.graph("engineer").unwrap();
    let given = samples["I1"].iter().find(|s| s["row"] == "done").unwrap()["given"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let e = g.take("I1", &text, &given).unwrap_err();
    assert!(
        e.contains("outcome.decisions_written must be a list, and is a string"),
        "{e}"
    );
    assert!(e.contains("outcome has no note"), "{e}");
    let e = g.take("I1", &no_file, &given).unwrap_err();
    assert!(e.contains("outcome.decisions_written[0] has no file"), "{e}");
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
    let given: Map<String, Value> = samples["L2"][0]["given"].as_object().cloned().unwrap();
    let mut learned = samples["L2"][0]["json"].clone();
    learned["outcome"]["ask"] = Value::Null;
    assert!(g.take("L2", &learned, &given).is_err());
    let schema = g.cell("L2").unwrap().schema.clone().unwrap();
    assert_eq!(flow::without_null_options(&schema, &mut learned), ["outcome.ask"]);
    assert_eq!(g.take("L2", &learned, &given).unwrap().name, "done");
    let mut only = samples["L2"][0]["json"].clone();
    only["outcome"] = serde_json::json!({ "ask": null });
    assert_eq!(flow::without_null_options(&schema, &mut only), Vec::<String>::new());
    assert!(g.take("L2", &only, &given).is_err());
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

// Only the engineer, its deep research and the architect tell the owner, the architect from its base and from each
// matter's session: an agent's cell of a line that helps them (the reader of the history, the review, adversarial
// testing, the advisor, the second eye on a matter's changes and the search of observations) has no `tell_owner` in its
// return, its description does not name one, and nothing is done with it or shown of it there. The helpers of a work
// still hand in side reports, which the architect reads.
#[test]
fn only_the_engineer_and_the_architect_tell_the_owner() {
    let c = compiled();
    let mut helpers = Vec::new();
    for g in &c.graphs {
        for cell in g.cells.iter().filter(|x| x.kind == "agent") {
            let tells = matches!(
                cell.line.as_deref(),
                Some("engineer" | "designer" | "architect" | "matter")
            );
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
                helpers.push((g.name.as_str(), id.as_str()));
                let reports = s["properties"].get("side_reports").is_some();
                assert_eq!(reports, g.name == "engineer", "{id}");
            }
        }
    }
    // No total of the graphs' cells is held here: the engineer's graph has helpers, and the architect's second eye and
    // its searches are among them.
    assert!(helpers.iter().any(|h| h.0 == "engineer"), "{helpers:?}");
    for id in ["G2", "X2", "X5"] {
        assert!(helpers.contains(&("architect", id)), "{id} is not among {helpers:?}");
    }
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
    assert_eq!((p.at.as_str(), p.recorded["stage"].as_str()), ("R0", "research"));
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
    let wrong = g.cell("A5").unwrap().rows.iter().find(|r| r.name == "back").unwrap();
    assert!(g.step(&p, wrong).is_err());
    // The research goes straight to implement, or to deep research, whose design goes to its review while the work
    // implements by it.
    assert_eq!(take(&p, "start").at, "I1");
    let deep = take(&p, "cannot_start");
    assert_eq!((deep.at.as_str(), deep.recorded["stage"].as_str()), ("D1", "design_on"));
    let p = take(&take(&deep, "done"), "done");
    assert_eq!(p.at, "D3");
    let p = take(&p, "posted");
    assert_eq!((p.at.as_str(), p.recorded["stage"].as_str()), ("I1", "code"));
    // What the work learned is written after it merged, before its end.
    let merged = g.cell("M2").unwrap().rows.iter().find(|r| r.name == "merged").unwrap();
    assert_eq!(merged.to, "L2");
    let l2 = g.cell("L2").unwrap();
    assert_eq!(l2.rows.iter().find(|r| r.name == "done").unwrap().to, "E1");
    assert_eq!(l2.rows.iter().find(|r| r.name == "verify").unwrap().to, "W1");
}

// A work whose acceptance holds a condition written in_effect goes on after what it learned: it waits, holding no slot
// and counting nothing, until its change took effect, and each row that lets it on starts a new session of the engineer's
// line, so that no session is kept warm through the wait. Its looks that find the evidence not there yet are counted,
// and past the limit, or for what does not hold, it reports and ends. The acceptance and the conditions take the kind,
// and a condition with no command to run fits no schema.
#[test]
fn a_work_with_a_condition_in_effect_waits_for_its_change_and_checks_it_with_a_real_run() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let wait = g.cell("W1").unwrap();
    assert_eq!(wait.kind, "engine");
    let row = |cell: &str, name: &str| {
        g.cell(cell)
            .unwrap()
            .rows
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{cell} has no row {name}"))
            .clone()
    };
    for name in ["not_carried", "cannot_tell", "not_due"] {
        let waits = row("W1", name);
        assert!(waits.waits && waits.to == "W1" && waits.counts.is_none(), "{name}");
    }
    let does = |r: &flow::CompiledRow, what: &str| r.actions.iter().any(|d| d.name() == what);
    assert!(does(&row("W1", "cannot_tell"), "tell_wait"));
    assert!(does(&row("W1", "never_in_effect"), "report_verification"));
    assert_eq!(row("W1", "never_in_effect").to, "E1");
    assert_eq!(
        g.cell("W2").unwrap().fresh_session,
        ["W1·words".to_string(), "W1·look".to_string()]
    );
    assert_eq!(row("W2", "again").to, "W1");
    assert_eq!(row("W2", "again").counts.as_deref(), Some("verification_looks"));
    for name in ["past_looks", "failed", "unjudged"] {
        assert!(does(&row("W2", name), "report_verification"), "{name}");
        assert_eq!(row("W2", name).to, "E1");
    }
    assert!(!does(&row("W2", "verified"), "report_verification"));
    // Waiting, each way on, then a look that sends it back, and a look that ends it.
    let waiting = sampled("L2", "verify", &[]);
    let look = engine_step(&[("interjection", "no"), ("in_effect", "yes"), ("look_due", "yes")]);
    let steps = [
        sampled("S1", "started", &[]),
        sampled("R0", "done", &[]),
        sampled("R1", "start", &[]),
        sampled("I1", "done", &[]),
        sampled("T2", "done", &[]),
        sampled("T3", "clear", &[]),
        engine_step(&[("gates", "clear"), ("interjection", "no"), ("cancelled", "no")]),
        sampled("M2", "merged", &[]),
        waiting,
        look.clone(),
        sampled("W2", "again", &[]),
        look,
        sampled("W2", "verified", &[]),
    ];
    assert_eq!(
        walked(g, &steps),
        [
            "S1", "R0", "R1", "I1", "T2", "T3", "M1", "M2", "L2", "W1", "W2", "W1", "W2", "E1"
        ]
    );
    // The kind as a return writes it: every field is asked for.
    let schema = g.cell("D2").unwrap().schema.clone().unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let mut design = samples["D2"][0]["json"].clone();
    let whole = serde_json::json!({
        "by": "in_effect", "id": "A2", "condition": "The median holds.", "when": "merged", "tool": "query",
        "command": "SELECT 1", "holds": "It is 1.", "enough": "none",
    });
    design["outcome"]["design"]["acceptance"] = serde_json::json!([whole.clone()]);
    assert!(flow::valid(&schema, &design));
    let mut no_command = whole;
    no_command.as_object_mut().unwrap().remove("command");
    design["outcome"]["design"]["acceptance"] = serde_json::json!([no_command]);
    assert!(!flow::valid(&schema, &design));
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

/// What an engine's cell is given, with nothing handed in: its values.
fn engine_step(given: &[(&str, &str)]) -> (Value, Map<String, Value>) {
    let given = given.iter().map(|(k, v)| (k.to_string(), Value::from(*v))).collect();
    (serde_json::json!({}), given)
}

// A work whose research settles its approach goes straight to implement in the research's session; one whose research
// cannot goes to deep research, whose design goes to its review while the work implements by it; then the review in two
// passes, the merge gate, the merge queue, what was learned and the end.
#[test]
fn a_work_goes_straight_to_implement_or_through_deep_research_and_its_review() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let tail = [
        sampled("I1", "done", &[]),
        sampled("T2", "done", &[]),
        sampled("T3", "clear", &[]),
        engine_step(&[("gates", "clear"), ("interjection", "no"), ("cancelled", "no")]),
        sampled("M2", "merged", &[]),
        sampled("L2", "done", &[]),
    ];
    let straight = [
        sampled("S1", "started", &[]),
        sampled("R0", "done", &[]),
        sampled("R1", "start", &[]),
    ];
    let steps: Vec<_> = straight.iter().chain(&tail).cloned().collect();
    assert_eq!(
        walked(g, &steps),
        ["S1", "R0", "R1", "I1", "T2", "T3", "M1", "M2", "L2", "E1"]
    );
    let deep = [
        sampled("S1", "started", &[]),
        sampled("R0", "done", &[]),
        sampled("R1", "cannot_start", &[]),
        sampled("D1", "done", &[]),
        sampled("D2", "done", &[]),
        engine_step(&[("interjection", "no")]),
    ];
    let steps: Vec<_> = deep.iter().chain(&tail).cloned().collect();
    assert_eq!(
        walked(g, &steps),
        [
            "S1", "R0", "R1", "D1", "D2", "D3", "I1", "T2", "T3", "M1", "M2", "L2", "E1"
        ]
    );
}

// No line of the flow says its session expires with the CLI's prompt cache: the architect's base is carried from work to
// work and read anew only when a fact calls for it, a cold cache being written once, cheaper than reading the project
// again, and each matter's session ends with its matter. A line that says so has every cell on it carry it, and no
// cell of another line; a flow version stored before lines could say so has no such field in its cells, and reads back
// with it false.
#[test]
fn a_line_expires_with_the_cache_only_where_it_says_so() {
    let c = compiled();
    assert!(c.graphs.iter().all(|g| g.cells.iter().all(|x| !x.cache_expiry)));
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let lines = v["flows"][1]["sessions"]["lines"].as_array_mut().unwrap();
    let base = lines.iter_mut().find(|l| l["name"] == "architect").unwrap();
    base["cacheExpiry"] = Value::Bool(true);
    let said = flow::compile(&v.to_string(), &context()).unwrap();
    let architect = said.graph("architect").unwrap();
    for x in &architect.cells {
        assert_eq!(x.cache_expiry, x.line.as_deref() == Some("architect"), "{}", x.id);
    }
    assert!(architect.cell("U1").unwrap().cache_expiry && architect.cell("N1").unwrap().cache_expiry);
    assert!(said.graph("engineer").unwrap().cells.iter().all(|x| !x.cache_expiry));

    // What is stored of a flow version from before holds the field for no cell; it reads back false for each.
    let mut stored: Value = serde_json::from_str(&said.json()).unwrap();
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
    let (first, given) = sample("D1", "done");
    assert_eq!(g.take("D1", &first, &given).unwrap().name, "done");
    let measure = |x: Value| {
        let mut ret = first.clone();
        ret["outcome"]["measure"] = x;
        g.take("D1", &ret, &given)
    };
    let read = serde_json::json!({ "verdict": "from_records", "read": "The rounds of the item in the database." });
    assert_eq!(measure(read).unwrap().name, "done");
    let not_measured = serde_json::json!({ "verdict": "not_measured", "why": "Nothing here is slow." });
    assert_eq!(measure(not_measured).unwrap().name, "done");
    // `from_records` has no `why`, and without its own `read` it is not an answer.
    let e = measure(serde_json::json!({ "verdict": "from_records", "why": "x" })).unwrap_err();
    assert!(e.contains("outcome.measure"), "{e}");
}

// Item #217, R241-28: an engineer writes a test once, as a row of the test changes with its kind, or as an `extended`
// row for a test that was there and tests more; the engine reads its two results from the runs, and the forms of
// implementing and of fixing have no tables of the same tests beside it, nor a result of the engineer's.
fn assert_a_test_is_written_once_and_the_forms_have_no_tables_of_it(g: &flow::CompiledGraph) {
    for cell in ["I1", "T6"] {
        let (done, given) = sample(cell, "done");
        let mut once = done.clone();
        once["outcome"]["test_changes"] = serde_json::json!([
            { "change": "extended", "test": "every_sample_takes_its_row", "catches": "c", "kind": "new_behaviour" },
            { "change": "added", "test": "a_new_one", "catches": "c", "kind": "old_behaviour", "worth": "w",
              "not_before": "It builds only with the change." },
        ]);
        assert_eq!(g.take(cell, &once, &given).unwrap().name, "done", "{cell}");
        // A result is the engine's to read: one written is refused, and so is a table of the old forms.
        let mut wrong = once.clone();
        wrong["outcome"]["test_changes"][0]["on_base"] = "red".into();
        assert!(g.take(cell, &wrong, &given).unwrap_err().contains("on_base"), "{cell}");
        for table in ["new_tests", "guard_tests"] {
            let mut old = once.clone();
            old["outcome"][table] = serde_json::json!([]);
            let e = g.take(cell, &old, &given).unwrap_err();
            assert!(e.contains(table), "{cell} {table}: {e}");
        }
    }
}

// The acceptance is no longer locked: the inputs that give it say that the implementation, a fix or the advisor may
// change it, saying why, and the advisor's field says that what it holds replaces all of it.
#[test]
fn the_inputs_say_how_the_acceptance_changes_and_the_advisors_field_says_it_replaces_all() {
    for id in ["I1", "T6"] {
        let cell = written(id);
        let input = cell["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == "Acceptance")
            .unwrap();
        let what = input["what"].as_str().unwrap();
        assert!(
            what.contains("as the implementation, a fix or the advisor changed them since, saying why"),
            "{id}: {what}"
        );
        assert!(!what.contains("locked"), "{id}: {what}");
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
        desc.contains("They replace all the conditions on the item")
            && desc.contains("a condition not listed is dropped"),
        "{desc}"
    );
}
