//! The events a committed write makes: each row it touched becomes the thing the page reads again.

use db::Changed;
use db::rusqlite::Connection;

use crate::Event;

fn one(conn: &Connection, sql: &str, rowid: i64) -> Option<i64> {
    conn.query_row(sql, [rowid], |r| r.get(0)).ok()
}

/// The events for the rows a write touched, each once, in the order first touched. Bookkeeping tables (`changes`,
/// `outbox`) and rows the page does not show (`sessions`, `known_red_checks`) make none.
pub fn events(conn: &Connection, rows: &[Changed]) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::new();
    for c in rows {
        let id = c.rowid;
        let event = match c.table.as_str() {
            "rounds" => {
                one(conn, "SELECT work FROM rounds WHERE id = ?1", id).map(|work| Event::Round { work, round: id })
            }
            "tool_runs" => conn
                .query_row(
                    "SELECT r.work, r.id FROM tool_runs t JOIN rounds r ON r.id = t.round WHERE t.id = ?1",
                    [id],
                    |r| {
                        Ok(Event::Round {
                            work: r.get(0)?,
                            round: r.get(1)?,
                        })
                    },
                )
                .ok(),
            "works" => Some(Event::Work { work: id }),
            "interjections" => {
                one(conn, "SELECT work FROM interjections WHERE id = ?1", id).map(|work| Event::Work { work })
            }
            "cards" => Some(Event::Card {
                card: id,
                work: one(
                    conn,
                    "SELECT r.work FROM cards c JOIN rounds r ON r.id = c.round WHERE c.id = ?1",
                    id,
                ),
            }),
            // The queue's key is the item.
            "items" | "dispatch_queue" => Some(Event::Item { item: Some(id) }),
            "item_links" => Some(Event::Item {
                item: one(conn, "SELECT item FROM item_links WHERE rowid = ?1", id),
            }),
            "flow_versions" => Some(Event::Flow { version: id }),
            _ => None,
        };
        if let Some(e) = event
            && !out.contains(&e)
        {
            out.push(e);
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
                tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '0')", [])?;
                tx.execute(
                    "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (1, '0', '{}', 'c', 1)",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO tool_runs (round, tool, args, attempt, queued_at) VALUES (1, 'gh', '{}', 1, 1)",
                    [],
                )?;
                tx.execute("INSERT INTO cards (round, kind, body, status) VALUES (1, 'ask', '{}', 'open')", [])?;
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
            touched("cards", 1),
            touched("items", 1),
            touched("dispatch_queue", 1),
            touched("flow_versions", 1),
            touched("outbox", 1),
        ];
        assert_eq!(
            events(db.conn(), &rows),
            [
                Event::Round { work: 1, round: 1 },
                Event::Work { work: 1 },
                Event::Card { card: 1, work: Some(1) },
                Event::Item { item: Some(1) },
                Event::Flow { version: 1 },
            ]
        );
    }
}
