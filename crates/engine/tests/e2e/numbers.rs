//! The numbers the engine gives a project's architect: how far up they count, the four weeks of each, and that the
//! System view counts the same items made.

use super::*;

const HOUR: i64 = 3_600_000;

/// The flow as `flow/flow.json` has it, with the plan given the numbers, which the cell that drafts a policy is given in
/// the flow, so that these tests can step to them: compiled and stored as a version of the flow, which the engine runs.
fn the_plan_given_numbers(e: &mut Engine) {
    let mut f: Value = serde_json::from_str(flow::FLOW).unwrap();
    let plan = f["flows"]
        .as_array_mut()
        .and_then(|gs| gs.iter_mut().find(|g| g["name"] == "architect"))
        .and_then(|g| g["nodes"].as_array_mut())
        .and_then(|ns| ns.iter_mut().find(|n| n["id"] == "B2"))
        .unwrap();
    plan["inputs"].as_array_mut().unwrap().push(json!({
        "what": "The numbers the engine measures",
        "from": [{ "engine": "numbers" }]
    }));
    let cx = flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    };
    let text = flow::compile(&f.to_string(), &cx)
        .unwrap_or_else(|x| panic!("{x}"))
        .json();
    e.flow_now =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (digest, compiled) VALUES ('the plan given numbers', ?1)",
                [&text],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
}

/// The whole of an engine's input of the standard context that a work's round at a cell was given (the first round or
/// the last), by the end of its address: the version `contents` keeps of it, whether the round was given it whole, only
/// what changed in it, or referred to it.
fn given_whole(e: &Engine, work: i64, cell: &str, last: bool, end: &str) -> Option<Value> {
    let input = input_at(e, work, cell, last);
    let key = input["conditional"]
        .as_array()?
        .iter()
        .find(|c| c["address"].as_str().is_some_and(|a| a.ends_with(end)))?["text"]
        .as_str()?
        .to_string();
    let text: String =
        e.db.conn()
            .query_row("SELECT text FROM contents WHERE hash = ?1", [&key], |r| r.get(0))
            .ok()?;
    serde_json::from_str(&text).ok()
}

// Item 231, A6: the numbers the architect is given count up to the moment they are given, not to the start of its work.
// After a work of the architect has started and been given its numbers, the flow merges an item, an item is made and one
// cancelled, and a round of the work itself spends: the numbers its next round at the plan is given count all of them in
// the latest week, say up to when they count (`up_to`, no earlier than those records and no later than the
// round's start), and leave the week before as it was. What each week spent is split between the items' works and the
// architect's, the work's own rounds until then among the architect's.
#[test]
fn the_numbers_count_up_to_the_round_that_gives_them() {
    let s = setup("numbers");
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
    the_plan_given_numbers(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let ago = db::now() - HOUR;
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE rounds SET usage = '{\"cost_usd\":0.25}' WHERE cell = 'H10'", [])?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
            params![item, v],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, usage, started_at, ended_at)
             VALUES (?1, 1, 'I1', '{}', 'tools', 'c', '{\"cost_usd\":0.5}', ?2, ?2)",
            params![tx.last_insert_rowid(), ago],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look at the numbers.");
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
    let first = given_whole(&e, arch, "B2", false, "·engine·numbers").expect("the first numbers");
    // What happens after the work started and was given its numbers: a merge by the flow, an item made and one
    // cancelled, and the work's own round that asked for tools costing a dollar.
    let after = db::now();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'M2', 'merged')",
            params![item, v],
        )?;
        let merged = json!({ "result": { "kind": "merged", "merge_commit": "a".repeat(40) } }).to_string();
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, 1, 'M2', '{}', ?2, 'merged', 'c', ?3, ?3)",
            params![tx.last_insert_rowid(), merged, after],
        )?;
        tx.execute(
            "UPDATE rounds SET usage = '{\"cost_usd\":1.0}' WHERE work = ?1 AND cell = 'B2' AND transition = 'tools'",
            [arch],
        )?;
        Ok(())
    })
    .unwrap();
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level)
             VALUES ('p', 'Made later', 'Missing.', 'Needed.', 'change', 'l')",
            [],
        )?;
        tx.execute(
            "INSERT INTO start_order (item, held) VALUES (?1, 1)",
            [tx.last_insert_rowid()],
        )?;
        tx.execute("UPDATE items SET cancelled = 'not needed' WHERE id = ?1", [item])?;
        Ok(())
    })
    .unwrap();
    let recorded = db::now();
    assert!(
        steps_until(&mut e, 10, took("planned")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let second = given_whole(&e, arch, "B2", true, "·engine·numbers").expect("the second numbers");
    let (n, started): (i64, i64) =
        e.db.conn()
            .query_row(
                "SELECT n, started_at FROM rounds WHERE work = ?1 AND cell = 'B2' ORDER BY n DESC LIMIT 1",
                [arch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    let up_to = second["up_to"].as_str().unwrap().to_string();
    assert!(
        engine::utc(recorded) <= up_to && up_to <= engine::utc(started),
        "{up_to}, recorded {}, the round started {}",
        engine::utc(recorded),
        engine::utc(started)
    );
    let latest = |x: &Value, k: &str| x["weeks"][0][k].as_i64().unwrap();
    for k in ["merged", "items_made", "cancelled"] {
        assert_eq!(latest(&second, k), latest(&first, k) + 1, "{k}: {first}\n{second}");
    }
    assert_eq!(second["weeks"][1], first["weeks"][1], "{first}\n{second}");
    // The other numbers count the merge in their latest week too, and leave the week before.
    let week = |x: &Value, m: &str, k: usize| x["measures"][m]["by_week"][k].clone();
    assert_eq!(week(&first, "work.to_merge", 0), Value::Null, "{first}");
    assert_eq!(week(&second, "work.to_merge", 0), json!(after - ago), "{second}");
    assert_eq!(week(&first, "ci.wait", 0), Value::Null, "{first}");
    assert_eq!(week(&second, "ci.wait", 0), json!(0), "{second}");
    assert_eq!(week(&second, "work.to_merge", 1), Value::Null, "{second}");
    // The architect's works spent the other architect work's round and each round of this one before the round given
    // the numbers, by what each reported; the items' works, the round an hour ago.
    let own: f64 = {
        let mut st =
            e.db.conn()
                .prepare("SELECT json_extract(usage, '$.cost_usd') FROM rounds WHERE work = ?1 AND n < ?2")
                .unwrap();
        st.query_map(params![arch, n], |r| r.get::<_, Option<f64>>(0))
            .unwrap()
            .map(|c| c.unwrap().unwrap_or_default())
            .sum()
    };
    let cents = |x: f64| (x * 100.0).round() / 100.0;
    assert_eq!(
        second["weeks"][0]["spent_usd"],
        json!({ "items": 0.5, "architect": cents(0.25 + own) }),
        "{second}"
    );
    assert!(own >= 1.0, "{own}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item 231, A2: the architect's numbers count the items made as the System view lists them, by the health numbers' one
// definition: an item imported from an earlier board was not made here. Both count two over the week before the round
// given the numbers.
#[test]
fn the_architect_s_numbers_and_the_system_view_count_the_same_items_made() {
    let s = setup("made");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    the_plan_given_numbers(&mut e);
    let ago = db::now() - HOUR;
    let imported =
        e.db.write(Actor::Owner, |tx| {
            for from in [None, None, Some("an earlier board")] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level, imported_from)
                     VALUES ('p', 'Made', 'Missing.', 'Needed.', 'change', 'l', ?1)",
                    [from],
                )?;
                tx.execute(
                    "INSERT INTO start_order (item, held) VALUES (?1, 1)",
                    [tx.last_insert_rowid()],
                )?;
            }
            tx.execute("UPDATE changes SET at = ?1 WHERE table_name = 'items'", [ago])?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    say(&mut e, "Look at the numbers.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| path_of(e, arch).iter().any(|c| c == "B2")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let numbers = given_value(&message_at(&e, arch, "B2", false), "·engine·numbers").unwrap();
    assert_eq!(numbers["weeks"][0]["items_made"], 2, "{numbers}");
    let start: i64 =
        e.db.conn()
            .query_row(
                "SELECT started_at FROM rounds WHERE work = ?1 AND cell = 'B2' ORDER BY n LIMIT 1",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    let (projects, limits) = (vec!["p".to_string()], BTreeMap::new());
    let ask = api::SystemAsk {
        weeks: Some(2),
        tz: 0,
        projects: &projects,
        limits: &limits,
        now: db::now(),
    };
    let system = api::system(e.db.conn(), &api::Flows::default(), &ask).unwrap();
    let week = start - 7 * 24 * HOUR..start;
    let made: Vec<i64> = system
        .output
        .made
        .iter()
        .filter(|m| week.contains(&m.at))
        .map(|m| m.item)
        .collect();
    assert_eq!(made.len(), 2, "{made:?}");
    assert!(!made.contains(&imported), "{made:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Seven cells' rounds of an item's work, each ended an hour ago with a context a thousand tokens larger than the cell
/// before, the last the largest; two runs of CI that overlap, 1,500 ms in all, and the merge after them; a blocker of each
/// of the five kinds, lasting 2,000 ms, and a pause of a work; and eight days ago the merge of another item, with no CI.
fn seed_a_week_of_numbers(e: &mut Engine, item: i64, other: i64) {
    let v = e.flow_now;
    let now = db::now();
    let merged = |commit: &str| json!({ "result": { "kind": "merged", "merge_commit": commit } }).to_string();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'M2', 'merged')",
            params![item, v],
        )?;
        let work = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, 'engineer', 'claude', 's')",
            [work],
        )?;
        let session = tx.last_insert_rowid();
        let cells = ["R1", "D2", "I1", "T3", "T4", "T6", "T1"];
        for (k, cell) in cells.into_iter().enumerate() {
            let tokens = (k as i64 + 1) * 1000;
            let usage = json!({ "usage": { "iterations": [{ "input_tokens": tokens }] } }).to_string();
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, transition, config_commit, usage, started_at, ended_at)
                 VALUES (?1, ?2, ?3, ?4, '{}', 'next', 'c', ?5, ?6, ?6)",
                params![work, k as i64 + 1, cell, session, usage, now - HOUR],
            )?;
        }
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, 8, 'M2', '{}', ?2, 'merged', 'c', ?3, ?3)",
            params![work, merged(&"a".repeat(40)), now - HOUR],
        )?;
        for (tool, from, to) in [("ci", 0, 1_000), ("ci_before", 500, 1_500)] {
            tx.execute(
                "INSERT INTO tool_runs (work, round, tool, args, attempt, queued_at, started_at, ended_at)
                 VALUES (?1, 1, ?2, '{}', 1, ?3, ?3, ?4)",
                params![work, tool, now - 2 * HOUR + from, now - 2 * HOUR + to],
            )?;
        }
        for kind in ["question", "tool_failure", "owner_ask", "items", "ruling"] {
            let holder = match kind {
                "owner_ask" => "owner",
                "items" => "items",
                _ => "advisor",
            };
            tx.execute(
                "INSERT INTO blockers (work, cell, kind, what, holder, items, at, since, ended, outcome, ended_at)
                 VALUES (?1, 'I1', ?2, 'w', ?3, CASE WHEN ?3 = 'items' THEN '[]' END, ?4, ?4, 'resolved', '{}', ?5)",
                params![work, kind, holder, now - 2 * HOUR, now - 2 * HOUR + 2_000],
            )?;
        }
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'M2', 'merged')",
            params![other, v],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, 1, 'M2', '{}', ?2, 'merged', 'c', ?3, ?3)",
            params![tx.last_insert_rowid(), merged(&"b".repeat(40)), now - 8 * 24 * HOUR],
        )?;
        // A work paused and let go on, and over with before the engine could take a step of it.
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'I1')",
            params![other, v],
        )?;
        let paused = tx.last_insert_rowid();
        tx.execute(
            "UPDATE works SET paused = 'held', paused_by = 'owner' WHERE id = ?1",
            [paused],
        )?;
        tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [paused])?;
        tx.execute("UPDATE works SET ended = 'stopped: by hand' WHERE id = ?1", [paused])?;
        Ok(())
    })
    .unwrap();
}

// Item 231, A11: over seeded records, the numbers give the architect each measure with what it is, its unit and its four
// weeks, the latest first, in milliseconds for a time; a week with no value is null and says why, once; the numbers
// under `weeks` are not given again; of the seven cells with a context, the five largest in the latest week, and that two
// more there are; every reason a work stopped; what nothing records listed with why; and the policy's signal that names
// a measure is given that measure's weeks.
#[test]
fn the_numbers_give_each_measure_four_weeks_and_none_twice() {
    let s = setup("weeks");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    the_plan_given_numbers(&mut e);
    let (item, other) = (an_item(&mut e), an_item_more(&mut e));
    seed_a_week_of_numbers(&mut e, item, other);
    let arch = architect_work_at(&mut e, "B2");
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let numbers = given_value(&m, "·engine·numbers").expect(&m);
    let measures = numbers["measures"].as_object().unwrap();
    for gone in ["items.made", "items.merged", "items.cancelled", "spend.usd"] {
        assert!(!measures.contains_key(gone), "{gone}: {numbers}");
    }
    for (name, m) in measures {
        assert!(m["what"].as_str().is_some_and(|t| !t.is_empty()), "{name}: {m}");
        assert!(m["unit"].as_str().is_some_and(|t| !t.is_empty()), "{name}: {m}");
        assert_eq!(m["by_week"].as_array().map(Vec::len), Some(4), "{name}: {m}");
    }
    for name in [
        "ci.wait",
        "work.to_merge",
        "owner.reply",
        "work.stopped",
        "queue.oldest",
        "page.read_ms",
    ] {
        assert_eq!(measures[name]["unit"], "ms", "{name}: {numbers}");
    }
    // The wait for CI: the latest week's merge, then the merge of the week before with no CI, then no merge.
    let wait = &measures["ci.wait"];
    assert_eq!(wait["by_week"], json!([1500, 0, null, null]), "{numbers}");
    assert_eq!(wait["no_value"], "no item merged", "{numbers}");
    // Nothing in a week of a count of the records is a zero, with no reason to give.
    assert_eq!(measures["merge.broken"]["by_week"], json!([0, 0, 0, 0]), "{numbers}");
    assert_eq!(measures["merge.broken"].get("no_value"), None, "{numbers}");
    assert_eq!(
        measures["engine.cpu"]["by_week"],
        json!([null, null, null, null]),
        "{numbers}"
    );
    assert!(
        measures["engine.cpu"]["no_value"]
            .as_str()
            .is_some_and(|t| !t.is_empty()),
        "{numbers}"
    );
    // The five cells with the largest context in the latest week, and two more.
    let context = &measures["round.context"];
    assert_eq!(context["by_week"], json!([4000, null, null, null]), "{numbers}");
    let cells: Vec<&str> = context["by_cell"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        cells,
        [
            "engineer/T1",
            "engineer/T6",
            "engineer/T4",
            "engineer/T3",
            "engineer/I1"
        ],
        "{numbers}"
    );
    assert_eq!(
        context["by_cell"]["engineer/T1"],
        json!([7000, null, null, null]),
        "{numbers}"
    );
    assert_eq!(context["other_cells"], 2, "{numbers}");
    // Every reason a work stopped, with the time in milliseconds; waiting for a slot, which is not counted yet, is null.
    let stops = &measures["work.stops"]["by_reason"];
    let stopped = &measures["work.stopped"]["by_reason"];
    for reason in ["question", "tool_failure", "owner_ask", "items", "ruling"] {
        assert_eq!(stops[reason], json!([1, 0, 0, 0]), "{reason}: {numbers}");
        assert_eq!(stopped[reason], json!([2000, 0, 0, 0]), "{reason}: {numbers}");
    }
    assert_eq!(stops["paused"], json!([1, 0, 0, 0]), "{numbers}");
    assert_eq!(stops["slot"], json!([null, null, null, null]), "{numbers}");
    // Only what nothing records yet is listed as not measured, each with why, and only there.
    let listed = numbers["not_measured"].as_object().unwrap();
    let names: BTreeSet<&str> = listed.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "merge.defects",
            "work.stops for reason slot",
            "work.stopped for reason slot"
        ]),
        "{numbers}"
    );
    assert!(
        listed.values().all(|why| why.as_str().is_some_and(|t| !t.is_empty())),
        "{numbers}"
    );
    assert_eq!(
        measures["merge.defects"]["by_week"],
        json!([null, null, null, null]),
        "{numbers}"
    );
    assert_eq!(measures["merge.defects"].get("no_value"), None, "{numbers}");
    // A signal of the policy that names a measure is given it as the numbers give it: its four weeks, what it is, its
    // unit, and why a week has no value; a measure nothing records, or that has a part nothing records, says why, since
    // whoever reads only the signals is given neither `measures` nor `not_measured`.
    let policy = json!({ "direction": "d", "why": "w", "principles": [{ "no": 1, "text": "t" }],
                         "signals": [{ "what": "CI takes longer", "how": "ci wait grows week by week" },
                                     { "what": "Errors found late", "how": "merge defects go up" },
                                     { "what": "Works wait", "how": "work stops go up" }] });
    let written =
        json!({ "outcome": { "policy": policy, "new_measures": [] }, "progress": "Written.", "tell_owner": [] });
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended, project)
             VALUES (NULL, ?1, 'architect', 'R1', 'over', 'p')",
            [v],
        )?;
        let set = tx.last_insert_rowid();
        record_round(tx, set, 1, "P2", &written, "written")?;
        record_round(tx, set, 2, "P3", &json!({}), "next")
    })
    .unwrap();
    let check = architect_work_at(&mut e, "P1");
    let m = engine::next_message(&mut e, check).unwrap().text;
    let signals = given_value(&m, "·engine·policy_signals").expect(&m);
    assert_eq!(
        signals["1"]["numbers"]["ci.wait"]["by_week"],
        json!([1500, 0, null, null]),
        "{signals}"
    );
    let wait = &signals["1"]["numbers"]["ci.wait"];
    assert_eq!(wait["no_value"], "no item merged", "{signals}");
    assert_eq!(wait["unit"], "ms", "{signals}");
    assert_eq!(wait["what"], measures["ci.wait"]["what"], "{signals}");
    assert_eq!(wait.get("not_measured"), None, "{signals}");
    let defects = &signals["2"]["numbers"]["merge.defects"];
    assert_eq!(defects["by_week"], json!([null, null, null, null]), "{signals}");
    assert_eq!(
        defects["not_measured"]["merge.defects"], listed["merge.defects"],
        "{signals}"
    );
    let stops = &signals["3"]["numbers"]["work.stops"];
    assert_eq!(stops["by_reason"]["question"], json!([1, 0, 0, 0]), "{signals}");
    assert_eq!(
        stops["not_measured"],
        json!({ "work.stops for reason slot": listed["work.stops for reason slot"] }),
        "{signals}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Another item of project p, which no work runs on.
fn an_item_more(e: &mut Engine) -> i64 {
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level)
             VALUES ('p', 'Print it', 'No printer.', 'Needed.', 'change', 'l')",
            [],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .unwrap()
}
