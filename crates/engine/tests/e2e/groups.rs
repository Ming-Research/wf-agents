//! Side reports and matters the architect picks together as a group, and how a group comes apart.

use super::*;

/// What a matter stands as: the ask that holds it, the cell it goes back to with the answer, the matter that leads its
/// group, the cell it goes on at and the round that handled it.
type MatterState = (Option<i64>, Option<String>, Option<i64>, Option<String>, Option<i64>);

/// Whether any round of a work was sent a message with `text` in it.
fn was_sent(e: &Engine, work: i64, text: &str) -> bool {
    let mut st = e.db.conn().prepare("SELECT input FROM rounds WHERE work = ?1").unwrap();
    let inputs: Vec<String> = st
        .query_map([work], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    inputs.iter().any(|i| {
        db::put_back(e.db.conn(), &serde_json::from_str(i).unwrap())["message"]
            .as_str()
            .is_some_and(|m| m.contains(text))
    })
}

/// What the round of a cell that handed in its return closed: the matters it handled, by their numbers.
fn closed_by(e: &Engine, work: i64, cell: &str) -> Vec<i64> {
    let mut st = e
        .db
        .conn()
        .prepare(
            "SELECT m.id FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
                 WHERE m.handled_work = ?1 AND r.cell = ?2 ORDER BY m.id",
        )
        .unwrap();
    st.query_map(params![work, cell], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

// Item #228: side reports about one thing are picked together and taken in by one round of each cell. The architect
// picks one and names two more with it, finds what each is in one round and takes each in in one round, which closes
// the three; each one's engineer is told what became of its own report, the one recorded as an observation, the one
// reported once more on another, the one nothing came of, and an observation is recorded under the name of the matter
// it came of. The queue the pick is made from gives each report an excerpt of its text.
#[test]
fn side_reports_picked_together_are_found_and_taken_in_by_one_round_each() {
    let s = setup("group");
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let given = ids.clone();
    let take_in: Script = Box::new(move |_: &Request, n: usize| {
        if n > 1 {
            return (Vec::new(), Vec::new());
        }
        let ids = given.lock().unwrap();
        let record = json!({ "kind": "new", "title": "Slow parse", "what": "The parser is slow.", "tags": [],
                             "source": "Side reports.", "refs": [], "reason": "Recorded." });
        let again = json!({ "kind": "merge", "into": 1, "matter": ids[1], "seen": "Seen in the lexer.", "refs": [],
                            "reason": "The same slowness." });
        let others = json!([
            { "matter": ids[1], "way": "merged", "why": "Seen already." },
            { "matter": ids[2], "way": "nothing", "why": "Nothing to do." }
        ]);
        (
            vec![("outcome.take.way", "new_observation")],
            vec![
                ("outcome.take.why", json!("Not recorded yet.")),
                ("outcome.with", others),
                ("outcome.effects", json!({ "observation_changes": [record, again] })),
            ],
        )
    });
    let (mut e, said, reports) = reported(&s, &ids, take_in, |_| ());
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 120, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    let path = path_of(&e, arch);
    let count = |cell: &str| path.iter().filter(|c| *c == cell).count();
    assert_eq!((count("Q1"), count("H4"), count("H5")), (1, 1, 1), "{path:?}\n{log}");
    let at = path.iter().position(|c| c == "Q1").expect(&log);
    assert_eq!(
        &path[at..at + 7],
        ["Q1", "Q2", "H4", "Q0", "Q2", "H5", "B1"],
        "{path:?}\n{log}"
    );
    // The queue was given with an excerpt of each report's text, to the cell that plans and the one that picks.
    assert!(was_sent(&e, arch, "\"excerpt\""), "{log}");
    assert!(was_sent(&e, arch, "The lexer is slow on large files."), "{log}");
    // One round closed the three, the two named with the first showing it: each matter is a row of the work's handled
    // matters, and the batch counts each.
    assert_eq!(closed_by(&e, arch, "H5"), matters, "{log}");
    let led: Vec<Option<i64>> =
        e.db.conn()
            .prepare("SELECT group_lead FROM architect_matters WHERE kind = 'report' ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
    assert_eq!(led, [None, Some(matters[0]), Some(matters[0])], "{log}");
    let in_view: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM work_handled WHERE work = ?1 AND cell = 'H5' AND kind = 'report'",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(in_view, 3, "{log}");
    // The first is recorded as the observation 2 under its own name, the second reported once more on observation 1
    // under its own, and the third is nothing.
    let source: String =
        e.db.conn()
            .query_row("SELECT source FROM observations WHERE id = 2", [], |r| r.get(0))
            .unwrap();
    assert!(source.ends_with(&format!("(matter {})", matters[0])), "{source}");
    let (times, refs): (i64, String) =
        e.db.conn()
            .query_row("SELECT times_reported, refs FROM observations WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(times, 2, "{log}");
    assert!(
        refs.contains(&format!("Reported again (matter {}): Seen in the lexer.", matters[1])),
        "{refs}"
    );
    // Each engineer is told one line, of its own report only.
    let told = |work: i64| -> Vec<String> {
        e.db.conn()
            .prepare("SELECT text FROM interjections WHERE work = ?1 AND speaker = 'architect'")
            .unwrap()
            .query_map([work], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let (first, second, third) = (told(reports[0].0), told(reports[1].0), told(reports[2].0));
    assert_eq!(
        (first.len(), second.len(), third.len()),
        (1, 1, 1),
        "{first:?} {second:?} {third:?}"
    );
    assert!(first[0].contains("recorded as observation O2"), "{}", first[0]);
    assert!(!first[0].contains("once more"), "{}", first[0]);
    assert!(
        second[0].contains("reported once more on observation O1") && !second[0].contains("recorded as"),
        "{}",
        second[0]
    );
    assert!(third[0].contains("nothing to take in: Nothing to do"), "{}", third[0]);
    // The cell that takes the group in was given what the cell before found for each of the three.
    let back = message_at(&e, arch, "H5", false);
    let listed = work_from(&back).into_iter().next().unwrap_or_default();
    assert!(
        listed.contains("·H4·finding") && listed.contains("·H4·with"),
        "{listed}\n{back}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A group falls apart when the cell that takes it in asks the owner: only the matter picked waits for the answer, out
// of the queue, and the others are matters of the queue on their own again, in no group and going on at no cell.
#[test]
fn a_group_falls_apart_when_the_owner_is_asked_about_its_first_matter() {
    let s = setup("group-asks");
    let ask: Script = Box::new(|_: &Request, n: usize| {
        if n == 1 {
            return asks_the_owner("Should these reports wait for the parser?");
        }
        (Vec::new(), Vec::new())
    });
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let (mut e, said, reports) = reported(&s, &ids, ask, |_| ());
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 120, |e| path_of(e, arch).iter().any(|c| c == "A3")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    let state = |m: i64| -> MatterState {
        e.db.conn()
            .query_row(
                "SELECT asked, resume_cell, group_lead, goes_on_at, handled_round FROM architect_matters WHERE id = ?1",
                [m],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap()
    };
    let lead = state(matters[0]);
    assert!(lead.0.is_some(), "{lead:?}\n{log}");
    assert_eq!(
        (lead.1.as_deref(), lead.3.as_deref(), lead.4),
        (Some("H5"), None, None),
        "{lead:?}\n{log}"
    );
    for m in &matters[1..] {
        assert_eq!(state(*m), (None, None, None, None, None), "matter {m}\n{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// A member of a group that needs the owner is sent back by the cell that takes the group in, which answers for the rest:
// the matter picked and the other member are closed by the round and their engineers told their ways; the one sent back
// is not closed and its engineer is told nothing, and it is a matter of the queue on its own, in no group and going on
// at no cell. It is then taken alone through both cells, and what the cell that takes it in asks the owner holds it only.
#[test]
fn a_member_that_needs_the_owner_goes_back_to_the_queue_while_the_rest_of_its_group_is_taken_in() {
    let s = setup("group-back");
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let given = ids.clone();
    let take_in: Script = Box::new(move |_: &Request, n: usize| {
        if n > 1 {
            return asks_the_owner("Should the documentation link wait for the parser?");
        }
        let ids = given.lock().unwrap();
        let others = json!([
            { "matter": ids[1], "way": "nothing", "why": "Nothing to do." },
            { "matter": ids[2], "way": "back_for_owner", "why": "The owner decides about the link." }
        ]);
        (
            vec![("outcome.take.way", "nothing")],
            vec![("outcome.take.why", json!("Seen already.")), ("outcome.with", others)],
        )
    });
    let (mut e, said, reports) = reported(&s, &ids, take_in, |_| ());
    let arch = e.step(None).unwrap().work.unwrap();
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    let state = |e: &Engine, m: i64| -> MatterState {
        e.db.conn()
            .query_row(
                "SELECT asked, resume_cell, group_lead, goes_on_at, handled_round FROM architect_matters WHERE id = ?1",
                [m],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap()
    };
    let told = |e: &Engine, work: i64| -> Vec<String> {
        e.db.conn()
            .prepare("SELECT text FROM interjections WHERE work = ?1 AND speaker = 'architect'")
            .unwrap()
            .query_map([work], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    // The first round of the cell that takes the group in has handed in its return.
    let taken = |e: &mut Engine| cells_of(e, arch).iter().any(|c| c.0 == "H5" && c.1.is_some());
    assert!(steps_until(&mut e, 120, taken), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    // The matter picked and the other member are closed by that round, each with its engineer told its own way.
    assert_eq!(closed_by(&e, arch, "H5"), matters[..2], "{log}");
    let in_view: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM work_handled WHERE work = ?1 AND cell = 'H5' AND kind = 'report'",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(in_view, 2, "{log}");
    let (first, second, third) = (told(&e, reports[0].0), told(&e, reports[1].0), told(&e, reports[2].0));
    assert_eq!((first.len(), second.len()), (1, 1), "{first:?} {second:?}");
    assert!(first[0].contains("nothing to take in: Seen already"), "{}", first[0]);
    assert!(second[0].contains("nothing to take in: Nothing to do"), "{}", second[0]);
    // The one sent back is not closed, told nothing, nor in a group nor going on at a cell: a matter of the queue.
    assert!(third.is_empty(), "{third:?}\n{log}");
    assert_eq!(state(&e, matters[2]), (None, None, None, None, None), "{log}");
    // Taken alone it goes through both cells, and the owner is asked about it only.
    assert!(
        steps_until(&mut e, 160, |e| path_of(e, arch).iter().any(|c| c == "A3")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    let count = |cell: &str| path.iter().filter(|c| *c == cell).count();
    assert_eq!((count("H4"), count("H5")), (2, 2), "{path:?}\n{log}");
    let held: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(body, '$.holds') FROM owner_asks WHERE work = ?1",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(held, format!("[{}]", matters[2]), "{log}");
    let asked = state(&e, matters[2]);
    assert!(asked.0.is_some() && asked.4.is_none(), "{asked:?}\n{log}");
    assert!(told(&e, reports[2].0).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #228, from the adversarial tests. A cell that answers for one matter alone (here the answer to the owner's words)
// is sent back when an item change names another matter, with the reason, and the change is not kept under the matter
// it named: the round that follows hands it in without the name, and it is made and kept under the matter in hand.
// Reference: the schema of `architect_item_changes.matter`, which holds a matter of the table (a number out of it fails
// the key), and the design's rule that in a cell of a single matter `matter` can only be that one.
#[test]
fn a_change_that_names_another_matter_at_a_cell_of_one_matter_is_sent_back_and_not_kept() {
    let s = setup("stray-matter");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let mut change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Write the parser",
                                     "reason": "An action." });
            if n == 1 {
                change["matter"] = json!(9999);
            }
            let effects = if n > 2 {
                json!({})
            } else {
                json!({ "item_changes": [change] })
            };
            (Vec::new(), vec![("outcome.effects", effects)])
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let ok = json!({ "change": 1, "verdict": "ok", "why": "Sound." });
            (Vec::new(), vec![("outcome.review", json!([ok]))])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Name the parser's item.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H8' AND rejected IS NOT NULL ORDER BY n",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 1, "{rejected:?}\n{log}");
    assert!(
        rejected[0].contains("item change 1 comes of matter 9999, which is not the matter in hand"),
        "{}",
        rejected[0]
    );
    let kept: Vec<(i64, String, String)> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT matter, state, change FROM architect_item_changes ORDER BY id")
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(kept.len(), 1, "{kept:?}\n{log}");
    assert_ne!(kept[0].0, 9999, "{kept:?}");
    assert_eq!(kept[0].1, "made", "{kept:?}");
    assert!(!kept[0].2.contains("matter"), "{kept:?}");
    let title: String =
        e.db.conn()
            .query_row("SELECT title FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(title, "Write the parser", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #228, from the adversarial tests. Two members of a group sent back for the owner in one round: the matter picked
// is closed alone by that round and its engineer told, and each of the two is taken later on its own, by a round of its
// own, closed there and its engineer told once. Reference: acceptance condition A11 (a member sent back is not closed or
// answered by the round, and is taken from the queue as any matter) and the design's one rule that a matter outside the
// group is neither closed nor answered.
#[test]
fn two_members_sent_back_for_the_owner_are_each_taken_later_on_their_own() {
    let s = setup("group-back-two");
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let given = ids.clone();
    let take_in: Script = Box::new(move |_: &Request, n: usize| {
        if n > 1 {
            return (Vec::new(), Vec::new());
        }
        let ids = given.lock().unwrap();
        let others = json!([
            { "matter": ids[1], "way": "back_for_owner", "why": "The owner decides." },
            { "matter": ids[2], "way": "back_for_owner", "why": "The owner decides too." }
        ]);
        (
            vec![("outcome.take.way", "nothing")],
            vec![("outcome.take.why", json!("Lead why.")), ("outcome.with", others)],
        )
    });
    let (mut e, said, reports, arch) = group_found(&s, &ids, take_in, |_| ());
    // The three are one group before the round that takes it in sends two back.
    let led = |e: &Engine| -> Vec<Option<i64>> {
        e.db.conn()
            .prepare("SELECT group_lead FROM architect_matters WHERE kind = 'report' ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let first = reports[0].1;
    assert_eq!(led(&e), [None, Some(first), Some(first)]);
    assert!(
        steps_until(&mut e, 200, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    let round_of = |m: i64| -> i64 {
        e.db.conn()
            .query_row("SELECT handled_round FROM architect_matters WHERE id = ?1", [m], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .unwrap()
            .unwrap_or_else(|| panic!("matter {m} is not handled\n{log}"))
    };
    let rounds = (round_of(matters[0]), round_of(matters[1]), round_of(matters[2]));
    assert!(
        rounds.0 < rounds.1 && rounds.0 < rounds.2 && rounds.1 != rounds.2,
        "{rounds:?}\n{log}"
    );
    // The two sent back left the group, and none is left in one.
    assert_eq!(led(&e), [None, None, None], "{log}");
    // The matter picked is closed alone by the round that took the group in.
    assert_eq!(
        closed_by(&e, arch, "H5")
            .into_iter()
            .filter(|m| round_of(*m) == rounds.0)
            .collect::<Vec<_>>(),
        &matters[..1],
        "{log}"
    );
    // Each engineer is told once, and the matter picked its own way.
    for (i, (work, _)) in reports.iter().enumerate() {
        assert_eq!(lines_told(&e, *work).len(), 1, "report {i}\n{log}");
    }
    assert!(
        lines_told(&e, reports[0].0)[0].contains("nothing to take in: Lead why"),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #228, from the adversarial tests. The one return of the cell that takes a group in is sent back with every
// problem of the new way at once, together with a miss of A2: the matter in hand given the way `back_for_owner`, a member
// left out, and an item change that comes of a member sent back; the round sent back releases no member, so the sound
// round that follows takes the whole group in. Reference: acceptance condition A12 (one send-back lists them all with
// the A2 misses), and the rule of #216 that a check reports every problem it can find at once.
#[test]
fn a_return_that_misuses_back_for_the_owner_is_sent_back_with_all_its_problems_and_releases_no_member() {
    let s = setup("group-back-bad");
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let given = ids.clone();
    let take_in: Script = Box::new(move |_: &Request, n: usize| {
        let ids = given.lock().unwrap();
        if n == 1 {
            let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Write the parser",
                                 "reason": "r", "matter": ids[1] });
            let others = json!([{ "matter": ids[1], "way": "back_for_owner", "why": "The owner decides." }]);
            return (
                vec![("outcome.take.way", "back_for_owner")],
                vec![
                    ("outcome.take.why", json!("Lead why.")),
                    ("outcome.with", others),
                    ("outcome.effects", json!({ "item_changes": [change] })),
                ],
            );
        }
        if n > 2 {
            return (Vec::new(), Vec::new());
        }
        let others = json!([
            { "matter": ids[1], "way": "nothing", "why": "Second why." },
            { "matter": ids[2], "way": "nothing", "why": "Third why." }
        ]);
        (
            vec![("outcome.take.way", "nothing")],
            vec![("outcome.take.why", json!("Lead why.")), ("outcome.with", others)],
        )
    });
    let (mut e, said, reports, arch) = group_found(&s, &ids, take_in, |_| ());
    assert!(
        steps_until(&mut e, 160, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H5' AND rejected IS NOT NULL ORDER BY n",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 1, "{rejected:?}\n{log}");
    for part in [
        "take has the way back_for_owner",
        &format!("with has no entry for matter {}", matters[2]),
        &format!("item change 1 comes of matter {}", matters[1]),
    ] {
        assert!(rejected[0].contains(part), "{part}\n{}", rejected[0]);
    }
    // The sound round took the whole group in at once.
    assert_eq!(closed_by(&e, arch, "H5"), matters, "{log}");
    for (i, (work, _)) in reports.iter().enumerate() {
        assert_eq!(lines_told(&e, *work).len(), 1, "report {i}\n{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// The group does not fall apart between the cell that finds what its matters are and the cell that takes them in,
// though the owner speaks in between and the architect's session starts anew: the owner's words are answered first,
// then the same group is taken in, by the cell given what was found for each matter of it.
#[test]
fn a_group_goes_on_to_its_second_cell_after_the_owner_s_words_and_a_new_session() {
    let s = setup("group-between");
    understanding_each_session(&s);
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let given = ids.clone();
    let take_in: Script = Box::new(move |_: &Request, n: usize| {
        if n > 1 {
            return (Vec::new(), Vec::new());
        }
        let ids = given.lock().unwrap();
        let others: Vec<Value> = ids[1..]
            .iter()
            .map(|m| json!({ "matter": m, "way": "nothing", "why": "Nothing to do." }))
            .collect();
        (
            vec![("outcome.take.way", "nothing")],
            vec![("outcome.with", json!(others))],
        )
    });
    let (mut e, said, reports) = reported(&s, &ids, take_in, |agents| {
        agents.insert(
            "H8",
            Box::new(|_: &Request, _: usize| {
                (
                    vec![("outcome.answer", "done")],
                    vec![
                        ("outcome.answer.reply", json!("I see.")),
                        ("outcome.answer.why", json!("Nothing to change.")),
                    ],
                )
            }),
        );
    });
    let arch = e.step(None).unwrap().work.unwrap();
    let found = |e: &mut Engine| path_of(e, arch).last().map(String::as_str) == Some("H4");
    assert!(steps_until(&mut e, 120, found), "{}", said.lock().unwrap().join("\n"));
    // The matter picked goes on at its second cell; the others wait for it, picked with it.
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    let on: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT goes_on_at FROM architect_matters WHERE id = ?1",
                [matters[0]],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(on.as_deref(), Some("H5"));
    age_architect(&mut e, 90 * 60_000);
    say(&mut e, "Carry on.");
    assert!(
        steps_until(&mut e, 160, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    let count = |cell: &str| path.iter().filter(|c| *c == cell).count();
    assert_eq!((count("H4"), count("H5")), (1, 1), "{path:?}\n{log}");
    // The first round at the owner's words is the one that finds the session would start anew, which sends the work back
    // to understand the project; the second answers the words. The group is taken in after both.
    assert_eq!(rows_at(&e, arch, "H8"), ["session_anew", "done"], "{path:?}\n{log}");
    let at = |cell: &str| path.iter().rposition(|c| c == cell).expect(&log);
    assert!(
        at("H4") < at("S1") && at("S1") < at("H8") && at("H8") < at("H5"),
        "{path:?}\n{log}"
    );
    assert_eq!(closed_by(&e, arch, "H5"), matters, "{log}");
    let back = message_at(&e, arch, "H5", false);
    let listed = work_from(&back).into_iter().next().unwrap_or_default();
    assert!(
        listed.contains("·H4·finding") && listed.contains("·H4·with"),
        "{listed}\n{back}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What [`group_found`] gives: the engine, what it said, the work and the matter of each side report, and the work of
/// the architect.
type GroupFound = (Engine, Arc<Mutex<Vec<String>>>, Vec<(i64, i64)>, i64);

/// The architect's work over a group of three side reports, run until the cell that finds what they are has handed in
/// its return and the group goes on to the cell that takes it in: the engine, what it said, the matters of the three
/// reports with the works that handed them in, and the architect's work.
fn group_found(s: &Setup, ids: &Arc<Mutex<Vec<i64>>>, take_in: Script, on: impl FnOnce(&mut Engine)) -> GroupFound {
    let (mut e, said, reports) = reported(s, ids, take_in, |agents| {
        agents.insert(
            "B2",
            Box::new(|_: &Request, _: usize| {
                let ok = json!({ "change": 1, "verdict": "ok", "why": "Sound." });
                (Vec::new(), vec![("outcome.review", json!([ok]))])
            }),
        );
    });
    on(&mut e);
    let arch = e.step(None).unwrap().work.unwrap();
    let found = |e: &mut Engine| path_of(e, arch).last().map(String::as_str) == Some("H4");
    assert!(steps_until(&mut e, 120, found), "{}", said.lock().unwrap().join("\n"));
    (e, said, reports, arch)
}

// Item #228, from the adversarial tests. An item change in the return that takes a group in names the matter of the
// group it comes of: it is told to the engineer of that matter once the batch is made, the engineers of the matters that
// made no item change are told at once, each its own way, and no engineer is told twice. Reference: the change is the
// architect's own statement of which matter it comes of (acceptance condition A3: each reporter is told its own way,
// also after the batch).
#[test]
fn an_item_change_that_names_a_matter_of_the_group_is_told_to_the_engineer_of_that_matter_after_the_batch() {
    let s = setup("group-batch");
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let given = ids.clone();
    let take_in: Script = Box::new(move |_: &Request, n: usize| {
        if n > 1 {
            return (Vec::new(), Vec::new());
        }
        let ids = given.lock().unwrap();
        let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Write the parser",
                             "reason": "The second report asks for it.", "matter": ids[1] });
        let others = json!([
            { "matter": ids[1], "way": "nothing", "why": "Second why." },
            { "matter": ids[2], "way": "nothing", "why": "Third why." }
        ]);
        (
            vec![("outcome.take.way", "nothing")],
            vec![
                ("outcome.take.why", json!("Lead why.")),
                ("outcome.with", others),
                ("outcome.effects", json!({ "item_changes": [change] })),
            ],
        )
    });
    let (mut e, said, reports, arch) = group_found(&s, &ids, take_in, |_| ());
    assert!(
        steps_until(&mut e, 160, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    assert_eq!(closed_by(&e, arch, "H5"), matters, "{log}");
    // The change is the second matter's own, and was made.
    let (of, state): (i64, String) =
        e.db.conn()
            .query_row("SELECT matter, state FROM architect_item_changes", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!((of, state.as_str()), (matters[1], "made"), "{log}");
    let title: String =
        e.db.conn()
            .query_row("SELECT title FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(title, "Write the parser", "{log}");
    // One line to each engineer, of its own matter only.
    let (first, second, third) = (
        lines_told(&e, reports[0].0),
        lines_told(&e, reports[1].0),
        lines_told(&e, reports[2].0),
    );
    assert_eq!(
        (first.len(), second.len(), third.len()),
        (1, 1, 1),
        "{first:?} {second:?} {third:?}\n{log}"
    );
    assert!(
        first[0].contains("nothing to take in: Lead why") && !first[0].contains("item #1"),
        "{}",
        first[0]
    );
    assert!(
        second[0].contains("taken into item #1 \"Write the parser\"") && !second[0].contains("nothing to take in"),
        "{}",
        second[0]
    );
    assert!(
        third[0].contains("nothing to take in: Third why") && !third[0].contains("item #1"),
        "{}",
        third[0]
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #228, from the adversarial tests. A matter of a group withdrawn after the cell that finds what the matters are
// and before the cell that takes them in is not asked of that cell and is not closed by it: the cell is accepted with
// an entry for each matter left, and the withdrawn one stays unhandled with its engineer told nothing. Reference:
// acceptance condition A2 (the entries asked for are those of the matters in the group) and the queue's own rule that a
// withdrawn matter is no longer a matter.
#[test]
fn a_matter_withdrawn_between_the_cells_is_left_out_of_the_group_and_not_closed() {
    let s = setup("group-withdrawn");
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let given = ids.clone();
    let take_in: Script = Box::new(move |_: &Request, n: usize| {
        if n > 1 {
            return (Vec::new(), Vec::new());
        }
        let ids = given.lock().unwrap();
        let others = json!([{ "matter": ids[1], "way": "nothing", "why": "Second why." }]);
        (
            vec![("outcome.take.way", "nothing")],
            vec![("outcome.take.why", json!("Lead why.")), ("outcome.with", others)],
        )
    });
    let (mut e, said, reports, arch) = group_found(&s, &ids, take_in, |_| ());
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    // The other two were picked with the first, and are its group, before one is withdrawn.
    let led: Vec<Option<i64>> =
        e.db.conn()
            .prepare("SELECT group_lead FROM architect_matters WHERE kind = 'report' ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
    assert_eq!(led, [None, Some(matters[0]), Some(matters[0])]);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET withdrawn = 'Handed back.' WHERE id = ?1",
            [matters[2]],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(
        steps_until(&mut e, 160, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'H5' AND rejected IS NOT NULL",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(sent_back, 0, "{log}");
    assert_eq!(closed_by(&e, arch, "H5"), &matters[..2], "{log}");
    let handled: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT handled_round FROM architect_matters WHERE id = ?1",
                [matters[2]],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(handled, None, "{log}");
    assert_eq!(lines_told(&e, reports[2].0), Vec::<String>::new(), "{log}");
    assert_eq!(lines_told(&e, reports[0].0).len(), 1, "{log}");
    assert_eq!(lines_told(&e, reports[1].0).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #228, from the adversarial tests. The pick clears a group a work that ended left unhandled, so that its matters
// do not join the group picked now, even one that names the matter picked as its lead; the matters picked with it
// become its group. Reference: acceptance condition A1 (the pick writes the group's lead and clears the groups left)
// and the schema's definition of a group as the lead and the matters not handled that name it.
#[test]
fn the_pick_clears_a_group_left_unhandled_and_leaves_it_out_of_the_group_picked() {
    let s = setup("group-stale");
    let ids = Arc::new(Mutex::new(Vec::<i64>::new()));
    let stale = Arc::new(Mutex::new(Vec::<i64>::new()));
    let left = stale.clone();
    let named = ids.clone();
    let nothing: Script = Box::new(|_: &Request, _: usize| (Vec::new(), Vec::new()));
    let (e, said, reports, arch) = group_found(&s, &ids, nothing, move |e| {
        // Two more side reports, left in a group by a work that ended: the one led by the matter the next pick makes
        // its lead, the other led by the first.
        let first = side_report(e, "The parser lacks a test.").1;
        let second = side_report(e, "The parser lacks another test.").1;
        let lead = named.lock().unwrap()[0];
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE architect_matters SET group_lead = ?2 WHERE id = ?1",
                [first, lead],
            )?;
            tx.execute(
                "UPDATE architect_matters SET group_lead = ?2 WHERE id = ?1",
                [second, first],
            )?;
            Ok(())
        })
        .unwrap();
        *left.lock().unwrap() = vec![first, second];
    });
    let log = said.lock().unwrap().join("\n");
    let matters: Vec<i64> = reports.iter().map(|r| r.1).collect();
    let led = |m: i64| -> Option<i64> {
        e.db.conn()
            .query_row("SELECT group_lead FROM architect_matters WHERE id = ?1", [m], |r| {
                r.get(0)
            })
            .unwrap()
    };
    let stale = stale.lock().unwrap().clone();
    assert_eq!((led(stale[0]), led(stale[1])), (None, None), "{log}");
    assert_eq!(
        (led(matters[1]), led(matters[2])),
        (Some(matters[0]), Some(matters[0])),
        "{log}"
    );
    // The cell that found what the matters are was asked about the three picked, and no more: it was not sent back.
    let sent_back: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'H4' AND rejected IS NOT NULL",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(sent_back, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
