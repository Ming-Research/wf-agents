//! Copies of the engine's database, kept on the machine and off it.
//!
//! A copy is the whole store as one snapshot, compressed, named by the time it was taken. The store records that a
//! copy was taken and, separately, that it was uploaded to a place off the machine ([`Offsite`]), and what failed. A
//! [`step`] works out from those records and the files ([`Ground`]) what is left to do, and does it: it takes a copy
//! when none was taken in the last day, uploads each copy kept that was not uploaded, and removes the copies past
//! keeping. Each part can be done again, so a step that is stopped at any point is finished by the next one, with no
//! copy taken twice and none left out. [`keep`] runs a step each hour; [`report`] says how the copies stand, from the
//! records alone.
//!
//! - `snapshot`: one copy, made whole in a file that is not yet the copy.
//! - `ground`: the records and the files a step stands on, the real one and the seam a test stops a step at.
//! - `offsite`: the place off the machine, the real one on GitHub being in the crate `carry`.
//! - `state`: how the copies stand.

mod ground;
mod offsite;
mod snapshot;
mod state;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use store::Records;
use work::Time;

pub use ground::{Disk, Fail, Ground};
pub use offsite::{Nowhere, Offsite, RELEASE};
pub use state::{Report, Shown, State, report};

/// How long a copy is the day's: a copy is taken when none was taken in this many milliseconds.
pub const PERIOD: u64 = 24 * 60 * 60 * 1000;

/// How long the task waits between two steps, in milliseconds.
pub const TICK: u64 = 60 * 60 * 1000;

/// The name of the copy taken at `time`. The time has thirteen digits, so that names sort as the times do.
pub fn name_at(time: Time) -> String {
    format!("store-{:013}.sqlite.gz", time.0)
}

/// The time in a name made by [`name_at`]; none for any other name, which is not the engine's.
pub fn time_in(name: &str) -> Option<Time> {
    let digits = name.strip_prefix("store-")?.strip_suffix(".sqlite.gz")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(Time)
}

/// What a step did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The copy it took.
    pub taken: Option<String>,
    /// The copies it uploaded.
    pub uploaded: Vec<String>,
    /// The copies it removed.
    pub removed: Vec<String>,
    /// The step that failed and why, where one did: the step ended there.
    pub failed: Option<(String, String)>,
    /// Whether it was stopped where it was, as a test does: a stop is no failure, and nothing records it.
    pub stopped: bool,
}

/// What a step works on: the store's file, the directory of the copies, how many are kept, and the place off the machine.
#[derive(Clone)]
pub struct Plan<O> {
    /// The store's file.
    pub store: PathBuf,
    /// The directory the copies are kept in on the machine.
    pub dir: PathBuf,
    /// How many copies are kept, the newest.
    pub keep: usize,
    /// The place off the machine; none where the copies are kept on the machine alone. Such a plan is typed with
    /// [`Nowhere`], which is never asked.
    pub offsite: Option<O>,
}

impl<O> Plan<O> {
    /// Whether a place off the machine is given.
    pub fn configured(&self) -> bool {
        self.offsite.is_some()
    }
}

/// Why a step ended where it did, other than whole.
enum Halt {
    /// The process was stopped here, as a test makes it.
    Stopped,
    /// A step failed, about this copy; `record` where the failure can be written down.
    Failed {
        name: String,
        step: &'static str,
        why: String,
        record: bool,
    },
}

/// What a failure of the ground in this step, about this copy, ends the step as.
fn ending(name: &str, step: &'static str) -> impl FnOnce(Fail) -> Halt {
    let name = name.to_string();
    move |fail| match fail {
        Fail::Stopped => Halt::Stopped,
        Fail::Why(why) => Halt::Failed {
            name,
            step,
            why,
            record: true,
        },
    }
}

/// What a failure to read or write the records themselves ends the step as: it cannot be written down.
fn unrecorded(fail: Fail) -> Halt {
    match fail {
        Fail::Stopped => Halt::Stopped,
        Fail::Why(why) => Halt::Failed {
            name: "-".to_string(),
            step: "records",
            why,
            record: false,
        },
    }
}

/// What a failure of the place off the machine in this step, about this copy, ends the step as.
fn refused(name: &str, step: &'static str, why: String) -> Halt {
    Halt::Failed {
        name: name.to_string(),
        step,
        why,
        record: true,
    }
}

/// One step at `now`: works out from the records and the files what is to be done, and does it. It ends at the first
/// failure, which it records, or at a stop; the next step goes on from the records and the files, which is all it has.
pub async fn step<O: Offsite>(now: Time, ground: &dyn Ground, plan: &Plan<O>) -> Outcome {
    let mut out = Outcome::default();
    match work(now, ground, plan, &mut out).await {
        Ok(()) => {}
        Err(Halt::Stopped) => out.stopped = true,
        Err(Halt::Failed {
            name,
            step,
            why,
            record,
        }) => {
            // A failure that cannot be written down is still the step's end, and is said by the one who runs it.
            if record && ground.failed(now, &name, step, &why) == Err(Fail::Stopped) {
                out.stopped = true;
            } else {
                out.failed = Some((step.to_string(), why));
            }
        }
    }
    out
}

async fn work<O: Offsite>(now: Time, ground: &dyn Ground, plan: &Plan<O>, out: &mut Outcome) -> Result<(), Halt> {
    // Files not whole are the leavings of a stop in the middle of a copy.
    ground.clear_partial().map_err(ending("-", "snapshot"))?;
    let mut seen = ground.records().map_err(unrecorded)?;
    // A file whole under its name with no record is the leaving of a stop between the rename and the record.
    adopt(ground, &seen)?;
    seen = ground.records().map_err(unrecorded)?;
    // A copy of the last day, by the clock now: one taken later than a day from now is not the clock's to trust.
    let today = seen
        .copies
        .iter()
        .any(|k| k.taken.0.saturating_add(PERIOD) > now.0 && k.taken.0 <= now.0.saturating_add(PERIOD));
    if !today {
        let name = name_at(now);
        let (database, dir) = (plan.store.clone(), plan.dir.clone());
        let made = tokio::task::spawn_blocking(move || snapshot::take(&database, &dir, now)).await;
        match made {
            Ok(Ok(_)) => {}
            Ok(Err(why)) => return Err(refused(&name, "snapshot", why)),
            Err(e) => return Err(refused(&name, "snapshot", format!("the copy did not end: {e}"))),
        }
        ground.finish(&name).map_err(ending(&name, "snapshot"))?;
        ground.taken(&name, now).map_err(ending(&name, "snapshot"))?;
        out.taken = Some(name);
        seen = ground.records().map_err(unrecorded)?;
    }
    if let Some(offsite) = &plan.offsite {
        // Whether a copy is to be uploaded is read from the records, never taken from having just made it.
        for copy in state::kept(&seen, plan.keep)
            .iter()
            .filter(|k| !seen.uploads.contains_key(&k.name))
        {
            match offsite.private().await {
                Ok(true) => {}
                Ok(false) => {
                    let why = "the repository the copies go to is not private".to_string();
                    return Err(refused(&copy.name, "private", why));
                }
                Err(why) => return Err(refused(&copy.name, "private", why)),
            }
            // An asset of its name there already is the leaving of a stop between the upload and its record.
            let there = match offsite.list().await {
                Ok(listed) => listed.contains(&copy.name),
                Err(why) => return Err(refused(&copy.name, "upload", why)),
            };
            if !there && let Err(why) = offsite.upload(&ground.path(&copy.name)).await {
                return Err(refused(&copy.name, "upload", why));
            }
            ground.uploaded(&copy.name, now).map_err(ending(&copy.name, "upload"))?;
            out.uploaded.push(copy.name.clone());
        }
    }
    prune(ground, plan, &seen, out).await?;
    // A step that ends with nothing failing mends the failures before it; it is recorded only when there is one.
    // What is before it is told by the order of recording, not by time, which a clock that jumped makes wrong.
    let to_mend = seen
        .failures
        .last()
        .filter(|f| seen.mended.is_none_or(|mended| f.seq > mended))
        .map(|f| f.seq);
    if let Some(seq) = to_mend {
        ground.ended(now, seq).map_err(unrecorded)?;
    }
    Ok(())
}

/// Records a copy whole under its name that the records do not hold, when it is later than the newest the records
/// hold, and removes it when it is not: an old one cannot be the leaving of the last stop, and recorded as the newest
/// it would push a copy that was kept out of the copies kept.
fn adopt(ground: &dyn Ground, seen: &Records) -> Result<(), Halt> {
    let newest = seen.copies.last().and_then(|k| time_in(&k.name));
    for name in ground.files().map_err(ending("-", "snapshot"))? {
        if seen.copies.iter().any(|k| k.name == name) {
            continue;
        }
        let Some(time) = time_in(&name) else {
            continue;
        };
        if newest.is_none_or(|n| time > n) {
            ground.taken(&name, time).map_err(ending(&name, "snapshot"))?;
        } else {
            ground.remove(&name).map_err(ending(&name, "snapshot"))?;
        }
    }
    Ok(())
}

/// Removes the copies past keeping, from the place off the machine, where it lists them, and from the machine after.
/// The place is looked at each step while there are copies past keeping, so that an asset is removed whether or not
/// its file is still there; a file that is gone is no error.
async fn prune<O: Offsite>(ground: &dyn Ground, plan: &Plan<O>, seen: &Records, out: &mut Outcome) -> Result<(), Halt> {
    let kept = state::kept(seen, plan.keep).len();
    let past = &seen.copies[..seen.copies.len() - kept];
    if past.is_empty() {
        return Ok(());
    }
    if let Some(offsite) = &plan.offsite {
        let listed = offsite.list().await.map_err(|why| refused("-", "prune", why))?;
        for copy in past {
            if listed.contains(&copy.name) {
                (offsite.remove(&copy.name).await).map_err(|why| refused(&copy.name, "prune", why))?;
            }
        }
    }
    for copy in past {
        if ground.remove(&copy.name).map_err(ending(&copy.name, "prune"))? {
            out.removed.push(copy.name.clone());
        }
    }
    Ok(())
}

/// Keeps the copies until the process ends: a step at once and then each hour, one after another, each step ended
/// before the next begins, so that two never run together. `now` gives the time by the engine's clock, the only one
/// read; `said` is told what each step did.
pub async fn keep<O, G, N, F>(plan: Plan<O>, ground: G, now: N, said: impl Fn(&Outcome) + Send + 'static)
where
    O: Offsite,
    G: Ground,
    N: Fn() -> F + Send,
    F: Future<Output = Result<Time, String>> + Send,
{
    loop {
        match now().await {
            Ok(time) => said(&step(time, &ground, &plan).await),
            Err(why) => said(&Outcome {
                failed: Some(("snapshot".to_string(), format!("the clock cannot be read: {why}"))),
                ..Outcome::default()
            }),
        }
        tokio::time::sleep(Duration::from_millis(TICK)).await;
    }
}
