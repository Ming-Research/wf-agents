//! The copies, tested on real files and a real store, with the place off the machine made by a fake that can fail at
//! any call, and the process stopped at every point a step leaves a mark.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rusqlite::Connection;
use store::{Records, Store};
use tempfile::TempDir;

use super::*;

const T0: u64 = 1_700_000_000_000;
const HOUR: u64 = TICK;
const DAY: u64 = PERIOD;

/// A place off the machine that keeps its assets in memory and fails at the calls it is told to.
#[derive(Default)]
struct Fake {
    inner: Mutex<Inner>,
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
    fn private(&self) -> Result<bool, String> {
        self.call()?;
        let mut inner = self.inner();
        inner.private_calls += 1;
        Ok(!inner.public)
    }

    fn upload(&self, file: &Path) -> Result<(), String> {
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

    fn list(&self) -> Result<Vec<String>, String> {
        self.call()?;
        Ok(self.inner().assets.keys().cloned().collect())
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        self.call()?;
        self.inner().assets.remove(name);
        Ok(())
    }
}

/// A store with some items in it, a directory for the copies, and a plan over them.
struct Env {
    _dir: TempDir,
    _store: Store,
    plan: Plan,
    fake: Arc<Fake>,
    records: Backups,
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
    let fake = Arc::new(Fake::default());
    let plan = Plan {
        store: path.clone(),
        dir: dir.path().join("backups"),
        keep,
        offsite: off.then(|| -> Arc<dyn Offsite> { fake.clone() }),
    };
    let records = Backups::open(&path).expect("the records open");
    Env {
        _dir: dir,
        _store: store,
        plan,
        fake,
        records,
    }
}

impl Env {
    fn step(&self, at: u64) -> Outcome {
        step_with(Time(at), &self.records, &self.plan, &mut || false)
    }

    fn report(&self, at: u64) -> Report {
        report(
            &self.records.records().expect("the records"),
            Time(at),
            self.plan.configured(),
            self.plan.keep,
        )
    }

    fn rows(&self) -> Records {
        self.records.records().expect("the records")
    }

    /// The names of the files whole under the engine's names.
    fn files(&self) -> Vec<String> {
        let mut found: Vec<String> = (std::fs::read_dir(&self.plan.dir).into_iter().flatten())
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| time_in(n).is_some())
            .collect();
        found.sort();
        found
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

/// Stops after this many marks.
fn stop_after(n: usize) -> impl FnMut() -> bool {
    let mut left = n;
    move || {
        if left == 0 {
            true
        } else {
            left -= 1;
            false
        }
    }
}

// Acceptance 1, 2, 3: a step stopped at any point, with the place off the machine failing at any one call, is
// finished by the steps after it: one copy, whole, uploaded once, no leavings.
#[test]
fn a_step_stopped_anywhere_and_a_call_failing_anywhere_come_to_one_whole_copy_uploaded_once() {
    let mut stopped_at = 0;
    for fail in [None, Some(0), Some(1), Some(2)] {
        for n in 0..40 {
            let e = env(7, true);
            if let Some(f) = fail {
                e.fake.inner().fail_at.insert(f);
            }
            let first = step_with(Time(T0), &e.records, &e.plan, &mut stop_after(n));
            // Nothing under a final name is ever not whole.
            for name in e.files() {
                assert_eq!(e.items_in(&name), 50, "fail {fail:?}, stop {n}: {name}");
            }
            // The process starts again; each step after goes on from the records and the files.
            for k in 1..=3 {
                e.step(T0 + k * 1000);
            }
            let rows = e.rows();
            assert_eq!(rows.copies.len(), 1, "fail {fail:?}, stop {n}: {:?}", rows.copies);
            assert_eq!(e.files().len(), 1, "fail {fail:?}, stop {n}");
            assert_eq!(e.partial(), Vec::<String>::new(), "fail {fail:?}, stop {n}");
            let name = rows.copies[0].name.clone();
            assert_eq!(e.fake.names(), std::slice::from_ref(&name), "fail {fail:?}, stop {n}");
            assert_eq!(
                e.fake.inner().uploads,
                std::slice::from_ref(&name),
                "uploaded once: fail {fail:?}, stop {n}"
            );
            assert!(rows.uploads.contains_key(&name), "fail {fail:?}, stop {n}");
            assert_eq!(e.report(T0 + 5000).state, State::Ok, "fail {fail:?}, stop {n}");
            if first.stopped {
                stopped_at = stopped_at.max(n);
            }
        }
    }
    assert!(stopped_at >= 5, "the step has marks to stop at: {stopped_at}");
}

#[test]
fn a_copy_is_taken_once_a_day_and_uploaded_when_the_upload_failed_before() {
    let e = env(7, true);
    e.fake.inner().fail_uploads = true;
    let first = e.step(T0);
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
    let again = e.step(T0 + HOUR);
    assert_eq!(again.taken, None);
    assert_eq!(again.uploaded.len(), 1);
    assert_eq!(e.rows().copies.len(), 1);
    assert_eq!(e.report(T0 + HOUR).state, State::Ok);
    // Nothing more within the day, and a copy when the day is out.
    assert_eq!(e.step(T0 + 2 * HOUR), Outcome::default());
    let next = e.step(T0 + DAY + 1);
    assert!(next.taken.is_some());
    assert_eq!(e.rows().copies.len(), 2);
}

#[test]
fn a_leftover_file_not_whole_is_removed_and_one_whole_with_no_record_is_taken_in_only_when_newer() {
    // A copy whole under its name with no record, as a stop between the rename and the record leaves it.
    let e = env(7, false);
    let first = e.step(T0 - 1000);
    let orphan = first.taken.clone().expect("a copy");
    let fresh = env(7, false);
    std::fs::create_dir_all(&fresh.plan.dir).unwrap();
    std::fs::copy(e.plan.dir.join(&orphan), fresh.plan.dir.join(&orphan)).unwrap();
    std::fs::write(fresh.plan.dir.join(format!("{orphan}.raw.partial")), "half").unwrap();
    let out = fresh.step(T0);
    assert_eq!(out.taken, None, "the orphan is the day's copy");
    assert_eq!(fresh.rows().copies.len(), 1);
    assert_eq!(fresh.rows().copies[0].name, orphan);
    assert_eq!(fresh.partial(), Vec::<String>::new());
    // One older than the newest recorded is no leaving of the last stop: it is removed.
    let old = name_at(Time(T0 - 5 * DAY));
    std::fs::copy(fresh.plan.dir.join(&orphan), fresh.plan.dir.join(&old)).unwrap();
    fresh.step(T0 + HOUR);
    assert_eq!(fresh.files(), [orphan]);
    assert_eq!(fresh.rows().copies.len(), 1);
}

#[test]
fn a_copy_holds_the_database_as_it_stood_while_another_connection_goes_on_writing() {
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
    let out = e.step(T0);
    stop.store(1, Ordering::Relaxed);
    let written = writing.join().expect("the writer ends");
    let name = out.taken.expect("a copy");
    let held = e.items_in(&name);
    assert!(held >= 50 && held <= 50 + written, "{held} of {written}");
}

#[test]
fn the_copies_kept_are_the_newest_recorded_and_what_is_not_the_engines_is_left_alone() {
    let e = env(2, true);
    e.fake.inner().assets.insert("notes.txt".to_string(), vec![1]);
    let ours_without_record = name_at(Time(1));
    e.fake.inner().assets.insert(ours_without_record.clone(), vec![2]);
    e.step(T0);
    e.step(T0 + DAY + 1);
    // The place no longer lists the first copy, which is no error when it is pruned.
    let first = e.rows().copies[0].name.clone();
    e.fake.inner().assets.remove(&first);
    let third = e.step(T0 + 2 * DAY + 2);
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
    assert_eq!(e.step(T0 + 2 * DAY + 3), Outcome::default());
}

#[test]
fn a_place_that_is_not_private_is_refused_before_every_upload() {
    let e = env(7, true);
    e.fake.inner().public = true;
    let out = e.step(T0);
    assert_eq!(out.failed.as_ref().map(|f| f.0.as_str()), Some("private"));
    assert!(e.fake.inner().uploads.is_empty());
    assert_eq!(e.rows().failures.len(), 1);
    assert_eq!(e.rows().failures[0].step, "private");
    // Made private, two copies waiting are uploaded, and it was asked before each.
    e.fake.inner().public = false;
    e.fake.inner().fail_uploads = true;
    e.step(T0 + DAY + 1);
    e.fake.inner().fail_uploads = false;
    let before = e.fake.inner().private_calls;
    let out = e.step(T0 + DAY + 2);
    assert_eq!(out.uploaded.len(), 2);
    assert_eq!(e.fake.inner().private_calls - before, 2);
}

#[test]
fn with_no_place_off_the_machine_the_copies_stay_and_the_state_says_so() {
    let e = env(7, false);
    let out = e.step(T0);
    assert!(out.taken.is_some());
    assert_eq!(e.files().len(), 1);
    assert_eq!(e.report(T0).state, State::LocalOnly);
    assert_eq!(e.report(T0).pending, 0);
}

// A clock that jumped ahead and came back: the copy taken in the future is not the day's copy, and it does not
// displace the copies taken since from the copies kept.
#[test]
fn a_copy_taken_in_the_future_does_not_displace_the_copies_taken_after_it() {
    for keep in [1, 2] {
        let e = env(keep, true);
        let future = T0 + 10 * DAY;
        e.step(future);
        let out = e.step(T0);
        assert!(out.taken.is_some(), "keep {keep}: a copy is taken now");
        let rows = e.rows();
        assert_eq!(rows.copies.len(), 2);
        let newest = rows.copies[1].name.clone();
        assert_eq!(time_in(&newest), Some(Time(T0)));
        let kept: Vec<&str> = rows.copies[2 - keep..].iter().map(|k| k.name.as_str()).collect();
        assert!(kept.contains(&newest.as_str()));
        let mut files = e.files();
        files.sort();
        assert_eq!(files, kept, "keep {keep}");
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
#[test]
fn a_copy_past_keeping_that_was_never_uploaded_costs_nothing() {
    let e = env(2, true);
    e.fake.inner().fail_uploads = true;
    for day in 0..4 {
        e.step(T0 + day * (DAY + 1));
    }
    assert_eq!(e.rows().copies.len(), 4);
    let failures = e.rows().failures.len();
    e.fake.inner().fail_uploads = false;
    let at = T0 + 4 * (DAY + 1);
    let out = e.step(at);
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
    assert_eq!(e.step(at + HOUR), Outcome::default());
}

#[test]
fn keep_lowered_over_a_copy_not_uploaded_leaves_it_alone() {
    let mut e = env(3, true);
    e.fake.inner().fail_uploads = true;
    for day in 0..3 {
        e.step(T0 + day * (DAY + 1));
    }
    let failures = e.rows().failures.len();
    e.fake.inner().fail_uploads = false;
    e.plan.keep = 1;
    let at = T0 + 3 * (DAY + 1) - DAY / 2;
    let out = e.step(at);
    assert_eq!(out.failed, None);
    assert_eq!(out.uploaded.len(), 1);
    assert_eq!(e.files().len(), 1);
    assert_eq!(e.rows().failures.len(), failures);
    assert_eq!(e.report(at).state, State::Ok);
}

fn kept(name: &str, seq: u64, taken: u64) -> store::Kept {
    store::Kept {
        seq,
        name: name.to_string(),
        taken: Time(taken),
    }
}

fn failure(at: u64, name: &str, step: &str) -> store::Failure {
    store::Failure {
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
    // A failure shows until an upload after it, and a failure of the upload of a copy no longer kept does not show.
    let mut r = one(true);
    r.failures.push(failure(T0 + 10, &a, "upload"));
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
    r.uploads.insert("another".to_string(), Time(T0 + 20));
    assert_eq!(report(&r, now, true, 7).state, State::Ok);
    let mut gone = one(true);
    gone.failures
        .push(failure(T0 + 10, "store-0000000000001.sqlite.gz", "upload"));
    assert_eq!(report(&gone, now, true, 7).state, State::Ok);
    // A failure to take a copy shows until a copy is taken after it.
    let mut taking = Records::default();
    taking.copies.push(kept(&a, 1, T0));
    taking.uploads.insert(a.clone(), Time(T0));
    taking.failures.push(failure(T0 + DAY, "-", "snapshot"));
    assert!(matches!(
        report(&taking, Time(T0 + DAY + 1), true, 7).state,
        State::Failed { .. }
    ));
    taking.copies.push(kept(&name_at(Time(T0 + DAY + 5)), 2, T0 + DAY + 5));
    taking.uploads.insert(name_at(Time(T0 + DAY + 5)), Time(T0 + DAY + 6));
    assert_eq!(report(&taking, Time(T0 + DAY + 10), true, 7).state, State::Ok);
}

/// A `gh` that keeps one release in a directory: it is not there until made, and every call fails while the file
/// `down` is there.
#[cfg(unix)]
const FAKE_GH: &str = r#"#!/bin/sh
d='DIR'
if [ -f "$d/down" ]; then echo "network is down" >&2; exit 1; fi
case "$1 $2" in
"release view")
  if [ ! -f "$d/release" ]; then echo "release not found" >&2; exit 1; fi
  case "$*" in *assets*) cat "$d/assets" ;; esac ;;
"release create") touch "$d/release"; : > "$d/assets" ;;
"release upload") basename "$4" >> "$d/assets" ;;
*) echo "unexpected: $*" >&2; exit 2 ;;
esac
"#;

// The first upload to a place with no release made yet works, and a listing that fails for a real reason is an error,
// never an empty place.
#[cfg(unix)]
#[test]
fn the_first_upload_to_a_place_with_no_release_works_and_a_failing_listing_is_no_empty_place() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("a directory");
    let script = dir.path().join("gh");
    std::fs::write(&script, FAKE_GH.replace("DIR", &dir.path().display().to_string())).expect("written");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("made runnable");
    let gh = Gh::new("owner/name", "token", &script.display().to_string());
    std::fs::write(dir.path().join("down"), "").expect("written");
    assert!(gh.list().is_err(), "a place that cannot be reached is no empty place");
    std::fs::remove_file(dir.path().join("down")).expect("removed");
    assert_eq!(gh.list(), Ok(vec![]), "no release is an empty place");
    // A copy is looked for, not found, and uploaded.
    let name = name_at(Time(T0));
    let copy = dir.path().join(&name);
    std::fs::write(&copy, "copy").expect("written");
    gh.upload(&copy).expect("the first upload");
    assert_eq!(gh.list(), Ok(vec![name]));
}

#[test]
fn the_arguments_of_gh_name_the_release_the_repository_and_the_file() {
    let gh = Gh::new("owner/name", "token", "gh");
    assert_eq!(gh.private_args(), ["api", "repos/owner/name", "--jq", ".private"]);
    assert_eq!(
        gh.upload_args(Path::new("/d/store-1.sqlite.gz")),
        [
            "release",
            "upload",
            "database-backups",
            "/d/store-1.sqlite.gz",
            "--repo",
            "owner/name",
            "--clobber"
        ]
    );
    assert_eq!(
        gh.list_args(),
        [
            "release",
            "view",
            "database-backups",
            "--repo",
            "owner/name",
            "--json",
            "assets",
            "--jq",
            ".assets[].name"
        ]
    );
    assert_eq!(
        gh.remove_args("store-1.sqlite.gz"),
        [
            "release",
            "delete-asset",
            "database-backups",
            "store-1.sqlite.gz",
            "--repo",
            "owner/name",
            "--yes"
        ]
    );
    assert!(gh.create_args().contains(&"--latest=false".to_string()));
    assert!(gh.view_args().contains(&"database-backups".to_string()));
}

#[test]
fn nothing_in_the_step_or_the_state_reads_the_clock() {
    for (name, text) in [
        ("lib.rs", include_str!("lib.rs")),
        ("state.rs", include_str!("state.rs")),
        ("snapshot.rs", include_str!("snapshot.rs")),
    ] {
        let code = text.split("#[cfg(test)]").next().unwrap_or(text);
        for forbidden in ["SystemTime", "Instant::now", "UNIX_EPOCH"] {
            assert!(!code.contains(forbidden), "{name} says {forbidden}");
        }
    }
}

// A tick that comes while a step is running starts no second step: the task takes one step at a time.
#[tokio::test(start_paused = true)]
async fn a_tick_during_a_running_step_starts_no_second_one() {
    let e = env(7, true);
    // The first upload waits for the test to let it go.
    let (go, wait) = std::sync::mpsc::channel::<()>();
    let started = Arc::new(AtomicUsize::new(0));
    struct Slow {
        inner: Arc<Fake>,
        started: Arc<AtomicUsize>,
        wait: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl Offsite for Slow {
        fn private(&self) -> Result<bool, String> {
            self.inner.private()
        }
        fn upload(&self, file: &Path) -> Result<(), String> {
            self.started.fetch_add(1, Ordering::SeqCst);
            let _ = self.wait.lock().expect("held").recv();
            self.inner.upload(file)
        }
        fn list(&self) -> Result<Vec<String>, String> {
            self.inner.list()
        }
        fn remove(&self, name: &str) -> Result<(), String> {
            self.inner.remove(name)
        }
    }
    let plan = Plan {
        offsite: Some(Arc::new(Slow {
            inner: e.fake.clone(),
            started: started.clone(),
            wait: Mutex::new(wait),
        })),
        ..e.plan.clone()
    };
    let steps = Arc::new(AtomicUsize::new(0));
    let counted = steps.clone();
    let clock = Arc::new(AtomicUsize::new(0));
    let task = tokio::spawn(keep(
        plan,
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
    go.send(()).expect("released");
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
