//! The line a work stands on while a question it asked waits, and the count of words waiting for a line: what the
//! engine's value `held` reads to decide whether a question to an architect is withdrawn for words. Neither needs a
//! running engine, so these tests walk the compiled flow and a database in memory.

use db::rusqlite::params;
use db::{Actor, Db};
use engine::blockers::{standing_line, waiting_words};
use engine::{ENGINEER, Work};
use flow::{CompiledCell, CompiledGraph, CompiledRow, Context, Position};

fn compiled() -> flow::Compiled {
    let relations = db::relations(db::SCHEMA).unwrap();
    let tables = db::index_of(db::SCHEMA).unwrap();
    flow::compile(flow::FLOW, &Context { relations, tables }).unwrap()
}

fn engineers(f: &flow::Compiled) -> &CompiledGraph {
    f.graph(ENGINEER).unwrap()
}

fn work_at(pos: Position) -> Work {
    Work {
        id: 1,
        item: None,
        flow_version: 1,
        graph: ENGINEER.to_string(),
        pos,
        machine: None,
        worktree: None,
        branch: None,
        pr: None,
        pushed_head: None,
        paused: None,
        ended: None,
        paused_by: None,
        project: None,
    }
}

fn stack(at: &str, callers: &[&str]) -> Position {
    Position {
        at: at.to_string(),
        callers: callers.iter().map(|c| (*c).to_string()).collect(),
        ..Position::default()
    }
}

/// A cell's row that raises a blocker for the cell the row leads to.
fn raising<'a>(c: &'a CompiledCell, to: &str) -> Option<&'a CompiledRow> {
    c.rows
        .iter()
        .find(|r| r.to == to && r.actions.iter().any(|a| a.name() == "raise_blocker"))
}

fn row<'a>(g: &'a CompiledGraph, cell: &str, name: &str) -> &'a CompiledRow {
    g.cell(cell)
        .unwrap()
        .rows
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("{cell} has no row {name}"))
}

// The line the work stands on is the line of the nearest agent cell on its stack, counting from the cell it is at
// back through the cells that wait for an answer, the innermost first; an engine step, which has no line, and a cell
// of an older flow version that the graph no longer has are passed over; with no agent on the stack, the line of the
// graph's agents, the engineer's. The lines are those of the flow: cell `T1` is on the adversarial line, `T6` and the
// merge's cells on the engineer's.
fn the_nearest_agent_cell_on_the_stack_gives_the_line(g: &CompiledGraph) {
    let line = |at: &str, callers: &[&str]| standing_line(g, &work_at(stack(at, callers)));
    // The question's cell, with the asker below it.
    assert_eq!(line("A2", &["T1"]), "adversarial");
    assert_eq!(line("A2", &["T6"]), "engineer");
    // An engine step that asks for an agent's tools: the agent it runs them for.
    assert_eq!(line("A2", &["T1", "X1"]), "adversarial");
    assert_eq!(line("A2", &["T6", "X1"]), "engineer");
    // The nearest agent is the innermost, not the outermost.
    assert_eq!(line("A2", &["T6", "T1"]), "adversarial");
    assert_eq!(line("A2", &["T1", "T6"]), "engineer");
    // The cell the work is at counts first.
    assert_eq!(line("T1", &["T6"]), "adversarial");
    assert_eq!(line("X1", &["T1"]), "adversarial");
    // No agent on the stack: the engineer's line, whether the stack is empty or holds only engine steps.
    assert_eq!(line("A2", &[]), "engineer");
    assert_eq!(line("A2", &["X1"]), "engineer");
    assert_eq!(line("X1", &[]), "engineer");
    // A cell the graph does not have is passed over.
    assert_eq!(line("A2", &["T1", "Z9"]), "adversarial");
    assert_eq!(line("A2", &["Z9"]), "engineer");
}

// Whoever asks, the line the engine takes the work to stand on is the line of the cell the answer reaches: the flow
// takes the work back to the asker by its row `@asker`, or to the cell that receives answers when that is on the
// asker's line. The two are compared over every agent cell of the engineer's graph that can ask, directly and through
// the tools cell, with the work taken to the question's cell and back by the flow's own `step`, and both are compared
// with the line the flow gives the asker. Stacks written by hand, with the nearest agent cell the line, come first, on
// the one compiled flow, which is most of the time the test takes.
#[test]
fn the_line_a_work_stands_on_at_its_question_is_the_line_its_answer_goes_back_to() {
    let f = compiled();
    let g = engineers(&f);
    the_nearest_agent_cell_on_the_stack_gives_the_line(g);
    let answered = row(g, "A2", "answered");
    let mut asked_by = Vec::new();
    for c in g.cells.iter().filter(|c| c.kind == "agent") {
        let own = c.line.clone().unwrap_or_else(|| ENGINEER.to_string());
        let start = Position {
            at: c.id.clone(),
            ..Position::default()
        };
        // The agent asks itself, or asks for tools and the tools cell asks for it.
        let mut asking = Vec::new();
        if let Some(ask) = raising(c, "A2") {
            asking.push(g.step(&start, ask).unwrap());
        }
        if let Some(tools) = c.rows.iter().find(|r| r.to == "X1" && r.name == "tools") {
            let at_tools = g.step(&start, tools).unwrap();
            if let Some(failed) = raising(g.cell("X1").unwrap(), "A2") {
                asking.push(g.step(&at_tools, failed).unwrap());
            }
        }
        for at_question in asking {
            assert_eq!(at_question.at, "A2", "{}", c.id);
            let stands = standing_line(g, &work_at(at_question.clone()));
            assert_eq!(stands, own, "asked by {}: {at_question:?}", c.id);
            let back = g.step(&at_question, answered).unwrap();
            let lands = g.cell(&back.at).unwrap();
            let there = if lands.kind == "agent" {
                lands.line.clone().unwrap_or_else(|| ENGINEER.to_string())
            } else {
                // An engine step the answer goes back to runs for the agent below it.
                standing_line(g, &work_at(back.clone()))
            };
            assert_eq!(stands, there, "asked by {}, answered to {}", c.id, lands.id);
            asked_by.push(c.id.clone());
        }
    }
    for expected in ["T1", "T6"] {
        let found = asked_by.iter().any(|a| a == expected);
        assert!(found, "{expected} not among {asked_by:?}");
    }
    // Adversarial testing asks through its tools twice over: its own ask row and the tools cell.
    assert!(asked_by.iter().filter(|a| *a == "T1").count() >= 2, "{asked_by:?}");
}

/// A database with one work, whose words the tests write.
fn db_with_work() -> Db {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        for _ in 0..2 {
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'R1')",
                [],
            )?;
        }
        Ok(())
    })
    .unwrap();
    db
}

fn say(db: &mut Db, work: i64, line: &str, informs: i64, cancelled: bool, delivered: bool) {
    db.write(Actor::Engine, |tx| {
        let item = if cancelled {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level, cancelled)
                 VALUES ('p', 'Parse it', 'No parser.', 'Needed.', 'change', 'l', 'Not needed.')",
                [],
            )?;
            Some(tx.last_insert_rowid())
        } else {
            None
        };
        if delivered {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at)
                 VALUES (?1, 1, 'R1', '{}', 'c', 1) ON CONFLICT DO NOTHING",
                [work],
            )?;
        }
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text, informs, cancelled_item, delivered_round)
             VALUES (?1, ?2, 'architect', 'Words.', ?3, ?4, ?5)",
            params![work, line, informs, item, delivered.then_some(1)],
        )?;
        Ok(())
    })
    .unwrap();
}

/// How many words wait for the engineer's line of a work: all of them, and the notices of a cancelled item among them.
fn both(db: &Db, work: i64) -> (i64, i64) {
    let count = |only_cancelled| waiting_words(db.conn(), work, "engineer", only_cancelled).unwrap();
    (count(false), count(true))
}

fn words_for(db: &Db, work: i64, line: &str) -> i64 {
    waiting_words(db.conn(), work, line, false).unwrap()
}

// Words waiting for a line are the words said to that line of that work, not yet delivered, that need an answer; the
// notices that an item was cancelled are a part of them. Words for another line, for another work, already delivered,
// or telling and asking nothing (`informs`) turn the work no way, so none is counted, whichever the line asked about.
// The meaning of `informs`, `delivered_round` and `cancelled_item` is as db/schema.sql defines them.
#[test]
fn words_waiting_for_a_line_are_those_not_delivered_that_need_an_answer() {
    let mut db = db_with_work();
    assert_eq!(both(&db, 1), (0, 0));
    // None of these is waiting for the engineer's line of work 1.
    say(&mut db, 1, "adversarial", 0, false, false);
    say(&mut db, 1, "engineer", 1, false, false);
    say(&mut db, 1, "engineer", 1, true, false);
    say(&mut db, 1, "engineer", 0, false, true);
    say(&mut db, 1, "engineer", 0, true, true);
    say(&mut db, 2, "engineer", 0, false, false);
    say(&mut db, 2, "engineer", 0, true, false);
    assert_eq!(both(&db, 1), (0, 0));
    // A word for the engineer's line: counted by every word, not by the notices.
    say(&mut db, 1, "engineer", 0, false, false);
    assert_eq!(both(&db, 1), (1, 0));
    // A notice: counted by both.
    say(&mut db, 1, "engineer", 0, true, false);
    assert_eq!(both(&db, 1), (2, 1));
    // Each line has its own words, and work 2 its own.
    assert_eq!(words_for(&db, 1, "adversarial"), 1);
    assert_eq!(words_for(&db, 1, "review"), 0);
    assert_eq!(both(&db, 2), (2, 1));
}
