//! Returns sent back that the tests of the report and the pause (`sent_back_looks`) leave out: a return sent back by one
//! of the engine's checks and not by its schema, a lone report the architect takes while the work goes on, and
//! returns sent back that a return taken between them keeps from being three in a row. The tests run the flow as
//! built.

use super::sent_back_looks::*;
use super::*;

/// The scripts as `scripts` makes them, but for the cells named: at the call numbers given, each hands in the default
/// return with the fields of its `Wrong` written over it, so that it is sent back.
fn sent_back_at(wrong: Vec<(&'static str, Vec<(usize, Wrong)>)>) -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    for (cell, calls) in wrong {
        let then = agents.remove(cell).unwrap();
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                let (picks, mut sets) = then(r, n);
                if let Some((_, w)) = calls.iter().find(|(call, _)| *call == n) {
                    sets.extend(w.iter().cloned());
                }
                (picks, sets)
            }),
        );
    }
    agents
}

/// The rows a work took at one cell, oldest first.
fn rows_at(e: &Engine, work: i64, at: &str) -> Vec<Option<String>> {
    rows_taken(e, work)
        .into_iter()
        .filter(|(cell, _)| cell == at)
        .map(|(_, row)| row)
        .collect()
}

// A return that fits its schema and is sent back by one of the engine's checks has a report as one sent back by its
// schema has: the report holds the problems as the column `rounds.rejected` keeps them, under the name of the check that
// found each, and says them in the same lines. The reference is the column itself, read apart from the report. The
// work goes on, as it does for any return sent back.
#[test]
fn a_return_sent_back_by_a_check_is_reported_by_the_name_of_that_check() {
    let s = setup("look-check");
    // The plan says a condition is checked once the program is installed, which no project but wf-agents' own can say.
    let installed = json!([
        { "by": "test", "id": "A1", "condition": "The feature is there." },
        {
            "by": "in_effect", "id": "A2", "condition": "A real run gives seven.", "when": "installed",
            "tool": "query", "command": "SELECT 7 AS seven", "holds": "It gives 7.", "enough": "none",
        },
    ]);
    let wrong: Wrong = vec![("outcome.can_start.acceptance", installed)];
    let (mut e, said, item) = start(&s, sent_back_at(vec![("R1", vec![(1, wrong)])]));
    let work = e.step(None).unwrap().work.unwrap();
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let on = turns_until(&mut e, &held, |e| {
        rows_at(e, work, "R1")
            .iter()
            .any(|r| r.as_deref().is_some_and(|r| r != "sent_back"))
    });
    let log = said.lock().unwrap().join("\n");
    assert!(on, "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    let stored: String =
        e.db.conn()
            .query_row("SELECT rejected FROM rounds WHERE rejected IS NOT NULL", [], |r| {
                r.get(0)
            })
            .unwrap();
    let column: Value = serde_json::from_str(&stored).unwrap();
    let lines = db::sent_back_lines(&column);
    assert!(!lines.is_empty(), "{stored}");
    let all = looks(&e);
    assert_eq!(all.len(), 1, "{log}");
    let body = &all[0].0;
    // The problems are the column's own, and the check of one of them is the one the flow names.
    assert_eq!(body["problems"], column["problems"], "{body}");
    let by_check: Vec<&str> = body["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["check"].as_str())
        .collect();
    assert!(by_check.contains(&"in_effect_known"), "{body}");
    assert_eq!(body["reason"].as_str().unwrap(), lines.join("\n"), "{body}");
    let reason = body["reason"].as_str().unwrap();
    assert!(reason.contains("in_effect_known ("), "{reason}");
    assert!(!reason.contains('{') && !reason.contains("problems"), "{reason}");
    // The report names the check that found the problem.
    let what: String =
        e.db.conn()
            .query_row(
                "SELECT what FROM architect_matters WHERE detail = 'sent_back'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert!(what.contains("in_effect_known"), "{what}");
    assert_eq!(
        number(
            &e,
            &format!("SELECT item FROM architect_matters WHERE detail = 'sent_back' AND item = {item}")
        ),
        item,
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A lone return sent back, the architect free to run: its work takes the report, and the work goes on as if there were
// none, never paused and never lifted by anyone. What the architect says of it is told to no running work: the work's
// interjections stay empty, since the report has no round the work waits at.
#[test]
fn the_architect_takes_a_lone_report_while_the_work_goes_on_and_tells_the_work_nothing() {
    let s = setup("look-lone");
    let (mut e, said, item) = start(&s, returns_sent_back(vec![both_wrong()]));
    let work = e.step(None).unwrap().work.unwrap();
    let handled = turns_until(&mut e, &Go::default(), |e| {
        looks(e).first().is_some_and(|l| l.1) && state(e, item).is_some_and(|w| w.1 != "R1")
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(handled, "{log}");
    assert!(pauses(&e, work).is_empty(), "{log}");
    assert!(lifted_by(&e, work).is_empty(), "{log}");
    assert_eq!(looks(&e).len(), 1, "{log}");
    assert_eq!(
        number(
            &e,
            "SELECT count(*) FROM work_handled WHERE kind = 'report' AND detail = 'sent_back'"
        ),
        1,
        "{log}"
    );
    assert_eq!(
        number(&e, &format!("SELECT count(*) FROM line_sayings WHERE work = {work}")),
        0,
        "{log}"
    );
    // The cell ran twice: the return sent back, and the one taken.
    let rows = rows_at(&e, work, "R1");
    assert_eq!(rows.first(), Some(&Some("sent_back".to_string())), "{rows:?}\n{log}");
    assert_ne!(rows[1].as_deref(), Some("sent_back"), "{rows:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Returns sent back are three in a row only when none was taken between them, whatever the cell: two at R1, the third
// taken, and two at I1 after that tool round are four sent back and no three in a row, so the work is never paused,
// and each of the four has its report, at its own cell. The reference is the flow's own words for the pause: three
// returns in a row, with no return of an agent or a tool taken in between.
#[test]
fn returns_sent_back_with_a_return_taken_between_them_are_not_three_in_a_row() {
    let s = setup("look-split");
    let agents = sent_back_at(vec![
        ("R1", vec![(1, both_wrong()), (2, outcome_wrong())]),
        // The first call of I1 hands in the tool round that pushes the branch; the next two are sent back.
        ("I1", vec![(2, outcome_wrong()), (3, progress_wrong())]),
    ]);
    let (mut e, said, _item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let done = turns_until(&mut e, &held, |e| {
        let at_i1 = rows_at(e, work, "I1");
        at_i1.len() >= 4 && at_i1.last().is_some_and(|r| r.as_deref() != Some("sent_back"))
            || paused_by(e, work).is_some()
    });
    let log = said.lock().unwrap().join("\n");
    assert!(done, "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    assert!(pauses(&e, work).is_empty(), "{log}");
    assert_eq!(counted(&e, work, "sent_back"), 4, "{log}");
    let cells: Vec<String> = looks(&e)
        .iter()
        .map(|l| l.0["cell"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(cells, ["R1", "R1", "I1", "I1"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
