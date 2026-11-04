//! Cancels, starts over and resumes asked from outside a work's step, against the step and the merge they meet.

use super::*;

/// The works with a request to lift their pause waiting in the outbox.
fn resumes_waiting(e: &Engine) -> Vec<i64> {
    let mut st =
        e.db.conn()
            .prepare("SELECT work FROM outbox WHERE kind = 'resume_work' ORDER BY work")
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

/// How the pull requests on the fake GitHub stand, in the order of their numbers.
fn pr_states(s: &Setup) -> Vec<PrState> {
    s.prs.lock().unwrap().values().map(|p| p.state).collect()
}

// The owner cancels a work's item while its first CI runs: nothing ends yet. The pull request is recorded before that
// wait, so that an engine that dies in it still knows what to close; the step goes on to its end, and the loop then ends
// the work and closes it. A cancel that ended the work at once found no pull request recorded and left it open with no
// one to close it, and so did one carried out first thing after a crash, when the tool wrote it only on coming back.
#[test]
fn a_cancel_during_the_first_ci_waits_for_the_step_and_closes_its_pull_request() {
    let s = setup("cancel-in-ci");
    s.ci.shut();
    let (mut e, said, item) = start(&s, scripts());
    let ci = turns_until(&mut e, &Go::default(), |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let work = state(&e, item).unwrap().0;
    assert!(tool_round_open(&e, work).is_some(), "{log}");
    assert_eq!(pr_states(&s), [PrState::Open], "{log}");
    assert_eq!(works_of(&e, item)[0].4, Some(1), "{log}");
    cancel(&mut e, item, "owner");
    let ended_at_once = ended_why(&e, work);
    let round_cut_off = tool_round_open(&e, work).is_none();
    s.ci.open();
    // Until it closes, or the step has ended and the outbox holds nothing that could close it.
    let settled = turns_until(&mut e, &Go::default(), |e| {
        pr_states(&s) == [PrState::Closed]
            || (e.in_flight().is_empty() && number(e, "SELECT count(*) FROM outbox") == 0)
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(settled, "{log}");
    assert_eq!(pr_states(&s), [PrState::Closed], "{log}");
    assert_eq!(ended_at_once, None, "{log}");
    assert!(!round_cut_off, "{log}");
    assert_eq!(cancelled_why(&e, item).as_deref(), Some("no longer needed"), "{log}");
    assert!(ended_why(&e, work).is_some(), "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'X1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(took.as_deref(), Some("ran"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A start over asked while a step of the work runs, here its CI, waits until that step has ended: the loop's turn
// meanwhile leaves it in the outbox and the step's round ends as usual; the next turn carries it out.
#[test]
fn a_start_over_asked_during_a_step_waits_until_the_step_ends() {
    let s = setup("start-over-in-ci");
    s.ci.shut();
    let (mut e, said, item) = start(&s, scripts());
    let ci = turns_until(&mut e, &Go::default(), |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let work = state(&e, item).unwrap().0;
    api::start_over(&mut e.db, work).unwrap();
    e.dispatch(&Go::default()).unwrap();
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    assert!(tool_round_open(&e, work).is_some(), "{log}");
    assert_eq!(stopping(&e), [work], "{log}");
    s.ci.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'X1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(took.as_deref(), Some("ran"), "{log}");
    e.dispatch(&Go::default()).unwrap();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    assert!(works[0].5.is_some(), "{works:?}\n{log}");
    assert_eq!(stopping(&e), Vec::<i64>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner cancels a work's item while its agent's round runs: nothing ends yet, and nobody is told, which a cancel
// that ended the work at once would have got wrong had the merge come first. The owner's page says the work is stopping.
// The round's call returns and what it handed in is not taken; the loop then ends the work, cuts the round off keeping
// what it cost, and tells the item that depended on it; the page says stopping no more, and the work is not paused.
#[test]
fn a_cancel_during_an_agent_s_round_takes_nothing_of_it_and_tells_no_one_until_the_step_ends() {
    let s = setup("cancel-in-a-round");
    let hold = Gate::default();
    let h = hold.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                h.pass();
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    let work = state(&e, item).unwrap().0;
    assert_eq!(open_rounds(&e, work), 1, "{log}");
    // An item that depends on it and has no work: its architect is told, by a matter to set straight.
    let dependent =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'After it', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            db::add_dependency(tx, id, item, "its tokens", "blocking")?;
            Ok(id)
        })
        .unwrap();
    let told = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'set_straight' AND item = ?1",
                [dependent],
                |r| r.get(0),
            )
            .unwrap()
    };
    cancel(&mut e, item, "owner");
    // The loop's turn with the step in flight leaves it all as it was.
    e.dispatch(&Go::default()).unwrap();
    assert_eq!(told(&e), 0, "{log}");
    assert_eq!(ended_why(&e, work), None, "{log}");
    assert_eq!(open_rounds(&e, work), 1, "{log}");
    assert_eq!(stopping(&e), [work], "{log}");
    hold.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_why(&e, work), None, "{log}");
    assert_eq!(told(&e), 0, "{log}");
    e.dispatch(&Go::default()).unwrap();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(cancelled_why(&e, item).as_deref(), Some("no longer needed"), "{log}");
    assert_eq!(
        ended_why(&e, work).as_deref(),
        Some("item cancelled: no longer needed"),
        "{log}"
    );
    assert_eq!(told(&e), 1, "{log}");
    assert_eq!(stopping(&e), Vec::<i64>::new(), "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    assert_eq!(open_rounds(&e, work), 0, "{log}");
    // Nothing of what the round handed in is taken; what it cost is kept on it.
    let (output, transition, rejected, cost): (Option<String>, Option<String>, Option<String>, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT output, transition, rejected, usage FROM rounds WHERE work = ?1 AND cell = 'R1'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
    assert_eq!((output, transition), (None, None), "{log}");
    assert!(rejected.is_some_and(|r| r.contains("interrupted")), "{log}");
    assert!(cost.is_some(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What is asked of a work from outside its step: a merge, which cannot be undone, against a stop asked around it, and
// the slots against a pause lifted or a round that runs on.

/// An engine over a setup whose one work stands at the merge step, which it has not begun, with its item and the work.
fn at_the_merge(name: &str) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64, i64) {
    let s = setup(name);
    let (mut e, said, item) = start(&s, scripts());
    let at = turns_until(&mut e, &Go::default(), |e| state(e, item).is_some_and(|w| w.1 == "M2"));
    assert!(at, "{}", said.lock().unwrap().join("\n"));
    let work = state(&e, item).unwrap().0;
    (s, e, said, item, work)
}

/// Takes turns until the merge step waits for the full CI, which stays shut.
fn held_at_full_ci(e: &mut Engine, s: &Setup, said: &Mutex<Vec<String>>) {
    s.ci.shut();
    let came = s.ci.came();
    let held = turns_until(e, &Go::default(), |_| s.ci.came() > came);
    assert!(held, "{}", said.lock().unwrap().join("\n"));
}

/// Takes turns until the merge step is in its merge call, which stays shut.
fn held_at_the_merge_call(e: &mut Engine, s: &Setup, said: &Mutex<Vec<String>>) {
    s.merging.shut();
    let held = turns_until(e, &Go::default(), |_| s.merging.came() > 0);
    assert!(held, "{}", said.lock().unwrap().join("\n"));
}

/// Takes turns until a work is done or has ended short of it, and gives what was said.
fn done_after_merging(e: &mut Engine, work: i64, said: &Mutex<Vec<String>>) -> String {
    let done = turns_until(e, &Go::default(), |e| {
        if ended_why(e, work).is_some() {
            return true;
        }
        let w = e.work(work).unwrap();
        e.done(&w).unwrap()
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(done, "{log}");
    log
}

// A pull request that GitHub merged before the merge step began, as when the answer of an earlier merge call was lost
// (a 502 after GitHub had merged it), is recorded as merged, with the commit GitHub made. The step reads the pull
// request first: catching up with main, pushing and waiting for the full CI guard nothing in a merged pull request, and
// the merge call is refused by `gh pr ready`, as the fake GitHub refuses it. Without the read, the step makes its merge
// call on every try, is refused each time, and takes the row `not_run`; a read only in the merge call leaves the step
// to push and wait for the full CI first, which its record of the merge tool shows.
#[test]
fn a_pull_request_merged_on_github_before_the_merge_step_is_recorded_without_merging_again() {
    let s = setup("merged-before-the-merge-step");
    let (origin, root, prs) = (s.origin.clone(), s.root.clone(), s.prs.clone());
    let made = Arc::new(Mutex::new(None::<String>));
    let kept = made.clone();
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            // The second round hands in the end of the work: its branch and its pull request are on GitHub by then.
            if n == 2 {
                let pr = *prs.lock().unwrap().keys().next().unwrap();
                *kept.lock().unwrap() = Some(FakeHub::squash_merge(&origin, &root, &prs, "o/p", pr).unwrap());
            }
            implement(r, n)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let at = turns_until(&mut e, &Go::default(), |e| state(e, item).is_some_and(|w| w.1 == "M2"));
    assert!(at, "{}", said.lock().unwrap().join("\n"));
    let work = state(&e, item).unwrap().0;
    // The step ends in one round: with the row `merged`, or, when its merge calls are refused, `not_run`.
    let ended = turns_until(&mut e, &Go::default(), |e| {
        transitions_at(e, work, "M2").iter().any(Option::is_some)
    });
    let log = said.lock().unwrap().join("\n");
    assert!(ended, "{log}");
    assert_eq!(transitions_at(&e, work, "M2"), [Some("merged".to_string())], "{log}");
    let log = done_after_merging(&mut e, work, &said);
    let made = made.lock().unwrap().clone().expect("GitHub merged the pull request");
    assert_eq!(pr_states(&s), [PrState::Merged], "{log}");
    let recorded: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(output, '$.result.merge_commit') FROM rounds WHERE work = ?1 AND cell = 'M2'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(recorded, made, "{log}");
    // One look at the pull request, and nothing after it: no push, no full CI, no merge call.
    let tries: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM tool_runs WHERE work = ?1 AND tool = 'merge'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(tries, 1, "{log}");
    assert_eq!(s.merging.came(), 0, "{log}");
    assert!(!transitions_at(&e, work, "L2").is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cancel that comes while the merge step waits for the full CI stops the merge: nothing reaches GitHub's merge, the
// step takes the row `stopped`, and the loop then ends the work and closes its pull request. Without the check before
// the merge call, the merge goes through under a cancel the owner was told of.
#[test]
fn a_cancel_during_the_merge_s_full_ci_stops_the_merge() {
    let (s, mut e, said, item, work) = at_the_merge("cancel-before-merge");
    held_at_full_ci(&mut e, &s, &said);
    cancel(&mut e, item, "owner");
    s.ci.open();
    let settled = turns_until(&mut e, &Go::default(), |_| pr_states(&s) != [PrState::Open]);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(s.merging.came(), 0, "{log}");
    assert!(settled, "{log}");
    assert_eq!(pr_states(&s), [PrState::Closed], "{log}");
    assert_eq!(transitions_at(&e, work, "M2"), [Some("stopped".to_string())], "{log}");
    assert_eq!(cancelled_why(&e, item).as_deref(), Some("no longer needed"), "{log}");
    assert!(ended_why(&e, work).is_some(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// GitHub merges a pull request while the merge step waits for the full CI, outside the engine's merge call, as when the
// answer of that call was lost, so the engine's record has no merge. A cancel that comes then ends the work and queues
// the close of its pull request. GitHub refuses to close a merged pull request, so the close must find the merge on
// GitHub and close nothing: no close is called, and no row is left in the outbox to be tried again.
#[test]
fn a_cancel_after_github_merged_the_pull_request_closes_nothing() {
    let (s, mut e, said, item, work) = at_the_merge("cancel-after-github-merge");
    held_at_full_ci(&mut e, &s, &said);
    let pr = *s.prs.lock().unwrap().keys().next().unwrap();
    FakeHub::squash_merge(&s.origin, &s.root, &s.prs, "o/p", pr).unwrap();
    cancel(&mut e, item, "owner");
    s.ci.open();
    let settled = turns_until(&mut e, &Go::default(), |e| {
        ended_why(e, work).is_some() && number(e, "SELECT count(*) FROM outbox WHERE kind = 'close_pr'") == 0
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(settled, "{log}");
    assert_eq!(s.closing.came(), 0, "{log}");
    assert_eq!(pr_states(&s), [PrState::Merged], "{log}");
    assert_eq!(transitions_at(&e, work, "M2"), [Some("stopped".to_string())], "{log}");
    assert_eq!(cancelled_why(&e, item).as_deref(), Some("no longer needed"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A cancel asked by `by` while the merge call is under way: the merge came first, and stands.
fn a_cancel_during_the_merge_call(name: &str, by: &str) -> (Setup, Engine, i64) {
    let (s, mut e, said, item, work) = at_the_merge(name);
    held_at_the_merge_call(&mut e, &s, &said);
    cancel(&mut e, item, by);
    s.merging.open();
    let log = done_after_merging(&mut e, work, &said);
    assert_eq!(pr_states(&s), [PrState::Merged], "{log}");
    assert_eq!(cancelled_why(&e, item), None, "{log}");
    assert_eq!(transitions_at(&e, work, "M2"), [Some("merged".to_string())], "{log}");
    assert!(!transitions_at(&e, work, "L2").is_empty(), "{log}");
    assert_eq!(ended_why(&e, work), None, "{log}");
    assert_eq!(notices(&e, "is not cancelled"), 1, "{log}");
    let closing: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM outbox WHERE kind = 'close_pr'", [], |r| r.get(0))
            .unwrap();
    assert_eq!(closing, 0, "{log}");
    (s, e, item)
}

/// The reports the architect has that say an item is not cancelled.
fn reports_of_a_merge_first(e: &Engine, item: i64) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT count(*) FROM architect_matters
             WHERE kind = 'report' AND item = ?1 AND instr(what, 'is not cancelled') > 0",
            [item],
            |r| r.get(0),
        )
        .unwrap()
}

// A cancel that comes while the merge call is under way comes too late: the database says merged as GitHub does, the
// merged pull request is not closed, nothing says the item cancelled, the work writes what it learned as after any
// merge, and the
// owner is told. The cancel's request stands in the outbox until the merge is known, so no one was told otherwise.
#[test]
fn a_merge_under_way_outruns_the_owner_s_cancel() {
    let (s, e, item) = a_cancel_during_the_merge_call("owner-cancel-in-merge", "owner");
    assert_eq!(reports_of_a_merge_first(&e, item), 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

// So for a cancel of the architect's, which is told by a report as well.
#[test]
fn a_merge_under_way_outruns_the_architect_s_cancel_and_it_is_told() {
    let (s, e, item) = a_cancel_during_the_merge_call("architect-cancel-in-merge", "architect");
    assert_eq!(reports_of_a_merge_first(&e, item), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cancel asked of a work in the merge step keeps it at the head of the merge queue until the loop carries the cancel
// out, so that the work behind it does not begin its merge beside that step. A cancel that ended the work at once
// let the next work in while the first merge could still go through.
#[test]
fn the_work_behind_waits_while_a_cancel_of_the_merge_step_is_asked() {
    let (s, mut e, said, item, work) = at_the_merge("cancel-keeps-queue");
    held_at_full_ci(&mut e, &s, &said);
    let v = e.flow_now;
    let behind =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Add another', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let other = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, pr) VALUES (?1, ?2, 'engineer', 'M2', 9)",
                params![other, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let came = s.ci.came();
    cancel(&mut e, item, "owner");
    // A round of a row that waits is written open and then ended, by two writes of the step on its own thread. A work
    // behind that began its merge instead comes to the full CI, or pauses on a pull request GitHub does not have.
    let waits = turns_until(&mut e, &Go::default(), |e| {
        transitions_at(e, behind, "M2").iter().any(Option::is_some)
            || s.ci.came() > came
            || paused_by(e, behind).is_some()
    });
    let log = said.lock().unwrap().join("\n");
    assert!(waits, "{log}");
    assert_eq!(transitions_at(&e, behind, "M2"), [Some("behind".to_string())], "{log}");
    assert_eq!(ended_why(&e, work), None, "{log}");
    assert_eq!(s.merging.came(), 0, "{log}");
    s.ci.open();
    e.drain().unwrap();
    let _ = std::fs::remove_dir_all(&s.root);
}

// A start over that comes while the merge step waits for the full CI stops the merge, and a new work takes the item
// over, with its pull request still open.
#[test]
fn a_start_over_during_the_merge_s_full_ci_stops_the_merge() {
    let (s, mut e, said, item, work) = at_the_merge("start-over-before-merge");
    held_at_full_ci(&mut e, &s, &said);
    api::start_over(&mut e.db, work).unwrap();
    s.ci.open();
    let over = turns_until(&mut e, &Go::default(), |e| works_of(e, item).len() == 2);
    let log = said.lock().unwrap().join("\n");
    assert!(over, "{log}");
    assert_eq!(s.merging.came(), 0, "{log}");
    assert_eq!(transitions_at(&e, work, "M2"), [Some("stopped".to_string())], "{log}");
    assert_eq!(pr_states(&s), [PrState::Open], "{log}");
    e.drain().unwrap();
    let _ = std::fs::remove_dir_all(&s.root);
}

// A start over that comes while the merge call is under way comes too late: no new work takes the item over, the work
// writes what it learned as after any merge, and whoever asked is told.
#[test]
fn a_merge_under_way_outruns_a_start_over() {
    let (s, mut e, said, item, work) = at_the_merge("start-over-in-merge");
    held_at_the_merge_call(&mut e, &s, &said);
    api::start_over(&mut e.db, work).unwrap();
    s.merging.open();
    let log = done_after_merging(&mut e, work, &said);
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    assert_eq!(ended_why(&e, work), None, "{log}");
    assert_eq!(pr_states(&s), [PrState::Merged], "{log}");
    assert_eq!(notices(&e, "is not started over"), 1, "{log}");
    assert!(!transitions_at(&e, work, "L2").is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Once its merge stands on main, an item is not cancelled nor its work started over, whether the work writes what it
// learned or
// is done; whoever asks is told, and nothing waits in the outbox.
#[test]
fn an_item_whose_merge_stands_on_main_is_not_cancelled() {
    let (s, mut e, said, item, work) = at_the_merge("cancel-after-merge");
    held_at_the_merge_call(&mut e, &s, &said);
    s.merging.open();
    let learning = turns_until(&mut e, &Go::default(), |e| state(e, item).is_some_and(|w| w.1 == "L2"));
    let log = said.lock().unwrap().join("\n");
    assert!(learning, "{log}");
    cancel(&mut e, item, "owner");
    assert_eq!(cancelled_why(&e, item), None, "{log}");
    assert_eq!(stopping(&e), Vec::<i64>::new(), "{log}");
    assert_eq!(
        api::start_over(&mut e.db, work),
        Err(format!("work {work} has merged, and its merge stands on main")),
        "{log}"
    );
    let log = done_after_merging(&mut e, work, &said);
    assert_eq!(ended_why(&e, work), None, "{log}");
    cancel(&mut e, item, "owner");
    assert_eq!(cancelled_why(&e, item), None, "{log}");
    assert_eq!(pr_states(&s), [PrState::Merged], "{log}");
    assert_eq!(notices(&e, "is not cancelled"), 2, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What was asked while the engine did not run, as the command line can, is carried out at its next start: nothing of it
// is held anywhere but the database.
#[test]
fn a_cancel_and_a_resume_asked_with_no_engine_running_are_carried_out_at_its_next_start() {
    let (s, mut e, said, first, second) = two_works_with("asked-with-no-engine", scripts(), Hooks::default());
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (a, b) = (state(&e, first).expect(&log).0, state(&e, second).expect(&log).0);
    api::pause(&mut e.db, b, None).unwrap();
    drop(e);
    let projects = config::committed(&s.data).unwrap().config.projects;
    let mut base = db::Db::open(&config::database(&s.data)).unwrap();
    base.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, first, "no longer needed", "owner", &projects)
            .map(drop)
            .map_err(db::Error::Abort)
    })
    .unwrap();
    api::resume(&mut base, b).unwrap();
    drop(base);
    let (mut e, said) = engine(&s, scripts());
    e.dispatch(&Go::default()).unwrap();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(cancelled_why(&e, first).as_deref(), Some("no longer needed"), "{log}");
    assert!(ended_why(&e, a).is_some(), "{log}");
    assert_eq!(paused_by(&e, b), None, "{log}");
    let left: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM outbox WHERE kind IN ('cancel_item', 'resume_work')",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(left, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Puts a second item in the queue of a setup.
fn another_item(e: &mut Engine) -> i64 {
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level)
             VALUES ('p', 'Add another', 'x', 'y', 'change', 'l')",
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

/// Scripts whose round of `R1` waits at the gate, but for the work `free` names.
fn held_in_r1(hold: &Gate, free: &Arc<AtomicI64>) -> BTreeMap<&'static str, Script> {
    let (h, f) = (hold.clone(), free.clone());
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, _: usize| {
            if r.work != f.load(Ordering::SeqCst) {
                h.pass();
            }
            (Vec::new(), Vec::new())
        }),
    );
    agents
}

/// Takes three turns of the loop, none of which may wait for the job in flight.
fn three_turns(e: &mut Engine) {
    for _ in 0..3 {
        e.reap().unwrap();
        e.rouse();
        e.dispatch(&Go::default()).unwrap();
    }
}

/// How many times the requests to lift a pause failed.
fn resume_failures(e: &Engine) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT coalesce(sum(attempts), 0) FROM outbox WHERE kind = 'resume_work'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

// `wf-agents resume` on a machine whose one slot is taken does not run a second work on it: the pause stays, the engine
// names the work as waiting for a free slot, and waiting is no failure of the request. Once the slot is free the pause
// is lifted and the work goes on. Lifting it at once ran two works on the one slot.
#[test]
fn the_owner_s_resume_waits_for_a_free_slot() {
    let s = setup("owner-resume-slot");
    let (hold, free) = (Gate::default(), Arc::new(AtomicI64::new(0)));
    let (mut e, said, _) = start(&s, held_in_r1(&hold, &free));
    let work = e.step(None).unwrap().work.unwrap();
    free.store(work, Ordering::SeqCst);
    api::pause(&mut e.db, work, None).unwrap();
    let second = another_item(&mut e);
    hold.shut();
    let came = hold.came();
    let held = turns_until(&mut e, &Go::default(), |_| hold.came() > came);
    let log = said.lock().unwrap().join("\n");
    assert!(held, "{log}");
    let other = state(&e, second).unwrap().0;
    let rounds = rounds_after(&e, work, 0).0;
    api::resume(&mut e.db, work).unwrap();
    three_turns(&mut e);
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds, "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), [work], "{log}");
    assert_eq!(resume_failures(&e), 0, "{log}");
    api::pause(&mut e.db, other, None).unwrap();
    hold.open();
    let goes_on = turns_until(&mut e, &Go::default(), |e| rounds_after(e, work, 0).0 > rounds);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(goes_on, "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's `resume_work` on a machine whose one slot is taken waits as the owner's does, and the loop lifts the
// pause once the slot is free, with no word to the architect, which asked for it itself.
#[test]
fn the_architect_s_resume_waits_for_a_free_slot() {
    let s = setup("architect-resume-slot");
    let (hold, free) = (Gate::default(), Arc::new(AtomicI64::new(0)));
    let resuming = Arc::new(AtomicI64::new(0));
    let r = resuming.clone();
    let mut agents = held_in_r1(&hold, &free);
    agents.insert(
        "H8",
        Box::new(move |_: &Request, n: usize| {
            let kind = match (r.load(Ordering::SeqCst), n) {
                (0, 1) => "pause_work",
                (1, _) => "resume_work",
                _ => return (Vec::new(), Vec::new()),
            };
            let action = json!({ "kind": kind, "item": 1, "reason": "It waits for the parser." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [action] }))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    free.store(work, Ordering::SeqCst);
    let second = another_item(&mut e);
    hold.shut();
    let came = hold.came();
    say(&mut e, "Stop the feature until the parser is in.");
    let stopped = turns_until(&mut e, &Go::default(), |e| {
        paused_by(e, work).is_some() && hold.came() > came
    });
    let log = said.lock().unwrap().join("\n");
    assert!(stopped, "{log}");
    assert_eq!(state(&e, item).unwrap().0, work, "{log}");
    let other = state(&e, second).unwrap().0;
    let rounds = rounds_after(&e, work, 0).0;
    resuming.store(1, Ordering::SeqCst);
    say(&mut e, "The parser is in: let the feature go on.");
    let asked = turns_until(&mut e, &Go::default(), |e| {
        resumes_waiting(e) == [work] || paused_by(e, work).is_none()
    });
    let log = said.lock().unwrap().join("\n");
    assert!(asked, "{log}");
    three_turns(&mut e);
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"), "{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds, "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), [work], "{log}");
    assert_eq!(resume_failures(&e), 0, "{log}");
    api::pause(&mut e.db, other, None).unwrap();
    hold.open();
    let goes_on = turns_until(&mut e, &Go::default(), |e| rounds_after(e, work, 0).0 > rounds);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(goes_on, "{log}");
    let released: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'owner_words' AND detail = 'release'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(released, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A pause takes effect when the round under way ends, and until the loop reaps that step the work keeps its slot: no new
// work starts in it. Counting only the works not paused ran two works on the one slot until that round ended.
#[test]
fn a_work_paused_while_its_round_runs_keeps_its_slot_until_the_step_ends() {
    let s = setup("pause-keeps-slot");
    let (hold, free) = (Gate::default(), Arc::new(AtomicI64::new(0)));
    let (mut e, said, item) = start(&s, held_in_r1(&hold, &free));
    hold.shut();
    let held = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(held, "{log}");
    let work = state(&e, item).unwrap().0;
    let second = another_item(&mut e);
    api::pause(&mut e.db, work, None).unwrap();
    three_turns(&mut e);
    assert!(state(&e, second).is_none(), "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    hold.open();
    let started = turns_until(&mut e, &Go::default(), |e| state(e, second).is_some());
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(started, "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Requests that meet: a request of the outbox against another one, against the owner's next word and against a pause
// that replaces the one it was for.

// A start over and then a cancel asked of one work wait in the outbox together. The loop carries them out in the order
// they were asked, so the start over must not leave the cancel with nothing to close: a cancel closes the pull request
// of a work that ends without merging, and a new work that has not taken the pull request over yet holds none, while
// the work that held it left it to that new work.
#[test]
fn a_cancel_asked_after_a_start_over_still_closes_the_pull_request() {
    let (s, mut e, said, item, work) = at_the_merge("start-over-then-cancel");
    let worktree = worktree_of(&e, work);
    held_at_full_ci(&mut e, &s, &said);
    api::start_over(&mut e.db, work).unwrap();
    cancel(&mut e, item, "owner");
    s.ci.open();
    let cancelled = turns_until(&mut e, &Go::default(), |e| cancelled_why(e, item).is_some());
    e.drain().unwrap();
    three_turns(&mut e);
    e.drain().unwrap();
    three_turns(&mut e);
    let log = said.lock().unwrap().join("\n");
    assert!(cancelled, "{log}");
    assert_eq!(s.merging.came(), 0, "{log}");
    let works = works_of(&e, item);
    assert!(works.iter().all(|w| w.5.is_some()), "{works:?}\n{log}");
    assert_eq!(pr_states(&s), [PrState::Closed], "{works:?}\n{log}");
    assert!(!worktree.exists(), "{works:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The other order of the two: once a cancel is asked the work is stopping, a start over is refused, and the cancel is
// carried out with no new work on the item.
#[test]
fn a_start_over_is_refused_once_a_cancel_is_asked() {
    let s = setup("cancel-then-start-over");
    let (mut e, said, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    cancel(&mut e, item, "owner");
    assert_eq!(
        api::start_over(&mut e.db, work),
        Err(format!("work {work} is stopping already"))
    );
    e.dispatch(&Go::default()).unwrap();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(cancelled_why(&e, item).as_deref(), Some("no longer needed"), "{log}");
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Pause after a resume that still waits for a slot is the owner's later word, and the later word wins, as it
// does over the owner's Start: the page's Pause answers that the work stays paused. So the request must not lift the
// pause once the slot is free.
#[test]
fn the_owner_s_pause_after_a_resume_keeps_the_work_paused() {
    let (s, mut e, said, hold, work, other) = a_resume_waiting_for_a_slot("pause-after-resume");
    let rounds = rounds_after(&e, work, 0).0;
    // As the page's Pause sends it, for which a work paused already stays paused.
    let _ = api::pause(&mut e.db, work, None);
    api::pause(&mut e.db, other, None).unwrap();
    hold.open();
    e.drain().unwrap();
    three_turns(&mut e);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds, "{log}");
    assert_eq!(resumes_waiting(&e), Vec::<i64>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// So is the owner's Step, after which a work stays paused: the slot that frees later lifts nothing.
#[test]
fn the_owner_s_step_after_a_resume_keeps_the_work_paused() {
    let (s, mut e, said, hold, work, other) = a_resume_waiting_for_a_slot("step-after-resume");
    let rounds = rounds_after(&e, work, 0).0;
    let go = Go {
        steps: vec![work],
        ..Go::default()
    };
    e.dispatch(&go).unwrap();
    assert_eq!(resumes_waiting(&e), Vec::<i64>::new());
    let stepped = turns_until(&mut e, &Go::default(), |e| {
        !e.in_flight().contains(&work) && rounds_after(e, work, 0).0 > rounds
    });
    api::pause(&mut e.db, other, None).unwrap();
    hold.open();
    e.drain().unwrap();
    three_turns(&mut e);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(stepped, "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds + 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A paused work whose resume waits for a slot: the one slot is held by a second work, whose round stays in the gate.
/// Gives the gate, the work and the second work.
fn a_resume_waiting_for_a_slot(name: &str) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, Gate, i64, i64) {
    let s = setup(name);
    let (hold, free) = (Gate::default(), Arc::new(AtomicI64::new(0)));
    let (mut e, said, _) = start(&s, held_in_r1(&hold, &free));
    let work = e.step(None).unwrap().work.unwrap();
    free.store(work, Ordering::SeqCst);
    api::pause(&mut e.db, work, None).unwrap();
    let second = another_item(&mut e);
    hold.shut();
    let came = hold.came();
    let held = turns_until(&mut e, &Go::default(), |_| hold.came() > came);
    let log = said.lock().unwrap().join("\n");
    assert!(held, "{log}");
    let other = state(&e, second).unwrap().0;
    api::resume(&mut e.db, work).unwrap();
    three_turns(&mut e);
    assert_eq!(e.waiting_for_slot().unwrap(), [work], "{log}");
    (s, e, said, hold, work, other)
}

// Two resumes on a machine with one slot free: each request reads the slots as the one before it left them, so that one
// work goes on and the other waits, and a resume asked twice is one request.
#[test]
fn two_resumes_take_one_free_slot_one_at_a_time() {
    let s = setup("two-resumes-one-slot");
    let (mut e, said, _) = start(&s, scripts());
    let first = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, first, None).unwrap();
    let item = another_item(&mut e);
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let second = state(&e, item).expect(&log).0;
    api::pause(&mut e.db, second, None).unwrap();
    api::resume(&mut e.db, first).unwrap();
    api::resume(&mut e.db, first).unwrap();
    api::resume(&mut e.db, second).unwrap();
    let asked: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM outbox WHERE kind = 'resume_work'", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(asked, 2, "{log}");
    e.rouse();
    e.dispatch(&Go::default()).unwrap();
    assert_eq!(paused_by(&e, first), None, "{log}");
    assert_eq!(paused_by(&e, second).as_deref(), Some("owner"), "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), [second], "{log}");
    assert_eq!(resume_failures(&e), 0, "{log}");
    e.drain().unwrap();
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work paused while its round runs, with a resume asked for that pause, whose round then fails in a way the owner must
// look at: the engine's pause replaces the owner's, and the resume asked for the one before lifts nothing.
#[test]
fn a_pause_for_a_problem_is_not_lifted_by_a_resume_asked_before_it() {
    let s = setup("problem-after-resume");
    let hold = Gate::default();
    let h = hold.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, _: usize| {
            h.pass();
            (Vec::new(), vec![("@error", json!("Prompt is too long"))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    let work = state(&e, item).unwrap().0;
    api::pause(&mut e.db, work, None).unwrap();
    api::resume(&mut e.db, work).unwrap();
    three_turns(&mut e);
    hold.open();
    e.drain().unwrap();
    three_turns(&mut e);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let paused = state(&e, item)
        .unwrap()
        .2
        .unwrap_or_else(|| panic!("the work went on:\n{log}"));
    assert!(paused.contains("prompt_too_long"), "{paused}\n{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"), "{log}");
    assert_eq!(resumes_waiting(&e), Vec::<i64>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The same for a step that cannot be taken, which pauses its work by the other way a pause for a problem is written, as
// a work's cell `wait_for_start` and a step whose thread panicked do too: a round that finds itself ended is such a step,
// since no work ends under its step any more. The resume asked for the owner's pause lifts nothing of this one.
#[test]
fn a_pause_for_a_step_that_failed_is_not_lifted_by_a_resume_asked_before_it() {
    let s = setup("failed-step-after-resume");
    let hold = Gate::default();
    let h = hold.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, _: usize| {
            h.pass();
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    let work = state(&e, item).unwrap().0;
    api::pause(&mut e.db, work, None).unwrap();
    api::resume(&mut e.db, work).unwrap();
    three_turns(&mut e);
    // The request waits for the step in flight, and the owner's pause stands until then.
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    assert_eq!(resumes_waiting(&e), [work], "{log}");
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET ended_at = 1 WHERE work = ?1 AND ended_at IS NULL",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    hold.open();
    e.drain().unwrap();
    three_turns(&mut e);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let paused = state(&e, item)
        .unwrap()
        .2
        .unwrap_or_else(|| panic!("the work went on:\n{log}"));
    assert!(paused.starts_with("cell R1:"), "{paused}\n{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"), "{log}");
    assert_eq!(resumes_waiting(&e), Vec::<i64>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A resume that reads a work as paused while the loop lifts that pause writes its request after the lifting. The loop
// drops such a request only at a turn with no step of the work in flight, which may be a long way off, since the work
// went on; meanwhile the owner's Pause finds a work that goes on and pauses it. The request must not then lift that
// later word, which it was never asked for. The thread stands for the loop, holding its transaction open for as long as
// it takes the resume to read.
#[test]
fn a_resume_that_met_the_lifting_of_its_pause_does_not_lift_the_owner_s_next_pause() {
    let s = setup("resume-meets-lifting");
    let (mut e, said, _) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    let (lifting, begun) = std::sync::mpsc::channel();
    let path = config::database(&s.data);
    let the_loop = std::thread::spawn(move || {
        let mut base = db::Db::open(&path).unwrap();
        base.write(Actor::Owner, |tx| {
            db::lift_pause(tx, work)?;
            lifting.send(()).unwrap();
            // Long enough for a resume that reads apart from its write to read this pause as still there.
            std::thread::sleep(Duration::from_millis(100));
            Ok(())
        })
        .unwrap();
    });
    begun.recv().unwrap();
    // Whichever way it answers: the work goes on by now.
    let _ = api::resume(&mut e.db, work);
    the_loop.join().unwrap();
    assert_eq!(paused_by(&e, work), None);
    api::pause(&mut e.db, work, None).unwrap();
    e.rouse();
    e.dispatch(&Go::default()).unwrap();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The loop reads what is due in the outbox once, and carries it out in turn; an action before a request to lift a pause
// can take a while on GitHub. The owner's Pause in that time withdraws the request and answers that the work stays
// paused, so the loop must find the request gone where it lifts, or it lifts the pause the owner was just told stands.
// The thread stands for the owner's command line, at the moment the loop's turn is held at the action before.
#[test]
fn a_resume_withdrawn_during_the_loop_s_turn_lifts_nothing() {
    let s = setup("resume-withdrawn-in-turn");
    let (mut e, said, _) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, 'close_pr', ?2, 0)",
            params![work, json!({ "repo": "r", "pr": 1, "comment": "c" }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    api::resume(&mut e.db, work).unwrap();
    s.closing.shut();
    let path = config::database(&s.data);
    let closing = s.closing.clone();
    let owner = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while closing.came() == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        let answer = api::pause(&mut db::Db::open(&path).unwrap(), work, None);
        closing.open();
        answer
    });
    e.rouse();
    e.dispatch(&Go::default()).unwrap();
    let answer = owner.join().unwrap().unwrap();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(s.closing.came(), 1, "{log}");
    assert_eq!(
        answer,
        format!("work {work} stays paused: the resume asked is withdrawn"),
        "{log}"
    );
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner and then the architect cancel an item while its merge call is under way. The merge comes first, and each
// asker is told: the architect by its report, as it is when it asks alone. The two cancels are one request, so a request
// that keeps only the first asker leaves the architect thinking its cancel was carried out.
#[test]
fn a_merge_under_way_outruns_two_cancels_and_the_architect_is_told_though_it_asked_second() {
    let (s, mut e, said, item, work) = at_the_merge("two-cancels-in-merge");
    held_at_the_merge_call(&mut e, &s, &said);
    cancel(&mut e, item, "owner");
    cancel(&mut e, item, "architect");
    s.merging.open();
    let log = done_after_merging(&mut e, work, &said);
    assert_eq!(pr_states(&s), [PrState::Merged], "{log}");
    assert_eq!(cancelled_why(&e, item), None, "{log}");
    assert_eq!(reports_of_a_merge_first(&e, item), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
