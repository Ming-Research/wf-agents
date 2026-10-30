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

// The architect's graph compiles with no finding, as the design the owner approved: where it waits for a work to start
// and the start of its work (`S0`, `S1`, `U1`, `P1`, `N1`), the setting of a new policy (`P2` to `P12`), the queue
// (`Q0`) and each matter's path, the review of a batch of item changes (`B1`, `B2`, `B3`, `B4`), the wrap-up (`E1`,
// `E2`), the owner's ask that holds only its matter (`A3`), the tools (`X1`), the observer (`X2`) and the wait for the
// owner's Start after a pause (`X3`): as many cells, rows and states. The owner's words reach the architect only
// through its queue: no cell of it takes an interjection, and none answers one.
#[test]
fn the_architect_graph_compiles_as_its_reference_has_it() {
    let c = compiled();
    let s = c.stats.iter().find(|s| s.graph == "architect").unwrap();
    assert_eq!((s.cells, s.rows, s.states), (40, 154, 211));
    let g = c.graph("architect").unwrap();
    assert!(g.cell("X4").is_none());
    assert!(
        g.cells
            .iter()
            .all(|x| x.rows.iter().all(|r| !r.interject && r.name != "answers"))
    );
    assert!(c.graph("policy").is_none());
}

// The setting of a new policy is a stage of the architect's graph, in its session: the check of the policy in force
// that sets it again goes on to the study (`P2`), holding new starts; the owner's reading of the study (`P3`) and
// approval of the text (`P11`) are asks that hold only the setting's matter while the architect goes on with its
// queue (`B1`), and the matter, routed back to the cell with the owner's answer (`Q2`), goes on by the owner's choice;
// the setting ends at what to do next (`N1`), new starts let go, its matter handled. An owner's cell of the architect's
// graph with no row that goes on without the answer is refused, since the architect never stops for the owner.
#[test]
fn the_setting_of_a_policy_is_a_stage_of_the_architect_s_work() {
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
    let set_again = row("P1", "set_again");
    assert_eq!(
        (set_again.to.as_str(), does(&set_again)),
        ("P2", vec!["hold_for_policy".to_string(), "start_setting".to_string()])
    );
    let given = |k: &str, v: &str| Map::from_iter([(k.to_string(), Value::String(v.into()))]);
    let none = Map::new();
    let ruling = |choice: &str| serde_json::json!({ "ruling": { "choice": choice } });
    for (at, ways) in [
        ("P3", [("satisfied", "P4"), ("study_on", "P2"), ("policy_stays", "N1")]),
        (
            "P11",
            [("approved", "P12"), ("fix_text", "P8"), ("not_good_enough", "P2")],
        ),
    ] {
        let held = g.take(at, &serde_json::json!({}), &none).unwrap();
        assert_eq!((held.name.as_str(), held.to.as_str(), held.drops), ("held", "B1", true));
        for (choice, to) in ways {
            let r = g.take(at, &ruling(choice), &none).unwrap();
            assert_eq!((r.name.as_str(), r.to.as_str()), (choice, to));
        }
        let back = g.take("Q2", &serde_json::json!({}), &given("route", at)).unwrap();
        assert_eq!(back.to, at);
    }
    for end in [row("P3", "policy_stays"), row("P12", "next")] {
        assert_eq!(
            (end.to.as_str(), does(&end)),
            (
                "N1",
                vec!["release_policy_hold".to_string(), "close_matter".to_string()]
            )
        );
    }
    // A new session at a cell of the setting goes back to understanding the project, and the setting, taken again, to
    // the cell it stood at.
    for at in ["P2", "P4", "P7", "P8", "P10"] {
        assert_eq!(row(at, "session_anew").to, "S1", "{at}");
        let back = g.take("Q2", &serde_json::json!({}), &given("route", at)).unwrap();
        assert_eq!(back.to, at);
    }
    assert!(g.cell("P6").unwrap().line.is_none());
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let p3 = v["flows"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "P3")
        .unwrap();
    p3["cases"][0].as_object_mut().unwrap().remove("drops");
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(
        e.contains("the architect's own graph stops: P3 goes on only with the owner's answer"),
        "{e}"
    );
}

// The owner's words are a talk with the architect, as the owner approved: the look at the queue takes them first,
// without a pick (`Q0`); the cell of the owner's words (`H8`) replies and waits for the owner (`H11`), whose next words
// or Done take the talk on in the same session, and whose silence past the installation's wait holds the talk back in
// the queue while the architect goes on (`B1`); only the owner ends it, and its last turn takes one way on: done (`B1`),
// what to do next again (`N1`), a new policy (`P2`, one setting at a time) or understanding the project again (`U1`).
// The owner decides on a study or a text by its buttons alone, its ruling holding no words. A row that waits in the
// architect's graph with no setting bounding the wait is refused, since the architect stops for no one.
#[test]
fn the_owner_s_words_are_a_talk_the_owner_ends() {
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
    assert_eq!(g.cell("Q0").unwrap().rows[0].name, "owner_words");
    for (name, to, then) in [
        ("replied", "H11", vec!["take_words", "reply_to_owner"]),
        ("done", "B1", vec!["take_words", "close_matter"]),
        ("re_plan", "N1", vec!["take_words", "close_matter"]),
        (
            "new_policy",
            "P2",
            vec!["take_words", "close_matter", "hold_for_policy", "start_setting"],
        ),
        ("to_setting", "B1", vec!["take_words", "close_matter"]),
        ("re_understand", "U1", vec!["take_words", "close_matter"]),
    ] {
        let r = row("H8", name);
        assert_eq!(
            (r.to.as_str(), does(&r)),
            (to, then.iter().map(|x| x.to_string()).collect()),
            "{name}"
        );
    }
    let given = |v: &str| Map::from_iter([("reply".to_string(), Value::String(v.into()))]);
    for (value, to) in [("said", "H8"), ("ended", "H8"), ("waits", "H11"), ("no_reply", "B1")] {
        let r = g.take("H11", &serde_json::json!({}), &given(value)).unwrap();
        assert_eq!((r.name.as_str(), r.to.as_str(), r.waits), (value, to, value == "waits"));
    }
    assert_eq!(does(&row("H11", "no_reply")), ["hold_talk"]);
    for at in ["P3", "P11"] {
        let schema = g.cell(at).unwrap().schema.clone().unwrap();
        assert!(schema.pointer("/properties/ruling/properties/text").is_none(), "{at}");
    }
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let wait = v["flows"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["id"] == "H11")
        .unwrap();
    wait["cases"][2].as_object_mut().unwrap().remove("until");
    let e = flow::compile(&v.to_string(), &context()).unwrap_err().to_string();
    assert!(e.contains("the architect's own graph stops: H11 waits"), "{e}");
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
        let shared: Vec<&flow::CompiledRow> = cell
            .rows
            .iter()
            .filter(|x| x.conditions.iter().any(|a| a.about_no_return()))
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
