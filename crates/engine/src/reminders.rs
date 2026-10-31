//! The reminder of the asks waiting for the owner's answer. An ask of the owner has no deadline and no answer the
//! engine gives in the owner's place: while any waits, the engine reminds the owner once an interval of the
//! installation's setting (`owner.remind_interval`), in one notice saying how many wait and how long the oldest has
//! waited, not one for each ask.

use db::rusqlite::{Connection, OptionalExtension};

/// The reminder due at `now`, reminders being `interval_ms` apart, and none when the interval is 0: due when an ask
/// waits for the owner's answer, as the owner's conversations count what waits, the oldest has waited an interval
/// since the owner was told of it, and the last reminder is an interval old or there was none.
pub fn due(conn: &Connection, now: i64, interval_ms: i64) -> Result<Option<String>, String> {
    if interval_ms <= 0 {
        return Ok(None);
    }
    let last: Option<i64> = conn
        .query_row("SELECT max(at) FROM engine_notices WHERE reminder = 1", [], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?;
    if last.is_some_and(|t| now - t < interval_ms) {
        return Ok(None);
    }
    let oldest: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT id, at, count(*) OVER () FROM conversation_entries WHERE source = 'ask' AND waiting
             ORDER BY at, id LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(oldest
        .filter(|(_, since, _)| now - since >= interval_ms)
        .map(|(ask, since, n)| text(n, ask, now - since)))
}

/// What the reminder says: how many asks wait for the owner's answer, and the oldest by its number with how long it
/// has waited.
pub fn text(n: i64, oldest: i64, waited_ms: i64) -> String {
    let waited = duration(waited_ms);
    if n == 1 {
        format!("Ask {oldest} waits for your answer; it has waited {waited}.")
    } else {
        format!("{n} asks wait for your answer; the oldest, ask {oldest}, has waited {waited}.")
    }
}

/// A duration in its two largest units, as the owner's page writes one: 2d 03h, 2h 05m, 3m 07s, 41s.
fn duration(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (d, h, m, sec) = (s / 86_400, s % 86_400 / 3_600, s % 3_600 / 60, s % 60);
    if d > 0 {
        format!("{d}d {h:02}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {sec:02}s")
    } else {
        format!("{sec}s")
    }
}

#[cfg(test)]
mod tests {
    use db::Actor;
    use db::rusqlite::params;

    use super::*;

    const HOUR: i64 = 3_600_000;
    const MINUTE: i64 = 60_000;

    /// A database with two works of the architect, each with an ask of the owner put at the start of its round, `at`
    /// and `later`, which parks a matter of the architect's, as its every question does; gives it with the asks.
    fn asks(at: i64, later: i64) -> (db::Db, Vec<i64>) {
        let mut db = db::Db::in_memory().unwrap();
        let asks = db
            .write(Actor::Engine, |tx| {
                tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
                let mut asks = Vec::new();
                for t in [at, later] {
                    tx.execute(
                        "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'P3', 'p')",
                        [],
                    )?;
                    let work = tx.last_insert_rowid();
                    tx.execute(
                        "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, 1, 'P3', '{}', 'c', ?2)",
                        params![work, t],
                    )?;
                    tx.execute(
                        "INSERT INTO owner_asks (work, round, body) VALUES (?1, 1, '{}')",
                        [work],
                    )?;
                    let ask = tx.last_insert_rowid();
                    tx.execute(
                        "INSERT INTO architect_matters (project, kind, what, body, at, asked, resume_cell)
                         VALUES ('p', 'check_policy', 'Check the policy', '{}', ?1, ?2, 'P1')",
                        params![t, ask],
                    )?;
                    asks.push(ask);
                }
                Ok(asks)
            })
            .unwrap();
        (db, asks)
    }

    /// Reminds at `now` when a reminder is due, as the engine does; gives what it said.
    fn remind(db: &mut db::Db, now: i64, interval: i64) -> Option<String> {
        let text = due(db.conn(), now, interval).unwrap()?;
        db.write(Actor::Engine, |tx| {
            crate::actions::remind(tx, now, &text).map_err(db::Error::Abort)
        })
        .unwrap();
        Some(text)
    }

    // While asks wait, the owner is reminded once an hour, an hour after the oldest was put, in one notice for all of
    // them; an answered ask counts no longer, and once none waits nothing is said, until one has waited an hour again.
    #[test]
    fn the_owner_is_reminded_once_an_hour_while_an_ask_waits() {
        let t0 = 1_000 * HOUR;
        let (mut db, asks) = asks(t0, t0 + 30 * MINUTE);
        assert_eq!(remind(&mut db, t0 + 59 * MINUTE, HOUR), None);
        assert_eq!(
            remind(&mut db, t0 + HOUR, HOUR).as_deref(),
            Some("2 asks wait for your answer; the oldest, ask 1, has waited 1h 00m.")
        );
        // Within the hour after a reminder, none comes, however many times the engine looks.
        for m in [1, 30, 59] {
            assert_eq!(remind(&mut db, t0 + HOUR + m * MINUTE, HOUR), None);
        }
        assert_eq!(
            remind(&mut db, t0 + 2 * HOUR + 5 * MINUTE, HOUR).as_deref(),
            Some("2 asks wait for your answer; the oldest, ask 1, has waited 2h 05m.")
        );
        db.write(Actor::Owner, |tx| {
            tx.execute(
                "UPDATE owner_asks SET answer = '{\"by\": \"owner\", \"text\": \"Yes.\"}' WHERE id = ?1",
                [asks[0]],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            remind(&mut db, t0 + 3 * HOUR + 5 * MINUTE, HOUR).as_deref(),
            Some("Ask 2 waits for your answer; it has waited 2h 35m.")
        );
        db.write(Actor::Owner, |tx| {
            tx.execute(
                "UPDATE owner_asks SET answer = '{\"by\": \"owner\", \"text\": \"No.\"}' WHERE id = ?1",
                [asks[1]],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(remind(&mut db, t0 + 5 * HOUR, HOUR), None);
        // Each reminder is one notice about no work, and goes to the notifications log.
        let (notices, logged): (i64, i64) = db
            .conn()
            .query_row(
                "SELECT (SELECT count(*) FROM engine_notices WHERE reminder = 1 AND work IS NULL),
                        (SELECT count(*) FROM outbox WHERE kind = 'notify')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((notices, logged), (3, 3));
    }

    // The interval is the installation's: at half an hour, an ask that has waited half an hour is reminded of; at 0,
    // nothing is.
    #[test]
    fn the_interval_is_the_installations() {
        let t0 = 1_000 * HOUR;
        let (mut db, _) = asks(t0, t0);
        assert_eq!(remind(&mut db, t0 + 10 * HOUR, 0), None);
        assert_eq!(
            remind(&mut db, t0 + 30 * MINUTE, 30 * MINUTE).as_deref(),
            Some("2 asks wait for your answer; the oldest, ask 1, has waited 30m 00s.")
        );
        assert_eq!(remind(&mut db, t0 + 59 * MINUTE, 30 * MINUTE), None);
        assert!(remind(&mut db, t0 + 60 * MINUTE, 30 * MINUTE).is_some());
    }

    // The reminder says how many asks wait and how long the oldest has, in the page's units.
    #[test]
    fn the_reminder_says_how_many_wait_and_how_long_the_oldest_has() {
        assert_eq!(
            text(3, 12, 26 * HOUR + 7 * MINUTE),
            "3 asks wait for your answer; the oldest, ask 12, has waited 1d 02h."
        );
        assert_eq!(
            text(1, 4, 61 * MINUTE + 5_000),
            "Ask 4 waits for your answer; it has waited 1h 01m."
        );
        assert_eq!(duration(41_000), "41s");
        assert_eq!(duration(3 * MINUTE + 7_000), "3m 07s");
    }
}
