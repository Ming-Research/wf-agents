//! The works' waits for a free slot (`slot_waits`): a work whose wait is over, its answer come or what its row waited
//! for, that waits on, holding no slot, until its machine has one free. The engine's loop alone opens and closes them,
//! as it reaps the work's steps and grants it a slot; whoever wants to know which works wait so, and since when, reads
//! them here.

use std::collections::BTreeSet;

use rusqlite::{Connection, Transaction, params};

use crate::Result;

/// How a work's wait for a free slot ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotWaitEnd {
    /// The loop granted the work a free slot.
    Granted,
    /// The work ended.
    Ended,
    /// The work was paused.
    Paused,
    /// The work went on without a slot, as by a round the owner asked for, or no longer waits where it waited.
    GoneOn,
}

impl SlotWaitEnd {
    /// The end as the table's `ended` column holds it.
    pub fn name(self) -> &'static str {
        match self {
            SlotWaitEnd::Granted => "granted",
            SlotWaitEnd::Ended => "ended",
            SlotWaitEnd::Paused => "paused",
            SlotWaitEnd::GoneOn => "gone_on",
        }
    }
}

/// Opens a work's wait for a free slot, begun at `at`, unless one is open: the open one goes on.
pub fn open_slot_wait(tx: &Transaction<'_>, work: i64, at: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO slot_waits (work, since) SELECT ?1, ?2
         WHERE NOT EXISTS (SELECT 1 FROM slot_waits WHERE work = ?1 AND until IS NULL)",
        params![work, at],
    )?;
    Ok(())
}

/// Closes a work's open wait for a free slot at `at`, as it ended. Gives whether one was open.
pub fn close_slot_wait(tx: &Transaction<'_>, work: i64, how: SlotWaitEnd, at: i64) -> Result<bool> {
    let closed = tx.execute(
        "UPDATE slot_waits SET until = ?2, ended = ?3 WHERE work = ?1 AND until IS NULL",
        params![work, at, how.name()],
    )?;
    Ok(closed > 0)
}

/// The works whose wait for a free slot is open.
pub fn slot_waiting(conn: &Connection) -> Result<BTreeSet<i64>> {
    let mut st = conn.prepare("SELECT work FROM slot_waits WHERE until IS NULL")?;
    let works = st
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<BTreeSet<i64>, _>>()?;
    Ok(works)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actor, Db};

    fn rows(db: &Db) -> Vec<(i64, i64, Option<i64>, Option<String>)> {
        let mut st = db
            .conn()
            .prepare("SELECT work, since, until, ended FROM slot_waits ORDER BY id")
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    }

    // A work has one wait for a slot open at most: opening it again keeps the one open and its start; once closed, with
    // how it ended, the next opens afresh, and closing none closes nothing.
    #[test]
    fn a_work_waits_for_a_slot_once_at_a_time_from_when_it_began_until_it_ended() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for _ in 0..2 {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, 1, 'architect', 'N0', 'p')",
                    [],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let write = |db: &mut Db, f: &dyn Fn(&Transaction<'_>) -> Result<bool>| -> bool {
            db.write(Actor::Engine, |tx| f(tx)).unwrap()
        };
        write(&mut db, &|tx| open_slot_wait(tx, 1, 10).map(|_| true));
        write(&mut db, &|tx| open_slot_wait(tx, 1, 20).map(|_| true));
        write(&mut db, &|tx| open_slot_wait(tx, 2, 15).map(|_| true));
        assert_eq!(slot_waiting(db.conn()).unwrap(), BTreeSet::from([1, 2]));
        assert!(write(&mut db, &|tx| close_slot_wait(tx, 1, SlotWaitEnd::Granted, 30)));
        assert!(!write(&mut db, &|tx| close_slot_wait(tx, 1, SlotWaitEnd::Paused, 40)));
        assert_eq!(slot_waiting(db.conn()).unwrap(), BTreeSet::from([2]));
        write(&mut db, &|tx| open_slot_wait(tx, 1, 50).map(|_| true));
        assert!(write(&mut db, &|tx| close_slot_wait(tx, 2, SlotWaitEnd::Paused, 60)));
        assert_eq!(
            rows(&db),
            [
                (1, 10, Some(30), Some("granted".to_string())),
                (2, 15, Some(60), Some("paused".to_string())),
                (1, 50, None, None),
            ]
        );
    }
}
