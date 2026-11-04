//! What only real runs can show, checked once a work's change took effect: a merged work whose acceptance holds a
//! condition written `in_effect` waits, holding no slot, and a round in a new session checks it with a real run.

use super::declared_checks::the_engine_s_own;
use super::*;

/// What the research's plan writes, and the answer of the work's first implementation says of it. The query is a real
/// one, which the engine runs against its records.
const SEVEN: &str = "SELECT 7 AS seven";

/// A condition written in_effect, numbered A2, that the query [`SEVEN`] checks.
fn condition(when: &str) -> Value {
    json!({
        "by": "in_effect", "id": "A2", "condition": "A real run gives seven.", "when": when, "tool": "query",
        "command": SEVEN, "holds": "It gives 7.", "enough": "none",
    })
}

/// The agents as `scripts` makes them, but for the first work: its plan's acceptance holds `written` beside the default
/// condition, and its implementation says it is checked after the merge. Another work's plan and implementation are the
/// default ones, by the rounds the first ran: one of the research and two of the implementation.
fn verifying(written: Value) -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    let research = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = research(r, n);
            if n == 1 {
                let both = json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }, written.clone()]);
                set.push(("outcome.can_start.acceptance", both));
            }
            (pick, set)
        }),
    );
    let implementing = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = implementing(r, n);
            if n <= 2 {
                let rows = json!([
                    { "how": "tested", "id": "A1", "tests": ["feature"] },
                    { "how": "in_effect", "id": "A2" },
                ]);
                set.push(("outcome.conditions", rows));
            }
            (pick, set)
        }),
    );
    agents
}

/// What a script makes a round hand in: the branches it picks and the values it sets.
type Return = (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>);

/// The rows the looks took by their own answers, in order: not those that only asked for a tool.
fn looks(e: &Engine, item: i64) -> Vec<String> {
    transitions(e, item, "W2")
        .into_iter()
        .filter(|t| t != "tools")
        .collect()
}

/// A look that asks the engine to run a query.
fn asks(sql: &str) -> Return {
    (
        vec![("outcome", "tools")],
        vec![("outcome.tools", json!([{ "tool": "query", "sql": sql }]))],
    )
}

/// A look's own answer: the verdicts.
fn answers(verdicts: Value) -> Return {
    (Vec::new(), vec![("outcome.verdicts", verdicts)])
}

/// A look that finds the evidence not there yet, and says to look again in a minute.
fn waits_a_minute() -> Return {
    (
        Vec::new(),
        vec![
            (
                "outcome.verdicts",
                json!([{ "verdict": "not_yet", "id": "A2", "missing": "No run yet." }]),
            ),
            ("outcome.again_after_minutes", json!(1)),
        ],
    )
}

/// Main's commit on the fake GitHub, which holds every merge so far.
fn main_head(s: &Setup) -> String {
    git(&s.origin, &["rev-parse", "main"])
}

/// A second item in the queue, with no condition of the kind.
fn another_item(e: &mut Engine) -> i64 {
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level)
             VALUES ('p', 'Another', 'It is missing.', 'People need it.', 'change', 'l')",
            [],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)",
            params![id],
        )?;
        Ok(id)
    })
    .unwrap()
}

/// How many rounds a work has.
fn rounds_of(e: &Engine, work: i64) -> i64 {
    e.db.conn()
        .query_row("SELECT count(*) FROM rounds WHERE work = ?1", [work], |r| r.get(0))
        .unwrap()
}

/// How many times the work took the rows that count the looks that found the evidence not there yet.
fn looks_counted(e: &Engine, work: i64) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT coalesce(sum(times), 0) FROM work_counters WHERE work = ?1 AND counter = 'verification_looks'",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

/// The time the work's last look said to wait has passed: the round ended two minutes earlier than it did.
fn the_wait_passes(e: &mut Engine, work: i64) {
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET ended_at = ended_at - 120000 WHERE work = ?1 AND cell = 'W2' AND transition = 'again'",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
}

/// The work merged `hours` ago.
fn merged_hours_ago(e: &mut Engine, work: i64, hours: i64) {
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET ended_at = ?2 WHERE work = ?1 AND cell = 'M2' AND transition = 'merged'",
            params![work, db::now() - hours * 3_600_000],
        )?;
        Ok(())
    })
    .unwrap();
}

/// The report of what became of the item's checks, once it is queued: its number, its text, whether it is urgent and its
/// body.
fn report_of(e: &Engine, item: i64) -> Option<(i64, String, Value)> {
    e.db.conn()
        .query_row(
            "SELECT id, what, body FROM architect_matters WHERE item = ?1 AND kind = 'report' AND detail = 'verification'",
            [item],
            |r| {
                let body: String = r.get(2)?;
                Ok((r.get(0)?, r.get(1)?, serde_json::from_str(&body).unwrap()))
            },
        )
        .ok()
}

/// What the view of the checks gives for the item, oldest first: the condition, its verdict and its round.
fn verifications(e: &Engine, item: i64) -> Vec<(String, String, i64)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT id, verdict, round FROM item_verifications WHERE item = ?1 ORDER BY round, id")
            .unwrap();
    st.query_map([item], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

// In the project the engine is built from, a merged work whose condition needs the installed program waits for it,
// recording its wait once and holding no slot, so that another work takes the one slot there is and merges. Once the
// installed build holds the merge, a round in a new session runs the query and finds the evidence not there yet, which
// counts one look and sends the work back to wait; while the time it said has not passed the work writes nothing, and
// then a round in another new session runs the query again and finds it holds. The work ends with no report, and the view
// gives each look's verdict and round.
#[test]
fn a_merged_work_waits_for_the_installed_program_holding_no_slot_and_checks_with_a_real_run() {
    let s = setup("in-effect-installed");
    the_engine_s_own(&s);
    let mut agents = verifying(condition("installed"));
    agents.insert(
        "W2",
        Box::new(|r: &Request, n: usize| match n {
            // Each look's first round, in a new session, is given how the change took effect.
            1 | 3 => {
                assert!(r.prompt.contains("merge_commit"), "{}", r.prompt);
                asks(SEVEN)
            }
            2 => waits_a_minute(),
            _ => answers(json!([{ "verdict": "holds", "id": "A2", "run": SEVEN, "seen": "7" }])),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    e.built_from = main_head(&s);
    step_to(&mut e, item, "W1");
    let (work, _, _) = state(&e, item).unwrap();

    // It waits by the one row, recorded once, and holds no slot.
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "W1"), ["not_carried"]);
    let w = e.work(work).unwrap();
    assert!(e.waiting(&w).unwrap());
    let rounds = rounds_of(&e, work);
    e.step(None).unwrap();
    assert_eq!(rounds_of(&e, work), rounds);

    // The other work takes the one slot, and merges.
    let other = another_item(&mut e);
    step_to(&mut e, other, "E1");
    assert_eq!(rounds_of(&e, work), rounds);
    assert_eq!(state(&e, item).unwrap().1, "W1");

    // The build installed now holds the merge, and the first look finds the evidence not there yet.
    e.built_from = main_head(&s);
    step_until(&mut e, item, &said, "the first look", |e| !looks(e, item).is_empty());
    assert_eq!(looks(&e, item).last().unwrap(), "again");
    assert_eq!(looks_counted(&e, work), 1);
    assert_eq!(state(&e, item).unwrap().1, "W1");

    // Waiting as long as it said, it writes nothing after the row that says so.
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "W1").last().unwrap(), "not_due");
    let rounds = rounds_of(&e, work);
    e.step(None).unwrap();
    assert_eq!(rounds_of(&e, work), rounds);

    the_wait_passes(&mut e, work);
    step_to(&mut e, item, "E1");
    assert_eq!(looks(&e, item).last().unwrap(), "verified");
    assert_eq!(looks_counted(&e, work), 1);
    assert!(report_of(&e, item).is_none());
    let seen = verifications(&e, item);
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!((seen[0].0.as_str(), seen[0].1.as_str()), ("A2", "not_yet"));
    assert_eq!((seen[1].0.as_str(), seen[1].1.as_str()), ("A2", "holds"));
    let latest: String =
        e.db.conn()
            .query_row(
                "SELECT cell FROM rounds WHERE work = ?1 AND n = ?2",
                params![work, seen[1].2],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(latest, "W2");

    // Each look began a session of its own, and not the one the work learned in.
    let session_at = |cell: &str, row: &str| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT session FROM rounds WHERE work = ?1 AND cell = ?2 AND transition = ?3",
                params![work, cell, row],
                |r| r.get(0),
            )
            .unwrap()
    };
    let (learned, first, second) = (
        session_at("L2", "verify"),
        session_at("W2", "again"),
        session_at("W2", "verified"),
    );
    assert!(learned != first && first != second && learned != second);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work waiting for its change to take effect keeps no session warm: at the time the session it learned in would be
// pinged, had it waited anywhere else, none is due.
#[test]
fn a_work_waiting_for_its_change_to_take_effect_keeps_no_session_warm() {
    let s = setup_priced("in-effect-warm");
    the_engine_s_own(&s);
    let mut agents = verifying(condition("installed"));
    agents.insert("L2", adding("L2", |_| vec![("@usage", used(1.5, 9_500))]));
    let (mut e, _, item) = start(&s, agents);
    e.built_from = main_head(&s);
    step_to(&mut e, item, "W1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "W1"), ["not_carried"]);
    let from = last_call(&e, work);
    assert_eq!(warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Runs a work whose look finds its condition does not hold, doing harm now or not, to the row that reports it: gives the
/// engine, the item and the work.
fn failing(name: &str, harm_now: bool) -> (Setup, Engine, i64, i64) {
    let s = setup(name);
    let mut agents = verifying(condition("merged"));
    agents.insert(
        "W2",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                return asks(SEVEN);
            }
            let (pick, mut set) = answers(json!([{
                "verdict": "fails", "id": "A2", "run": SEVEN, "seen": "7", "why": "It should give 8.", "harm_now": harm_now,
            }]));
            set.push(("tell_owner", json!(["The check of A2 found that it does not hold."])));
            (pick, set)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the look that fails", |e| {
        !looks(e, item).is_empty()
    });
    let (work, _, _) = state(&e, item).unwrap();
    (s, e, item, work)
}

// A condition that does not hold goes to the project's architect as a report that names the item, the condition and the
// run it rests on, the owner is told, and the work ends. It is urgent only when it does harm now.
#[test]
fn a_condition_that_does_not_hold_is_reported_urgent_only_when_it_does_harm_now() {
    for (name, harm_now, urgent) in [("in-effect-harm", true, 1), ("in-effect-no-harm", false, 0)] {
        let (s, mut e, item, work) = failing(name, harm_now);
        assert_eq!(looks(&e, item), ["failed"]);
        let (matter, what, body) = report_of(&e, item).unwrap();
        assert!(what.contains("A2 fails"), "{what}");
        assert_eq!(body["verdicts"][0]["id"], "A2");
        assert_eq!(body["verdicts"][0]["run"], SEVEN);
        assert_eq!(body["urgent"], harm_now);
        let (kind, of_item, queued_urgent): (String, i64, i64) =
            e.db.conn()
                .query_row(
                    "SELECT kind, item, urgent FROM architect_queue WHERE matter = ?1",
                    [matter],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
        assert_eq!((kind.as_str(), of_item, queued_urgent), ("report", item, urgent));
        let told: i64 =
            e.db.conn()
                .query_row("SELECT count(*) FROM owner_messages WHERE work = ?1", [work], |r| {
                    r.get(0)
                })
                .unwrap();
        assert_eq!(told, 1);
        step_to(&mut e, item, "E1");
        let _ = std::fs::remove_dir_all(&s.root);
    }
}

// With the project's limit of one look back, the first look that finds the evidence not there yet sends the work back to
// wait, and the second does not: it ends, and the report says the evidence did not come within the limit.
#[test]
fn a_look_past_the_projects_limit_reports_the_condition_and_ends() {
    let s = setup("in-effect-looks");
    let path = s.data.join("projects/p.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &text.replacen("repo = ", "verification_looks = 1\nrepo = ", 1),
    );
    git(&s.data, &["commit", "-q", "-am", "one look back"]);
    let mut agents = verifying(condition("merged"));
    agents.insert("W2", Box::new(|_: &Request, _: usize| waits_a_minute()));
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the first look", |e| !looks(e, item).is_empty());
    let (work, _, _) = state(&e, item).unwrap();
    assert_eq!(looks(&e, item), ["again"]);
    assert_eq!(state(&e, item).unwrap().1, "W1");
    assert!(report_of(&e, item).is_none());

    the_wait_passes(&mut e, work);
    step_to(&mut e, item, "E1");
    assert_eq!(looks(&e, item), ["again", "past_looks"]);
    assert_eq!(looks_counted(&e, work), 1);
    let (_, what, body) = report_of(&e, item).unwrap();
    assert!(what.contains("did not come within the limit of looks"), "{what}");
    assert!(what.contains("A2 not_yet"), "{what}");
    assert_eq!(body["conditions"], json!(["A2"]));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work whose acceptance holds no such condition ends after what it learned, and never comes to wait.
#[test]
fn a_work_with_no_condition_in_effect_ends_after_what_it_learned() {
    let s = setup("in-effect-none");
    let (mut e, _, item) = start(&s, scripts());
    step_to(&mut e, item, "E1");
    assert_eq!(transitions(&e, item, "L2"), ["done"]);
    assert!(transitions(&e, item, "W1").is_empty());
    let _ = std::fs::remove_dir_all(&s.root);
}

// A project the engine is not built from cannot change the installed program: a plan that says it carries the change is
// sent back saying so. Written as carried by main, it is taken, and the wait lets the work on at once.
#[test]
fn only_the_engine_s_own_project_can_say_the_installed_program_carries_its_change() {
    let s = setup("in-effect-other-project");
    let mut agents = verifying(condition("merged"));
    let plan = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                let both =
                    json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }, condition("installed")]);
                let (pick, mut set) = plan(r, 2);
                set.push(("outcome.can_start.acceptance", both));
                return (pick, set);
            }
            plan(r, 1)
        }),
    );
    agents.insert(
        "W2",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return asks(SEVEN);
            }
            answers(json!([{ "verdict": "holds", "id": "A2", "run": SEVEN, "seen": "7" }]))
        }),
    );
    let (mut e, _, item) = start(&s, agents);
    step_to(&mut e, item, "E1");
    let research = ended_at(&e, item, "R1");
    let sent_back = research[0].1.as_deref().unwrap();
    assert!(
        sent_back.contains("this project's changes cannot change the installed program"),
        "{sent_back}"
    );
    assert_eq!(research[1].0.as_deref(), Some("start"));
    assert_eq!(transitions(&e, item, "W1"), ["look"]);
    assert_eq!(looks(&e, item).last().unwrap(), "verified");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The real condition of a tool that must be installed and then configured: the change is in effect once the build holds
// it, and with the project's configuration not changed the tool is refused. A look that says it holds, resting on a
// `fetched` no run showed, is sent back; the look that says it fails, and that the configuration is what is missing, ends
// the work, and the architect's report says so.
#[test]
fn a_tool_refused_for_want_of_configuration_does_not_hold_and_the_report_says_what_is_missing() {
    let s = setup("in-effect-configuration");
    the_engine_s_own(&s);
    // The time is the test's: every run of the tool the test makes comes after this work took effect.
    let sql = "SELECT json_extract(result, '$.sources.verdict') AS verdict, json_extract(result, '$.sources.why') AS why \
               FROM tool_runs WHERE tool = 'dependency_sources' AND started_at > 0 ORDER BY id";
    let mut agents = verifying(json!({
        "by": "in_effect", "id": "A2",
        "condition": "Once the tool is installed and the project lists its lock files, a real call gives fetched.",
        "when": "installed", "tool": "query", "command": sql,
        "holds": "At least one row's verdict is fetched.", "enough": "At least one row.",
    }));
    // The other work's research asks for the tool, which the project's configuration does not list.
    let research = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            if n == 2 {
                let tools = json!([{ "tool": "dependency_sources", "lock": "Cargo.lock" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            research(r, n)
        }),
    );
    let not_yet = "No call of the tool yet.";
    let why = "The project's configuration lists no lock file: two [[dependency_sources]] tables are missing.";
    agents.insert(
        "W2",
        Box::new(move |_: &Request, n: usize| match n {
            1 | 3 => asks(sql),
            2 => {
                let (pick, mut set) = waits_a_minute();
                set[0].1[0]["missing"] = json!(not_yet);
                (pick, set)
            }
            4 => answers(json!([{ "verdict": "holds", "id": "A2", "run": sql, "seen": "fetched" }])),
            _ => answers(json!([{
                "verdict": "fails", "id": "A2", "run": sql, "seen": "refused", "why": why, "harm_now": false,
            }])),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    e.built_from = main_head(&s);
    step_to(&mut e, item, "W1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    // A later build holds the merge: the first look finds no call of the tool at all.
    let other = another_item(&mut e);
    step_to(&mut e, other, "E1");
    e.built_from = main_head(&s);
    step_until(&mut e, item, &said, "the first look", |e| !looks(e, item).is_empty());
    assert_eq!(looks(&e, item), ["again"]);

    the_wait_passes(&mut e, work);
    step_to(&mut e, item, "E1");
    let rounds = ended_at(&e, item, "W2");
    let sent_back = rounds
        .iter()
        .find_map(|(_, rejected)| rejected.as_deref())
        .expect("the look that rests on what no run showed is sent back");
    assert!(
        sent_back.contains("\\\"fetched\\\" is not in the result of that run"),
        "{sent_back}"
    );
    assert_eq!(looks(&e, item), ["again", "failed"]);
    let seen = verifications(&e, item);
    assert_eq!(seen.last().unwrap().1, "fails", "{seen:?}");
    let (_, what, body) = report_of(&e, item).unwrap();
    assert!(what.contains("configuration"), "{what}");
    assert_eq!(body["verdicts"][0]["why"], why);
    assert_eq!(body["verdicts"][0]["seen"], "refused");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Installing is by hand, and no one may install: past the project's limit of the wait since the merge, the work reports
// its conditions unverified and ends, its worktree let go, with no look; within the limit it waits on. The default limit
// is the one tried, on both sides.
#[test]
fn a_change_never_installed_within_the_limit_is_reported_unverified_and_the_work_ends() {
    let s = setup("in-effect-never");
    the_engine_s_own(&s);
    let (mut e, _, item) = start(&s, verifying(condition("installed")));
    e.built_from = main_head(&s);
    step_to(&mut e, item, "W1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "W1"), ["not_carried"]);

    merged_hours_ago(&mut e, work, 71);
    e.step(None).unwrap();
    assert_eq!(state(&e, item).unwrap().1, "W1");
    assert_eq!(transitions(&e, item, "W1"), ["not_carried"]);
    assert!(report_of(&e, item).is_none());

    merged_hours_ago(&mut e, work, 73);
    step_to(&mut e, item, "E1");
    assert_eq!(transitions(&e, item, "W1"), ["not_carried", "never_in_effect"]);
    assert!(looks(&e, item).is_empty());
    let (_, what, body) = report_of(&e, item).unwrap();
    assert!(what.contains("never took effect within the limit"), "{what}");
    assert!(what.contains("A2 not checked, and stand unverified"), "{what}");
    assert_eq!(body["conditions"], json!(["A2"]));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A build whose commit the engine cannot read leaves it unable to tell: the owner is told once, however long the work
// waits. What the owner says then takes the work on to a look, which may write skipped on those words and end it.
#[test]
fn a_build_that_cannot_be_told_tells_the_owner_once_and_words_take_the_work_on() {
    let s = setup("in-effect-cannot-tell");
    the_engine_s_own(&s);
    let mut agents = verifying(condition("installed"));
    agents.insert(
        "W2",
        Box::new(|r: &Request, _: usize| {
            assert!(r.prompt.contains("It was reverted."), "{}", r.prompt);
            answers(json!([{ "verdict": "skipped", "id": "A2", "words": "The owner: it was reverted." }]))
        }),
    );
    let (mut e, _, item) = start(&s, agents);
    e.built_from = "unknown".into();
    step_to(&mut e, item, "W1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "W1"), ["cannot_tell"]);
    let told: Vec<String> = notices_of(&e, work)
        .into_iter()
        .filter(|t| t.contains("cannot tell whether the installed program holds merge"))
        .collect();
    assert_eq!(told.len(), 1, "{told:?}");

    owner_says(&mut e, work, "It was reverted.");
    step_to(&mut e, item, "E1");
    assert_eq!(transitions(&e, item, "W1"), ["cannot_tell", "words"]);
    assert_eq!(looks(&e, item), ["verified"]);
    assert!(report_of(&e, item).is_none());
    let _ = std::fs::remove_dir_all(&s.root);
}
