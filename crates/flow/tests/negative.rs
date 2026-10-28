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

/// The flow with one edit fails, and one of its problems says `want`.
fn refused(edit: impl FnOnce(&mut Value), want: &str) {
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    edit(&mut v);
    let text = serde_json::to_string(&v).unwrap();
    match flow::compile(&text, &context()) {
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
    row(v, "18", "adversarial")["to"] = json!(["6", "5"]);
});
case!(an_input_said_to_be_in_the_session, "fits none of the forms", |v| {
    node(v, "2")["inputs"][0]["from"][0] = json!("session");
});
case!(an_unknown_key, "unknown field", |v| {
    node(v, "2")["colour"] = json!("red");
});
case!(an_engine_step_nothing_uses, "an engine step nothing uses: extra", |v| {
    v["engine"]["steps"]["extra"] = json!("x");
});
case!(no_such_engine_action, "no such engine action: nope", |v| {
    row(v, "3e", "report")["do"] = json!(["lock_acceptance", "post_design_card", "nope"]);
});
case!(no_such_engine_check, "no such engine check: nope", |v| {
    node(v, "5")["checks"] = json!([{ "check": "nope" }]);
});
case!(
    something_done_every_time_that_names_a_view,
    "names one action, and no view",
    |v| {
        node(v, "13")["always"] = json!([{ "at": "results", "view": "work_progress" }]);
    }
);
case!(no_such_view, "no table or view nope", |v| {
    node(v, "0")["inputs"][0]["from"][0]["db"] = json!("nope");
});
case!(
    a_view_read_by_a_column_it_lacks,
    "ready_items has no column work",
    |v| {
        node(v, "0")["inputs"][0]["from"][0]["by"] = json!("work");
    }
);
case!(a_tool_cell_with_no_step, "names its step", |v| {
    obj(node(v, "13")).remove("step");
});
case!(
    a_form_leaving_out_what_does_not_come_every_time,
    "a form leaves out a field",
    |v| {
        graph(v)["agentCommon"]["returns"]["forms"][2]["without"] = json!(["nothing"]);
    }
);
case!(
    a_branch_field_named_like_the_marker,
    "a branch has a field named like the union's marker kind",
    |v| {
        let out = &mut node(v, "7")["out"][0];
        out["branches"][4]["fields"][0] = json!({ "$ref": "failure_kind" });
    }
);
case!(
    a_field_every_branch_has_is_also_in_a_branch,
    "a field every branch has is also in a branch",
    |v| {
        v["shapes"]["review_columns"]["item"]["branches"][0]["fields"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "name": "id", "type": "text", "desc": "x" }));
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
    a_gate_that_asks_about_no_such_kind,
    "a gate asks about no such kind",
    |v| {
        node(v, "3a")["about"] = json!("gap");
    }
);
case!(
    a_gate_prompt_that_does_not_name_the_kind,
    "gate.prompt does not name the kind",
    |v| {
        graph(v)["gate"]["prompt"] = json!("If yes, ask.");
    }
);
case!(
    a_gate_not_applicable_prompt_without_the_example,
    "does not name the kind and the example",
    |v| {
        let p = graph(v)["gate"]["na"]["prompt"].as_str().unwrap().replace("{na}", "");
        graph(v)["gate"]["na"]["prompt"] = json!(p);
    }
);
case!(
    a_gate_that_may_not_apply_and_goes_on_by_rows,
    "a gate that may not apply goes on to one cell",
    |v| {
        node(v, "3e")["na"] = json!("only documents change");
    }
);
case!(
    a_gate_that_goes_on_both_ways,
    "a gate goes on to one cell or by rows",
    |v| {
        node(v, "3e")["to"] = json!("3f");
    }
);
case!(
    a_gate_with_rows_that_records_a_value,
    "a gate with rows records what each row does",
    |v| {
        node(v, "3e")["sets"] = json!({ "stage": "design_on" });
    }
);
case!(
    gate_cells_in_a_flow_with_no_gate,
    "gate cells in a flow with no gate",
    |v| {
        obj(graph(v)).remove("gate");
    }
);
case!(
    own_on_a_cell_that_is_not_an_agent,
    "only an agent has an own answer to tell apart",
    |v| {
        row(v, "18", "review_again")["if"] = json!([["own"]]);
    }
);
case!(a_cell_that_asks_itself, "a cell that asks itself", |v| {
    node(v, "12")["asks"] = json!(["advisor"]);
});
case!(a_role_nobody_has, "asks no such role", |v| {
    node(v, "12")["asks"] = json!(["boss"]);
});
case!(
    a_cell_that_is_asked_and_does_not_answer,
    "a cell that is asked does not answer",
    |v| {
        for r in rows(v, "16") {
            r["to"] = json!("11");
        }
    }
);
case!(
    an_answer_from_a_cell_nobody_asks,
    "only a cell that is asked answers the one that asked",
    |v| {
        row(v, "2b", "better")["to"] = json!("@asker");
    }
);
case!(a_row_counts_no_such_counter, "no such counter", |v| {
    row(v, "2b", "better")["counts"] = json!("x");
});
case!(a_counter_no_row_counts, "a counter no row counts", |v| {
    graph(v)["limits"]["counters"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "x", "desc": "x" }));
});
case!(a_cell_in_no_block, "a cell in no block", |v| {
    graph(v)["blocks"][2]["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|x| x != "18");
});
case!(a_cell_in_two_blocks, "in two blocks", |v| {
    graph(v)["blocks"][1]["nodes"].as_array_mut().unwrap().push(json!("18"));
});
case!(
    a_prompt_without_a_case_for_a_value,
    "promptBy has a case for each value",
    |v| {
        obj(&mut node(v, "4")["promptBy"]["cases"]).remove("report");
    }
);
case!(a_cell_on_no_such_line, "no such line", |v| {
    node(v, "4")["line"] = json!("test");
});
case!(
    a_group_of_kinds_only_for_no_such_role,
    "a group of kinds names no such role",
    |v| {
        graph(v)["askKinds"][2]["only"] = json!("boss");
    }
);
case!(a_rule_no_cell_uses, "a rule no cell uses", |v| {
    graph(v)["rules"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "x", "desc": "x", "text": ["x"] }));
});
case!(a_cell_given_no_such_rule, "no such rule", |v| {
    node(v, "2")["rules"] = json!(["x"]);
});
case!(
    a_rule_given_to_a_cell_that_is_not_an_agent,
    "only an agent is given rules",
    |v| {
        node(v, "19")["rules"] = json!(["review_table"]);
    }
);
case!(a_cell_at_two_stages, "a cell at two stages", |v| {
    graph(v)["stages"]["list"][2]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!("2"));
});
case!(
    a_cell_at_no_stage_that_goes_on_by_itself,
    "a cell at no stage that goes on by itself",
    |v| {
        graph(v)["stages"]["list"][4]["nodes"]
            .as_array_mut()
            .unwrap()
            .retain(|x| x != "4");
    }
);
case!(
    a_row_that_takes_an_interjection_elsewhere,
    "goes to the cell that receives it",
    |v| {
        row(v, "19", "interjection")["to"] = json!("8");
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
// A line starts a new session only at an agent's cell of its own: the review's first cell is not the engineer's.
case!(
    a_new_session_at_another_lines_cell,
    "a line starts a new session at a cell that is not one of its agents: engineer 5",
    |v| {
        graph(v)["sessions"]["lines"][0]["starts"] = json!(["4", "5"]);
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
        obj(node(v, "1")).remove("tools");
    }
);

// Every return takes one row.
case!(two_rows_hold_together, "overlap", |v| {
    row(v, "18", "review_again")["if"] = json!([["is", "plan.review", ["changed", "disputes"]]]);
});
case!(no_row_holds, "no row", |v| {
    remove_row(v, "19", "wait");
});
case!(a_design_of_only_the_design_paths_with_no_way_on, "no row", |v| {
    remove_row(v, "3e", "design_paths_only");
});
case!(a_split_that_leaves_nothing_with_no_way_to_the_end, "no row", |v| {
    remove_row(v, "11", "all_to_sub_items");
});
case!(a_way_on_that_a_guard_row_also_takes, "overlap", |v| {
    row(v, "11", "carry_on")["if"].as_array_mut().unwrap().pop();
});

// Every state.
case!(a_recorded_value_not_recorded_on_a_way_in, "not recorded", |v| {
    obj(row(v, "1", "done")).remove("sets");
});
case!(a_recorded_value_nobody_reads, "never read", |v| {
    row(v, "15", "answered")["sets"] = json!({ "extra": "x" });
});
case!(a_row_no_return_takes, "never taken", |v| {
    obj(row(v, "2b", "better")).remove("sets");
});
case!(a_value_no_way_records, "never taken", |v| {
    obj(node(v, "1a")).remove("sets");
});
case!(a_cell_asked_again_while_it_waits, "asked again", |v| {
    node(v, "12")["out"][0]["fields"].as_array_mut().unwrap().push(json!({
        "name": "again", "type": "text", "optional": true, "desc": "x"
    }));
    rows(v, "12").insert(
        0,
        json!({ "name": "ask_self", "if": [["own"], ["has", "reply.again"]], "to": "12" }),
    );
});
case!(
    a_cell_asked_that_goes_on_instead_of_answering,
    "still waits for an answer",
    |v| {
        rows(v, "12").insert(
            0,
            json!({ "name": "answered_new_acceptance", "if": [["own"], ["has", "reply.new_acceptance"]], "to": "11a" }),
        );
    }
);
case!(the_design_never_asked_about_the_plan, "unreached 3g", |v| {
    node(v, "3d")["to"] = json!("3e");
});

// Every loop counts.
case!(a_loop_with_no_counter, "no counter", |v| {
    obj(row(v, "8", "done")).remove("counts");
    rows(v, "0")[0]["counts"] = json!("fixes");
});
case!(a_pause_with_no_counter, "no counter", |v| {
    obj(&mut graph(v)["agentCommon"]["rows"][1]).remove("counts");
    rows(v, "0")[0]["counts"] = json!("total_time");
});
case!(an_interjection_at_an_engine_step_with_no_counter, "no counter", |v| {
    obj(row(v, "19", "interjection")).remove("counts");
});

// Every row into a block goes to an entry.
case!(
    a_row_into_the_asking_part_past_the_cells_that_are_asked,
    "side entry",
    |v| {
        node(v, "3b")["to"] = json!("11");
    }
);
case!(a_row_into_another_block_past_its_entries, "side entry", |v| {
    row(v, "3f", "yes")["to"] = json!("6");
});
case!(
    a_row_into_the_receiving_cell_not_marked_as_taking_an_interjection,
    "side entry",
    |v| {
        obj(row(v, "19", "interjection")).remove("interject");
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
    node(v, "4")["shown"] = json!(["deviations", "followup", "nope"]);
});
case!(an_input_every_cell_has_with_no_name, "goes by a name", |v| {
    obj(&mut graph(v)["agentCommon"]["inputs"][0]).remove("name");
});
case!(
    a_cell_own_input_with_a_name,
    "only an input every agent's cell has",
    |v| {
        node(v, "4")["inputs"][0]["name"] = json!("Design");
    }
);
case!(a_numbered_table_not_read_from_returns, "numbered with a prefix", |v| {
    node(v, "13")["inputs"][0]["numbered"] = json!("F");
});
case!(a_kind_that_needs_no_such_need, "no such need", |v| {
    graph(v)["askKinds"][0]["needs"] = json!({ "gap_under_test": "nothing_known" });
});
case!(a_cell_reads_part_of_an_input_and_drops_the_rest, "dropped", |v| {
    drop_source(v, "8", json!(["5a", "columns[findings]"]));
});
case!(the_design_tree_findings_read_by_nobody, "dropped", |v| {
    drop_source(v, "8", json!(["5b", "columns[findings]"]));
});
case!(a_reason_on_a_withdrawn_finding_nobody_reads, "dropped", |v| {
    let previous = node(v, "5a")["out"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["name"] == "previous")
        .unwrap();
    let branch = previous["item"]["branches"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|b| b["value"] == "withdraw")
        .unwrap();
    branch["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "reason", "type": "text", "desc": "x" }));
});
case!(the_blind_round_read_by_nobody, "nobody reads", |v| {
    node(v, "5a")["inputs"].as_array_mut().unwrap().remove(1);
});
case!(the_blind_conclusions_on_a_report_read_by_nobody, "nobody reads", |v| {
    drop_source(v, "5a", json!(["5", "conclusions"]));
    obj(node(v, "5")).remove("checks");
});
case!(
    a_fix_that_never_hears_the_plan_went_over,
    "nobody reads plan[over]",
    |v| {
        drop_source(v, "8", json!(["19a", "plan[over]"]));
    }
);
case!(
    an_answer_to_an_engine_step_that_the_step_does_not_read,
    "nobody reads reply.text",
    |v| {
        node(v, "13")["inputs"].as_array_mut().unwrap().pop();
        drop_source(v, "5a", json!(["15", "reply"]));
    }
);

// Nothing missing.
case!(an_input_missing_on_some_way, "missing", |v| {
    let x = node(v, "19")["inputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|x| x["id"] == "re")
        .unwrap();
    obj(x).remove("maybe");
});
case!(an_input_marked_maybe_that_is_always_there, "marked maybe", |v| {
    node(v, "5a")["inputs"][4]["maybe"] = json!(true);
});
case!(an_input_missing_behind_an_engine_step, "missing", |v| {
    drop_source(v, "8", json!(["5a", "previous[not_fixed]"]));
    drop_source(v, "8", json!(["5a", "previous[insist]"]));
});
