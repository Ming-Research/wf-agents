// Breaks one thing at a time in a copy of the engineer's graph and checks the compiler refuses it with the expected
// finding.

use serde_json::{Value, json};

const FLOW: &str = include_str!("../../../flow/flow.json");

fn context() -> flow::Context {
    flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    }
}

fn graph(v: &mut Value) -> &mut Value {
    &mut v["flows"][0]
}

fn node<'a>(v: &'a mut Value, id: &str) -> &'a mut Value {
    graph(v)["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == id)
        .unwrap()
}

fn rows<'a>(v: &'a mut Value, id: &str) -> &'a mut Vec<Value> {
    let n = node(v, id);
    let key = if n.get("rows").is_some() { "rows" } else { "cases" };
    n[key].as_array_mut().unwrap()
}

fn row<'a>(v: &'a mut Value, id: &str, name: &str) -> &'a mut Value {
    rows(v, id).iter_mut().find(|r| r["name"] == name).unwrap()
}

/// The field `name` of what cell `id` hands in.
fn out_field<'a>(v: &'a mut Value, id: &str, name: &str) -> &'a mut Value {
    node(v, id)["out"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["name"] == name)
        .unwrap()
}

fn obj(v: &mut Value) -> &mut serde_json::Map<String, Value> {
    v.as_object_mut().unwrap()
}

fn drop_source(v: &mut Value, id: &str, src: Value) {
    for x in node(v, id)["inputs"].as_array_mut().unwrap() {
        let from = x["from"].as_array_mut().unwrap();
        if let Some(i) = from.iter().position(|s| *s == src) {
            from.remove(i);
            return;
        }
    }
    panic!("no source {src} at {id}");
}

fn remove_row(v: &mut Value, id: &str, name: &str) {
    rows(v, id).retain(|r| r["name"] != name);
}

/// The input `name` of cell `id`.
fn input<'a>(v: &'a mut Value, id: &str, name: &str) -> &'a mut Value {
    node(v, id)["inputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|x| x["name"] == name)
        .unwrap()
}

/// The flow with a gate, `G9`, between the research and the implementation, as flows before had gates: the flow has
/// none now, and a flow at an older commit, which the flow page shows, may still have them.
fn gated(v: &mut Value) {
    row(v, "R1", "start")["to"] = json!("G9");
    graph(v)["nodes"].as_array_mut().unwrap().push(json!({
        "id": "G9", "kind": "gate", "title": "Whether it stands", "effort": "medium",
        "question": "Judge whether this work stands.", "to": "I1", "shown": ["answer"],
        "inputs": [{ "what": "How this work is understood", "from": [["R1", "understanding"]] }],
        "outcomes": { "on": ["stands", "It stands"], "problem": ["does_not_stand", "It does not stand"] }
    }));
    graph(v)["blocks"][1]["nodes"].as_array_mut().unwrap().push(json!("G9"));
    let stages = graph(v)["stages"]["list"].as_array_mut().unwrap();
    let research = stages.iter_mut().find(|s| s["name"] == "research").unwrap();
    research["nodes"].as_array_mut().unwrap().push(json!("G9"));
    let forms = graph(v)["agentCommon"]["returns"]["forms"].as_array_mut().unwrap();
    let tools = forms.iter_mut().find(|f| f["field"] == "tools").unwrap();
    tools["cells"].as_array_mut().unwrap().push(json!("G9"));
}

/// The gate `G9` going on by rows instead of to one cell.
fn by_rows(v: &mut Value) {
    let g = node(v, "G9");
    obj(g).remove("to");
    g["rows"] = json!([{ "name": "stands", "if": [], "to": "I1", "main": true }]);
}

/// The flow with one edit fails, and one of its problems says `want`.
fn refused(edit: impl FnOnce(&mut Value), want: &str) {
    refused_by(flow::compile, edit, want);
}

/// What compiles a flow: as a flow that runs, or only to show it.
type Compile = fn(&str, &flow::Context) -> Result<flow::Compiled, flow::Error>;

/// The flow with one edit fails to compile by `compile`, and one of its problems says `want`.
fn refused_by(compile: Compile, edit: impl FnOnce(&mut Value), want: &str) {
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    edit(&mut v);
    let text = serde_json::to_string(&v).unwrap();
    match compile(&text, &context()) {
        Ok(_) => panic!("compiled, but should fail with {want}"),
        Err(e) => assert!(e.to_string().contains(want), "wanted {want}, got:\n{e}"),
    }
}

macro_rules! case {
    ($name:ident, $want:expr, |$v:ident| $edit:block) => {
        #[test]
        fn $name() {
            refused(|$v: &mut Value| $edit, $want);
        }
    };
}

// The format and the names.
case!(a_row_with_two_targets, "expected a string", |v| {
    row(v, "T3", "adversarial")["to"] = json!(["T1", "T6"]);
});
case!(an_input_said_to_be_in_the_session, "fits none of the forms", |v| {
    node(v, "D1")["inputs"][0]["from"][0] = json!("session");
});
case!(an_unknown_key, "unknown field", |v| {
    node(v, "D1")["colour"] = json!("red");
});
// What a field was in a return handed in on an earlier version is a shape, which names none earlier in turn and whose
// faults name the field, or `absent` for a field a return handed in now may not leave out and that is no list's item.
// It is a field of a cell's own answer, which a work moving to the version compares, never one every agent's return
// has; and no row's condition reads it, since a row takes a return as it is handed in now.
case!(
    an_earlier_field_named_by_no_known_word,
    "no such earlier shape: gone, in field examined",
    |v| {
        out_field(v, "D2", "examined")["earlier"] = json!("gone");
    }
);
case!(
    an_earlier_absence_of_a_field_a_return_may_leave_out,
    "a field a return may leave out names no earlier absence: examined",
    |v| {
        let f = out_field(v, "D2", "examined");
        f["optional"] = json!(true);
        f["earlier"] = json!("absent");
    }
);
case!(
    an_earlier_absence_of_a_list_s_item,
    "a list's item names no earlier absence, as an item is never absent: examined",
    |v| {
        out_field(v, "D2", "examined")["item"]["earlier"] = json!("absent");
    }
);
case!(
    a_fault_of_an_earlier_shape,
    "a list with no item: , in what field examined was earlier",
    |v| {
        out_field(v, "D2", "examined")["earlier"] = json!({ "type": "list" });
    }
);
case!(
    an_earlier_shape_that_names_one_earlier_still,
    "a field names what it was earlier only once, in what field examined was earlier",
    |v| {
        out_field(v, "D2", "examined")["earlier"] = json!({ "type": "text", "earlier": "absent" });
    }
);
case!(
    an_earlier_shape_of_a_field_every_agent_return_has,
    "a field every agent's return has names what it was earlier: tell_owner",
    |v| {
        let out = graph(v)["agentCommon"]["out"].as_array_mut().unwrap();
        let told = out.iter_mut().find(|f| f["name"] == "tell_owner").unwrap();
        told["earlier"] = json!("absent");
    }
);
case!(
    an_earlier_shape_a_row_s_condition_reads,
    "a row's condition reads a field that names what it was earlier: T3 adversarial silent.verdict (silent)",
    |v| {
        out_field(v, "T3", "silent")["earlier"] = json!("absent");
    }
);
case!(a_line_with_no_colour, "a line with no colour: engineer", |v| {
    obj(&mut graph(v)["sessions"]["lines"][0]).remove("colour");
});
case!(a_colour_the_page_does_not_know, "unknown variant `mauve`", |v| {
    graph(v)["sessions"]["lines"][0]["colour"] = json!("mauve");
});
case!(a_colour_the_page_keeps_for_the_owner, "unknown variant `orange`", |v| {
    graph(v)["sessions"]["lines"][1]["colour"] = json!("orange");
});
// A line says whether its session expires with the cache as `cacheExpiry`, a yes or a no, and nothing else.
case!(an_unknown_key_of_a_line, "unknown field `cache_expiry`", |v| {
    graph(v)["sessions"]["lines"][0]["cache_expiry"] = json!(true);
});
case!(a_cache_expiry_that_is_not_a_yes_or_a_no, "invalid type", |v| {
    graph(v)["sessions"]["lines"][0]["cacheExpiry"] = json!("yes");
});
case!(
    two_lines_of_a_graph_with_one_colour,
    "two lines share the colour blue: engineer and designer",
    |v| {
        graph(v)["sessions"]["lines"][1]["colour"] = json!("blue");
    }
);
case!(
    two_lines_of_two_graphs_with_one_colour,
    "two lines share the colour blue: engineer and architect",
    |v| {
        v["flows"][1]["sessions"]["lines"][0]["colour"] = json!("blue");
    }
);
// A flow compiled only to be shown, as at an older commit, may have lines with no colour, and still neither two lines
// with one colour nor a colour the page does not know.
#[test]
fn a_flow_only_shown_with_two_lines_of_one_colour() {
    refused_by(
        flow::compile_shown,
        |v| {
            obj(&mut graph(v)["sessions"]["lines"][0]).remove("colour");
            graph(v)["sessions"]["lines"][3]["colour"] = json!("teal");
        },
        "two lines share the colour teal: review and adversarial",
    );
}

#[test]
fn a_flow_only_shown_with_a_colour_the_page_does_not_know() {
    refused_by(
        flow::compile_shown,
        |v| {
            obj(&mut graph(v)["sessions"]["lines"][0]).remove("colour");
            graph(v)["sessions"]["lines"][1]["colour"] = json!("mauve");
        },
        "unknown variant `mauve`",
    );
}

// A flow written when the page drew lines in olive and lime, as at an older commit, is read to be shown, those lines
// with no colour, and is refused as a flow to run, which gives every line one. Any other name the page does not draw
// is refused either way (the two tests before).
#[test]
fn a_flow_with_a_colour_the_page_no_longer_draws_is_only_shown() {
    // Both names go through one reading of a colour, so one flow holds them both: olive on the librarian line (the
    // engineer's graph) and lime on the second eye's line (the architect's).
    let edit = |v: &mut Value| {
        graph(v)["sessions"]["lines"][5]["colour"] = json!("olive");
        v["flows"][1]["sessions"]["lines"][2]["colour"] = json!("lime");
    };
    refused_by(flow::compile, edit, "a line with no colour: librarian");
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    edit(&mut v);
    let text = serde_json::to_string(&v).unwrap();
    if let Err(e) = flow::compile_shown(&text, &context()) {
        panic!("a flow with the old names is not read to be shown:\n{e}");
    }
}
case!(an_engine_step_nothing_uses, "an engine step nothing uses: extra", |v| {
    v["engine"]["steps"]["extra"] = json!("x");
});
case!(no_such_engine_action, "no such engine action: nope", |v| {
    row(v, "M1", "clear")["do"] = json!(["resolve_blocker", "nope"]);
});
case!(
    a_line_mixing_low_with_other_efforts,
    "a line mixes the effort low, which does not think, with others: engineer",
    |v| {
        node(v, "I1")["effort"] = json!("low");
    }
);
case!(no_such_engine_check, "no such engine check: nope", |v| {
    node(v, "T3")["checks"] = json!([{ "check": "nope" }]);
});
case!(
    something_done_every_time_that_names_a_view,
    "names one action, and no view",
    |v| {
        node(v, "X1")["always"] = json!([{ "at": "results", "view": "work_progress" }]);
    }
);
case!(no_such_view, "no table or view nope", |v| {
    node(v, "S1")["inputs"][0]["from"][0]["db"] = json!("nope");
});
case!(
    a_view_read_by_a_column_it_lacks,
    "ready_items has no column work",
    |v| {
        node(v, "S1")["inputs"][0]["from"][0]["by"] = json!("work");
    }
);
case!(a_tool_cell_with_no_step, "names its step", |v| {
    obj(node(v, "X1")).remove("step");
});
case!(
    a_form_leaving_out_what_does_not_come_every_time,
    "a form leaves out a field",
    |v| {
        graph(v)["agentCommon"]["returns"]["forms"][2]["without"] = json!(["nothing"]);
    }
);
case!(
    a_cell_leaving_out_a_form_the_return_has_not,
    "a cell leaves out a form the return has not",
    |v| {
        node(v, "R1")["without"] = json!(["nothing"]);
    }
);
case!(
    an_engine_cell_leaving_out_forms,
    "only an agent's cell leaves out forms of the return",
    |v| {
        node(v, "M1")["without"] = json!(["ask"]);
    }
);
case!(
    a_row_outside_the_asking_part_leaving_the_callers_waiting,
    "a row that goes on without answering the cells that wait goes from the asking part or an owner's cell to a cell",
    |v| {
        row(v, "T3", "adversarial")["drops"] = json!(true);
    }
);
case!(
    a_branch_field_named_like_the_marker,
    "a branch has a field named like the union's marker verdict",
    |v| {
        let must = out_field(v, "T3", "must_fix");
        must["branches"][0]["fields"][0] = json!({ "name": "verdict", "type": "text", "desc": "x" });
    }
);
case!(
    a_field_every_branch_has_is_also_in_a_branch,
    "a field every branch has is also in a branch",
    |v| {
        out_field(v, "T3", "must_fix")["common"] = json!([{ "name": "why", "type": "text", "desc": "x" }]);
    }
);
case!(
    returns_say_without_the_forms,
    "returns.say does not hold {forms}",
    |v| {
        graph(v)["agentCommon"]["returns"]["say"] = json!("Hand in one, in {in}. {every} every time.");
    }
);
case!(
    a_gate_prompt_that_does_not_name_the_outcomes,
    "gate.prompt does not name the two outcomes",
    |v| {
        graph(v)["gate"]["prompt"] = json!("If yes, ask.");
    }
);
case!(
    a_gate_not_applicable_prompt_without_the_example,
    "does not name the two outcomes and the example",
    |v| {
        let p = graph(v)["gate"]["na"]["prompt"].as_str().unwrap().replace("{na}", "");
        graph(v)["gate"]["na"]["prompt"] = json!(p);
    }
);
case!(
    a_gate_that_may_not_apply_and_goes_on_by_rows,
    "a gate that may not apply goes on to one cell",
    |v| {
        gated(v);
        by_rows(v);
        node(v, "G9")["na"] = json!("only documents change");
    }
);
case!(
    a_gate_that_goes_on_both_ways,
    "a gate goes on to one cell or by rows",
    |v| {
        gated(v);
        node(v, "G9")["rows"] = json!([{ "name": "stands", "if": [], "to": "I1", "main": true }]);
    }
);
case!(
    a_gate_with_rows_that_records_a_value,
    "a gate with rows records what each row does",
    |v| {
        gated(v);
        by_rows(v);
        node(v, "G9")["sets"] = json!({ "stage": "design_on" });
    }
);
case!(
    gate_cells_in_a_flow_with_no_gate,
    "gate cells in a flow with no gate",
    |v| {
        gated(v);
        obj(graph(v)).remove("gate");
    }
);
case!(
    own_on_a_cell_that_is_not_an_agent,
    "only an agent has an own answer to tell apart",
    |v| {
        row(v, "M1", "clear")["if"] = json!([["own"]]);
    }
);
case!(a_cell_that_asks_itself, "a cell that asks itself", |v| {
    node(v, "A1")["asks"] = json!(["advisor"]);
});
case!(a_role_nobody_has, "asks no such role", |v| {
    node(v, "A1")["asks"] = json!(["boss"]);
});
case!(
    a_cell_that_is_asked_and_does_not_answer,
    "a cell that is asked does not answer",
    |v| {
        for r in rows(v, "A1") {
            r["to"] = json!("A4");
        }
    }
);
case!(
    an_answer_from_a_cell_nobody_asks,
    "only a cell that is asked answers the one that asked",
    |v| {
        row(v, "T4", "clear")["to"] = json!("@asker");
    }
);
case!(a_row_counts_no_such_counter, "no such counter", |v| {
    row(v, "T4", "clear")["counts"] = json!("x");
});
case!(a_counter_no_row_counts, "a counter no row counts", |v| {
    graph(v)["limits"]["counters"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "x", "desc": "x" }));
});
case!(a_cell_in_no_block, "a cell in no block", |v| {
    graph(v)["blocks"][3]["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|x| x != "T2");
});
case!(a_cell_in_two_blocks, "in two blocks", |v| {
    graph(v)["blocks"][1]["nodes"].as_array_mut().unwrap().push(json!("T2"));
});
case!(
    a_prompt_without_a_case_for_a_value,
    "promptBy has a case for each value",
    |v| {
        obj(&mut node(v, "I1")["promptBy"]["cases"]).remove("report");
    }
);
case!(a_cell_on_no_such_line, "no such line", |v| {
    node(v, "I1")["line"] = json!("test");
});
case!(
    a_flow_whose_asks_carry_kinds,
    "an ask carries no kind, and askKinds lists some",
    |v| {
        graph(v)["askKinds"] = json!([{ "group": "technical", "kinds": [["split", "It must be split"]] }]);
    }
);
// A flow anomaly goes to the architect: a way on that hands it to the owner leaves the owner nothing asked.
case!(
    a_way_on_that_still_hands_a_flow_anomaly_to_the_owner,
    "missing A3 1",
    |v| {
        row(v, "A4", "should_wait")["to"] = json!("A3");
    }
);
case!(
    a_blocker_the_owner_holds_at_a_cell_the_graph_lost,
    "a blocker is held at no such cell: A9",
    |v| {
        graph(v)["blockers"]["cells"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "cell": "A9", "holders": ["owner"] }));
    }
);
case!(
    a_gate_that_names_a_kind,
    "an ask carries no kind, and gate G9 names one",
    |v| {
        gated(v);
        node(v, "G9")["about"] = json!("split");
    }
);
// A flow from before asks lost their kinds, as at an older commit, is still shown: its kinds of question, the kind its
// ask's points name from them and the kind each gate asks about are read as they were. It does not run.
#[test]
fn a_flow_from_before_asks_lost_their_kinds_is_shown_and_does_not_run() {
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    graph(&mut v)["askKinds"] = json!([{ "group": "technical", "kinds": [["an_old_kind", "A kind a flow before had"]],
                                          "only": "advisor", "needs": {} }]);
    v["shapes"]["ask_form"]["common"][0]["item"]["fields"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({ "name": "kind", "type": "enum", "desc": "Which kind", "values": "askKinds" }),
        );
    gated(&mut v);
    node(&mut v, "G9")["about"] = json!("an_old_kind");
    let text = serde_json::to_string(&v).unwrap();
    let shown = flow::compile_shown(&text, &context()).unwrap();
    let ask = shown
        .graph("engineer")
        .unwrap()
        .cell("R1")
        .unwrap()
        .schema
        .clone()
        .unwrap();
    assert!(ask.to_string().contains("an_old_kind"), "{ask}");
    let e = flow::compile(&text, &context()).unwrap_err().to_string();
    assert!(e.contains("an ask carries no kind"), "{e}");
}
case!(a_rule_no_cell_uses, "a rule no cell uses", |v| {
    graph(v)["rules"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "x", "desc": "x", "text": ["x"] }));
});
case!(a_cell_given_no_such_rule, "no such rule", |v| {
    node(v, "D1")["rules"] = json!(["x"]);
});
case!(
    a_rule_given_to_a_cell_that_is_not_an_agent,
    "only an agent is given rules",
    |v| {
        node(v, "M1")["rules"] = json!(["review_table"]);
    }
);
case!(a_cell_at_two_stages, "a cell at two stages", |v| {
    let stages = graph(v)["stages"]["list"].as_array_mut().unwrap();
    let research = stages.iter_mut().find(|s| s["name"] == "research").unwrap();
    research["nodes"].as_array_mut().unwrap().push(json!("D1"));
});
case!(
    a_cell_at_no_stage_that_goes_on_by_itself,
    "a cell at no stage that goes on by itself",
    |v| {
        let stages = graph(v)["stages"]["list"].as_array_mut().unwrap();
        let implement = stages.iter_mut().find(|s| s["name"] == "implement").unwrap();
        implement["nodes"].as_array_mut().unwrap().retain(|x| x != "I1");
    }
);
case!(
    a_row_that_takes_an_interjection_elsewhere,
    "goes to the cell that receives it",
    |v| {
        row(v, "M1", "interjection")["to"] = json!("T6");
    }
);
case!(
    a_form_on_every_line_that_takes_an_interjection,
    "an agent off the line of the cell that receives",
    |v| {
        obj(&mut graph(v)["agentCommon"]["returns"]["forms"][5]).remove("lines");
    }
);
case!(a_form_for_no_such_line, "a form for no such line", |v| {
    graph(v)["agentCommon"]["returns"]["forms"][5]["lines"] = json!(["test"]);
});
// Every agent's cell runs at an effort the command line names; a cell that is not an agent's runs at none.
case!(
    an_effort_the_command_line_does_not_name,
    "an effort the command line does not name: I1",
    |v| {
        node(v, "I1")["effort"] = json!("highest");
    }
);
case!(
    an_effort_on_a_cell_that_is_not_an_agent,
    "only an agent runs at an effort: M1",
    |v| {
        node(v, "M1")["effort"] = json!("high");
    }
);
case!(
    an_agents_cell_with_no_effort,
    "an agent's cell declares no effort: I1",
    |v| {
        obj(node(v, "I1")).remove("effort");
    }
);
// A row opens a new session only into an agent's cell: an engine's step has no session.
case!(
    a_new_session_into_an_engine_step,
    "a row opens a new session into a cell that is not an agent's: T4 clear",
    |v| {
        row(v, "T4", "clear")["do"] = json!(["open_session"]);
    }
);
case!(
    two_who_may_interject_with_one_name,
    "two who may interject with one name",
    |v| {
        let first = graph(v)["interject"]["from"][0].clone();
        graph(v)["interject"]["from"].as_array_mut().unwrap().push(first);
    }
);
case!(
    a_common_input_brought_back_by_no_form,
    "a common input brought back by no form",
    |v| {
        graph(v)["agentCommon"]["inputs"][0]["with"] = json!("x");
    }
);
case!(
    an_agent_that_may_ask_for_tools_names_none,
    "every agent that may ask for tools",
    |v| {
        obj(node(v, "R1")).remove("tools");
    }
);

// Every return takes one row.
case!(two_rows_hold_together, "overlap", |v| {
    let clear = row(v, "T4", "clear")["if"].clone();
    row(v, "T4", "fix_again")["if"] = clear;
});
case!(no_row_holds, "no row", |v| {
    remove_row(v, "M1", "wait");
});
case!(a_split_that_leaves_nothing_with_no_way_to_the_end, "no row", |v| {
    remove_row(v, "A4", "all_to_sub_items");
});
case!(a_way_on_that_a_guard_row_also_takes, "overlap", |v| {
    row(v, "A4", "carry_on")["if"].as_array_mut().unwrap().pop();
});

// Every state.
case!(a_recorded_value_not_recorded_on_a_way_in, "not recorded", |v| {
    obj(row(v, "S1", "started")).remove("sets");
});
case!(a_recorded_value_nobody_reads, "never read", |v| {
    row(v, "A2", "answered")["sets"] = json!({ "extra": "x" });
});
case!(a_row_no_return_takes, "never taken", |v| {
    row(v, "T3", "clear")["if"]
        .as_array_mut()
        .unwrap()
        .push(json!(["is", "must_fix.verdict", ["yes"]]));
});
case!(a_value_no_way_records, "never taken", |v| {
    obj(row(v, "R1", "start")).remove("sets");
    obj(row(v, "D3", "posted")).remove("sets");
});
case!(a_cell_asked_again_while_it_waits, "asked again", |v| {
    node(v, "A1")["out"][0]["fields"].as_array_mut().unwrap().push(json!({
        "name": "again", "type": "text", "optional": true, "desc": "x"
    }));
    rows(v, "A1").insert(
        0,
        json!({ "name": "ask_self", "if": [["own"], ["has", "reply.again"]], "to": "A1" }),
    );
});
case!(
    a_cell_asked_that_goes_on_instead_of_answering,
    "still waits for an answer",
    |v| {
        rows(v, "A1").insert(
            0,
            json!({ "name": "answered_new_acceptance", "if": [["own"], ["has", "reply.new_acceptance"]], "to": "A5",
                    "do": ["resolve_blocker"] }),
        );
    }
);
case!(a_design_never_reviewed, "unreached D3", |v| {
    row(v, "D2", "done")["to"] = json!("I1");
});

// A cell whose rows test a value the engine works out lists what it is worked out from.
case!(
    a_value_tested_from_nothing,
    "a cell whose rows test a value the engine works out lists the inputs it is worked out from: M2",
    |v| {
        node(v, "M2")["inputs"] = json!([]);
    }
);

// Every loop counts.
case!(a_loop_with_no_counter, "no counter", |v| {
    obj(row(v, "T6", "done")).remove("counts");
    rows(v, "S1")[0]["counts"] = json!("fixes");
});
// A pause counts nothing: the owner's Start it waits for closes its loop.
case!(a_pause_with_no_counter, "no counter", |v| {
    obj(row(v, "X3", "started")).remove("waits");
});
// The total time is measured, never counted by a row.
case!(
    a_row_that_counts_the_total_time,
    "a row counts the total time, which the engine measures",
    |v| {
        graph(v)["agentCommon"]["rows"][2]["counts"] = json!("total_time");
    }
);
case!(an_interjection_at_an_engine_step_with_no_counter, "no counter", |v| {
    obj(row(v, "M1", "interjection")).remove("counts");
});

// The cell a work on an older flow version comes to is a cell of the graph, which the engine brings a work to and no
// row does, and whose rows only take it back to where it stopped, keep it there or end it at the end for a work stopped
// before its end, which no other row leads to; only an engine's cell pauses a work by a row.
case!(
    a_cell_for_an_older_version_that_is_not_there,
    "no such cell for a work on an older flow version",
    |v| {
        graph(v)["version"] = json!("V9");
    }
);
case!(
    a_row_from_the_cell_for_an_older_version_to_another_cell,
    "keep it there or end it at the end for a work stopped before its end: Y1",
    |v| {
        row(v, "Y1", "went_back")["to"] = json!("T1");
    }
);
case!(
    a_row_into_the_end_for_a_work_stopped_before_its_end,
    "no row leads to the end for a work stopped before its end but one of the cell for a work on an older flow version",
    |v| {
        row(v, "T6", "done")["to"] = json!("C1");
    }
);
case!(
    a_row_into_the_cell_for_an_older_version,
    "no row leads to the cell for a work on an older flow version",
    |v| {
        row(v, "T6", "done")["to"] = json!("Y1");
    }
);
case!(
    a_row_of_an_agent_s_cell_that_pauses,
    "only an engine's cell has a row that pauses the work: I1",
    |v| {
        rows(v, "I1")[0]["pauses"] = json!(true);
    }
);

// Every row into a block goes to an entry.
case!(
    a_row_into_the_asking_part_past_the_cells_that_are_asked,
    "side entry",
    |v| {
        row(v, "D1", "done")["to"] = json!("A4");
    }
);
case!(a_row_into_another_block_past_its_entries, "side entry", |v| {
    row(v, "R1", "start")["to"] = json!("T2");
});
case!(
    a_row_into_the_receiving_cell_not_marked_as_taking_an_interjection,
    "side entry",
    |v| {
        obj(row(v, "M1", "cancelled")).remove("interject");
    }
);

// Nothing dropped.
case!(side_reports_no_role_reads, "nobody reads side_reports.kind", |v| {
    obj(&mut graph(v)["agentCommon"]).remove("readBy");
});
case!(
    a_field_read_by_no_role_an_agent_may_ask,
    "read by no role an agent may ask",
    |v| {
        graph(v)["agentCommon"]["readBy"][0]["role"] = json!("nobody");
    }
);
case!(a_shown_place_the_return_does_not_have, "nope", |v| {
    node(v, "I1")["shown"] = json!(["deviations", "followup", "nope"]);
});
case!(an_input_every_cell_has_with_no_name, "goes by a name", |v| {
    obj(&mut graph(v)["agentCommon"]["inputs"][0]).remove("name");
});
// An input is known by what it is and a field by its name: the name of an input not every agent's cell has, or the
// title of a field, reaches no agent and shows on no chart, and is refused where it is.
case!(
    an_input_with_a_name,
    "flows.engineer.nodes.S0.inputs[0].name: text that reaches no agent and shows on no chart",
    |v| {
        node(v, "S0")["inputs"][0]["name"] = json!("Start order");
    }
);
case!(
    a_return_field_with_a_title,
    "flows.engineer.nodes.R1.out.understanding.fields.grounds.item.branches.verified.fields.where.title: text that \
     reaches no agent and shows on no chart",
    |v| {
        let grounds = &mut node(v, "R1")["out"][0]["fields"][4];
        grounds["item"]["branches"][0]["fields"][1]["title"] = json!("Where checked");
    }
);
case!(a_numbered_table_not_read_from_returns, "numbered with a prefix", |v| {
    node(v, "X1")["inputs"][0]["numbered"] = json!("F");
});
case!(
    a_table_to_answer_with_no_way_on_when_it_has_no_row,
    "a numbered table the cell must answer says where the work goes when it has no row",
    |v| {
        obj(&mut node(v, "T6")["inputs"][0]).remove("empty");
    }
);
case!(
    a_table_with_no_row_that_sends_the_work_nowhere,
    "a table with no row sends the work to no such cell",
    |v| {
        node(v, "T6")["inputs"][0]["empty"]["to"] = json!("Z9");
    }
);
case!(
    a_way_on_for_no_row_on_an_input_that_is_no_table,
    "only a numbered table says where the work goes when it has no row",
    |v| {
        node(v, "T6")["inputs"][1]["empty"] = json!({ "to": "T4", "note": "Nothing." });
    }
);
// Which way a work goes is a cell of its own with a row each way: a cell of a flow that runs needs nothing to apply,
// which a flow from before, as at an older commit, still shows.
#[test]
fn a_cell_that_needs_something_to_apply_is_only_shown() {
    let edit = |v: &mut Value| {
        gated(v);
        node(v, "G9")["needs"] = json!("flow_feedback");
    };
    refused(
        edit,
        "cell G9 needs something to apply, which a cell of its own with a row each way decides",
    );
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    edit(&mut v);
    flow::compile_shown(&v.to_string(), &context()).unwrap();
}
case!(a_field_that_needs_no_such_need, "a field needs no such need", |v| {
    graph(v)["agentCommon"]["needs"] = json!({ "flow_feedback": "nothing_known" });
});
case!(
    a_need_for_a_field_that_does_not_come_every_time,
    "a need for a field that does not come every time",
    |v| {
        graph(v)["agentCommon"]["needs"] = json!({ "ask": "flow_feedback" });
    }
);
case!(a_field_kept_to_no_such_line, "a field kept to no such line", |v| {
    graph(v)["agentCommon"]["lines"] = json!({ "tell_owner": ["owner"] });
});
case!(
    a_field_kept_to_lines_that_does_not_come_every_time,
    "a field kept to some lines that does not come every time",
    |v| {
        graph(v)["agentCommon"]["lines"] = json!({ "ask": ["engineer"] });
    }
);
// The fix reads what adversarial testing did: the directions it took, what it wrote and what it suspects. Reading only
// part of it drops the rest.
case!(a_cell_reads_part_of_an_input_and_drops_the_rest, "dropped", |v| {
    drop_source(v, "T6", json!(["T1", "directions"]));
});
case!(what_adversarial_testing_suspects_read_by_nobody, "nobody reads", |v| {
    drop_source(v, "T6", json!(["T1", "suspects"]));
});
case!(
    an_answer_to_an_engine_step_that_the_step_does_not_read,
    "nobody reads reply.text",
    |v| {
        node(v, "X1")["inputs"].as_array_mut().unwrap().pop();
    }
);

// Nothing missing.
// The design, which the implementation straight from the research has not, is taken as one it always has.
case!(an_input_missing_on_some_way, "missing", |v| {
    obj(input(v, "I1", "The design")).remove("maybe");
});
case!(an_input_marked_maybe_that_is_always_there, "marked maybe", |v| {
    input(v, "T3", "The first pass's findings")["maybe"] = json!(true);
});
// The fix's table, which on the way from an interjection at the merge's judge holds only the change the way on asked,
// is taken as one it must always have a row of without the reviews' findings and that change.
case!(an_input_missing_behind_an_engine_step, "missing", |v| {
    obj(&mut node(v, "T6")["inputs"][0]).remove("maybe");
    drop_source(v, "T6", json!(["A4", "next.change"]));
    drop_source(v, "T6", json!(["T2", "findings"]));
    drop_source(v, "T6", json!(["T3", "findings"]));
});

// A rule given with an input goes with it: a cell given the policy, in force or being set, is given what a policy
// governs, so that a new cell cannot miss it.
case!(
    a_cell_given_the_draft_without_what_a_policy_governs,
    "cell `P2` is given policy_draft without the rule policy_scope that goes with it",
    |v| {
        let write = v["flows"][1]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|n| n["id"] == "P2")
            .unwrap();
        obj(write).remove("rules");
        let inputs = write["inputs"].as_array_mut().unwrap();
        inputs.retain(|x| x["from"] != json!([{ "engine": "policy" }]));
    }
);

// The same for where the history is: a cell given it is given how to gather history, so that a new gatherer cannot miss
// the rule that holds the check of paths. T4 is on no line the rule is given to, and names none.
case!(
    a_cell_given_where_the_history_is_without_the_rule_for_gathering_it,
    "cell `T4` is given history_sources without the rule history that goes with it",
    |v| {
        let input = json!({
            "what": "The database's read-only address and the directory your git runs in",
            "from": [{ "engine": "history_sources" }]
        });
        node(v, "T4")["inputs"].as_array_mut().unwrap().push(input);
    }
);

// An owner's cell that rules by buttons has one for each value of its ruling, in order, each saying what it does and
// what happens next, so that the owner's page shows every value the cell takes, as the flow says it; one whose answer
// is no ruling has none.
case!(
    an_owner_s_cell_with_a_button_for_no_value_of_its_ruling,
    "an owner's cell's buttons are not the values of its ruling.choice, in order: A3",
    |v| {
        let ask = v["flows"][1]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|n| n["id"] == "A3")
            .unwrap();
        ask["choices"] =
            json!([{ "value": "study_more", "title": "Study more", "desc": "The architect studies more." }]);
    }
);

// Every cell a text names is a cell of its graph: a rename or a removal that leaves a text naming the cell as it was is
// refused, saying where the text is and what it names.
case!(
    a_text_that_names_a_cell_its_graph_has_not,
    "engineer: flows.engineer.limits.counters.fixes.desc names cell `repair`, which its graph has not",
    |v| {
        graph(v)["limits"]["counters"][3]["desc"] = json!("The times cell `repair` returned from a fix");
    }
);
case!(
    a_text_that_names_a_cell_of_the_other_graph,
    "architect: flows.architect.nodes.P2.prompt[0] names cell `A4`, which its graph has not",
    |v| {
        let write = v["flows"][1]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|n| n["id"] == "P2")
            .unwrap();
        write["prompt"][0] = json!("Coming from cell `A4`, write the policy.");
    }
);
case!(
    a_text_every_graph_shares_that_names_a_cell_no_graph_has,
    "engine.steps.merge names cell `merging`, which no graph has",
    |v| {
        v["engine"]["steps"]["merge"] = json!("Puts the PR in the merge queue, as cell `merging` says.");
    }
);

// A row of the rounds section that runs again is bounded and ends in a pause: a kind of failed run run again with no
// bound, with a bound of none or with no pause, a pause that is no pause, a step run again with no backoff or whose
// pause is the engine's other one, a bound on a kind that is not run again, and a counter the flow has not, or that
// no row counts, or that a row counts which waits.
case!(
    a_failed_run_run_again_with_no_bound,
    "rounds.failed.kinds[transient] runs again with no bound",
    |v| {
        obj(&mut graph(v)["rounds"]["failed"]["kinds"][2]).remove("inRow");
    }
);
case!(
    a_failed_run_run_again_with_a_bound_of_none,
    "rounds.failed.kinds[transient] runs again with no bound",
    |v| {
        graph(v)["rounds"]["failed"]["kinds"][2]["inRow"] = json!(0);
    }
);
case!(
    a_failed_run_run_again_with_no_pause,
    "rounds.failed.kinds[transient] runs again with no bound",
    |v| {
        obj(&mut graph(v)["rounds"]["failed"]["kinds"][2]).remove("then");
    }
);
case!(a_bound_that_ends_in_no_pause, "unknown variant `wait`", |v| {
    graph(v)["rounds"]["failed"]["kinds"][2]["then"] = json!("wait");
});
case!(
    a_bound_on_a_kind_that_is_not_run_again,
    "rounds.failed.kinds[flagged] has inRow or then",
    |v| {
        graph(v)["rounds"]["failed"]["kinds"][3]["inRow"] = json!(2);
    }
);
case!(
    a_step_run_again_with_no_backoff,
    "rounds.engine[step_network] runs again with no bound",
    |v| {
        graph(v)["rounds"]["engine"][2]["backoff"] = json!([]);
    }
);
case!(
    a_step_run_again_with_no_bound,
    "rounds.engine[step_network] runs again with no bound",
    |v| {
        obj(&mut graph(v)["rounds"]["engine"][2]).remove("inRow");
    }
);
case!(
    a_step_run_again_that_also_pauses,
    "rounds.engine[step_network] runs again, and its pause is the row for being taken too often",
    |v| {
        graph(v)["rounds"]["engine"][2]["pauses"] = json!(true);
    }
);
case!(
    a_row_of_the_rounds_that_counts_a_counter_the_flow_has_not,
    "rounds section: no such counter, or one the engine measures: nothing",
    |v| {
        graph(v)["rounds"]["failed"]["kinds"][2]["counts"] = json!("nothing");
    }
);
case!(
    a_row_of_the_rounds_that_waits_and_counts,
    "rounds.failed.kinds[offline] waits for someone, and a row that waits counts nothing",
    |v| {
        graph(v)["rounds"]["failed"]["kinds"][1]["counts"] = json!("retries");
    }
);
case!(
    a_counter_that_only_the_rounds_would_count,
    "a counter no row counts: [\"sent_back\"]",
    |v| {
        obj(&mut graph(v)["rounds"]["sentBack"]).remove("counts");
    }
);

// The step run again is bounded on every side of its case, not only on its backoff: with no pause to end in, or with a
// bound of none, it is refused as a kind of failed run is.
case!(
    a_step_run_again_with_no_pause,
    "rounds.engine[step_network] runs again with no bound",
    |v| {
        obj(&mut graph(v)["rounds"]["engine"][2]).remove("then");
    }
);
case!(
    a_step_run_again_with_a_bound_of_none,
    "rounds.engine[step_network] runs again with no bound",
    |v| {
        graph(v)["rounds"]["engine"][2]["inRow"] = json!(0);
    }
);
