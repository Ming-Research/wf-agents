//! The items as the pages read them, kept between reads: each item's summary, with its JSON, is worked out again only
//! where what it is made from changed, and a read gives the page only the items that changed since the cursor it was
//! last given.
//!
//! What a summary is made from ([`Inputs`]) is cheap to read and holds none of the records' content: how far the
//! work's records and its item's changes have come, where each of its round and calls under way stands as a CI run,
//! the room the engine leaves it (the pause of everything, a slot, the turn), the items it waits for, and what holds
//! every work at once. One more thing changes it with no record: time, for a work with a round or a call under way,
//! whose time bar counts what is under way to now; such a summary is worked out again once the bound has passed.
//! [`Item::of`] is what a summary is worked out by, and stays the one reading of it: the cache only decides when to
//! call it again.
//!
//! What git says of a work (the lines its work changed) is not worked out in a read. A read gives what the last look
//! gave and, where that is old, starts one look off the loop and does not wait for it ([`ItemsCache::look`]); where it
//! ends, the pages are told with a notice of the items. An item no look has ended for is waited for. A look that
//! failed gives no lines, as a look that cannot be made always has, and is made again once the bound has passed. The
//! rule for when what git says is old is here and nowhere else.

use std::collections::BTreeMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use decide::Room;
use driver::{Along, Driver, Under};
use service::{Handle, Levels, OfWork, Refusal, Seen};
use store::WorkNumber;
use work::Time;

use crate::works::{Broken, EveryWork, InstallationWait, Item, Items, Now, Others, Outages, Summary, asked};

/// How long a summary of a work with something under way, and what git says of such a work, are kept before they
/// are worked out again: the page reads again at this interval while such a work is on its screen.
pub const BOUND_MS: u64 = 30_000;

/// What a read of the items could not give: the loop refused it, or the records name what the work does not hold.
pub(crate) enum Failure {
    Refused(Refusal),
    Broken(Broken),
}

/// What a summary is made from, besides the work's records as far as they have come ([`Inputs::moments`]).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Inputs {
    /// How many things have happened to the work: every entry of its records belongs to one.
    moments: usize,
    /// How many changes were made to its item.
    item_changes: usize,
    /// Each round or call of the work under way that stands as a CI run, with its link and the runner it waits for
    /// where it waits for one. What the rest of a round under way has come to changes with every call and is no part of
    /// a summary.
    runs: Vec<(Under, String, Option<String>)>,
    /// What the engine leaves the work room for, which says what holds its next round back.
    room: Room,
    /// The items it waits for, those its pause waits for and whose turn to check and merge it is.
    others: Others,
}

impl Inputs {
    fn of(
        driver: &Driver,
        number: WorkNumber,
        moments: usize,
        item_changes: usize,
        at_once: &driver::AtOnce,
    ) -> Inputs {
        Inputs {
            moments,
            item_changes,
            runs: (driver.along_of(number).into_iter())
                .filter_map(|(under, along)| match along {
                    Along::Runs { at, queued_for } => Some((under, at.clone(), queued_for.clone())),
                    Along::Going(_) => None,
                })
                .collect(),
            room: driver.room_at(number, at_once),
            others: Others::of(driver, number, at_once),
        }
    }
}

/// What the last look at a work's repository came to.
#[derive(Clone, Debug)]
enum LastLook {
    /// No look has ended.
    Never,
    /// The last look could not be made, when it ended.
    Failed { at: Time },
    /// The last look ended well: what it saw of the work as it stood after this many things had happened to it, when it
    /// ended.
    Looked { moments: usize, at: Time, seen: Seen },
}

/// An item as the cache keeps it.
struct Entry {
    work: WorkNumber,
    key: Inputs,
    built_at: Time,
    /// Whether a round of the work or a tool run for it was under way as the summary was worked out: only then does
    /// time and the worktree change it with no record.
    under_way: bool,
    /// The item as [`Item::of`] gives it, with no lines of what git says.
    base: Item,
    looked: LastLook,
    /// Whether a look at the work is under way, off the loop.
    looking: bool,
    /// The item as the pages are given it, written out.
    json: String,
    /// The place of the last change of what the pages are given, among the changes of every item.
    rev: u64,
}

/// What a check of the cache against the driver found: the items no look has ended for.
pub(crate) struct Checked {
    never: Vec<u64>,
}

struct Inner {
    /// Told apart from every other process's, so that a cursor of an engine that was stopped gives everything.
    boot: String,
    /// How many changes of an item there have been.
    rev: u64,
    entries: BTreeMap<u64, Entry>,
    /// What holds every work at once as the last check found it, which every summary is made from.
    outages: Option<Outages>,
    /// What the whole installation waits on the owner for as the last check found it.
    waits: Vec<InstallationWait>,
}

/// The items the pages read, worked out again only where they changed.
pub struct ItemsCache {
    handle: Handle,
    levels: Levels,
    bound: u64,
    inner: Mutex<Inner>,
    /// Told each time a look ends.
    looked: tokio::sync::Notify,
    /// How many summaries have been worked out.
    built: AtomicUsize,
}

impl ItemsCache {
    /// A cache of the items of this loop's driver, whose rounds run at these levels.
    pub fn new(handle: Handle, levels: Levels) -> ItemsCache {
        ItemsCache::bounded(handle, levels, BOUND_MS)
    }

    /// The same, keeping what is under way for this many milliseconds.
    pub fn bounded(handle: Handle, levels: Levels, bound: u64) -> ItemsCache {
        ItemsCache {
            handle,
            levels,
            bound,
            inner: Mutex::new(Inner {
                boot: format!("{:x}", RandomState::new().build_hasher().finish()),
                rev: 0,
                entries: BTreeMap::new(),
                outages: None,
                waits: Vec::new(),
            }),
            looked: tokio::sync::Notify::new(),
            built: AtomicUsize::new(0),
        }
    }

    /// How many summaries have been worked out since the cache was made.
    pub fn built(&self) -> usize {
        self.built.load(Ordering::Relaxed)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // Each entry is whole once it is in.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Checks every item against the driver, as the loop's read does: works out again each whose inputs changed, and
    /// starts the looks at the repositories that are due. It reads nothing but the driver and the cache, and runs on the
    /// loop. A look is marked as under way and started together, here, so that a read the page gave up after the loop
    /// ran this leaves no item marked with no look under way.
    pub(crate) fn check(self: &Arc<Self>, driver: &Driver, now: Time) -> Result<Checked, Broken> {
        let at_once = driver.at_once();
        let outages = Outages::of(&at_once);
        let mut inner = self.lock();
        let everything_changed = inner.outages.as_ref() != Some(&outages);
        let mut every: Option<EveryWork> = None;
        let mut checked = Checked { never: Vec::new() };
        for kept in driver.works() {
            let (number, item) = (kept.head.number, kept.item.number.0);
            let moments = kept.work.moments.len();
            let key = Inputs::of(driver, number, moments, kept.item.changes.len(), &at_once);
            let stale = match inner.entries.get(&item) {
                None => true,
                Some(entry) => {
                    everything_changed
                        || entry.key != key
                        || (entry.under_way && now.0.saturating_sub(entry.built_at.0) >= self.bound)
                }
            };
            if stale {
                let every = every.get_or_insert_with(|| EveryWork::with(driver, at_once.clone()));
                let base = Item::of(kept, driver, every, &self.levels, now);
                // A work whose state or summary cannot be worked out is given as such; saying why here, where the entry
                // is worked out and not where it is read, says it once for each time it is worked out.
                if let Now::Broken { words } = &base.work.now {
                    eprintln!("{number} cannot be shown as it is: {words}");
                }
                self.built.fetch_add(1, Ordering::Relaxed);
                let (looked, looking, rev, old) = match inner.entries.remove(&item) {
                    Some(entry) => (entry.looked, entry.looking, entry.rev, Some(entry.json)),
                    None => (LastLook::Never, false, 0, None),
                };
                let mut entry = Entry {
                    work: number,
                    key,
                    built_at: now,
                    under_way: kept.work.under_way(),
                    base,
                    looked,
                    looking,
                    json: String::new(),
                    rev,
                };
                let json = written(&entry)?;
                if old.as_ref() != Some(&json) {
                    inner.rev += 1;
                    entry.rev = inner.rev;
                }
                entry.json = json;
                inner.entries.insert(item, entry);
            }
            let Some(entry) = inner.entries.get_mut(&item) else {
                continue;
            };
            let old = match &entry.looked {
                LastLook::Never => true,
                LastLook::Failed { at } => now.0.saturating_sub(at.0) >= self.bound,
                LastLook::Looked { moments: seen, at, .. } => {
                    *seen != moments || (entry.under_way && now.0.saturating_sub(at.0) >= self.bound)
                }
            };
            if old
                && !entry.looking
                && let Some(of) = asked(driver, &entry.base.work)
            {
                entry.looking = true;
                self.look(item, moments, of, now);
            }
            if matches!(entry.looked, LastLook::Never) {
                checked.never.push(item);
            }
        }
        inner.outages = Some(outages);
        inner.waits = (driver.installation_waits(&at_once).into_iter())
            .map(InstallationWait::of)
            .collect();
        Ok(checked)
    }

    /// Starts a look at this item's work, off the loop, which was asked as the work stood after this many things had
    /// happened to it and at this time, and gives what comes of it to the item: the pages are told where it is not what
    /// it was.
    fn look(self: &Arc<Self>, item: u64, moments: usize, of: OfWork, now: Time) {
        let cache = self.clone();
        tokio::spawn(async move {
            let said = (cache.handle.seen(vec![of]).await.pop())
                .unwrap_or_else(|| Err("nothing came of the look".to_string()));
            cache.ended(item, moments, now, said);
        });
    }

    /// Takes the end of the look at this item's work, which was asked as the work stood after this many things had
    /// happened to it, at this time: how old it is is counted from there, as a summary's is from when it was asked.
    /// The pages are told where what they are given changed, and where the work has moved since the look was asked: the
    /// look is then of what the work was, a read starts the look of what it is, and a page that was told nothing would
    /// not read again to have it started.
    fn ended(&self, item: u64, moments: usize, at: Time, said: Result<Seen, String>) {
        let (changed, moved) = {
            let mut inner = self.lock();
            let Some(entry) = inner.entries.get(&item) else {
                return;
            };
            let work = entry.work;
            let mut entry = match inner.entries.remove(&item) {
                Some(entry) => entry,
                None => return,
            };
            entry.looking = false;
            let moved = entry.key.moments != moments;
            entry.looked = match said {
                Ok(seen) => LastLook::Looked { moments, at, seen },
                Err(why) => {
                    eprintln!("what git says of {work} could not be worked out: {why}");
                    LastLook::Failed { at }
                }
            };
            let changed = match written(&entry) {
                Ok(json) if json != entry.json => {
                    inner.rev += 1;
                    entry.rev = inner.rev;
                    entry.json = json;
                    true
                }
                Ok(_) => false,
                Err(broken) => {
                    eprintln!("{work} cannot be shown: {}", broken.0);
                    false
                }
            };
            inner.entries.insert(item, entry);
            (changed, moved)
        };
        self.looked.notify_waiters();
        if changed || moved {
            self.handle.items_changed();
        }
    }

    /// Waits until a look has ended for each of these items.
    pub(crate) async fn waited_for(&self, items: &[u64]) {
        loop {
            let notified = self.looked.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let waiting = {
                let inner = self.lock();
                items.iter().any(|item| {
                    inner
                        .entries
                        .get(item)
                        .is_some_and(|entry| matches!(entry.looked, LastLook::Never))
                })
            };
            if !waiting {
                return;
            }
            notified.await;
        }
    }

    /// Checks the items against the driver on the loop, which starts the looks that are due, and waits for the first
    /// look at each item that has had none.
    async fn fresh(self: &Arc<Self>) -> Result<(), Failure> {
        let cache = self.clone();
        let checked = (self.handle.read_at(move |driver, now| cache.check(driver, now)).await)
            .map_err(Failure::Refused)?
            .map_err(Failure::Broken)?;
        self.waited_for(&checked.never).await;
        Ok(())
    }

    /// What the pages are given of the items: all of them, as a page that has read none is given them, or those that
    /// changed since the cursor it was given last, as a body of JSON.
    pub(crate) async fn read(self: &Arc<Self>, since: Option<&str>) -> Result<String, Failure> {
        self.fresh().await?;
        let inner = self.lock();
        let after = since.and_then(|cursor| {
            let (boot, rev) = cursor.split_once('.')?;
            let rev: u64 = rev.parse().ok()?;
            (boot == inner.boot && rev <= inner.rev).then_some(rev)
        });
        let changed: Vec<&str> = (inner.entries.values())
            .filter(|entry| after.is_none_or(|after| entry.rev > after))
            .map(|entry| entry.json.as_str())
            .collect();
        let outage = inner.outages.as_ref().and_then(|outages| outages.agents.as_ref());
        let cursor = format!("{}.{}", inner.boot, inner.rev);
        Ok(format!(
            "{{\"cursor\":{},\"full\":{},\"items\":[{}],\"outage\":{},\"waits\":{},\"againAfterMs\":{}}}",
            quoted(&cursor),
            after.is_none(),
            changed.join(","),
            serde_json::to_string(&outage).map_err(|e| Failure::Broken(Broken(e.to_string())))?,
            serde_json::to_string(&inner.waits).map_err(|e| Failure::Broken(Broken(e.to_string())))?,
            self.bound,
        ))
    }

    /// Every item, as the pages are given them whole.
    pub(crate) async fn all(self: &Arc<Self>) -> Result<Items, Failure> {
        self.fresh().await?;
        let inner = self.lock();
        Ok(Items {
            items: inner.entries.values().map(shown).collect(),
            outage: inner.outages.as_ref().and_then(|outages| outages.agents.clone()),
            waits: inner.waits.clone(),
        })
    }

    /// The work of this item in brief, as the items give it, after the driver was checked ([`ItemsCache::check`]) and
    /// the first look at the work, if none had ended, did.
    pub(crate) async fn summary(&self, item: u64) -> Option<Summary> {
        self.waited_for(&[item]).await;
        let inner = self.lock();
        inner.entries.get(&item).map(|entry| shown(entry).work)
    }
}

/// The item as the pages are given it: as it was worked out, with the lines the last look found where it ended well.
fn shown(entry: &Entry) -> Item {
    let mut item = entry.base.clone();
    if let LastLook::Looked { seen, .. } = &entry.looked {
        item.work.saw(seen.clone());
    }
    item
}

/// The item as the pages are given it, written out.
fn written(entry: &Entry) -> Result<String, Broken> {
    serde_json::to_string(&shown(entry)).map_err(|e| Broken(e.to_string()))
}

/// A text as a JSON string.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
}
