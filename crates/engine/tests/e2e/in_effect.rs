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
    verifying_all(vec![written])
}

/// The agents as [`verifying`] makes them, with each of `written` beside the default condition.
fn verifying_all(written: Vec<Value>) -> BTreeMap<&'static str, Script> {
    let mut acceptance = vec![json!({ "by": "test", "id": "A1", "condition": "The feature is there." })];
    let mut rows = vec![json!({ "how": "tested", "id": "A1", "tests": ["feature"] })];
    for c in written {
        rows.push(json!({ "how": "in_effect", "id": c["id"] }));
        acceptance.push(c);
    }
    let mut agents = scripts();
    let research = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = research(r, n);
            if n == 1 {
                set.push(("outcome.can_start.acceptance", json!(acceptance)));
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
                set.push(("outcome.conditions", json!(rows)));
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
    transitions(e, item, "V2")
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
            "UPDATE rounds SET ended_at = ended_at - 120000 WHERE work = ?1 AND cell = 'V2' AND transition = 'again'",
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
        "V2",
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
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();

    // It waits by the one row, recorded once, and holds no slot.
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["not_carried"]);
    let w = e.work(work).unwrap();
    assert!(e.waiting(&w).unwrap());
    let rounds = rounds_of(&e, work);
    e.step(None).unwrap();
    assert_eq!(rounds_of(&e, work), rounds);

    // The other work takes the one slot, and merges.
    let other = another_item(&mut e);
    step_to(&mut e, other, "E1");
    assert_eq!(rounds_of(&e, work), rounds);
    assert_eq!(state(&e, item).unwrap().1, "V1");

    // The build installed now holds the merge, and the first look finds the evidence not there yet.
    e.built_from = main_head(&s);
    step_until(&mut e, item, &said, "the first look", |e| !looks(e, item).is_empty());
    assert_eq!(looks(&e, item).last().unwrap(), "again");
    assert_eq!(looks_counted(&e, work), 1);
    assert_eq!(state(&e, item).unwrap().1, "V1");

    // Waiting as long as it said, it writes nothing after the row that says so.
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1").last().unwrap(), "not_due");
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
    assert_eq!(latest, "V2");

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
        session_at("V2", "again"),
        session_at("V2", "verified"),
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
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["not_carried"]);
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
        "V2",
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
    agents.insert("V2", Box::new(|_: &Request, _: usize| waits_a_minute()));
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the first look", |e| !looks(e, item).is_empty());
    let (work, _, _) = state(&e, item).unwrap();
    assert_eq!(looks(&e, item), ["again"]);
    assert_eq!(state(&e, item).unwrap().1, "V1");
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
        "V2",
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
    assert_eq!(transitions(&e, item, "V1"), ["look"]);
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
        "V2",
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
    step_to(&mut e, item, "V1");
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
    let rounds = ended_at(&e, item, "V2");
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
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["not_carried"]);

    merged_hours_ago(&mut e, work, 71);
    e.step(None).unwrap();
    assert_eq!(state(&e, item).unwrap().1, "V1");
    assert_eq!(transitions(&e, item, "V1"), ["not_carried"]);
    assert!(report_of(&e, item).is_none());

    merged_hours_ago(&mut e, work, 73);
    step_to(&mut e, item, "E1");
    assert_eq!(transitions(&e, item, "V1"), ["not_carried", "never_in_effect"]);
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
        "V2",
        Box::new(|r: &Request, _: usize| {
            assert!(r.prompt.contains("It was reverted."), "{}", r.prompt);
            answers(json!([{ "verdict": "skipped", "id": "A2", "words": "The owner: it was reverted." }]))
        }),
    );
    let (mut e, _, item) = start(&s, agents);
    e.built_from = "unknown".into();
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["cannot_tell"]);
    let told: Vec<String> = notices_of(&e, work)
        .into_iter()
        .filter(|t| t.contains("cannot tell whether the installed program holds merge"))
        .collect();
    assert_eq!(told.len(), 1, "{told:?}");

    owner_says(&mut e, work, "It was reverted.");
    step_to(&mut e, item, "E1");
    assert_eq!(transitions(&e, item, "V1"), ["cannot_tell", "words"]);
    assert_eq!(looks(&e, item), ["verified"]);
    assert!(report_of(&e, item).is_none());
    let _ = std::fs::remove_dir_all(&s.root);
}

// A condition that does not hold is reported at once, though another's evidence is not there yet: waiting for it could
// leave what does harm now unknown to the architect for the whole of the looks. The report says what was not checked, and
// the look need not say how long to wait, as the work ends.
#[test]
fn a_condition_that_fails_is_reported_at_once_though_another_is_not_yet() {
    let s = setup("in-effect-fails-at-once");
    let mut agents = verifying_all(vec![on_main("A2", "query", SEVEN), on_main("A3", "query", SEVEN)]);
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return asks(SEVEN);
            }
            answers(json!([fails("A2", SEVEN, "7", true), not_yet("A3")]))
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the look", |e| !looks(e, item).is_empty());
    let (work, _, _) = state(&e, item).unwrap();
    assert!(why_sent_back(&e, work, "V2").is_empty());
    assert_eq!(looks(&e, item), ["failed"]);
    assert_eq!(looks_counted(&e, work), 0);
    let (matter, what, body) = report_of(&e, item).unwrap();
    for part in ["A2 fails", "A3 not_yet", "not checked, and stand unverified"] {
        assert!(what.contains(part), "{part} not in {what}");
    }
    assert_eq!(body["urgent"], true);
    let urgent: i64 =
        e.db.conn()
            .query_row("SELECT urgent FROM architect_queue WHERE matter = ?1", [matter], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(urgent, 1);
    step_to(&mut e, item, "E1");
    let _ = std::fs::remove_dir_all(&s.root);
}

// With no evidence still to wait for and nothing found not to hold, a condition that cannot be judged goes to the
// architect by a row of its own.
#[test]
fn a_condition_that_cannot_be_judged_is_reported_once_nothing_is_awaited() {
    let s = setup("in-effect-unjudged");
    let mut agents = verifying_all(vec![on_main("A2", "query", SEVEN), on_main("A3", "query", SEVEN)]);
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return asks(SEVEN);
            }
            answers(json!([cannot_judge("A2"), holds("A3", SEVEN, "7")]))
        }),
    );
    let (mut e, _, item) = start(&s, agents);
    step_to(&mut e, item, "E1");
    assert_eq!(looks(&e, item), ["unjudged"]);
    let (_, what, _) = report_of(&e, item).unwrap();
    assert!(what.contains("A2 cannot_judge"), "{what}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Words not to check a condition hold for every look after, though each is a session of its own: the look they reached
// skips it and finds another's evidence not there yet, and the next, given no words, may still skip it. A build that
// cannot be told sends the work back to wait by the same row after that look, and the owner, told once, is not told again.
#[test]
fn words_not_to_check_a_condition_hold_for_each_look_after() {
    let s = setup("in-effect-skip");
    the_engine_s_own(&s);
    let mut second = on_main("A3", "query", SEVEN);
    second["when"] = json!("installed");
    let mut agents = verifying_all(vec![condition("installed"), second]);
    let skip = || json!({ "verdict": "skipped", "id": "A2", "words": "The owner: it was reverted." });
    agents.insert(
        "V2",
        Box::new(move |_: &Request, n: usize| match n {
            1 => again_after(json!([skip(), not_yet("A3")]), 1),
            2 => asks(SEVEN),
            _ => answers(json!([skip(), holds("A3", SEVEN, "7")])),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    e.built_from = "unknown".into();
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["cannot_tell"]);
    owner_says(&mut e, work, "It was reverted, so do not check A2.");
    step_until(&mut e, item, &said, "the look on words", |e| !looks(e, item).is_empty());
    assert_eq!(looks(&e, item), ["again"]);
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["cannot_tell", "words", "cannot_tell"]);
    let told = notices_of(&e, work)
        .iter()
        .filter(|t| t.contains("cannot tell whether the installed program holds merge"))
        .count();
    assert_eq!(told, 1);
    let notice = notices_of(&e, work)
        .into_iter()
        .find(|t| t.contains("cannot tell whether the installed program holds merge"))
        .unwrap();
    assert!(
        notice.contains("Only installing a build of main settles it") && notice.contains("buys one look"),
        "{notice}"
    );

    e.built_from = main_head(&s);
    the_wait_passes(&mut e, work);
    step_to(&mut e, item, "E1");
    assert!(
        why_sent_back(&e, work, "V2").is_empty(),
        "{:?}",
        why_sent_back(&e, work, "V2")
    );
    assert_eq!(looks(&e, item), ["again", "verified"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return that changes the acceptance says how its conditions are met against the list it hands in, a condition checked
// after the merge among them: the rows of the old list are sent back, and those of the new one are taken.
#[test]
fn a_return_that_changes_the_acceptance_answers_for_the_list_it_hands_in() {
    let s = setup("in-effect-changed");
    let mut agents = scripts();
    let implementing = agents.remove("I1").unwrap();
    let changed = json!({
        "acceptance": [
            { "by": "test", "id": "A1", "condition": "The feature is there." },
            condition("merged"),
        ],
        "why": "A real run is what shows it.",
    });
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = implementing(r, n);
            if n >= 2 {
                let tested = json!({ "how": "tested", "id": "A1", "tests": ["feature"] });
                let rows = if n == 2 {
                    json!([tested])
                } else {
                    json!([tested, { "how": "in_effect", "id": "A2" }])
                };
                set.push(("outcome.acceptance_changed", changed.clone()));
                set.push(("outcome.conditions", rows));
            }
            (pick, set)
        }),
    );
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return asks(SEVEN);
            }
            answers(json!([holds("A2", SEVEN, "7")]))
        }),
    );
    let (mut e, _, item) = start(&s, agents);
    // The return sent back queues a report for the project's architect, whose work would run beside this one and take
    // the passes that carry it past a cell; so the work is stepped alone, as the tests of returns sent back do.
    let work = e.step(None).unwrap().work.unwrap();
    let at = |e: &Engine| state(e, item).unwrap().1;
    advance_until(&mut e, work, |e| at(e) == "V1");
    assert_eq!(at(&e), "V1");
    let sent_back = why_sent_back(&e, work, "I1");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}");
    assert!(sent_back[0].contains("A2"), "{sent_back:?}");
    advance_until(&mut e, work, |e| at(e) == "E1");
    assert_eq!(at(&e), "E1");
    assert_eq!(looks(&e, item), ["verified"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A query's result too long to give is saved to a file, and what a look saw is looked for there: a value the file does
// not hold is sent back, and one it holds goes on.
#[test]
fn a_look_finds_what_it_saw_in_the_file_a_long_result_was_saved_to() {
    const LONG: &str = "SELECT hex(zeroblob(15000)) AS pad, 7 AS seven";
    let s = setup("in-effect-long");
    let mut agents = verifying(on_main("A2", "query", LONG));
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| match n {
            1 => asks(LONG),
            2 => answers(json!([holds("A2", LONG, "8")])),
            _ => answers(json!([holds("A2", LONG, "7")])),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the look", |e| !looks(e, item).is_empty());
    let (work, _, _) = state(&e, item).unwrap();
    let sent_back = why_sent_back(&e, work, "V2");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}");
    assert!(
        sent_back[0].contains("is not in the result of that run"),
        "{sent_back:?}"
    );
    assert_eq!(looks(&e, item), ["verified"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A condition carried by main, numbered `id`, that the tool's real run of `command` checks.
fn on_main(id: &str, tool: &str, command: &str) -> Value {
    json!({
        "by": "in_effect", "id": id, "condition": "A real run gives what it should.", "when": "merged", "tool": tool,
        "command": command, "holds": "It gives what it should.", "enough": "none",
    })
}

/// A look that asks the engine to run queries, and gh commands where one is named.
fn asks_all(queries: &[&str], gh: &[&str]) -> Return {
    let queries = queries.iter().map(|sql| json!({ "tool": "query", "sql": sql }));
    let gh = gh.iter().map(|command| json!({ "tool": "gh", "command": command }));
    let tools: Vec<Value> = queries.chain(gh).collect();
    (vec![("outcome", "tools")], vec![("outcome.tools", json!(tools))])
}

fn holds(id: &str, run: &str, seen: &str) -> Value {
    json!({ "verdict": "holds", "id": id, "run": run, "seen": seen })
}

fn fails(id: &str, run: &str, seen: &str, harm_now: bool) -> Value {
    json!({
        "verdict": "fails", "id": id, "run": run, "seen": seen, "why": "It gives what it should not.", "harm_now": harm_now,
    })
}

fn not_yet(id: &str) -> Value {
    json!({ "verdict": "not_yet", "id": id, "missing": "No run yet." })
}

fn cannot_judge(id: &str) -> Value {
    json!({ "verdict": "cannot_judge", "id": id, "why": "The command gave nothing the condition can be judged by." })
}

/// A look's answer that says to look again after `minutes`.
fn again_after(verdicts: Value, minutes: i64) -> Return {
    (
        Vec::new(),
        vec![
            ("outcome.verdicts", verdicts),
            ("outcome.again_after_minutes", json!(minutes)),
        ],
    )
}

/// The time a round of the work at a cell that took a row began or ended, by the column named.
fn time_of(e: &Engine, work: i64, cell: &str, row: &str, column: &str) -> i64 {
    e.db.conn()
        .query_row(
            &format!(
                "SELECT {column} FROM rounds WHERE work = ?1 AND cell = ?2 AND transition = ?3 ORDER BY n LIMIT 1"
            ),
            params![work, cell, row],
            |r| r.get(0),
        )
        .unwrap()
}

// The field `seen` says what the verdict rests on is "a value, or part of a row" of the run's result, and the cell's
// prompt says a command that cannot run is cannot_judge and that a command is run as it is written. So none of these
// is a basis for a verdict, and the look that gives them is sent back naming each: the name of a field of the tool's
// answer where the query gave no row; a column's name where the value says otherwise, the case of a tool refused whose
// query names its column `fetched`; a digit of a larger number; the text of a query that did not run, or of a gh
// command that was not allowed; and another query than the condition's. The same look's verdict on an allowed gh
// command, named with or without `gh`, is sound and is not named, and the look that rests on the values goes on.
#[test]
fn a_verdict_rests_only_on_a_value_its_own_command_gave() {
    const NO_ROW: &str = "SELECT 1 AS one WHERE 0";
    const REFUSED: &str = "SELECT 'refused' AS fetched";
    const LARGER: &str = "SELECT 217 AS ms";
    const BROKEN: &str = "SELECT nope FROM nowhere";
    const OWN: &str = "SELECT 9 AS nine";
    const ANOTHER: &str = "SELECT 8 AS eight";
    const NOT_ALLOWED: &str = "auth status";
    const ALLOWED: &str = "pr view 1";
    let s = setup("in-effect-basis");
    let mut agents = verifying_all(vec![
        on_main("A2", "query", NO_ROW),
        on_main("A3", "query", REFUSED),
        on_main("A4", "query", LARGER),
        on_main("A5", "query", BROKEN),
        on_main("A6", "gh", NOT_ALLOWED),
        on_main("A7", "query", OWN),
        on_main("A8", "gh", ALLOWED),
    ]);
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| match n {
            1 => asks_all(
                &[NO_ROW, REFUSED, LARGER, BROKEN, OWN, ANOTHER],
                &[NOT_ALLOWED, ALLOWED],
            ),
            2 => answers(json!([
                holds("A2", NO_ROW, "rows"),
                holds("A3", REFUSED, "fetched"),
                holds("A4", LARGER, "7"),
                holds("A5", BROKEN, "no such table"),
                holds("A6", NOT_ALLOWED, "not_allowed"),
                holds("A7", ANOTHER, "8"),
                holds("A8", "gh pr view 1", "pr view 1"),
            ])),
            _ => answers(json!([
                cannot_judge("A2"),
                fails("A3", REFUSED, "refused", false),
                holds("A4", LARGER, "217"),
                cannot_judge("A5"),
                cannot_judge("A6"),
                holds("A7", OWN, "9"),
                holds("A8", ALLOWED, "pr view 1"),
            ])),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the look", |e| !looks(e, item).is_empty());
    let (work, _, _) = state(&e, item).unwrap();
    let sent_back = why_sent_back(&e, work, "V2").join("\n");
    let taken: Vec<&str> = ["A2", "A3", "A4", "A5", "A6", "A7"]
        .into_iter()
        .filter(|id| !sent_back.contains(id))
        .collect();
    assert!(taken.is_empty(), "taken as sound: {taken:?}; sent back: {sent_back}");
    assert!(!sent_back.contains("A8"), "{sent_back}");
    assert_eq!(looks(&e, item), ["failed"]);
    assert_eq!(verifications(&e, item).len(), 7);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The design has a look put in only the time the change took effect, and rest only on runs since. For a condition the
// installed program carries that time is the merge's until the wait finds a build that holds it, and then that round's
// start. A look on words, while no build holds the merge, runs both queries; the next look, once one does, is sent back
// when it names those runs, the query that takes the time and the one that does not alike, and goes on when it has run
// them again. A not_yet with no time to wait is sent back too, which would otherwise spend the limit of looks at once.
// The view holds only the returns taken.
#[test]
fn a_look_takes_no_run_from_before_the_change_took_effect() {
    const PLACED: &str = "SELECT 7 AS seven WHERE :in_effect_at > 0";
    let placed = |at: i64| PLACED.replace(":in_effect_at", &at.to_string());
    let s = setup("in-effect-before");
    the_engine_s_own(&s);
    let mut second = on_main("A3", "query", PLACED);
    second["when"] = json!("installed");
    let mut agents = verifying_all(vec![condition("installed"), second]);
    let effects: Arc<Mutex<Vec<Value>>> = Arc::default();
    let given = effects.clone();
    agents.insert(
        "V2",
        Box::new(move |r: &Request, n: usize| {
            let mut given = given.lock().unwrap();
            if n == 1 || n == 4 {
                given.push(given_value(&r.prompt, "effect").expect(&r.prompt));
            }
            let at = |i: usize| placed(given[i]["at"].as_i64().unwrap());
            match n {
                1 => asks_all(&[SEVEN, &at(0)], &[]),
                2 => answers(json!([not_yet("A2"), not_yet("A3")])),
                3 => again_after(json!([not_yet("A2"), not_yet("A3")]), 1),
                4 => answers(json!([holds("A2", SEVEN, "7"), holds("A3", &at(0), "7")])),
                5 => asks_all(&[SEVEN, &at(1)], &[]),
                _ => answers(json!([holds("A2", SEVEN, "7"), holds("A3", &at(1), "7")])),
            }
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    e.built_from = main_head(&s);
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    owner_says(&mut e, work, "Look now.");
    step_until(&mut e, item, &said, "the look on words", |e| !looks(e, item).is_empty());
    assert_eq!(looks(&e, item), ["again"]);
    e.step(None).unwrap();

    e.built_from = main_head(&s);
    the_wait_passes(&mut e, work);
    step_to(&mut e, item, "E1");
    assert_eq!(
        transitions(&e, item, "V1"),
        ["not_carried", "words", "not_carried", "look"]
    );
    assert_eq!(looks(&e, item), ["again", "verified"]);
    let effects = effects.lock().unwrap();
    assert_eq!(effects[0]["in_effect"], "no");
    assert_eq!(effects[0]["at"], time_of(&e, work, "M2", "merged", "ended_at"));
    assert_eq!(effects[1]["in_effect"], "yes");
    assert_eq!(effects[1]["at"], time_of(&e, work, "V1", "look", "started_at"));
    assert!(time_of(&e, work, "V1", "look", "started_at") > time_of(&e, work, "M2", "merged", "ended_at"));
    let sent_back = why_sent_back(&e, work, "V2");
    assert_eq!(sent_back.len(), 2, "{sent_back:?}");
    assert!(sent_back[0].contains("again_after_minutes"), "{sent_back:?}");
    assert!(
        sent_back[1].contains("A2") && sent_back[1].contains("A3"),
        "{sent_back:?}"
    );
    let verdicts: Vec<String> = verifications(&e, item).into_iter().map(|v| v.1).collect();
    assert_eq!(verdicts, ["not_yet", "not_yet", "holds", "holds"]);
    assert!(report_of(&e, item).is_none());
    let _ = std::fs::remove_dir_all(&s.root);
}

// Looks over several conditions, by the flow's rows: a not_yet with none that fails sends the work back to wait, as long
// as the latest look said and not an earlier one; then a condition that fails ends the work with a report of what each
// came to, urgent when any one does harm now. The time a condition carried by main took effect is the merge's.
#[test]
fn looks_over_several_conditions_wait_as_the_latest_says_and_report_what_each_came_to() {
    let s = setup("in-effect-several");
    let mut agents = verifying_all(vec![
        on_main("A2", "query", SEVEN),
        on_main("A3", "query", SEVEN),
        on_main("A4", "query", SEVEN),
    ]);
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| match n {
            1 | 3 | 5 => asks(SEVEN),
            2 => again_after(json!([cannot_judge("A2"), not_yet("A3"), holds("A4", SEVEN, "7")]), 1),
            4 => again_after(json!([not_yet("A2"), not_yet("A3"), holds("A4", SEVEN, "7")]), 2880),
            _ => answers(json!([
                fails("A2", SEVEN, "7", false),
                fails("A3", SEVEN, "7", true),
                holds("A4", SEVEN, "7"),
            ])),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the first look", |e| !looks(e, item).is_empty());
    let (work, _, _) = state(&e, item).unwrap();
    assert_eq!(looks(&e, item), ["again"]);
    let at = given_value(&message_at(&e, work, "V2", false), "effect").unwrap();
    assert_eq!(at["at"], time_of(&e, work, "M2", "merged", "ended_at"));

    the_wait_passes(&mut e, work);
    step_until(&mut e, item, &said, "the second look", |e| looks(e, item).len() == 2);
    assert_eq!(looks(&e, item), ["again", "again"]);

    // Past what the first look said, and short of what the latest said, it waits.
    the_wait_passes(&mut e, work);
    e.step(None).unwrap();
    e.step(None).unwrap();
    assert_eq!(looks(&e, item).len(), 2);
    assert_eq!(state(&e, item).unwrap().1, "V1");
    assert_eq!(transitions(&e, item, "V1").last().unwrap(), "not_due");

    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET ended_at = ended_at - 2880 * 60000 WHERE work = ?1 AND cell = 'V2' AND transition = 'again'",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    step_to(&mut e, item, "E1");
    assert_eq!(looks(&e, item), ["again", "again", "failed"]);
    assert_eq!(looks_counted(&e, work), 2);
    let (matter, what, body) = report_of(&e, item).unwrap();
    for part in ["A2 fails", "A3 fails", "A4 holds"] {
        assert!(what.contains(part), "{what}");
    }
    assert_eq!(body["conditions"], json!(["A2", "A3", "A4"]));
    assert_eq!(body["verdicts"].as_array().unwrap().len(), 3);
    let urgent: i64 =
        e.db.conn()
            .query_row("SELECT urgent FROM architect_queue WHERE matter = ?1", [matter], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(urgent, 1);
    let verdicts: Vec<String> = verifications(&e, item).into_iter().map(|v| v.1).collect();
    assert_eq!(
        verdicts,
        [
            "cannot_judge",
            "not_yet",
            "holds",
            "not_yet",
            "not_yet",
            "holds",
            "fails",
            "fails",
            "holds"
        ]
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The wait goes by the build as it stands: a build before the merge waits by one row, a build that cannot be told by
// another, which tells the owner once, a new flow version in between included, and a build the checkout has not fetched
// yet that holds the merge lets the work on, the wait's limit being only for a change not known to be in effect.
#[test]
fn the_wait_follows_the_build_through_not_carried_and_cannot_tell_to_a_look() {
    let s = setup("in-effect-walk");
    the_engine_s_own(&s);
    let mut agents = verifying(condition("installed"));
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return asks(SEVEN);
            }
            answers(json!([holds("A2", SEVEN, "7")]))
        }),
    );
    let (mut e, _, item) = start(&s, agents);
    e.built_from = main_head(&s);
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["not_carried"]);

    e.built_from = "unknown".into();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1"), ["not_carried", "cannot_tell"]);
    let told = |e: &Engine| {
        notices_of(e, work)
            .iter()
            .filter(|t| t.contains("installed program"))
            .count()
    };
    assert_eq!(told(&e), 1);

    older(&mut e, &[work], |g| {
        let w1 = g.cells.iter_mut().find(|c| c.id == "V1").unwrap();
        w1.rows.retain(|r| r.name != "never_in_effect");
    });
    e.step(None).unwrap();
    assert_eq!(versions_of(&e, work).0, e.flow_now);
    assert_eq!(told(&e), 1);

    main_moves_on(&s.root, &[("later.txt", "later")], "Another change");
    e.built_from = main_head(&s);
    merged_hours_ago(&mut e, work, 73);
    step_to(&mut e, item, "E1");
    assert_eq!(transitions(&e, item, "V1"), ["not_carried", "cannot_tell", "look"]);
    assert_eq!(looks(&e, item), ["verified"]);
    assert!(report_of(&e, item).is_none());
    let _ = std::fs::remove_dir_all(&s.root);
}

// The project's own limit of the wait holds for a build that cannot be told as for one before the merge: within it the
// work waits, and past it what was said to the work still comes first; only then does the work report its conditions
// unverified and end. The default limit of 72 hours would still be waiting.
#[test]
fn past_the_projects_limit_a_build_not_told_ends_the_wait_after_what_was_said() {
    let s = setup("in-effect-limit");
    the_engine_s_own(&s);
    let path = s.data.join("projects/p.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &text.replacen("repo = ", "install_wait_hours = 2\nrepo = ", 1),
    );
    git(&s.data, &["commit", "-q", "-am", "two hours of waiting"]);
    let mut agents = verifying(condition("installed"));
    agents.insert("V2", Box::new(|_: &Request, _: usize| waits_a_minute()));
    let (mut e, _, item) = start(&s, agents);
    e.built_from = "unknown".into();
    step_to(&mut e, item, "V1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    merged_hours_ago(&mut e, work, 1);
    e.step(None).unwrap();
    assert_eq!(state(&e, item).unwrap().1, "V1");
    assert_eq!(transitions(&e, item, "V1"), ["cannot_tell"]);

    merged_hours_ago(&mut e, work, 3);
    owner_says(&mut e, work, "Is it in?");
    step_to(&mut e, item, "E1");
    assert_eq!(transitions(&e, item, "V1"), ["cannot_tell", "words", "never_in_effect"]);
    assert_eq!(looks(&e, item), ["again"]);
    let (_, what, body) = report_of(&e, item).unwrap();
    assert_eq!(body["conditions"], json!(["A2"]));
    assert!(what.contains("cannot tell whether the change took effect"), "{what}");
    assert!(!what.contains("never took effect"), "{what}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work waiting at the cell after a look moves to a new flow version where it stands, however that version changed the
// wait's rows or the inputs of the cell that looks: the look that was taken stays taken and counted, its time to wait
// stands, and the next look comes when it has passed, in a session of its own.
#[test]
fn a_new_flow_version_leaves_a_work_waiting_between_looks_where_it_stands() {
    let s = setup("in-effect-version");
    let mut agents = verifying(condition("merged"));
    agents.insert(
        "V2",
        Box::new(|_: &Request, n: usize| match n {
            1 => waits_a_minute(),
            2 => asks(SEVEN),
            _ => answers(json!([holds("A2", SEVEN, "7")])),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the first look", |e| !looks(e, item).is_empty());
    let (work, _, _) = state(&e, item).unwrap();
    e.step(None).unwrap();
    assert_eq!(transitions(&e, item, "V1").last().unwrap(), "not_due");
    let rounds = rounds_of(&e, work);

    older(&mut e, &[work], |g| {
        let w1 = g.cells.iter_mut().find(|c| c.id == "V1").unwrap();
        w1.rows.retain(|r| r.name != "never_in_effect");
        let w2 = g.cells.iter_mut().find(|c| c.id == "V2").unwrap();
        w2.inputs.pop();
    });
    e.step(None).unwrap();
    assert_eq!(versions_of(&e, work).0, e.flow_now);
    assert_eq!(state(&e, item).unwrap().1, "V1");
    assert_eq!(rounds_of(&e, work), rounds);

    the_wait_passes(&mut e, work);
    step_to(&mut e, item, "E1");
    assert_eq!(looks(&e, item), ["again", "verified"]);
    assert_eq!(looks_counted(&e, work), 1);
    let session_of_look = |row: &str| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT session FROM rounds WHERE work = ?1 AND cell = 'V2' AND transition = ?2",
                params![work, row],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_ne!(session_of_look("again"), session_of_look("verified"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// The design has the report's material be the item, the merge commit, each condition's result and its run, and the
// architect judge what it means: a work of the architect takes it at its cell for reports, given all that, and handles
// it.
#[test]
fn the_architect_handles_a_check_s_report_given_what_it_rests_on() {
    let (s, mut e, item, _) = failing("in-effect-architect", true);
    step_to(&mut e, item, "E1");
    let (matter, _, body) = report_of(&e, item).unwrap();
    let merge = body["merge_commit"].as_str().unwrap().to_string();
    assert!(!merge.is_empty());
    e.pass(&Go::default()).unwrap();
    let arch = *architect_works(&e).first().expect("a work of the architect");
    assert!(steps_until(&mut e, 40, |e| architect_work_done(e, arch)));
    assert!(path_of(&e, arch).contains(&"H1".to_string()));
    let given = message_at(&e, arch, "H1", false);
    for part in ["It should give 8.", SEVEN, merge.as_str(), "A2"] {
        assert!(given.contains(part), "{part} not in {given}");
    }
    let handled: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM work_handled WHERE work = ?1 AND matter = ?2",
                params![arch, matter],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(handled, 1);
    let _ = std::fs::remove_dir_all(&s.root);
}
