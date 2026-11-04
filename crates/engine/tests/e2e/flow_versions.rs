//! Works brought to a new flow version, a work's graph, and what a flow's cost says the engine then does with
//! each work.

use super::*;

/// The work an item had first, at its second cell `R2` on a flow version before the one the engine runs, changed as
/// `change` says, with the engine started again over it; gives the engine, what it says, the item and the work.
fn at_r2_on_an_older_version(
    s: &Setup,
    change: impl FnOnce(&mut flow::CompiledGraph),
) -> (Engine, Arc<Mutex<Vec<String>>>, i64, i64) {
    let (mut e, said, item) = start(s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    older(&mut e, &[work], change);
    drop(e);
    let (e, said) = engine(s, scripts());
    (e, said, item, work)
}

// A work on an older flow version that goes on moves to the version the engine runs when the cell it stands at is
// unchanged there and every return it handed in still fits its cell there, whatever else changed: the same work goes
// on at its cell in the same session, and says which version it moved from.
#[test]
fn a_work_at_an_unchanged_cell_moves_and_keeps_its_session() {
    let s = setup("moves");
    let (mut e, said, item, work) = at_r2_on_an_older_version(&s, |g| renamed(g, "D4"));
    let v: i64 =
        e.db.conn()
            .query_row("SELECT flow_version FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    let stepped = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(stepped.work, Some(work), "{log}");
    assert_eq!(stepped.cell.as_deref(), Some("R2"), "{log}");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 1, "{works:?}\n{log}");
    assert_eq!(works[0].5, None);
    let (now, from): (i64, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT flow_version, moved_from FROM works WHERE id = ?1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((now, from), (e.flow_now, Some(v)), "{log}");
    assert_eq!(session_of(&e, work, "R2"), session_of(&e, work, "R1"), "{log}");
    assert!(
        log.contains(&format!("work {work} moved from flow version {v} to {now}, at cell R2")),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A change of a flow version at cell `R1`: the outcome of its return, in any of its forms, may hold a field `before`,
/// which the version the engine runs does not have.
fn with_before(g: &mut flow::CompiledGraph) {
    let r1 = g.cells.iter_mut().find(|c| c.id == "R1").unwrap();
    let forms = r1
        .schema
        .as_mut()
        .and_then(|s| s["properties"]["outcome"]["anyOf"].as_array_mut());
    for form in forms.into_iter().flatten() {
        if let Some(Value::Object(props)) = form.get_mut("properties") {
            props.insert("before".into(), json!({ "type": "string" }));
        }
    }
}

// A work on an older flow version whose earlier return's cell has another schema in the version the engine runs, which
// still takes the return as the round stored it, moves: a field the return does not hold, taken out.
#[test]
fn a_work_whose_earlier_returns_still_fit_moves() {
    let s = setup("refit");
    let (mut e, said, item, work) = at_r2_on_an_older_version(&s, with_before);
    let stepped = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(stepped.work, Some(work), "{log}");
    assert_eq!(stepped.cell.as_deref(), Some("R2"), "{log}");
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A round that takes a row keeps the counter the row counts (`rounds.counts`), by the flow the work runs on: the
// implementer's asking for tools gives `tool_requests`, a row that counts nothing and a round that took no row give
// none. The expected counter of each round is read from the compiled flow, by its cell and the row it took.
// The views read the counter from the round, not from the version the work is on now.
#[test]
fn a_round_keeps_the_counter_of_the_row_it_took() {
    let s = setup("round-counts");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "X1");
    let log = said.lock().unwrap().join("\n");
    let (work, _, _) = state(&e, item).unwrap();
    let flow = e.flow(e.flow_now).unwrap();
    let engineer = flow.graphs.iter().find(|g| g.name == engine::ENGINEER).unwrap();
    let rounds = engine::query_json(
        e.db.conn(),
        "SELECT n, cell, transition, counts, rejected FROM rounds WHERE work = ?1 ORDER BY n",
        &[&work],
        1000,
    )
    .unwrap();
    let (mut counting, mut counting_nothing) = (0, 0);
    for r in &rounds {
        // A round that ended in no return took a shared row, which the test of `finish_round` checks.
        if !r["rejected"].is_null() {
            continue;
        }
        let row = r["cell"].as_str().zip(r["transition"].as_str()).map(|(cell, row)| {
            engineer
                .cells
                .iter()
                .find(|c| c.id == cell)
                .and_then(|c| c.rows.iter().find(|x| x.name == row))
                .unwrap_or_else(|| panic!("round {r}: the flow has no row {row} at cell {cell}"))
        });
        let expected = row.and_then(|x| x.counts.as_deref());
        assert_eq!(r["counts"].as_str(), expected, "round {r}\n{log}");
        match (row, expected) {
            (Some(_), None) => counting_nothing += 1,
            (Some(_), Some(_)) => counting += 1,
            (None, _) => {}
        }
    }
    let tools: Vec<&Value> = rounds.iter().filter(|r| r["transition"] == "tools").collect();
    assert_eq!(tools.len(), 1, "{rounds:?}\n{log}");
    assert_eq!(tools[0]["counts"], "tool_requests", "{log}");
    assert!(counting >= 1 && counting_nothing >= 1, "{rounds:?}");
    // And it keeps it whatever flow version its work is on afterwards: a work put on a version in which the same row
    // counts another counter (`tools` as `asks`) keeps the counter of the round it took before, and the counters the
    // views give do not change with it.
    let counters = |e: &Engine| -> Vec<(String, i64)> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT counter, times FROM work_counters WHERE work = ?1 AND counter IN ('asks', 'tool_requests')
                     ORDER BY counter",
                )
                .unwrap();
        st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let asked = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT asks FROM work_asking WHERE work = ?1", [work], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(counters(&e), [("tool_requests".to_string(), 1)], "{log}");
    assert_eq!(asked(&e), 0);
    older(&mut e, &[work], |g| {
        let row = g
            .cells
            .iter_mut()
            .find(|c| c.id == "I1")
            .and_then(|c| c.rows.iter_mut().find(|r| r.name == "tools"))
            .unwrap();
        row.counts = Some("asks".into());
    });
    assert_eq!(counters(&e), [("tool_requests".to_string(), 1)], "{log}");
    assert_eq!(asked(&e), 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230: a work on an older flow version, one of whose earlier answers the version the engine runs no longer takes,
// is the same work after. It comes to cell `V1`, which records the versions, the cell it goes back to and why, and goes
// back from there to the cell of that answer, the owner told; it does that cell again and goes on to the merge, and its
// design's open question, answered before, is not put to the architect again.
#[test]
fn a_work_whose_earlier_answer_no_longer_fits_goes_back_to_that_cell_and_on_to_the_merge() {
    let s = setup("reshaped");
    let mut agents = scripts();
    agents.insert("D2", designs_with_an_open_question());
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let asked = open_questions(&e, work);
    assert_eq!(asked.len(), 1, "{}", said.lock().unwrap().join("\n"));
    let v = older(&mut e, &[work], with_before);
    let n: i64 =
        e.db.write(Actor::Engine, |tx| {
            // The question has its answer, as the architect's reply leaves it.
            tx.execute(
                "UPDATE architect_matters SET body = json_set(body, '$.answer', 'One table.') WHERE id = ?1",
                [asked[0].0],
            )?;
            // The answer of cell R1 holds a field the version the engine runs does not take.
            let n: i64 = tx.query_row(
                "SELECT n FROM rounds WHERE work = ?1 AND cell = 'R1' AND transition IS NOT NULL",
                [work],
                |r| r.get(0),
            )?;
            tx.execute(
                "UPDATE rounds SET output = json_set(output, '$.outcome.before', 'kept') WHERE work = ?1 AND n = ?2",
                params![work, n],
            )?;
            Ok(n)
        })
        .unwrap();
    e.advance(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let now = e.flow_now;
    assert_eq!(versions_of(&e, work), (now, Some(v)), "{log}");
    let v1 = brought(&e, work);
    assert_eq!(v1.len(), 1, "{v1:?}\n{log}");
    let (took, output) = &v1[0];
    assert_eq!(
        (took.as_str(), &output["back_to"], &output["from"], &output["to"]),
        ("went_back", &json!("R1"), &json!(v), &json!(now)),
        "{log}"
    );
    let why = output["why"].as_str().unwrap_or_default();
    assert!(
        why.starts_with(&format!(
            "the return of cell R1 in round R{work}-{n} no longer fits it ("
        )),
        "{why}"
    );
    let told = notices_of(&e, work);
    assert!(
        told.iter()
            .any(|t| t.contains("goes back to cell R1 ") && t.contains(why)),
        "{told:?}"
    );
    assert_eq!(transitions(&e, item, "R1").len(), 2, "{log}");
    for _ in 0..200 {
        if transitions(&e, item, "M2").iter().any(|t| t == "merged") {
            break;
        }
        e.advance(work).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert!(transitions(&e, item, "M2").iter().any(|t| t == "merged"), "{log}");
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    assert_eq!(open_questions(&e, work).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A version before the one the engine runs as flow 34 was before 35: without the rows of the engine's fallback that 35
/// gave every agent's cell, for a step of the engine's that reaches the network and fails.
fn without_the_network_rows(g: &mut flow::CompiledGraph) {
    let added = ["transient_too_often", "step_network_too_often", "step_network"];
    for c in g.cells.iter_mut().filter(|c| c.kind == "agent") {
        c.rows.retain(|r| !(r.fallback && added.contains(&r.name.as_str())));
    }
}

// Item #230, flow 35: a version that only gives every agent's cell more rows of the engine's fallback moves a work where
// it stands, at any point of the cell it is at: just come to it, back from its tools with their results, or with its
// return sent back. Each time it is the same work at the same cell, in the same session, through no cell of its own.
#[test]
fn a_version_that_adds_rows_every_agent_s_cell_shares_moves_a_work_at_any_point_of_its_cell() {
    let s = setup("shared-rows");
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    // Its second round at cell I1 hands in a head that is no text, which is sent back.
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 2 {
                return (Vec::new(), vec![("outcome.head", json!(5))]);
            }
            implement(r, n)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let mut session = None;
    for point in ["just come", "back from its tools", "sent back"] {
        let v = older(&mut e, &[work], without_the_network_rows);
        e.advance(work).unwrap();
        let log = said.lock().unwrap().join("\n");
        assert_eq!(versions_of(&e, work), (e.flow_now, Some(v)), "{point}\n{log}");
        assert!(brought(&e, work).is_empty(), "{point}\n{log}");
        assert_eq!(works_of(&e, item).len(), 1, "{point}\n{log}");
        let now = session_of(&e, work, "I1");
        assert_eq!(*session.get_or_insert(now), now, "{point}\n{log}");
        if point == "just come" {
            // The cell asked for tools; they run, and the work is back at it with their results.
            e.advance(work).unwrap();
            assert_eq!(state(&e, item).unwrap().1, "I1", "{log}");
        }
    }
    let log = said.lock().unwrap().join("\n");
    let sent: usize = ended_at(&e, item, "I1")
        .iter()
        .filter(|(took, why)| took.is_none() && why.is_some())
        .count();
    assert_eq!(sent, 1, "{log}");
    assert_eq!(transitions(&e, item, "I1"), ["tools", "done"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230, W245: a work at an engine's cell that has not run moves when the version gives that cell a row of its own,
// and takes that row there.
#[test]
fn a_work_at_an_engine_s_cell_not_run_moves_and_takes_the_row_its_version_added() {
    let s = setup("engine-row");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "D4");
    let (work, _, _) = state(&e, item).unwrap();
    // The version before had no row for a project that names nothing under test.
    let v = older(&mut e, &[work], |g| {
        let d4 = g.cells.iter_mut().find(|c| c.id == "D4").unwrap();
        d4.rows.retain(|r| r.name != "no");
    });
    e.advance(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(versions_of(&e, work), (e.flow_now, Some(v)), "{log}");
    assert!(brought(&e, work).is_empty(), "{log}");
    assert_eq!(transitions(&e, item, "D4"), ["no"], "{log}");
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230, as the owner asked: a cell under way that the version the engine runs puts on another line, or has read
// other inputs of its own, is done again from its start. The same work comes to cell `V1` and goes back from there to
// that cell, whose next round, in the session of its line in that version, is told it came from `V1`.
#[test]
fn a_cell_under_way_that_the_version_changed_is_done_again_from_its_start() {
    let changes: [Changed; 2] = [
        ("is on another line", |g| {
            g.cells.iter_mut().find(|c| c.id == "I1").unwrap().line = Some("review".into());
        }),
        ("reads other inputs", |g| {
            let i1 = g.cells.iter_mut().find(|c| c.id == "I1").unwrap();
            let own = i1.inputs.iter_mut().find(|x| !x.common).unwrap();
            own.from = vec![flow::format::Src::Place("R2".into(), "answer".into())];
        }),
    ];
    for (what, change) in changes {
        let s = setup(if what.ends_with("line") {
            "begun-line"
        } else {
            "begun-inputs"
        });
        let (mut e, said, item) = start(&s, scripts());
        step_to(&mut e, item, "I1");
        let (work, _, _) = state(&e, item).unwrap();
        let line = what.ends_with("line");
        // For the line, cell I1 runs on the version that puts it on the review line, so that its rounds are in a
        // session of that line; the engine then runs the version it started with again, where I1 is the engineer's.
        let now = e.flow_now;
        let v = if line {
            let v = older(&mut e, &[work], change);
            e.flow_now = v;
            v
        } else {
            0
        };
        // Cell I1 asks for tools, and is back from them with their results.
        e.advance(work).unwrap();
        e.advance(work).unwrap();
        let log = said.lock().unwrap().join("\n");
        assert_eq!(state(&e, item).unwrap().1, "I1", "{log}");
        let before = session_of(&e, work, "I1");
        let v = if line {
            e.flow_now = now;
            v
        } else {
            older(&mut e, &[work], change)
        };
        let line_of = |e: &Engine, session: i64| -> String {
            e.db.conn()
                .query_row("SELECT line FROM sessions WHERE id = ?1", [session], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(
            line_of(&e, before),
            if line { "review" } else { "engineer" },
            "{what}\n{log}"
        );
        e.advance(work).unwrap();
        let log = said.lock().unwrap().join("\n");
        let v1 = brought(&e, work);
        assert_eq!(v1.len(), 1, "{what}: {v1:?}\n{log}");
        assert_eq!(
            (v1[0].0.as_str(), &v1[0].1["back_to"], &v1[0].1["why"]),
            (
                "went_back",
                &json!("I1"),
                &json!(format!("cell I1, which the work had begun, {what}"))
            ),
            "{log}"
        );
        assert_eq!(versions_of(&e, work), (e.flow_now, Some(v)), "{log}");
        assert_eq!(works_of(&e, item).len(), 1, "{log}");
        let m = message_at(&e, work, "I1", true);
        assert!(
            m.contains("Came from cell V1 (Bring the work to the new version) by its row `went_back`"),
            "{m}"
        );
        let engineer: i64 =
            e.db.conn()
                .query_row(
                    "SELECT id FROM sessions WHERE work = ?1 AND line = 'engineer' ORDER BY id DESC LIMIT 1",
                    [work],
                    |r| r.get(0),
                )
                .unwrap();
        // The round done again runs in the session of I1's line in the version the engine runs: for the line, not the
        // review session its earlier rounds ran in; for the inputs, the same session as before.
        let after = session_of(&e, work, "I1");
        assert_eq!(
            (after, line_of(&e, after).as_str()),
            (engineer, "engineer"),
            "{what}\n{log}"
        );
        assert_eq!(after != before, line, "{what}\n{log}");
        let _ = std::fs::remove_dir_all(&s.root);
    }
}

// Item #230: the advisor's session is under way while the advisor's own tools run, as much as while it stands at its
// cell: a version that puts the advisor on another line while the work is at the tools for it does not move the work,
// whose tools' results would go back to a session of the new line that never asked for them. The same work comes to
// cell `V1` and goes back to the cell that asked the advisor, as it does when the advisor stands at its cell.
#[test]
fn an_advisor_whose_tools_run_when_its_line_changes_is_done_again() {
    let s = setup("advisor-tools");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!("{install}\n[agents.levels.m]\ndesc = \"one level up\"\ncli = \"claude\"\nmodel = \"m-up\"\n"),
    );
    git(&s.data, &["commit", "-q", "-am", "a level above"]);
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return an_ask("advisor");
            }
            (Vec::new(), Vec::new())
        }),
    );
    agents.insert(
        "A1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                let tools = json!([{ "tool": "query", "sql": "SELECT 1" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "X1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    let pos = e.work(work).unwrap().pos;
    // The advisor, which R1 asked, waits for its tools.
    assert_eq!(
        (pos.stopped.as_deref(), pos.callers.last().map(String::as_str)),
        (Some("R1"), Some("A1")),
        "{log}"
    );
    let v = older(&mut e, &[work], |g| {
        g.cells.iter_mut().find(|c| c.id == "A1").unwrap().line = Some("review".into());
    });
    e.advance(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(versions_of(&e, work), (e.flow_now, Some(v)), "{log}");
    let v1 = brought(&e, work);
    assert_eq!(v1.len(), 1, "{v1:?}\n{log}");
    assert_eq!(
        (v1[0].0.as_str(), &v1[0].1["back_to"]),
        ("went_back", &json!("R1")),
        "{log}"
    );
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230, W245: an architect's work brought to a new flow version in the middle of its batch makes the item changes
// it collected before, whether it moves where it stands or goes back through cell `V1` to the review of the batch it had
// begun, which the version puts on another line; its ask of the owner stays the owner's to answer, and is answered.
#[test]
fn an_architect_work_brought_up_mid_batch_makes_its_changes_and_keeps_its_ask() {
    for back in [false, true] {
        let s = setup(if back { "batch-back" } else { "batch-moves" });
        let mut agents = scripts();
        let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Parse the input first",
                             "reason": "Clearer." });
        agents.insert(
            "H8",
            Box::new(move |_: &Request, _: usize| {
                (
                    vec![("outcome.answer", "done")],
                    vec![
                        ("outcome.answer.reply", json!("I retitle it as an action.")),
                        (
                            "outcome.effects",
                            json!({ "item_changes": [change.clone()], "observation_changes": null }),
                        ),
                    ],
                )
            }),
        );
        // The review's first return has a verdict it does not have, and is sent back.
        agents.insert(
            "B2",
            Box::new(|_: &Request, n: usize| {
                let verdict = if n == 1 { "maybe" } else { "ok" };
                let row = json!({ "change": 1, "verdict": verdict, "why": "It reads better." });
                (Vec::new(), vec![("outcome.review", json!([row]))])
            }),
        );
        let (mut e, said) = engine(&s, agents);
        quiet(&mut e);
        an_item(&mut e);
        say(&mut e, "Make the parser's item clearer.");
        let arch = e.step(None).unwrap().work.unwrap();
        // It stands at the review of the batch: before it ran, or, to go back, once its first return was sent back.
        let at_review = |e: &mut Engine| {
            let sent: i64 =
                e.db.conn()
                    .query_row(
                        "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'B2' AND rejected IS NOT NULL",
                        [arch],
                        |r| r.get(0),
                    )
                    .unwrap();
            e.work(arch).unwrap().pos.at == "B2" && (sent > 0) == back
        };
        assert!(
            steps_until(&mut e, 40, at_review),
            "{}",
            said.lock().unwrap().join("\n")
        );
        let ask =
            e.db.write(Actor::Engine, |tx| {
                let round: i64 = tx.query_row("SELECT max(n) FROM rounds WHERE work = ?1", [arch], |r| r.get(0))?;
                tx.execute(
                    "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, '{\"to\":\"owner\"}')",
                    [arch, round],
                )?;
                Ok(tx.last_insert_rowid())
            })
            .unwrap();
        let v = older_graph(&mut e, &[arch], engine::ARCHITECT, |g| {
            let b2 = g.cells.iter_mut().find(|c| c.id == "B2").unwrap();
            if back {
                b2.line = Some("observer".into());
            } else {
                b2.rows[0].name = format!("{} before", b2.rows[0].name);
            }
        });
        assert!(
            steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
            "{}",
            said.lock().unwrap().join("\n")
        );
        let log = said.lock().unwrap().join("\n");
        assert_eq!(versions_of(&e, arch), (e.flow_now, Some(v)), "{log}");
        let v1 = brought(&e, arch);
        match back {
            true => assert_eq!(
                (v1.len(), v1[0].0.as_str(), &v1[0].1["back_to"]),
                (1, "went_back", &json!("B2")),
                "{log}"
            ),
            false => assert!(v1.is_empty(), "{v1:?}\n{log}"),
        }
        let title: String =
            e.db.conn()
                .query_row("SELECT title FROM items WHERE id = 1", [], |r| r.get(0))
                .unwrap();
        assert_eq!(title, "Parse the input first", "{log}");
        let answer: Option<String> =
            e.db.conn()
                .query_row("SELECT answer FROM owner_asks WHERE id = ?1", [ask], |r| r.get(0))
                .unwrap();
        assert_eq!(answer, None, "{log}");
        let answer = api::Answer {
            text: "The first.".into(),
            speaker: "owner".into(),
            relayed: None,
            why: None,
            reply: None,
            to: None,
            images: Vec::new(),
        };
        api::answer(&mut e.db, ask, &answer).unwrap();
        let _ = std::fs::remove_dir_all(&s.root);
    }
}

// Item #230: a work that the version the engine runs can take on at none of the cells it walked comes to cell `V1` and
// stays there, paused, the owner told why and its project's architect asked to set it straight; no new work takes it
// over. The owner's Start has the engine look again: it stays, and the architect is asked nothing more.
#[test]
fn a_work_the_version_can_take_on_nowhere_stays_at_v1_paused() {
    let s = setup("nowhere");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = a_ready_item(&mut e, "Gone");
    let v = older(&mut e, &[], |g| {
        for (id, like) in [("Z0", "R1"), ("Z1", "R2")] {
            let mut c = g.cell(like).unwrap().clone();
            c.id = id.into();
            g.cells.push(c);
        }
    });
    let work =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'Z1')",
                params![item, v],
            )?;
            let work = tx.last_insert_rowid();
            record_round(tx, work, 1, "Z0", &json!({}), "done")?;
            Ok(work)
        })
        .unwrap();
    let asked = |e: &Engine| -> Vec<i64> {
        let mut st = e
            .db
            .conn()
            .prepare(
                "SELECT id FROM architect_matters WHERE kind = 'set_straight' AND json_extract(body, '$.work') = ?1",
            )
            .unwrap();
        st.query_map([work], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    for time in 1..=2 {
        e.step(None).unwrap();
        let log = said.lock().unwrap().join("\n");
        let (_, at, paused) = state(&e, item).unwrap();
        assert_eq!(at, "V1", "{log}");
        assert_eq!(paused_by(&e, work).as_deref(), Some("engine"), "{log}");
        assert!(
            paused.as_deref().is_some_and(|p| p.starts_with(&format!(
                "it cannot go on in flow version {}: cell Z1 is gone",
                e.flow_now
            ))),
            "{paused:?}"
        );
        // Its merge does not stand, so a start over and a cancel are offered beside the Start.
        assert!(
            paused.as_deref().is_some_and(|p| p.ends_with(
                "Start it over, or cancel its item; or Start it once a flow version that has a cell it walked is \
                 installed, and it looks again"
            )),
            "{paused:?}"
        );
        let v1 = brought(&e, work);
        assert_eq!(v1.len(), time, "{v1:?}\n{log}");
        assert_eq!(
            (v1[time - 1].0.as_str(), &v1[time - 1].1["back_to"]),
            ("stuck", &Value::Null),
            "{log}"
        );
        assert_eq!(asked(&e).len(), 1, "{log}");
        let what: String =
            e.db.conn()
                .query_row(
                    "SELECT what FROM architect_matters WHERE id = ?1",
                    [asked(&e)[0]],
                    |r| r.get(0),
                )
                .unwrap();
        assert!(
            what.ends_with("it waits, paused, for the owner's Start, a start over or a cancel of its item"),
            "{what}"
        );
        assert_eq!(works_of(&e, item).len(), 1, "{log}");
        // The owner starts it again.
        e.db.write(Actor::Owner, |tx| {
            tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work])?;
            Ok(())
        })
        .unwrap();
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230, A8 and A13: an engineer's work whose merge stands and that the version the engine runs can take on at none
// of the cells it walked since its merge stays at cell `V1`, paused; neither its pause nor the matter its architect is
// given offers a start over or a cancel of its item, which its merge refuses, only the owner's Start once a version
// with a cell it walked is installed. Its architect is asked once on this version: once the architect has handled that
// matter, the owner's Start, which finds it stuck again, asks nothing more.
#[test]
fn a_merged_work_the_version_can_take_on_nowhere_is_offered_only_the_start_and_asks_once() {
    let s = setup("nowhere-merged");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = a_ready_item(&mut e, "Merged");
    let v = older(&mut e, &[], |g| {
        let mut c = g.cell("L2").unwrap().clone();
        c.id = "Z1".into();
        g.cells.push(c);
    });
    let work =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'Z1')",
                params![item, v],
            )?;
            let work = tx.last_insert_rowid();
            record_round(tx, work, 1, "M2", &json!({}), "merged")?;
            Ok(work)
        })
        .unwrap();
    let asked = |e: &Engine| -> Vec<(i64, String)> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT id, what FROM architect_matters
                 WHERE kind = 'set_straight' AND json_extract(body, '$.work') = ?1 ORDER BY id",
                )
                .unwrap();
        st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "V1", "{log}");
    let paused = paused.unwrap_or_default();
    assert!(
        paused.ends_with(". Start it once a flow version that has a cell it walked is installed, and it looks again")
            && !paused.contains("Start it over")
            && !paused.contains("cancel"),
        "{paused}"
    );
    let first = asked(&e);
    assert_eq!(first.len(), 1, "{log}");
    assert!(
        first[0].1.ends_with(
            "it waits, paused, for the owner's Start once a flow version that has a cell it walked is installed"
        ),
        "{}",
        first[0].1
    );
    // The architect handles the matter, and the owner starts the work again on the same version.
    e.db.write(Actor::Engine, |tx| {
        let arch: i64 = tx.query_row(
            "SELECT id FROM works WHERE graph = 'architect' AND ended IS NOT NULL ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE architect_matters SET handled_work = ?2, handled_round = 1 WHERE id = ?1",
            params![first[0].0, arch],
        )?;
        tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work])?;
        Ok(())
    })
    .unwrap();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).unwrap().1, "V1", "{log}");
    assert_eq!(brought(&e, work).len(), 2, "{log}");
    assert_eq!(asked(&e).len(), 1, "{:?}\n{log}", asked(&e));
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230, A8: an architect's work that the version the engine runs can take on at none of the cells it walked stays
// at cell `V1`, paused, as an engineer's does; it asks no architect to set it straight, since that architect is this
// work, and its pause alone tells the owner, saying what the owner's Start will do.
#[test]
fn an_architect_work_the_version_can_take_on_nowhere_stays_at_v1_and_asks_no_one() {
    let s = setup("nowhere-architect");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the item.");
    let arch = e.step(None).unwrap().work.unwrap();
    // On the version before, it stood at a cell the version the engine runs lacks, having walked only such a cell.
    older_graph(&mut e, &[arch], engine::ARCHITECT, |g| {
        for (id, like) in [("Z0", "Q0"), ("Z1", "Q1")] {
            let mut c = g.cell(like).unwrap().clone();
            c.id = id.into();
            g.cells.push(c);
        }
    });
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET at = 'Z1', resume_at = NULL, waiting = '[]' WHERE id = ?1",
            [arch],
        )?;
        tx.execute("UPDATE rounds SET cell = 'Z0' WHERE work = ?1", [arch])?;
        Ok(())
    })
    .unwrap();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let w = e.work(arch).unwrap();
    assert_eq!(
        (w.pos.at.as_str(), w.paused_by.as_deref()),
        ("V1", Some("engine")),
        "{log}"
    );
    let paused = w.paused.unwrap_or_default();
    assert!(
        paused.starts_with(&format!("it cannot go on in flow version {}: ", e.flow_now))
            && paused
                .ends_with("Start it once a flow version that has a cell it walked is installed, and it looks again"),
        "{paused}"
    );
    let v1 = brought(&e, arch);
    assert_eq!(
        v1.iter()
            .map(|(row, out)| (row.as_str(), &out["back_to"]))
            .collect::<Vec<_>>(),
        [("stuck", &Value::Null)],
        "{log}"
    );
    let asked: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'set_straight'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(asked, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230, A11: a work whose graph the version the engine runs has no more, as the setting of a policy that was a
// graph of its own, has no cell of that version that can run it: it ends, the owner told why, and what it held is let
// go: its project's hold of new starts for setting it straight, and the worktree the setting read in. It is neither
// paused nor put to the architect, and no work starts in its place.
#[test]
fn a_work_whose_graph_the_version_lacks_ends_and_lets_go_of_what_it_held() {
    let s = setup("graph-gone");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let tree = s.data.join("worktrees").join(engine::POLICY).join("p");
    git(
        &s.root.join("checkout"),
        &["worktree", "add", "-q", "--detach", &tree.to_string_lossy()],
    );
    let now = e.flow_now;
    let mut f = flow::Compiled::clone(&e.flow(now).unwrap());
    let mut policy = f.graph(engine::ARCHITECT).unwrap().clone();
    policy.name = engine::POLICY.into();
    f.graphs.push(policy);
    let text = f.json();
    let work =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (digest, compiled) VALUES ('with the policy graph', ?1)",
                [&text],
            )?;
            let v = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, ?1, ?2, 'P1', 'p')",
                params![v, engine::POLICY],
            )?;
            let work = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'set_straight', 1)",
                [],
            )?;
            Ok(work)
        })
        .unwrap();
    assert!(tree.exists());
    let works = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM works", [], |r| r.get(0))
            .unwrap()
    };
    let before = works(&e);
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let w = e.work(work).unwrap();
    let why = "the version has no graph policy";
    assert_eq!(
        (w.ended.as_deref(), w.paused.as_deref()),
        (Some(format!("stopped: {why}").as_str()), None),
        "{log}"
    );
    assert_eq!(notices_of(&e, work), [format!("Work {work} stopped: {why}.")], "{log}");
    let count = |sql: &str| -> i64 { e.db.conn().query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(
        count("SELECT count(*) FROM dispatch_holds WHERE reason = 'set_straight'"),
        0,
        "{log}"
    );
    assert_eq!(
        count("SELECT count(*) FROM architect_matters WHERE kind = 'set_straight'"),
        0,
        "{log}"
    );
    assert!(!tree.exists(), "{log}");
    assert_eq!(works(&e), before, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// An engine made only to read over a setup, beside the one a test runs, running the flow `compiled`, as `wf-agents
/// flow cost` makes it.
fn trying(s: &Setup, compiled: flow::Compiled) -> Engine {
    let agents = Scripted {
        scripts: scripts(),
        rounds: Mutex::new(BTreeMap::new()),
    };
    let hub = FakeHub {
        origin: s.origin.clone(),
        scratch: s.root.clone(),
        prs: s.prs.clone(),
        ci: s.ci.clone(),
        merging: s.merging.clone(),
        closing: s.closing.clone(),
        by_hand: s.by_hand.clone(),
    };
    let committed = config::committed(&s.data).unwrap();
    let (e, at) = Engine::trying(&s.data, committed, compiled, Box::new(agents), Box::new(hub)).unwrap();
    assert_eq!(at, db::VERSION);
    e
}

/// How many rows the tables a step writes hold, every change of a table of state among them.
fn rows_held(e: &Engine) -> Vec<i64> {
    let n = |q: &str| -> i64 { e.db.conn().query_row(q, [], |r| r.get(0)).unwrap() };
    vec![
        n("SELECT count(*) FROM flow_versions"),
        n("SELECT count(*) FROM works"),
        n("SELECT count(*) FROM works WHERE ended IS NOT NULL"),
        n("SELECT count(*) FROM rounds"),
        n("SELECT count(*) FROM changes"),
        n("SELECT count(*) FROM engine_notices"),
        n("SELECT count(*) FROM outbox"),
    ]
}

// Item #272: what installing a flow would do with each work that is neither ended nor done, worked out beside the
// engine on one made only to read, writes nothing and is what the engine then does: a work at a cell the version lacks
// goes back through `V1` to the cell it stopped at, doing again the round it had begun there; a paused one that walked
// no cell the version has is told with its pause, and stays stuck at `V1`, paused for that, once the owner steps it; one
// whose graph the version lacks ends; one on the version already stays. A flow the database has no version of is told
// as the next, and what a work does at `V1` is read from the row that version's `V1` takes: one whose row back pauses
// the work leaves it stuck at the cell it went back to.
#[test]
fn the_cost_of_a_flow_is_what_the_engine_then_does_with_each_work() {
    let s = setup("flow-cost");
    let (mut e, said, _, back) = at_r2_on_an_older_version(&s, |g| {
        // The older version had an owner's cell the version the engine runs lacks.
        let mut owner = g.cell("A2").unwrap().clone();
        owner.id = "A3".into();
        owner.kind = "owner".into();
        owner.title = "Owner answers".into();
        owner.step = Some("ask_owner".into());
        g.cells.push(owner);
    });
    let (v, _) = versions_of(&e, back);
    let now = e.flow_now;
    let item = a_ready_item(&mut e, "Stuck");
    let mut with_policy = flow::Compiled::clone(&e.flow(now).unwrap());
    let mut policy = with_policy.graph(engine::ARCHITECT).unwrap().clone();
    policy.name = engine::POLICY.into();
    with_policy.graphs.push(policy);
    let (begun, stuck, ends, stays, last) =
        e.db.write(Actor::Engine, |tx| {
            // Its round at R2 was sent back, and it asked the owner from there.
            let begun = next_round(tx, back)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, rejected, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, 'R2', '{}', '{\"problems\":[\"x\"]}', 'c', 1, 1)",
                [back, begun],
            )?;
            tx.execute("UPDATE works SET at = 'A3', resume_at = 'R2' WHERE id = ?1", [back])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, paused, paused_by)
                 VALUES (?1, ?2, 'engineer', 'A3', 'Held.', 'owner')",
                [item, v],
            )?;
            let stuck = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO flow_versions (digest, compiled) VALUES ('with the policy graph', ?1)",
                [&with_policy.json()],
            )?;
            let last = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, ?1, ?2, 'P1', 'p')",
                params![last, engine::POLICY],
            )?;
            let ends = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project, paused, paused_by)
                 VALUES (NULL, ?1, 'architect', 'Q0', 'p', 'Held.', 'owner')",
                [now],
            )?;
            Ok((begun, stuck, ends, tx.last_insert_rowid(), last))
        })
        .unwrap();
    let before = rows_held(&e);
    let cost = trying(&s, flow::Compiled::clone(&e.flow(now).unwrap()))
        .flow_cost()
        .unwrap();
    assert_eq!(rows_held(&e), before);
    assert_eq!((cost.version, cost.stored), (now, true));
    // The architect's work over before the test is not told.
    let told: Vec<i64> = cost.works.iter().map(|c| c.id).collect();
    assert_eq!(told, [back, stuck, ends, stays]);
    let effect = |work: i64| cost.works.iter().find(|c| c.id == work).unwrap().effect.clone();
    let gone = "cell A3 is gone";
    assert_eq!(
        effect(back),
        Ok(Effect::GoesBack {
            through: "V1".into(),
            to: "R2".into(),
            why: gone.into(),
            from: Some(engine::RoundId::new(back, begun)),
            redone: 1,
        })
    );
    let nowhere = format!("{gone}, and the version has none of the cells the work walked");
    let pause = format!(
        "it cannot go on in flow version {now}: {nowhere}. Start it over, or cancel its item; or Start it once a flow \
         version that has a cell it walked is installed, and it looks again"
    );
    assert_eq!(
        effect(stuck),
        Ok(Effect::Stuck {
            at: "V1".into(),
            why: nowhere.clone(),
            back_to: None,
            pause: pause.clone(),
        })
    );
    let paused = cost.works[1].work.as_ref().unwrap();
    assert_eq!(
        (paused.paused.as_deref(), paused.paused_by.as_deref()),
        (Some("Held."), Some("owner"))
    );
    let no_graph = "the version has no graph policy";
    assert_eq!(
        effect(ends),
        Ok(Effect::Ends {
            why: no_graph.into(),
            taken_over: false,
        })
    );
    assert_eq!(effect(stays), Ok(Effect::Stays));
    // A flow the database has no version of would be stored as the next one, and a work on the version the engine runs
    // is brought to it.
    let mut other = flow::Compiled::clone(&e.flow(now).unwrap());
    renamed(
        other.graphs.iter_mut().find(|g| g.name == engine::ENGINEER).unwrap(),
        "D4",
    );
    let next = trying(&s, other.clone()).flow_cost().unwrap();
    assert_eq!((next.version, next.stored), (last + 1, false));
    assert_eq!(
        next.works.iter().find(|c| c.id == stays).unwrap().effect,
        Ok(Effect::Moves)
    );
    // That version's V1 pauses the work it sends back.
    let v1 = other
        .graphs
        .iter_mut()
        .find(|g| g.name == engine::ENGINEER)
        .and_then(|g| g.cells.iter_mut().find(|c| c.id == "V1"))
        .unwrap();
    v1.rows.iter_mut().find(|r| r.name == "went_back").unwrap().pauses = true;
    let next = trying(&s, other).flow_cost().unwrap();
    assert_eq!(
        next.works.iter().find(|c| c.id == back).unwrap().effect,
        Ok(Effect::Stuck {
            at: "R2".into(),
            why: gone.into(),
            back_to: Some("R2".into()),
            pause: format!(
                "it cannot go on in flow version {}: {gone}. Start takes it back to cell R2 all the same",
                last + 1
            ),
        })
    );
    assert_eq!(rows_held(&e), before);
    // The engine does what was told: the work goes back to R2 through V1, doing again the round it had begun there,
    // and the work of a graph the version lacks ends.
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let v1 = brought(&e, back);
    assert_eq!(
        v1.iter()
            .map(|(row, out)| (row.as_str(), &out["back_to"], &out["why"], &out["redoes_from"]))
            .collect::<Vec<_>>(),
        [("went_back", &json!("R2"), &json!(gone), &json!(begun))],
        "{log}"
    );
    assert_eq!(
        e.work(ends).unwrap().ended,
        Some(format!("stopped: {no_graph}")),
        "{log}"
    );
    // The paused work stays where it was until the owner steps it, and then stays stuck at V1.
    assert_eq!(versions_of(&e, stuck), (v, None), "{log}");
    e.step_work(stuck).unwrap();
    let log = said.lock().unwrap().join("\n");
    let v1 = brought(&e, stuck);
    assert_eq!(
        v1.iter()
            .map(|(row, out)| (row.as_str(), &out["back_to"], &out["why"]))
            .collect::<Vec<_>>(),
        [("stuck", &Value::Null, &json!(nowhere))],
        "{log}"
    );
    assert_eq!(e.work(stuck).unwrap().paused, Some(pause), "{log}");
    assert_eq!(versions_of(&e, stays), (now, None), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230: when the flow moves to a new version, a work on the old one is not brought up when the engine starts: the
// owner still answers its asks. Once it goes on, a work whose cell the version changed only in its rows moves where it
// stands: the same work, with its worktree, branch and pull request, its asks and its question to the architect as they
// were, through no cell of its own. An item whose works ended can be queued again.
#[test]
fn a_new_flow_version_moves_a_work_whose_cell_changed_its_rows_when_it_next_goes_on() {
    let s = setup("version");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..80 {
        e.tick().unwrap();
        if works_of(&e, item)[0].4.is_some() {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (old, at, worktree, branch, pr, _) = works_of(&e, item)[0].clone();
    assert!(pr.is_some() && worktree.is_some() && branch.is_some(), "{log}");
    // The work ran on a flow version the engine no longer has built in, where the cell it stands at was another.
    older(&mut e, &[old], |g| renamed(g, &at));
    // It put two asks to the owner, and a question to the architect.
    let asks: Vec<i64> =
        e.db.write(Actor::Engine, |tx| {
            let round: i64 = tx.query_row("SELECT max(n) FROM rounds WHERE work = ?1", [old], |r| r.get(0))?;
            let mut asks = Vec::new();
            for _ in 0..2 {
                tx.execute(
                    "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, '{\"to\": \"owner\"}')",
                    [old, round],
                )?;
                asks.push(tx.last_insert_rowid());
            }
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: "engineer_question".into(),
                    detail: Some("other_question".into()),
                    what: format!("Work {old} on item {item} asks which name to keep"),
                    body: json!({ "to": "architect", "work": old }),
                    item: Some(item),
                    raised: Some((old, round)),
                },
            )?;
            Ok(asks)
        })
        .unwrap();
    drop(e);
    let (mut e, said) = engine(&s, scripts());
    let works = works_of(&e, item);
    assert_eq!(works.len(), 1, "{works:?}");
    assert_eq!(works[0].5, None);
    let answer = api::Answer {
        text: "Keep the old name.".into(),
        speaker: "owner".into(),
        relayed: None,
        why: None,
        reply: None,
        to: None,
        images: Vec::new(),
    };
    api::answer(&mut e.db, asks[0], &answer).unwrap();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 1, "{works:?}\n{log}");
    assert_eq!(works[0].5, None, "{log}");
    let (on, from) = versions_of(&e, old);
    assert!(on == e.flow_now && from.is_some(), "{log}");
    assert!(brought(&e, old).is_empty(), "{log}");
    assert!(
        log.contains(&format!("work {old} moved from flow version")) && log.contains(&format!(", at cell {at}")),
        "{log}"
    );
    // Its worktree, branch and pull request are its own still; the ask the owner answered keeps the owner's answer, the
    // other is still the owner's to answer, and its question waits in the architect's queue.
    assert_eq!(
        (&works[0].2, &works[0].3, works[0].4),
        (&worktree, &branch, pr),
        "{log}"
    );
    let by = |e: &Engine, ask: i64| -> Option<String> {
        e.db.conn()
            .query_row(
                "SELECT json_extract(answer, '$.by') FROM owner_asks WHERE id = ?1",
                [ask],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!((by(&e, asks[0]), by(&e, asks[1])), (Some("owner".into()), None));
    api::answer(&mut e.db, asks[1], &answer).unwrap();
    let withdrawn: Option<String> =
        e.db.conn()
            .query_row("SELECT withdrawn FROM architect_matters WHERE work = ?1", [old], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(withdrawn, None, "{log}");
    // Once its works ended, the item can be queued again.
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET ended = 'stopped by hand' WHERE id = ?1", [old])?;
        tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [item])?;
        Ok(())
    })
    .unwrap();
    let ready: i64 =
        e.db.conn()
            .query_row("SELECT item FROM ready_items", [], |r| r.get(0))
            .unwrap();
    assert_eq!(ready, item);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #230: a work standing at the owner's cell of an older flow version, waiting for its ask of the owner, cannot stay
// where it stands in the version the engine runs, which has no such cell. When it next goes on it is the same work: it
// comes to cell `V1` and goes back from there to the cell it stopped at, which does it again; the ask it had out needs no
// answer, and the owner is told where the work went back and why. No new work starts.
#[test]
fn a_work_at_a_cell_the_version_lacks_goes_back_through_v1_to_where_it_stopped() {
    let s = setup("owner-cell-gone");
    let (mut e, said, item, work) = at_r2_on_an_older_version(&s, |g| {
        // The older version still had the owner's cell.
        let mut owner = g.cell("A2").unwrap().clone();
        owner.id = "A3".into();
        owner.kind = "owner".into();
        owner.title = "Owner answers".into();
        owner.step = Some("ask_owner".into());
        g.cells.push(owner);
    });
    let ask =
        e.db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET at = 'A3', resume_at = 'R2' WHERE id = ?1", [work])?;
            let round = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, 'A3', '{}', 'c', 1)",
                [work, round],
            )?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, '{\"to\":\"owner\"}')",
                [work, round],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 1, "{works:?}\n{log}");
    assert_eq!(works[0].5, None, "{log}");
    let v1 = brought(&e, work);
    assert_eq!(v1.len(), 1, "{v1:?}\n{log}");
    assert_eq!(
        (v1[0].0.as_str(), &v1[0].1["back_to"], &v1[0].1["why"]),
        ("went_back", &json!("R2"), &json!("cell A3 is gone")),
        "{log}"
    );
    // The work went on from cell R2 in the same pass.
    assert!(!transitions(&e, item, "R2").is_empty(), "{log}");
    let (by, text): (Option<String>, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT json_extract(answer, '$.by'), json_extract(answer, '$.text') FROM owner_asks WHERE id = ?1",
                [ask],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(by.as_deref(), Some("engine"), "{log}");
    assert!(
        text.as_deref()
            .is_some_and(|t| t.starts_with("No answer is needed: the work goes back to cell R2 of flow version")),
        "{text:?}"
    );
    let told = notices_of(&e, work);
    assert!(
        told.iter()
            .any(|t| t.contains("goes back to cell R2 ") && t.ends_with(": cell A3 is gone.")),
        "{told:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The loop asks for a work's graph many times a pass: a copy of it each time, for every work not ended, kept a core of
// the machine busy with nothing to do.
#[test]
fn a_works_graph_is_made_once_and_shared() {
    let s = setup("graph-once");
    let (mut e, _, item) = start(&s, scripts());
    e.tick().unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    let w = e.work(work).unwrap();
    let first = e.graph_of(&w).unwrap();
    let again = e.graph_of(&w).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    let _ = std::fs::remove_dir_all(&s.root);
}
