//! The copies, tested on real files and a real store, with the place off the machine made by a fake that can fail at
//! any call, and the process stopped at every write a step makes, by a ground that counts them.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use store::{Records, Store};
use tempfile::TempDir;
use tokio::sync::Notify;

use super::*;

const T0: u64 = 1_700_000_000_000;
const HOUR: u64 = TICK;
const DAY: u64 = PERIOD;

/// A place off the machine that keeps its assets in memory and fails at the calls it is told to.
#[derive(Clone, Default)]
struct Fake {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    assets: BTreeMap<String, Vec<u8>>,
    /// How many calls were made, counting each of the four.
    calls: usize,
    /// The calls, by their number, that fail once.
    fail_at: BTreeSet<usize>,
    /// Whether every upload fails.
    fail_uploads: bool,
    /// Whether every removal fails.
    fail_removes: bool,
    public: bool,
    uploads: Vec<String>,
    private_calls: usize,
}

impl Fake {
    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("the fake is not held by a failed test")
    }

    fn call(&self) -> Result<(), String> {
        let mut inner = self.inner();
        inner.calls += 1;
        if inner.fail_at.contains(&(inner.calls - 1)) {
            Err("the place cannot be reached".to_string())
        } else {
            Ok(())
        }
    }

    fn names(&self) -> Vec<String> {
        self.inner().assets.keys().cloned().collect()
    }
}

impl Offsite for Fake {
    async fn private(&self) -> Result<bool, String> {
        self.call()?;
        let mut inner = self.inner();
        inner.private_calls += 1;
        Ok(!inner.public)
    }

    async fn upload(&self, file: &Path) -> Result<(), String> {
        self.call()?;
        let mut inner = self.inner();
        if inner.fail_uploads {
            return Err("the upload is refused".to_string());
        }
        let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
        let name = file.file_name().and_then(|n| n.to_str()).expect("a name").to_string();
        inner.assets.insert(name.clone(), bytes);
        inner.uploads.push(name);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, String> {
        self.call()?;
        Ok(self.inner().assets.keys().cloned().collect())
    }

    async fn remove(&self, name: &str) -> Result<(), String> {
        self.call()?;
        let mut inner = self.inner();
        if inner.fail_removes {
            return Err("the removal is refused".to_string());
        }
        inner.assets.remove(name);
        Ok(())
    }
}

/// The real ground, with every write counted, and ended as a stop, instead of done, at the one a test names.
struct Counting {
    disk: Disk,
    stop_at: Option<usize>,
    writes: AtomicUsize,
}

impl Counting {
    fn over(store: &Path, dir: &Path, stop_at: Option<usize>) -> Counting {
        Counting {
            disk: Disk::open(store, dir).expect("the records open"),
            stop_at,
            writes: AtomicUsize::new(0),
        }
    }

    fn write<T>(&self, done: impl FnOnce() -> Result<T, Fail>) -> Result<T, Fail> {
        let this = self.writes.fetch_add(1, Ordering::SeqCst);
        if self.stop_at == Some(this) {
            return Err(Fail::Stopped);
        }
        done()
    }
}

impl Ground for Counting {
    fn records(&self) -> Result<Records, Fail> {
        self.disk.records()
    }

    fn files(&self) -> Result<Vec<String>, Fail> {
        self.disk.files()
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.disk.path(name)
    }

    fn clear_partial(&self) -> Result<(), Fail> {
        self.write(|| self.disk.clear_partial())
    }

    fn finish(&self, name: &str) -> Result<(), Fail> {
        self.write(|| self.disk.finish(name))
    }

    fn remove(&self, name: &str) -> Result<bool, Fail> {
        self.write(|| self.disk.remove(name))
    }

    fn taken(&self, name: &str, at: Time) -> Result<(), Fail> {
        self.write(|| self.disk.taken(name, at))
    }

    fn uploaded(&self, name: &str, at: Time) -> Result<(), Fail> {
        self.write(|| self.disk.uploaded(name, at))
    }

    fn failed(&self, at: Time, name: &str, step: &str, why: &str) -> Result<(), Fail> {
        self.write(|| self.disk.failed(at, name, step, why))
    }

    fn ended(&self, at: Time, mended: u64) -> Result<(), Fail> {
        self.write(|| self.disk.ended(at, mended))
    }
}

/// A store with some items in it, a directory for the copies, and a plan over them.
struct Env {
    _dir: TempDir,
    _store: Store,
    plan: Plan<Fake>,
    fake: Fake,
    disk: Disk,
}

fn env(keep: usize, off: bool) -> Env {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("store.sqlite");
    let store = Store::open(&path).expect("the store opens");
    let conn = Connection::open(&path).expect("the file opens");
    for i in 0..50 {
        conn.execute("INSERT INTO items (title, added) VALUES (?1, 1)", [format!("item {i}")])
            .expect("an item");
    }
    let fake = Fake::default();
    let backups = dir.path().join("backups");
    let plan = Plan {
        store: path.clone(),
        dir: backups.clone(),
        keep,
        offsite: off.then(|| fake.clone()),
    };
    let disk = Disk::open(&path, &backups).expect("the records open");
    Env {
        _dir: dir,
        _store: store,
        plan,
        fake,
        disk,
    }
}

impl Env {
    async fn step(&self, at: u64) -> Outcome {
        step(Time(at), &self.disk, &self.plan).await
    }

    fn report(&self, at: u64) -> Report {
        report(&self.rows(), Time(at), self.plan.configured(), self.plan.keep)
    }

    fn rows(&self) -> Records {
        self.disk.records().expect("the records")
    }

    /// The names of the files whole under the engine's names.
    fn files(&self) -> Vec<String> {
        self.disk.files().expect("the files")
    }

    fn partial(&self) -> Vec<String> {
        (std::fs::read_dir(&self.plan.dir).into_iter().flatten())
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.ends_with(".partial"))
            .collect()
    }

    /// The items a copy holds, after checking it is a whole database.
    fn items_in(&self, name: &str) -> i64 {
        let bytes = std::fs::read(self.plan.dir.join(name)).expect("the copy is read");
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(&bytes[..])
            .read_to_end(&mut raw)
            .expect("the copy is gzip");
        let out = self._dir.path().join(format!("check-{name}"));
        std::fs::write(&out, raw).expect("written");
        let conn = Connection::open(&out).expect("the copy opens");
        let ok: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .expect("checked");
        assert_eq!(ok, "ok");
        conn.query_row("SELECT count(*) FROM items", [], |r| r.get(0))
            .expect("counted")
    }
}

#[test]
fn a_name_carries_its_time_and_only_such_a_name_is_the_engines() {
    let name = name_at(Time(T0));
    assert_eq!(name, "store-1700000000000.sqlite.gz");
    assert_eq!(time_in(&name), Some(Time(T0)));
    assert!(name_at(Time(5)) < name_at(Time(T0)), "names sort as times do");
    for foreign in [
        "notes.txt",
        "store-.sqlite.gz",
        "store-12a.sqlite.gz",
        "store-12.sqlite",
        "daily-1.sqlite.gz",
    ] {
        assert_eq!(time_in(foreign), None, "{foreign}");
    }
}

/// What a history that has been stopped and gone on must have come to: one whole copy, uploaded once, no leavings.
fn assert_come_to_one_copy(e: &Env, label: &str) {
    for name in e.files() {
        assert_eq!(e.items_in(&name), 50, "{label}: {name}");
    }
    let rows = e.rows();
    assert_eq!(rows.copies.len(), 1, "{label}: {:?}", rows.copies);
    assert_eq!(e.files().len(), 1, "{label}");
    assert_eq!(e.partial(), Vec::<String>::new(), "{label}");
    let name = rows.copies[0].name.clone();
    assert_eq!(e.fake.names(), std::slice::from_ref(&name), "{label}");
    assert_eq!(
        e.fake.inner().uploads,
        std::slice::from_ref(&name),
        "uploaded once: {label}"
    );
    assert!(rows.uploads.contains_key(&name), "{label}");
    // No stop is a failure: only the place's own failures are recorded.
    assert!(
        rows.failures.iter().all(|f| f.step == "private" || f.step == "upload"),
        "{label}: {:?}",
        rows.failures
    );
    assert_eq!(e.report(T0 + 5000).state, State::Ok, "{label}");
}

// Acceptance 1, 2, 3: a step stopped at each of its writes, with the place off the machine failing at any one call, is
// finished by the steps after it: one copy, whole, uploaded once, no leavings, and no stop recorded as a failure. The
// stops are the grounds's own writes, so the window between the rename of a copy and its record is one of them.
#[tokio::test]
async fn a_step_stopped_at_each_write_and_a_call_failing_anywhere_come_to_one_whole_copy_uploaded_once() {
    let mut most = 0;
    let mut between_rename_and_record = 0;
    for fail in [None, Some(0), Some(1), Some(2)] {
        for n in 0.. {
            let e = env(7, true);
            if let Some(f) = fail {
                e.fake.inner().fail_at.insert(f);
            }
            let ground = Counting::over(&e.plan.store, &e.plan.dir, Some(n));
            let first = step(Time(T0), &ground, &e.plan).await;
            // Nothing under a final name is ever not whole.
            for name in e.files() {
                assert_eq!(e.items_in(&name), 50, "fail {fail:?}, stop {n}: {name}");
            }
            if first.stopped && e.files().len() == 1 && e.rows().copies.is_empty() {
                between_rename_and_record += 1;
            }
            // The process starts again; each step after goes on from the records and the files.
            for k in 1..=3 {
                e.step(T0 + k * 1000).await;
            }
            assert_come_to_one_copy(&e, &format!("fail {fail:?}, stop {n}"));
            if !first.stopped {
                break;
            }
            most = n;
        }
    }
    // A step that takes a copy and uploads it writes four times: it clears the leavings, makes the copy whole, records it,
    // and records its upload; a failure is one more.
    assert!(most >= 3, "the step has writes to stop at: {most}");
    assert!(
        between_rename_and_record >= 1,
        "a stop between the rename of a copy and its record was made"
    );
}

// Stops in a step that goes on from a failure: the place refused the upload, and the step after it, which uploads and
// mends the failure, is stopped at each of its writes.
#[tokio::test]
async fn a_step_that_mends_a_failure_stopped_at_each_write_comes_to_the_same() {
    for n in 0.. {
        let e = env(7, true);
        e.fake.inner().fail_uploads = true;
        let first = e.step(T0).await;
        assert_eq!(first.failed.as_ref().map(|f| f.0.as_str()), Some("upload"));
        e.fake.inner().fail_uploads = false;
        let ground = Counting::over(&e.plan.store, &e.plan.dir, Some(n));
        let second = step(Time(T0 + HOUR), &ground, &e.plan).await;
        for k in 2..=4 {
            e.step(T0 + k * HOUR).await;
        }
        let rows = e.rows();
        assert_eq!(rows.copies.len(), 1, "stop {n}");
        assert_eq!(e.fake.inner().uploads.len(), 1, "stop {n}");
        assert_eq!(rows.failures.len(), 1, "stop {n}: only the place's failure is recorded");
        assert_eq!(e.report(T0 + 5 * HOUR).state, State::Ok, "stop {n}");
        if !second.stopped {
            break;
        }
    }
}

#[tokio::test]
async fn a_copy_is_taken_once_a_day_and_uploaded_when_the_upload_failed_before() {
    let e = env(7, true);
    e.fake.inner().fail_uploads = true;
    let first = e.step(T0).await;
    assert!(first.taken.is_some());
    assert_eq!(first.failed.as_ref().map(|f| f.0.as_str()), Some("upload"));
    assert_eq!(
        e.report(T0).state,
        State::Failed {
            at: Time(T0),
            step: "upload".to_string(),
            why: "the upload is refused".to_string()
        }
    );
    // The retry an hour later takes no second copy.
    e.fake.inner().fail_uploads = false;
    let again = e.step(T0 + HOUR).await;
    assert_eq!(again.taken, None);
    assert_eq!(again.uploaded.len(), 1);
    assert_eq!(e.rows().copies.len(), 1);
    assert_eq!(e.report(T0 + HOUR).state, State::Ok);
    // Nothing more within the day, and a copy when the day is out.
    assert_eq!(e.step(T0 + 2 * HOUR).await, Outcome::default());
    let next = e.step(T0 + DAY + 1).await;
    assert!(next.taken.is_some());
    assert_eq!(e.rows().copies.len(), 2);
}

#[tokio::test]
async fn a_leftover_file_not_whole_is_removed_and_one_whole_with_no_record_is_taken_in_only_when_newer() {
    // A copy whole under its name with no record, as a stop between the rename and the record leaves it.
    let e = env(7, false);
    let first = e.step(T0 - 1000).await;
    let orphan = first.taken.clone().expect("a copy");
    let fresh = env(7, false);
    std::fs::create_dir_all(&fresh.plan.dir).unwrap();
    std::fs::copy(e.plan.dir.join(&orphan), fresh.plan.dir.join(&orphan)).unwrap();
    std::fs::write(fresh.plan.dir.join(format!("{orphan}.raw.partial")), "half").unwrap();
    let out = fresh.step(T0).await;
    assert_eq!(out.taken, None, "the orphan is the day's copy");
    assert_eq!(fresh.rows().copies.len(), 1);
    assert_eq!(fresh.rows().copies[0].name, orphan);
    assert_eq!(fresh.partial(), Vec::<String>::new());
    // One older than the newest recorded is no leaving of the last stop: it is removed.
    let old = name_at(Time(T0 - 5 * DAY));
    std::fs::copy(fresh.plan.dir.join(&orphan), fresh.plan.dir.join(&old)).unwrap();
    fresh.step(T0 + HOUR).await;
    assert_eq!(fresh.files(), [orphan]);
    assert_eq!(fresh.rows().copies.len(), 1);
}

#[tokio::test]
async fn a_copy_holds_the_database_as_it_stood_while_another_connection_goes_on_writing() {
    let e = env(7, false);
    let path = e.plan.store.clone();
    let stop = Arc::new(AtomicUsize::new(0));
    let writing = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let conn = Connection::open(&path).expect("opens");
            conn.busy_timeout(Duration::from_secs(10)).expect("waits");
            let mut n = 0;
            while stop.load(Ordering::Relaxed) == 0 {
                conn.execute("INSERT INTO items (title, added) VALUES (?1, 2)", [format!("more {n}")])
                    .expect("a write");
                n += 1;
            }
            n
        })
    };
    let out = e.step(T0).await;
    stop.store(1, Ordering::Relaxed);
    let written = writing.join().expect("the writer ends");
    let name = out.taken.expect("a copy");
    let held = e.items_in(&name);
    assert!(held >= 50 && held <= 50 + written, "{held} of {written}");
}

#[tokio::test]
async fn the_copies_kept_are_the_newest_recorded_and_what_is_not_the_engines_is_left_alone() {
    let e = env(2, true);
    e.fake.inner().assets.insert("notes.txt".to_string(), vec![1]);
    let ours_without_record = name_at(Time(1));
    e.fake.inner().assets.insert(ours_without_record.clone(), vec![2]);
    e.step(T0).await;
    e.step(T0 + DAY + 1).await;
    // The place no longer lists the first copy, which is no error when it is pruned.
    let first = e.rows().copies[0].name.clone();
    e.fake.inner().assets.remove(&first);
    let third = e.step(T0 + 2 * DAY + 2).await;
    assert_eq!(third.failed, None);
    assert_eq!(e.rows().copies.len(), 3);
    let kept: Vec<String> = e.rows().copies[1..].iter().map(|k| k.name.clone()).collect();
    assert_eq!(e.files(), kept);
    let mut remote = e.fake.names();
    remote.sort();
    let mut want = vec!["notes.txt".to_string(), ours_without_record];
    want.extend(kept);
    want.sort();
    assert_eq!(remote, want);
    assert_eq!(third.removed.len(), 1);
    // Nothing more is done for a copy that was pruned.
    assert_eq!(e.step(T0 + 2 * DAY + 3).await, Outcome::default());
}

#[tokio::test]
async fn a_place_that_is_not_private_is_refused_before_every_upload() {
    let e = env(7, true);
    e.fake.inner().public = true;
    let out = e.step(T0).await;
    assert_eq!(out.failed.as_ref().map(|f| f.0.as_str()), Some("private"));
    assert!(e.fake.inner().uploads.is_empty());
    assert_eq!(e.rows().failures.len(), 1);
    assert_eq!(e.rows().failures[0].step, "private");
    // Made private, two copies waiting are uploaded, and it was asked before each.
    e.fake.inner().public = false;
    e.fake.inner().fail_uploads = true;
    e.step(T0 + DAY + 1).await;
    e.fake.inner().fail_uploads = false;
    let before = e.fake.inner().private_calls;
    let out = e.step(T0 + DAY + 2).await;
    assert_eq!(out.uploaded.len(), 2);
    assert_eq!(e.fake.inner().private_calls - before, 2);
}

#[tokio::test]
async fn with_no_place_off_the_machine_the_copies_stay_and_the_state_says_so() {
    let e = env(7, false);
    let out = e.step(T0).await;
    assert!(out.taken.is_some());
    assert_eq!(e.files().len(), 1);
    assert_eq!(e.report(T0).state, State::LocalOnly);
    assert_eq!(e.report(T0).pending, 0);
}

// The plan of an installation that gives no place is typed with the one that is never asked.
#[tokio::test]
async fn a_plan_with_no_place_is_typed_with_nowhere_and_keeps_the_copies_on_the_machine() {
    let e = env(7, false);
    let plan: Plan<Nowhere> = Plan {
        store: e.plan.store.clone(),
        dir: e.plan.dir.clone(),
        keep: 7,
        offsite: None,
    };
    let out = step(Time(T0), &e.disk, &plan).await;
    assert!(out.taken.is_some());
    assert_eq!(out.failed, None);
    assert!(!plan.configured());
}

// A clock that jumped ahead and came back: the copy taken in the future is not the day's copy, and it does not
// displace the copies taken since from the copies kept.
#[tokio::test]
async fn a_copy_taken_in_the_future_does_not_displace_the_copies_taken_after_it() {
    for keep in [1, 2] {
        let e = env(keep, true);
        let future = T0 + 10 * DAY;
        e.step(future).await;
        let out = e.step(T0).await;
        assert!(out.taken.is_some(), "keep {keep}: a copy is taken now");
        let rows = e.rows();
        assert_eq!(rows.copies.len(), 2);
        let newest = rows.copies[1].name.clone();
        assert_eq!(time_in(&newest), Some(Time(T0)));
        let mut kept: Vec<&str> = rows.copies[2 - keep..].iter().map(|k| k.name.as_str()).collect();
        assert!(kept.contains(&newest.as_str()));
        // The files are listed by name, and the names sort as the times do, not as the copies were recorded.
        kept.sort();
        assert_eq!(e.files(), kept, "keep {keep}");
        assert!(e.fake.names().contains(&newest), "keep {keep}");
        let state = e.report(T0 + 1000).state;
        if keep == 2 {
            assert_eq!(state, State::Clock);
        } else {
            assert_eq!(state, State::Ok);
        }
    }
}

// Uploads that fail for more days than copies are kept, and then work: only the copies kept are uploaded, nothing is
// tried for the rest, and no failure is recorded for them.
#[tokio::test]
async fn a_copy_past_keeping_that_was_never_uploaded_costs_nothing() {
    let e = env(2, true);
    e.fake.inner().fail_uploads = true;
    for day in 0..4 {
        e.step(T0 + day * (DAY + 1)).await;
    }
    assert_eq!(e.rows().copies.len(), 4);
    let failures = e.rows().failures.len();
    e.fake.inner().fail_uploads = false;
    let at = T0 + 4 * (DAY + 1);
    let out = e.step(at).await;
    assert_eq!(out.failed, None);
    let kept: Vec<String> = e.rows().copies[e.rows().copies.len() - 2..]
        .iter()
        .map(|k| k.name.clone())
        .collect();
    assert_eq!(e.fake.inner().uploads.len(), 2, "{:?}", e.fake.inner().uploads);
    assert_eq!(e.fake.names(), kept);
    assert_eq!(
        e.rows().failures.len(),
        failures,
        "no failure for the copies past keeping"
    );
    assert_eq!(e.report(at).state, State::Ok);
    // The files past keeping are gone and nothing is done for them later.
    assert_eq!(e.files(), kept);
    assert_eq!(e.step(at + HOUR).await, Outcome::default());
}

#[tokio::test]
async fn keep_lowered_over_a_copy_not_uploaded_leaves_it_alone() {
    let mut e = env(3, true);
    e.fake.inner().fail_uploads = true;
    for day in 0..3 {
        e.step(T0 + day * (DAY + 1)).await;
    }
    let failures = e.rows().failures.len();
    e.fake.inner().fail_uploads = false;
    e.plan.keep = 1;
    let at = T0 + 3 * (DAY + 1) - DAY / 2;
    let out = e.step(at).await;
    assert_eq!(out.failed, None);
    assert_eq!(out.uploaded.len(), 1);
    assert_eq!(e.files().len(), 1);
    assert_eq!(e.rows().failures.len(), failures);
    assert_eq!(e.report(at).state, State::Ok);
}

// A failure to prune in the step that also took and uploaded a copy is shown, and a step that ends clean mends it.
#[tokio::test]
async fn a_failed_prune_is_shown_even_in_the_step_that_took_a_copy_and_is_mended_by_the_next_clean_step() {
    let e = env(1, true);
    e.step(T0).await;
    e.fake.inner().fail_removes = true;
    let out = e.step(T0 + DAY + 1).await;
    assert!(out.taken.is_some());
    assert_eq!(out.uploaded.len(), 1);
    assert_eq!(out.failed.as_ref().map(|f| f.0.as_str()), Some("prune"));
    assert!(matches!(
        e.report(T0 + DAY + 2).state,
        State::Failed { ref step, .. } if step == "prune"
    ));
    e.fake.inner().fail_removes = false;
    let again = e.step(T0 + DAY + HOUR).await;
    assert_eq!(again.failed, None);
    assert_eq!(e.report(T0 + DAY + HOUR).state, State::Ok);
    let newest = e.rows().copies[1].name.clone();
    assert_eq!(e.files(), std::slice::from_ref(&newest));
    assert_eq!(e.fake.names(), [newest]);
}

// A failure that is about no copy, such as the files of the directory, is mended by the next step that ends clean.
#[tokio::test]
async fn a_failure_about_no_copy_is_mended_by_a_later_clean_step() {
    let e = env(7, true);
    e.step(T0).await;
    e.disk
        .failed(Time(T0 + 10), "-", "snapshot", "the directory cannot be read")
        .unwrap();
    assert!(matches!(e.report(T0 + 20).state, State::Failed { .. }));
    assert_eq!(e.step(T0 + HOUR).await.failed, None);
    assert_eq!(e.report(T0 + HOUR).state, State::Ok);
    // A clean step with nothing to mend records nothing.
    let records = e.rows();
    e.step(T0 + 2 * HOUR).await;
    assert_eq!(e.rows(), records);
}

fn kept(name: &str, seq: u64, taken: u64) -> store::Kept {
    store::Kept {
        seq,
        name: name.to_string(),
        taken: Time(taken),
    }
}

fn failure(seq: u64, at: u64, name: &str, step: &str) -> store::Failure {
    store::Failure {
        seq,
        at: Time(at),
        name: name.to_string(),
        step: step.to_string(),
        why: "because".to_string(),
    }
}

#[test]
fn the_state_is_worked_out_from_the_records_and_the_time_now() {
    let a = name_at(Time(T0));
    let one = |uploaded: bool| {
        let mut r = Records::default();
        r.copies.push(kept(&a, 1, T0));
        if uploaded {
            r.uploads.insert(a.clone(), Time(T0 + 5));
        }
        r
    };
    let now = Time(T0 + HOUR);
    assert_eq!(report(&one(true), now, true, 7).state, State::Ok);
    assert_eq!(report(&one(false), now, true, 7).state, State::Pending { n: 1 });
    assert_eq!(report(&one(true), now, false, 7).state, State::LocalOnly);
    // Too old: a day and an hour passed since the newest copy.
    assert_eq!(
        report(&one(true), Time(T0 + DAY + HOUR + 1), true, 7).state,
        State::Stale { since: Some(Time(T0)) }
    );
    assert_eq!(
        report(&Records::default(), now, true, 7).state,
        State::Stale { since: None }
    );
    // A copy taken after a day from now.
    assert_eq!(report(&one(true), Time(T0 - DAY - 1), true, 7).state, State::Clock);
    // A failure shows until a step ends with nothing failing after it.
    let mut r = one(true);
    r.failures.push(failure(1, T0 + 10, &a, "upload"));
    let shown = report(&r, now, true, 7);
    assert_eq!(
        shown.state,
        State::Failed {
            at: Time(T0 + 10),
            step: "upload".to_string(),
            why: "because".to_string()
        }
    );
    assert_eq!(shown.last_failure.map(|f| f.step), Some("upload".to_string()));
    // An upload does not mend it, nor does a step that mended the failures before it; a step that mended it does.
    r.uploads.insert("another".to_string(), Time(T0 + 20));
    r.mended = Some(0);
    assert!(matches!(report(&r, now, true, 7).state, State::Failed { .. }));
    r.mended = Some(1);
    assert_eq!(report(&r, now, true, 7).state, State::Ok);
    // A failure of any step is mended so, one about no copy as much as one about a copy.
    let mut taking = one(true);
    taking.failures.push(failure(1, T0 + 10, "-", "snapshot"));
    taking
        .failures
        .push(failure(2, T0 + 12, "store-0000000000001.sqlite.gz", "prune"));
    assert!(matches!(report(&taking, now, true, 7).state, State::Failed { .. }));
    taking.mended = Some(2);
    assert_eq!(report(&taking, now, true, 7).state, State::Ok);
    // What is after is told by the order recorded in, not by time: a failure recorded after a clean step is shown
    // though its time is before the time of the step.
    taking.failures.push(failure(3, T0, "-", "snapshot"));
    assert!(matches!(report(&taking, now, true, 7).state, State::Failed { .. }));
}

#[test]
fn nothing_in_the_step_or_the_state_reads_the_clock() {
    for (name, text) in [
        ("lib.rs", include_str!("lib.rs")),
        ("state.rs", include_str!("state.rs")),
        ("snapshot.rs", include_str!("snapshot.rs")),
        ("ground.rs", include_str!("ground.rs")),
    ] {
        let code = text.split("#[cfg(test)]").next().unwrap_or(text);
        for forbidden in ["SystemTime", "Instant::now", "UNIX_EPOCH"] {
            assert!(!code.contains(forbidden), "{name} says {forbidden}");
        }
    }
}

/// A place off the machine whose first upload waits for the test to let it go.
#[derive(Clone)]
struct Slow {
    inner: Fake,
    started: Arc<AtomicUsize>,
    go: Arc<Notify>,
}

impl Offsite for Slow {
    async fn private(&self) -> Result<bool, String> {
        self.inner.private().await
    }

    async fn upload(&self, file: &Path) -> Result<(), String> {
        self.started.fetch_add(1, Ordering::SeqCst);
        self.go.notified().await;
        self.inner.upload(file).await
    }

    async fn list(&self) -> Result<Vec<String>, String> {
        self.inner.list().await
    }

    async fn remove(&self, name: &str) -> Result<(), String> {
        self.inner.remove(name).await
    }
}

// A tick that comes while a step is running starts no second step: the task takes one step at a time.
#[tokio::test(start_paused = true)]
async fn a_tick_during_a_running_step_starts_no_second_one() {
    let e = env(7, true);
    let started = Arc::new(AtomicUsize::new(0));
    let go = Arc::new(Notify::new());
    let plan = Plan {
        store: e.plan.store.clone(),
        dir: e.plan.dir.clone(),
        keep: 7,
        offsite: Some(Slow {
            inner: e.fake.clone(),
            started: started.clone(),
            go: go.clone(),
        }),
    };
    let steps = Arc::new(AtomicUsize::new(0));
    let counted = steps.clone();
    let clock = Arc::new(AtomicUsize::new(0));
    let ground = Disk::open(&e.plan.store, &e.plan.dir).expect("the records open");
    let task = tokio::spawn(keep(
        plan,
        ground,
        move || {
            let at = T0 + clock.fetch_add(1, Ordering::SeqCst) as u64 * HOUR;
            async move { Ok(Time(at)) }
        },
        move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
        },
    ));
    // Let the step reach the upload, which is held.
    while started.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    // Five hours of ticks pass while it is held.
    tokio::time::advance(Duration::from_millis(5 * HOUR)).await;
    assert_eq!(started.load(Ordering::SeqCst), 1);
    assert_eq!(
        steps.load(Ordering::SeqCst),
        0,
        "no step has ended, and none has begun beside it"
    );
    // Let it go: it ends, and the next step begins an hour after.
    go.notify_one();
    while steps.load(Ordering::SeqCst) == 0 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    tokio::time::advance(Duration::from_millis(HOUR)).await;
    while steps.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(e.fake.inner().uploads.len(), 1, "one copy, uploaded once");
    assert_eq!(e.rows().copies.len(), 1);
    task.abort();
}

// With the network gone the upload fails as any other does, as a row; the owner's pause of everything is nothing a step
// reads. The step is tried again at the next period, and the copy, taken while the place was out of reach, is there.
#[tokio::test]
async fn with_the_place_out_of_reach_the_copy_is_taken_the_failure_recorded_and_the_next_step_uploads() {
    let e = env(7, true);
    e.fake.inner().fail_at.extend(0..3);
    let out = e.step(T0).await;
    assert!(out.taken.is_some());
    assert_eq!(out.failed.as_ref().map(|f| f.0.as_str()), Some("private"));
    assert_eq!(e.rows().failures.len(), 1);
    assert!(matches!(e.report(T0 + 1).state, State::Failed { .. }));
    // Each call fails once: the next two periods are tried and fail the same, each one a row, and no second copy is
    // taken; the fourth reaches the place.
    for k in 1..=2 {
        let again = e.step(T0 + k * HOUR).await;
        assert_eq!(again.taken, None);
        assert_eq!(
            again.failed.as_ref().map(|f| f.0.as_str()),
            Some("private"),
            "period {k}"
        );
    }
    assert_eq!(e.rows().failures.len(), 3);
    let reached = e.step(T0 + 3 * HOUR).await;
    assert_eq!(reached.taken, None);
    assert_eq!(reached.uploaded.len(), 1);
    assert_eq!(e.rows().copies.len(), 1);
    assert_eq!(e.report(T0 + 3 * HOUR).state, State::Ok);
}

// A prune that failed is shown even after a step that took a copy, and the prune done at the next step mends it.
#[tokio::test]
async fn a_failed_prune_in_the_step_that_took_a_copy_is_shown_and_mended_when_done() {
    let e = env(1, true);
    e.step(T0).await;
    e.fake.inner().fail_removes = true;
    e.step(T0 + DAY + 1).await;
    let out = e.step(T0 + DAY + HOUR).await;
    assert_eq!(out.failed.as_ref().map(|f| f.0.as_str()), Some("prune"), "{out:?}");
    e.fake.inner().fail_removes = false;
    let out = e.step(T0 + DAY + 2 * HOUR).await;
    assert_eq!(out.failed, None);
    assert_eq!(e.files().len(), 1, "the prune was done");
    assert_eq!(e.report(T0 + DAY + 2 * HOUR).state, State::Ok);
}

// Reference: acceptance 5 (the uploaded assets are pruned to the `keep` newest rows recorded; a missing local file is
// no error). A copy whose local file is gone is still a row past keeping: its asset is to be pruned.
#[tokio::test]
async fn an_asset_whose_local_file_is_gone_is_still_pruned_when_it_falls_past_keeping() {
    let e = env(1, true);
    e.step(T0).await;
    let first = e.rows().copies[0].name.clone();
    std::fs::remove_file(e.plan.dir.join(&first)).expect("the file is removed");
    let out = e.step(T0 + DAY + 1).await;
    assert_eq!(out.failed, None);
    let second = e.rows().copies[1].name.clone();
    assert_eq!(
        e.fake.names(),
        vec![second],
        "the first copy's asset is pruned with its row"
    );
}

// Reference: acceptance 11 (only a success after a failure hides it) read with acceptance 12 (the order recorded, not
// the time, is what counts once the clock has gone ahead and come back). A failure recorded after a clean step is
// after it, whatever time the clock gave it.
#[tokio::test]
async fn a_failure_after_a_clean_step_is_shown_when_the_clock_has_come_back() {
    let e = env(1, true);
    let future = T0 + 10 * DAY;
    e.fake.inner().fail_uploads = true;
    e.step(future).await;
    e.fake.inner().fail_uploads = false;
    let mended = e.step(future + HOUR).await;
    assert_eq!(mended.failed, None);
    assert!(!matches!(e.report(future + HOUR).state, State::Failed { .. }));
    // The clock is back before them: a copy is taken and its upload fails.
    e.fake.inner().fail_uploads = true;
    let out = e.step(T0).await;
    assert_eq!(out.failed.as_ref().map(|f| f.0.as_str()), Some("upload"), "{out:?}");
    let state = e.report(T0 + 1000).state;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
}

// A failure with the same time, copy and step as one a clean step mended is a new failure: it takes its own place in
// the order, and the mending of the first does not hide it.
#[test]
fn a_failure_like_a_mended_one_in_time_copy_and_step_is_shown_again() {
    let e = env(7, false);
    e.disk.failed(Time(T0), "-", "prune", "the first").unwrap();
    e.disk.ended(Time(T0 + 1), 1).unwrap();
    assert!(!matches!(e.report(T0 + 2).state, State::Failed { .. }));
    e.disk.failed(Time(T0), "-", "prune", "the second").unwrap();
    let records = e.rows();
    assert_eq!(records.failures.len(), 2);
    assert_eq!(records.failures[1].seq, 2);
    let state = e.report(T0 + 2).state;
    assert!(
        matches!(&state, State::Failed { why, .. } if why == "the second"),
        "{state:?}"
    );
}
