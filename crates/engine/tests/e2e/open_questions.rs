//! A design's open questions: to the architect or pinned for the owner, and how they hold the merge.

use super::*;

// The ways ask the owner only about an open question: an engineer waits for the reply to its question, and the ask of the
// owner is sent back, nothing is pinned, and the reply that follows reaches the engineer.
#[test]
fn an_engineer_s_question_cannot_be_pinned_for_the_owner() {
    let s = setup("pin-engineer-question");
    let ways: Script = Box::new(|_: &Request, n: usize| {
        if n == 1 {
            return asks_the_owner("Is a 3% slowdown acceptable? See [the design](/works/1?round=R1-1).");
        }
        let reply = json!({
            "text": "Put the slowdown to the owner as an open question of your design, and go on.",
            "why": "Accepting a slowdown is the owner's choice.",
        });
        (Vec::new(), vec![("outcome.reply", reply)])
    });
    let ask = asks_about_a_gap("The change makes parsing 3% slower, measured over 30 pairs.");
    let (e, said, _, work, arch) = a_gap_asked(&s, ask, vec![("H3", ways)]);
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "H3");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    assert!(sent_back[0].contains("cannot be parked for the owner"), "{sent_back:?}");
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks", [], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0, "{log}");
    assert_eq!(question_took(&e, work).as_deref(), Some("answered"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
