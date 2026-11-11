//! Copies of the engine's database, kept on the machine and off it.
//!
//! A copy is the whole store as one snapshot, compressed, named by the time it was taken. The store records that a
//! copy was taken and, separately, that it was uploaded to a place off the machine ([`Offsite`]), and what failed. A
//! [`step`] works out from those records and the files what is left to do, and does it: it takes a copy when none was
//! taken in the last day, uploads each copy kept that was not uploaded, and removes the copies past keeping. Each part
//! can be done again, so a step that is stopped at any point is finished by the next one, with no copy taken twice and
//! none left out. [`keep`] runs a step each hour; [`report`] says how the copies stand, from the records alone.
//!
//! - `snapshot`: one copy, whole under its name or not there.
//! - `offsite`: the place off the machine, the real one on GitHub.
//! - `state`: how the copies stand.

mod offsite;
mod snapshot;
mod state;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use store::{Backups, Records};
use work::Time;

pub use offsite::{Gh, Offsite, RELEASE};
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
    /// Whether it was stopped where it was, as a test does.
    pub stopped: bool,
}

/// What a step works on.
#[derive(Clone)]
pub struct Plan {
    /// The store's file.
    pub store: PathBuf,
    /// The directory the copies are kept in on the machine.
    pub dir: PathBuf,
    /// How many copies are kept, the newest.
    pub keep: usize,
    /// The place off the machine; none where the copies are kept on the machine alone.
    pub offsite: Option<Arc<dyn Offsite>>,
}

impl Plan {
    /// Whether a place off the machine is given.
    pub fn configured(&self) -> bool {
        self.offsite.is_some()
    }

    /// One step at `now`.
    pub fn step(&self, now: Time) -> Outcome {
        match Backups::open(&self.store) {
            Ok(records) => step_with(now, &records, self, &mut || false),
            Err(e) => Outcome {
                failed: Some(("records".to_string(), format!("{}: {e:?}", self.store.display()))),
                ..Outcome::default()
            },
        }
    }
}

/// Records a failure of a step, and gives it as the step's end.
fn fail(out: &mut Outcome, records: &Backups, now: Time, name: &str, step: &str, why: String) {
    // A failure that cannot be recorded is still the step's end, and is said by the one who runs it.
    let _ = records.failed(now, name, step, &why);
    out.failed = Some((step.to_string(), why));
}

/// One step at `now`: works out from the records and the files what is to be done, and does it. `halt` is asked after
/// each thing done that leaves a mark, and where it answers true the step ends there, as if the process had stopped.
pub fn step_with(now: Time, records: &Backups, plan: &Plan, halt: &mut dyn FnMut() -> bool) -> Outcome {
    let mut out = Outcome::default();
    let dir = plan.dir.as_path();
    // Files not whole are the leavings of a stop in the middle of a copy.
    if let Err(why) = clear_partial(dir) {
        fail(&mut out, records, now, "-", "snapshot", why);
        return out;
    }
    if halt() {
        out.stopped = true;
        return out;
    }
    let mut seen = match records.records() {
        Ok(seen) => seen,
        Err(e) => {
            out.failed = Some(("records".to_string(), format!("{e:?}")));
            return out;
        }
    };
    // A file whole under its name with no record is the leaving of a stop between the rename and the record.
    if let Err(why) = adopt(dir, records, &seen) {
        fail(&mut out, records, now, "-", "snapshot", why);
        return out;
    }
    if halt() {
        out.stopped = true;
        return out;
    }
    seen = match records.records() {
        Ok(seen) => seen,
        Err(e) => {
            out.failed = Some(("records".to_string(), format!("{e:?}")));
            return out;
        }
    };
    // A copy of the last day, by the clock now: one taken later than a day from now is not the clock's to trust.
    let today = seen
        .copies
        .iter()
        .any(|k| k.taken.0.saturating_add(PERIOD) > now.0 && k.taken.0 <= now.0.saturating_add(PERIOD));
    if !today {
        match snapshot::take(&plan.store, dir, now, halt) {
            Ok(Some(name)) => {
                if let Err(e) = records.taken(&name, now) {
                    fail(&mut out, records, now, &name, "snapshot", format!("{e:?}"));
                    return out;
                }
                out.taken = Some(name);
                if halt() {
                    out.stopped = true;
                    return out;
                }
            }
            Ok(None) => {
                out.stopped = true;
                return out;
            }
            Err(why) => {
                fail(&mut out, records, now, &crate::name_at(now), "snapshot", why);
                return out;
            }
        }
        seen = match records.records() {
            Ok(seen) => seen,
            Err(e) => {
                out.failed = Some(("records".to_string(), format!("{e:?}")));
                return out;
            }
        };
    }
    let kept = state::kept(&seen, plan.keep);
    if let Some(offsite) = &plan.offsite {
        for copy in kept.iter().filter(|k| !seen.uploads.contains_key(&k.name)) {
            match offsite.private() {
                Ok(true) => {}
                Ok(false) => {
                    let why = "the repository the copies go to is not private".to_string();
                    fail(&mut out, records, now, &copy.name, "private", why);
                    return out;
                }
                Err(why) => {
                    fail(&mut out, records, now, &copy.name, "private", why);
                    return out;
                }
            }
            // An asset of its name there already is the leaving of a stop between the upload and its record.
            let there = match offsite.list() {
                Ok(listed) => listed.contains(&copy.name),
                Err(why) => {
                    fail(&mut out, records, now, &copy.name, "upload", why);
                    return out;
                }
            };
            if !there && let Err(why) = offsite.upload(&dir.join(&copy.name)) {
                fail(&mut out, records, now, &copy.name, "upload", why);
                return out;
            }
            if halt() {
                out.stopped = true;
                return out;
            }
            if let Err(e) = records.uploaded(&copy.name, now) {
                fail(&mut out, records, now, &copy.name, "upload", format!("{e:?}"));
                return out;
            }
            out.uploaded.push(copy.name.clone());
            if halt() {
                out.stopped = true;
                return out;
            }
        }
    }
    prune(now, records, plan, &seen, &mut out, halt);
    out
}

/// Removes the files ending as a copy not whole does.
fn clear_partial(dir: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "partial") {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// The copies in `dir` under the engine's names, by time.
fn files(dir: &Path) -> Vec<(String, Time)> {
    let mut found: Vec<(String, Time)> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .filter_map(|name| time_in(&name).map(|t| (name, t)))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// Records a copy whole under its name that the records do not hold, when it is later than the newest the records
/// hold, and removes it when it is not: an old one cannot be the leaving of the last stop, and recorded as the newest
/// it would push a copy that was kept out of the copies kept.
fn adopt(dir: &Path, records: &Backups, seen: &Records) -> Result<(), String> {
    let newest = seen.copies.last().and_then(|k| time_in(&k.name));
    for (name, time) in files(dir) {
        if seen.copies.iter().any(|k| k.name == name) {
            continue;
        }
        if newest.is_none_or(|n| time > n) {
            records.taken(&name, time).map_err(|e| format!("{e:?}"))?;
        } else {
            std::fs::remove_file(dir.join(&name)).map_err(|e| format!("{}: {e}", dir.join(&name).display()))?;
        }
    }
    Ok(())
}

/// Removes the copies past keeping, from the place off the machine first and from the machine after, so that a copy
/// whose removal was stopped is found again by its file. A copy with no file is done with.
fn prune(now: Time, records: &Backups, plan: &Plan, seen: &Records, out: &mut Outcome, halt: &mut dyn FnMut() -> bool) {
    let kept = state::kept(seen, plan.keep).len();
    let past = &seen.copies[..seen.copies.len() - kept];
    let with_file: Vec<&store::Kept> = past.iter().filter(|k| plan.dir.join(&k.name).exists()).collect();
    if with_file.is_empty() {
        return;
    }
    if let Some(offsite) = &plan.offsite {
        match offsite.list() {
            Ok(listed) => {
                for copy in &with_file {
                    if listed.contains(&copy.name)
                        && let Err(why) = offsite.remove(&copy.name)
                    {
                        fail(out, records, now, &copy.name, "prune", why);
                        return;
                    }
                }
            }
            Err(why) => {
                fail(out, records, now, "-", "prune", why);
                return;
            }
        }
    }
    for copy in with_file {
        let path = plan.dir.join(&copy.name);
        match std::fs::remove_file(&path) {
            Ok(()) => out.removed.push(copy.name.clone()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                fail(
                    out,
                    records,
                    now,
                    &copy.name,
                    "prune",
                    format!("{}: {e}", path.display()),
                );
                return;
            }
        }
        if halt() {
            out.stopped = true;
            return;
        }
    }
}

/// Keeps the copies until the process ends: a step at once and then each hour, one after another, each step ended
/// before the next begins, so that two never run together. `now` gives the time by the engine's clock, the only one
/// read; `said` is told what each step did.
pub async fn keep<N, F>(plan: Plan, now: N, said: impl Fn(&Outcome) + Send + 'static)
where
    N: Fn() -> F + Send,
    F: Future<Output = Result<Time, String>> + Send,
{
    loop {
        match now().await {
            Ok(time) => {
                let plan = plan.clone();
                match tokio::task::spawn_blocking(move || plan.step(time)).await {
                    Ok(out) => said(&out),
                    Err(e) => said(&Outcome {
                        failed: Some(("snapshot".to_string(), format!("the step did not end: {e}"))),
                        ..Outcome::default()
                    }),
                }
            }
            Err(why) => said(&Outcome {
                failed: Some(("snapshot".to_string(), format!("the clock cannot be read: {why}"))),
                ..Outcome::default()
            }),
        }
        tokio::time::sleep(Duration::from_millis(TICK)).await;
    }
}
