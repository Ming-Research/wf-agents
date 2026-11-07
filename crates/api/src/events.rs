//! The events a committed write makes: each row it touched becomes the thing the page reads again.

use db::Changed;
use db::rusqlite::Connection;

use crate::Event;

fn one(conn: &Connection, sql: &str, rowid: i64) -> Option<i64> {
    conn.query_row(sql, [rowid], |r| r.get(0)).ok()
}

/// The round a row names by its work and number, as an event.
fn round(conn: &Connection, sql: &str, rowid: i64) -> Option<Event> {
    conn.query_row(sql, [rowid], |r| {
        Ok(Event::Round {
            work: r.get(0)?,
            round: r.get(1)?,
        })
    })
    .ok()
}

/// The event of a message, found by `sql` from a row's rowid: the work's, for what is said to one of its lines; the
/// owner's message's, for any other.
fn said(conn: &Connection, sql: &str, rowid: i64) -> Option<Event> {
    let message = one(conn, sql, rowid)?;
    conn.query_row(
        "SELECT m.work, EXISTS (SELECT 1 FROM deliveries d WHERE d.message = m.id AND d.recipient = 'line')
         FROM messages m WHERE m.id = ?1",
        [message],
        |r| {
            let work: Option<i64> = r.get(0)?;
            let to_line: bool = r.get(1)?;
            Ok(match (to_line, work) {
                (true, Some(work)) => Event::Work { work },
                _ => Event::OwnerMessage { message, work },
            })
        },
    )
    .ok()
}

/// Whether a work is a work of the architect, whose tab shows it.
fn is_architect_work(conn: &Connection, work: i64) -> bool {
    conn.query_row(
        "SELECT graph IN ('architect', 'policy') FROM works WHERE id = ?1",
        [work],
        |r| r.get(0),
    )
    .unwrap_or(false)
}

/// The events for the rows a write touched, each once, in the order first touched. Bookkeeping tables (`changes`,
/// `outbox`) and rows the page does not show (`sessions`, `known_red_checks`) make none. A round is known by its work
/// and number; the row SQLite tells of is found by its rowid, which nothing else reads. What the architect's tab shows
/// (its matters and the entries of its talks, what stops new works, the item changes its batches made, and a work of it
/// with its rounds, asks and what it told the owner) makes the architect's event too; an observation makes the
/// observations' event.
pub fn events(conn: &Connection, rows: &[Changed]) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::new();
    for c in rows {
        let id = c.rowid;
        let event = match c.table.as_str() {
            "rounds" => round(conn, "SELECT work, n FROM rounds WHERE rowid = ?1", id),
            "tool_runs" => round(conn, "SELECT work, round FROM tool_runs WHERE id = ?1", id),
            "guard_events" => round(conn, "SELECT work, round FROM guard_events WHERE id = ?1", id),
            "works" => Some(Event::Work { work: id }),
            "design_cards" => Some(Event::DesignCard {
                design_card: id,
                work: one(conn, "SELECT work FROM design_cards WHERE id = ?1", id),
            }),
            "owner_asks" => Some(Event::Ask {
                ask: id,
                work: one(conn, "SELECT work FROM owner_asks WHERE id = ?1", id),
            }),
            // What is said to a work's line is the work's; any other message is the owner's to read.
            "messages" => said(conn, "SELECT id FROM messages WHERE id = ?1", id),
            // A message read, passed on, or taken by a round.
            "deliveries" => said(conn, "SELECT message FROM deliveries WHERE id = ?1", id),
            // A cache miss added to a message changes what the message holds; one the engine expects is in no message.
            "cache_misses" => conn
                .query_row(
                    "SELECT message, work FROM cache_misses WHERE rowid = ?1 AND message IS NOT NULL",
                    [id],
                    |r| {
                        Ok(Event::OwnerMessage {
                            message: r.get(0)?,
                            work: r.get(1)?,
                        })
                    },
                )
                .ok(),
            "items" => Some(Event::Item { item: Some(id) }),
            // The owner opening an item marks it seen; the items list the page holds shows the mark.
            "owner_seen" => Some(Event::Item { item: Some(id) }),
            "item_links" => Some(Event::Item {
                item: one(conn, "SELECT item FROM item_links WHERE rowid = ?1", id),
            }),
            "flow_versions" => Some(Event::Flow { version: id }),
            "architect_matters" | "dispatch_holds" | "architect_item_changes" => Some(Event::Architect),
            "observations" => Some(Event::Observations),
            _ => None,
        };
        let of = match &event {
            Some(Event::Work { work } | Event::Round { work, .. }) => Some(*work),
            Some(Event::Ask { work, .. } | Event::OwnerMessage { work, .. }) => *work,
            _ => None,
        };
        let also = of
            .is_some_and(|w| is_architect_work(conn, w))
            .then_some(Event::Architect);
        for e in event.into_iter().chain(also) {
            if !out.contains(&e) {
                out.push(e);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use db::{Actor, Db};

    use super::*;

    #[test]
    fn each_row_a_write_touched_names_what_to_read_again() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Owner, |tx| {
                tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
                    [],
                )?;
                tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'S0')", [])?;
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 3, 'S0', '{}', 'c', 1)",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO tool_runs (work, round, tool, args, attempt, queued_at) VALUES (1, 3, 'gh', '{}', 1, 1)",
                    [],
                )?;
                tx.execute("INSERT INTO owner_asks (work, round, body) VALUES (1, 3, '{}')", [])?;
                Ok(())
            })
            .unwrap();
        let touched = |t: &str, rowid: i64| Changed {
            table: t.to_string(),
            rowid,
        };
        let rows = [
            touched("changes", 1),
            touched("rounds", 1),
            touched("tool_runs", 1),
            touched("works", 1),
            touched("owner_asks", 1),
            touched("items", 1),
            touched("flow_versions", 1),
            touched("outbox", 1),
        ];
        assert_eq!(
            events(db.conn(), &rows),
            [
                Event::Round { work: 1, round: 3 },
                Event::Work { work: 1 },
                Event::Ask { ask: 1, work: Some(1) },
                Event::Item { item: Some(1) },
                Event::Flow { version: 1 },
            ]
        );
    }

    // A message sent, and one of its deliveries marked read or passed on, name the message and its work, once.
    #[test]
    fn a_message_and_its_deliveries_name_the_message() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'S0')",
                [],
            )?;
            let m = db::Message {
                sender: db::Sender::Engine,
                work: Some(1),
                round: None,
                text: "Work 1 stopped.",
                body: None,
                ask: None,
            };
            db::send(tx, &m, db::Recipient::Owner)?;
            Ok(())
        })
        .unwrap();
        let touched = |t: &str, rowid: i64| Changed {
            table: t.to_string(),
            rowid,
        };
        assert_eq!(
            events(db.conn(), &[touched("messages", 1), touched("deliveries", 1)]),
            [Event::OwnerMessage {
                message: 1,
                work: Some(1)
            }]
        );
    }

    // A round of the architect's work names the work and the architect's tab; the architect's matters and what stops new
    // works name the tab alone, once; an observation names the observations.
    #[test]
    fn what_the_architect_tab_shows_names_it_too() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (NULL, 1, 'architect', 'S1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'S1', '{}', 'c', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let touched = |t: &str, rowid: i64| Changed {
            table: t.to_string(),
            rowid,
        };
        let rows = [
            touched("rounds", 1),
            touched("architect_matters", 1),
            touched("dispatch_holds", 1),
            touched("architect_matters", 2),
            touched("observations", 1),
        ];
        assert_eq!(
            events(db.conn(), &rows),
            [
                Event::Round { work: 1, round: 1 },
                Event::Architect,
                Event::Observations
            ]
        );
    }
}
