//! The holds on a project's new starts (`dispatch_holds`): one definition of how a hold is written and released, which
//! the owner's page and command line and the project's architect all use, so that no two places write the table.
//! The engine only reads it: while a project has a row, no new work of it starts, and works already running go on.

use rusqlite::{Transaction, params};

use crate::{Result, now};

/// Why a project's new starts are held, which is who holds them: the owner, the project's architect by its own
/// action, or the architect setting the project straight. A project has at most one hold of each reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldReason {
    Owner,
    Architect,
    SetStraight,
}

impl HoldReason {
    /// Every reason, in the order the owner's page lists them.
    pub const ALL: [HoldReason; 3] = [HoldReason::Owner, HoldReason::Architect, HoldReason::SetStraight];

    /// The reason as the table's `reason` column holds it.
    pub fn name(self) -> &'static str {
        match self {
            HoldReason::Owner => "owner",
            HoldReason::Architect => "architect",
            HoldReason::SetStraight => "set_straight",
        }
    }

    /// The reason a name stands for, or what the names allowed are.
    pub fn parse(name: &str) -> std::result::Result<HoldReason, String> {
        HoldReason::ALL
            .into_iter()
            .find(|r| r.name() == name.trim())
            .ok_or_else(|| {
                let names: Vec<&str> = HoldReason::ALL.iter().map(|r| r.name()).collect();
                format!(
                    "no hold of the reason {}; the reasons: {}",
                    name.trim(),
                    names.join(", ")
                )
            })
    }
}

/// Holds a project's new starts for a reason, saying why in the words of whoever holds. A hold of that reason there
/// already keeps its time and takes the new words, which a hold with none leaves as they were. The project is not
/// checked here: a caller that takes it from outside checks it against the configuration.
pub fn hold_starts(tx: &Transaction<'_>, project: &str, reason: HoldReason, why: Option<&str>) -> Result<()> {
    tx.execute(
        "INSERT INTO dispatch_holds (project, reason, since, why) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (project, reason) DO UPDATE SET why = coalesce(excluded.why, why)",
        params![project, reason.name(), now(), why],
    )?;
    Ok(())
}

/// The reasons a project's new starts are held for now, as the table names them and in the order they began, such as
/// `architect`.
pub fn holds_of(tx: &Transaction<'_>, project: &str) -> Result<Vec<String>> {
    let mut st = tx.prepare("SELECT reason FROM dispatch_holds WHERE project = ?1 ORDER BY since, reason")?;
    let reasons = st
        .query_map(params![project], |r| r.get(0))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(reasons)
}

/// Releases a project's hold of a reason. Gives whether there was one: a release of a hold not there is no error here,
/// since the other side may have released it just before, and what that means is the caller's to say.
pub fn release_starts(tx: &Transaction<'_>, project: &str, reason: HoldReason) -> Result<bool> {
    let released = tx.execute(
        "DELETE FROM dispatch_holds WHERE project = ?1 AND reason = ?2",
        params![project, reason.name()],
    )?;
    Ok(released > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actor, Db};

    fn rows(db: &Db) -> Vec<(String, String, i64, Option<String>)> {
        let mut st = db
            .conn()
            .prepare("SELECT project, reason, since, why FROM dispatch_holds ORDER BY project, reason")
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    }

    // A hold is a row of its reason with the time it began and why; holding again keeps the time and takes the new
    // words, and a hold with none leaves the words as they were. The change is recorded as the holder's.
    #[test]
    fn a_hold_again_changes_only_why() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Owner, |tx| {
            hold_starts(tx, "p", HoldReason::Owner, Some("leaving"))
        })
        .unwrap();
        let first = rows(&db);
        assert_eq!(first.len(), 1);
        assert_eq!((first[0].0.as_str(), first[0].1.as_str()), ("p", "owner"));
        assert_eq!(first[0].3.as_deref(), Some("leaving"));
        std::thread::sleep(std::time::Duration::from_millis(5));
        db.write(Actor::Owner, |tx| {
            hold_starts(tx, "p", HoldReason::Owner, Some("installing"))
        })
        .unwrap();
        let again = rows(&db);
        assert_eq!((again[0].2, again[0].3.as_deref()), (first[0].2, Some("installing")));
        db.write(Actor::Owner, |tx| hold_starts(tx, "p", HoldReason::Owner, None))
            .unwrap();
        assert_eq!(rows(&db)[0].3.as_deref(), Some("installing"));
        let actors: Vec<String> = {
            let mut st = db
                .conn()
                .prepare("SELECT actor FROM changes WHERE table_name = 'dispatch_holds' ORDER BY id")
                .unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
        };
        assert_eq!(actors, ["owner", "owner"]);
    }

    // The three reasons of a project are three rows; releasing one takes only that one and says whether it was there.
    #[test]
    fn a_release_takes_only_its_own_reason() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Owner, |tx| {
            for r in HoldReason::ALL {
                hold_starts(tx, "p", r, Some("x"))?;
            }
            hold_starts(tx, "q", HoldReason::Owner, Some("x"))
        })
        .unwrap();
        let gone = db
            .write(Actor::Architect, |tx| release_starts(tx, "p", HoldReason::Architect))
            .unwrap();
        assert!(gone);
        let left: Vec<(String, String)> = rows(&db).into_iter().map(|r| (r.0, r.1)).collect();
        assert_eq!(
            left,
            [
                ("p".to_string(), "owner".to_string()),
                ("p".to_string(), "set_straight".to_string()),
                ("q".to_string(), "owner".to_string()),
            ]
        );
        let again = db
            .write(Actor::Architect, |tx| release_starts(tx, "p", HoldReason::Architect))
            .unwrap();
        assert!(!again);
    }

    #[test]
    fn a_reason_is_parsed_from_its_name_or_refused_with_the_names_allowed() {
        assert_eq!(HoldReason::parse(" set_straight "), Ok(HoldReason::SetStraight));
        let e = HoldReason::parse("policy").unwrap_err();
        assert!(e.contains("owner, architect, set_straight"), "{e}");
    }
}
