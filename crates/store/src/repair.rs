//! What the one-off repair of the costs recorded before a round's cost was what its results say it spent reads and
//! writes (`wf-agents repair-costs`). It rewrites the cost of recorded rounds and forks in place, the one exception to
//! the rule that rows are only added to (decision `a-wrongly-recorded-cost-is-corrected-once-in-place`), and is removed
//! with the command.

use rusqlite::params;

use crate::{Fault, SCHEMA_VERSION, Store, before_migration};

/// The cost recorded of a round of a run, and where the round stands among the work's records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundCost {
    pub work: u64,
    pub run: u64,
    pub round: usize,
    /// The place of the round's first try among the work's records; none was recorded as 0.
    pub at: u64,
    /// The number of the round's last try, the first being one.
    pub attempt: usize,
    pub usd_micros: u64,
}

/// The cost recorded of a fork of a round's session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForkCost {
    pub work: u64,
    pub at: u64,
    pub run: u64,
    pub round: usize,
    pub usd_micros: u64,
}

/// The costs the store holds: each work's rounds, in the order they began, its forks, and where its keep-warm pings
/// were made, as a work and a place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rows {
    pub rounds: Vec<RoundCost>,
    pub forks: Vec<ForkCost>,
    pub pings: Vec<(u64, u64)>,
}

/// A round whose recorded cost is to be another: its work, run and number, and the cost.
pub type RoundCorrection = (u64, u64, usize, u64);

/// A fork whose recorded cost is to be another: its work and place, and the cost.
pub type ForkCorrection = (u64, u64, u64);

impl Store {
    /// Takes the copy of the database that is taken before a migration, so that a repair that goes wrong loses nothing.
    pub fn copy_before_repair(&self) -> Result<(), Fault> {
        before_migration::copy(&self.db, SCHEMA_VERSION)
    }

    /// The costs the store holds.
    pub fn cost_rows(&self) -> Result<Rows, Fault> {
        let mut rounds = self.db.prepare(
            "SELECT s.work, s.run, s.round, COALESCE(r.at, 0), COALESCE(r.attempt, 1), s.usd_micros \
             FROM spent s LEFT JOIN \
             (SELECT work, run, round, MIN(at) AS at, MAX(attempt) AS attempt FROM ran GROUP BY work, run, round) r \
             ON r.work = s.work AND r.run = s.run AND r.round = s.round \
             ORDER BY s.work, COALESCE(r.at, 0), s.run, s.round",
        )?;
        let rounds = rounds
            .query_map([], |row| {
                Ok(RoundCost {
                    work: row.get(0)?,
                    run: row.get(1)?,
                    round: row.get(2)?,
                    at: row.get(3)?,
                    attempt: row.get(4)?,
                    usd_micros: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut forks = self
            .db
            .prepare("SELECT work, at, run, round, usd_micros FROM forked ORDER BY work, at")?;
        let forks = forks
            .query_map([], |row| {
                Ok(ForkCost {
                    work: row.get(0)?,
                    at: row.get(1)?,
                    run: row.get(2)?,
                    round: row.get(3)?,
                    usd_micros: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut pings = self.db.prepare("SELECT work, at FROM pinged ORDER BY work, at")?;
        let pings = pings
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Rows { rounds, forks, pings })
    }

    /// Sets the recorded costs of these rounds and forks, all of them or none.
    pub fn correct_costs(&mut self, rounds: &[RoundCorrection], forks: &[ForkCorrection]) -> Result<(), Fault> {
        let tx = self.db.transaction()?;
        for (work, run, round, usd_micros) in rounds {
            tx.execute(
                "UPDATE spent SET usd_micros = ?4 WHERE work = ?1 AND run = ?2 AND round = ?3",
                params![work, run, round, usd_micros],
            )?;
        }
        for (work, at, usd_micros) in forks {
            tx.execute(
                "UPDATE forked SET usd_micros = ?3 WHERE work = ?1 AND at = ?2",
                params![work, at, usd_micros],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
