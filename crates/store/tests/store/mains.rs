//! The main dependency of each item, which the store keeps itself in every write (module `mains` of the store).
//!
//! Random sequences of writes, each through the store: an item added depending on some of those there, a dependency
//! added, taken away or made the main one, an item cancelled or reopened; and the store opened anew now and then. After
//! every write the store gives exactly one main dependency of each item some item depends on, of an item that depends
//! on it, of a cancelled item only where every item that depends on it is cancelled, and none of any other item; and it
//! is the one the rules give, followed here a write at a time as they are said: the first dependency on an item is its
//! main one; one made the main one takes the place of the one before; where the main one is taken away, or its item is
//! cancelled, the one of those left that is of an item not cancelled, then of the item's own project, then made first,
//! then of the lowest number takes its place, and the main one stays where every item that depends on the item is
//! cancelled; and a dependency added, or an item reopened, takes the place of a main one whose item is cancelled. Each
//! case the property is there to reach is counted, and the test fails where one was not reached.
//!
//! A file of version 37 of the schema, as the store was before it kept main dependencies, is opened with the main
//! dependency of each item marked by the same choice.

use store::{First, ItemChange, ItemNumber, Mains, SCHEMA_VERSION};

use super::opening::{SCHEMA_1, versions};
use super::*;

/// How many sequences are run.
const SEEDS: u64 = 30;

/// How many writes each sequence makes.
const STEPS: usize = 80;

/// The projects the items are of.
const PROJECTS: [&str; 2] = ["one", "two"];

/// A small deterministic generator (xorshift), so that a seed gives the same sequence every time.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(
            seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(0x2545_F491_4F6C_DD1D)
                | 1,
        )
    }

    fn below(&mut self, n: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x % n as u64) as usize
    }

    fn pick<T: Copy>(&mut self, of: &[T]) -> Option<T> {
        (!of.is_empty()).then(|| of[self.below(of.len())])
    }
}

/// A write of a sequence, as its failure prints it.
#[derive(Clone, Debug)]
enum Write {
    Add { project: &'static str, on: Vec<u64> },
    DependsOn { item: u64, on: u64 },
    NoLongerDependsOn { item: u64, on: u64 },
    Main { item: u64, on: u64 },
    Cancel(u64),
    Reopen(u64),
}

/// How many times each case the property is there to reach was reached.
#[derive(Debug, Default)]
struct Reached {
    /// An item came to have its first dependency, which became its main one.
    first: usize,
    /// A dependency was made the main one in place of another.
    chosen: usize,
    /// The main one was taken away, and another took its place.
    taken_away: usize,
    /// The last dependency on an item was taken away, which leaves it no main one.
    none_left: usize,
    /// The main one's item was cancelled, and the dependency of an item not cancelled took its place.
    cancelled_moved: usize,
    /// The main one's item was cancelled, and it stayed, every item that depends on the item being cancelled.
    cancelled_stayed: usize,
    /// A dependency was added on an item whose main one's item is cancelled, and took its place.
    over_a_cancelled_one: usize,
    /// An item was reopened, and its dependency took the place of a main one whose item is cancelled.
    reopened: usize,
    /// One of an item of the item's own project was chosen over one made before it of an item of another project.
    own_project_first: usize,
}

/// The items as the rules are followed here, apart from the store.
#[derive(Default)]
struct Model {
    project: BTreeMap<u64, &'static str>,
    cancelled: BTreeSet<u64>,
    /// When each dependency was made, by the item that depends and the item depended on.
    made: BTreeMap<(u64, u64), u64>,
    /// Of each item some item depends on, the item whose dependency on it is the main one.
    main: BTreeMap<u64, u64>,
}

impl Model {
    /// The items that depend on `on`, each with when its dependency was made.
    fn dependents(&self, on: u64) -> Vec<(u64, u64)> {
        (self.made.iter())
            .filter(|((_, of), _)| *of == on)
            .map(|((item, _), made)| (*item, *made))
            .collect()
    }

    /// Of the items that depend on `on`, the one whose dependency the rules choose as the main one.
    fn chosen(&self, on: u64, reached: &mut Reached) -> Option<u64> {
        let key = |&(item, made): &(u64, u64)| {
            let other = self.project[&item] != self.project[&on];
            (self.cancelled.contains(&item), other, made, item)
        };
        let by = self.dependents(on);
        let best = key(by.iter().min_by_key(|dependency| key(dependency))?);
        if (by.iter().map(key)).any(|k| k.0 == best.0 && k.1 && k.2 < best.2) {
            reached.own_project_first += 1;
        }
        Some(best.3)
    }

    fn depend(&mut self, item: u64, on: u64, at: u64, reached: &mut Reached) {
        let before = self.dependents(on);
        self.made.insert((item, on), at);
        match self.main.get(&on).copied() {
            None => {
                assert!(
                    before.is_empty(),
                    "{on} is depended on, and the model has no main one of it"
                );
                self.main.insert(on, item);
                reached.first += 1;
            }
            Some(main) if self.cancelled.contains(&main) => {
                self.main.insert(on, item);
                reached.over_a_cancelled_one += 1;
            }
            Some(_) => {}
        }
    }

    fn write(&mut self, write: &Write, number: u64, at: u64, reached: &mut Reached) {
        match write {
            Write::Add { project, on } => {
                self.project.insert(number, *project);
                for on in on {
                    self.depend(number, *on, at, reached);
                }
            }
            Write::DependsOn { item, on } => self.depend(*item, *on, at, reached),
            Write::NoLongerDependsOn { item, on } => {
                self.made.remove(&(*item, *on));
                if self.main.get(on) == Some(item) {
                    match self.chosen(*on, reached) {
                        Some(main) => {
                            self.main.insert(*on, main);
                            reached.taken_away += 1;
                        }
                        None => {
                            self.main.remove(on);
                            reached.none_left += 1;
                        }
                    }
                }
            }
            Write::Main { item, on } => {
                if self.main.insert(*on, *item) != Some(*item) {
                    reached.chosen += 1;
                }
            }
            Write::Cancel(item) => {
                self.cancelled.insert(*item);
                let mained: Vec<u64> = (self.main.iter())
                    .filter(|(_, m)| *m == item)
                    .map(|(on, _)| *on)
                    .collect();
                for on in mained {
                    let open = self.dependents(on).iter().any(|(d, _)| !self.cancelled.contains(d));
                    if open {
                        let main = self.chosen(on, reached).expect("an item depends on it");
                        self.main.insert(on, main);
                        reached.cancelled_moved += 1;
                    } else {
                        reached.cancelled_stayed += 1;
                    }
                }
            }
            Write::Reopen(item) => {
                self.cancelled.remove(item);
                let on: Vec<u64> = (self.made.keys())
                    .filter(|(d, _)| d == item)
                    .map(|(_, on)| *on)
                    .collect();
                for on in on {
                    if self.cancelled.contains(&self.main[&on]) {
                        self.main.insert(on, *item);
                        reached.reopened += 1;
                    }
                }
            }
        }
    }

    /// A write the driver would let through, as the store is given it: none of an item that is cancelled.
    fn some_write(&self, rng: &mut Rng, items: u64) -> Option<Write> {
        let all: Vec<u64> = (1..=items).collect();
        let open: Vec<u64> = all.iter().copied().filter(|i| !self.cancelled.contains(i)).collect();
        let edges: Vec<(u64, u64)> = (self.made.keys())
            .copied()
            .filter(|(d, _)| !self.cancelled.contains(d))
            .collect();
        match rng.below(10) {
            0 | 1 => {
                let on: BTreeSet<u64> = (0..rng.below(3)).filter_map(|_| rng.pick(&all)).collect();
                let project = PROJECTS[rng.below(PROJECTS.len())];
                Some(Write::Add {
                    project,
                    on: on.into_iter().collect(),
                })
            }
            2 | 3 => {
                let item = rng.pick(&open)?;
                let on = rng.pick(&all[..item as usize - 1])?;
                (!self.made.contains_key(&(item, on))).then_some(Write::DependsOn { item, on })
            }
            4 => rng.pick(&edges).map(|(item, on)| Write::NoLongerDependsOn { item, on }),
            5 | 6 => rng.pick(&edges).map(|(item, on)| Write::Main { item, on }),
            7 | 8 => rng.pick(&open).map(Write::Cancel),
            _ => {
                let cancelled: Vec<u64> = self.cancelled.iter().copied().collect();
                rng.pick(&cancelled).map(Write::Reopen)
            }
        }
    }

    fn mains(&self) -> Mains {
        (self.main.iter())
            .map(|(on, main)| (ItemNumber(*on), ItemNumber(*main)))
            .collect()
    }
}

/// The main dependencies the store gives, checked against the items it gives: one of each item some item depends on,
/// of an item that depends on it, of a cancelled item only where every item that depends on it is, and none of any
/// other.
fn checked(store: &Store, cancelled: &BTreeSet<u64>, said: &str) -> Mains {
    let mains = store
        .mains()
        .unwrap_or_else(|fault| panic!("{said}: the store refuses its mains: {fault:?}"));
    let mut by: BTreeMap<ItemNumber, Vec<ItemNumber>> = BTreeMap::new();
    for item in store.items().expect("the items are read") {
        for on in &item.depends_on {
            by.entry(*on).or_default().push(item.number);
        }
    }
    assert_eq!(
        mains.keys().collect::<Vec<_>>(),
        by.keys().collect::<Vec<_>>(),
        "{said}: the items with a main dependency"
    );
    let is_cancelled = |item: &ItemNumber| cancelled.contains(&item.0);
    for (on, dependents) in &by {
        let main = &mains[on];
        assert!(
            dependents.contains(main),
            "{said}: the main dependency of {on} is {main}'s"
        );
        assert!(
            !is_cancelled(main) || dependents.iter().all(is_cancelled),
            "{said}: the main dependency of {on} is that of {main}, which is cancelled, while another is not"
        );
    }
    mains
}

fn one_sequence(seed: u64, reached: &mut Reached) {
    let mut rng = Rng::new(seed);
    let db = Db::new();
    let mut store = db.open();
    let mut model = Model::default();
    // The work of each item, by its number, as the engine's loop grows it, with its number in the store.
    let mut works: BTreeMap<u64, (WorkNumber, Driver)> = BTreeMap::new();
    let mut written: Vec<Write> = Vec::new();
    let mut at = START;
    for step in 0..STEPS {
        let items = works.len() as u64;
        let write = match items {
            0 => Some(Write::Add {
                project: PROJECTS[0],
                on: Vec::new(),
            }),
            _ => model.some_write(&mut rng, items),
        };
        let Some(write) = write else {
            continue;
        };
        at += 1000;
        written.push(write.clone());
        let said = format!("seed {seed}, step {step}, after {written:?}");
        let one = |item: u64, change: ItemChange| [(ItemNumber(item), change)];
        let kept = match &write {
            Write::Add { project, on } => {
                let new = First {
                    title: format!("item {}", items + 1),
                    level: Some("2".to_string()),
                    project: Some(project.to_string()),
                    depends_on: on.iter().copied().map(ItemNumber).collect(),
                    tags: Vec::new(),
                    continues: None,
                };
                store
                    .add(&new, "judged", Time(at), &Work::default())
                    .map(|(item, work)| {
                        assert_eq!(item, ItemNumber(items + 1), "{said}");
                        works.insert(item.0, (work, Driver::new()));
                    })
            }
            Write::DependsOn { item, on } => {
                let made = one(*item, ItemChange::DependsOn(ItemNumber(*on)));
                store.change(Time(at), Some("it uses that one"), &made, &[])
            }
            Write::NoLongerDependsOn { item, on } => {
                let made = one(*item, ItemChange::NoLongerDependsOn(ItemNumber(*on)));
                store.change(Time(at), Some("it no longer uses that one"), &made, &[])
            }
            Write::Main { item, on } => {
                let made = one(*item, ItemChange::Main(ItemNumber(*on)));
                store.change(Time(at), Some("it stands under this one"), &made, &[])
            }
            Write::Cancel(item) | Write::Reopen(item) => {
                let (work, d) = works.get_mut(item).expect("the item has a work");
                d.on(match write {
                    Write::Cancel(_) => Event::Cancelled {
                        why: Some("no longer wanted".into()),
                    },
                    _ => Event::Reopened {
                        why: "wanted again".into(),
                    },
                });
                store.change(Time(at), Some("as the owner said"), &[], &[(*work, &d.work)])
            }
        };
        kept.unwrap_or_else(|fault| panic!("{said}: the store refuses the write: {fault:?}"));
        model.write(&write, items + 1, at, reached);
        // Now and then the store is opened anew on the file, and gives what it kept.
        if rng.below(8) == 0 {
            store = db.open();
        }
        let mains = checked(&store, &model.cancelled, &said);
        assert_eq!(
            mains,
            model.mains(),
            "{said}: the main dependencies are not those the rules give"
        );
    }
}

#[test]
fn every_item_some_item_depends_on_has_exactly_one_main_dependency_as_the_rules_give_whatever_is_written() {
    let mut reached = Reached::default();
    for seed in 0..SEEDS {
        one_sequence(seed, &mut reached);
    }
    let Reached {
        first,
        chosen,
        taken_away,
        none_left,
        cancelled_moved,
        cancelled_stayed,
        over_a_cancelled_one,
        reopened,
        own_project_first,
    } = reached;
    let counts = [
        first,
        chosen,
        taken_away,
        none_left,
        cancelled_moved,
        cancelled_stayed,
        over_a_cancelled_one,
        reopened,
        own_project_first,
    ];
    assert!(counts.iter().all(|n| *n > 0), "a case was never reached: {reached:?}");
}

/// Keeps an item of number `number` in a file of version 37, of its project, added at `added` depending on `on`, with
/// its work of the same number.
fn kept_at_37(file: &Connection, number: u64, project: &str, added: u64, on: &[u64]) {
    file.execute(
        "INSERT INTO items (number, title, added, level, project) VALUES (?1, ?2, ?3, '2', ?4)",
        rusqlite::params![number, format!("item {number}"), added, project],
    )
    .expect("an item is kept");
    file.execute(
        "INSERT INTO works (number, item, flow, added) VALUES (?1, ?1, 'judged', ?2)",
        rusqlite::params![number, added],
    )
    .expect("its work is kept");
    for on in on {
        file.execute(
            "INSERT INTO depends (item, on_item) VALUES (?1, ?2)",
            rusqlite::params![number, on],
        )
        .expect("a dependency is kept");
    }
}

/// Keeps the `n`th change of an item, made at `at`.
fn changed_at_37(file: &Connection, item: u64, n: u64, at: u64, change: &str) {
    file.execute(
        "INSERT INTO item_changes (item, n, at, change, why) VALUES (?1, ?2, ?3, ?4, 'as it was')",
        rusqlite::params![item, n, at, change],
    )
    .expect("a change is kept");
}

// The store before it kept main dependencies drew an item under the item of the lowest number that depends on it, a
// cancelled one too. Opened, a file of then marks each item's main dependency by the store's own choice: of an item not
// cancelled before a cancelled one, of the item's own project before another, made first, of the lowest number.
#[test]
fn a_file_of_version_37_is_opened_with_the_main_dependency_of_each_item_marked() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        store::migrate(&file, 1, 37).expect("the migrations run");
        assert_eq!(versions(&db.path), [37]);
        // 2, cancelled, was added depending on 1; 3, open, came to depend on 1 later.
        kept_at_37(&file, 1, "one", 10, &[]);
        kept_at_37(&file, 2, "one", 20, &[1]);
        kept_at_37(&file, 3, "one", 30, &[]);
        changed_at_37(&file, 3, 1, 50, r#"{"DependsOn":1}"#);
        file.execute("INSERT INTO cancelled (work, at) VALUES (2, 1)", [])
            .expect("its cancel is kept");
        // 5, of another project, and then 6 were added depending on 4.
        kept_at_37(&file, 4, "one", 40, &[]);
        kept_at_37(&file, 5, "two", 45, &[4]);
        kept_at_37(&file, 6, "one", 60, &[4]);
        // 8 and 9, both cancelled, depend on 7.
        kept_at_37(&file, 7, "one", 70, &[]);
        kept_at_37(&file, 8, "one", 80, &[7]);
        kept_at_37(&file, 9, "one", 90, &[7]);
        file.execute("INSERT INTO cancelled (work, at) VALUES (8, 1), (9, 1)", [])
            .expect("their cancels are kept");
        // 11, cancelled and reopened since, and then 12 depend on 10.
        kept_at_37(&file, 10, "one", 100, &[]);
        kept_at_37(&file, 11, "one", 110, &[10]);
        kept_at_37(&file, 12, "one", 120, &[10]);
        file.execute("INSERT INTO cancelled (work, at) VALUES (11, 1)", [])
            .expect("its cancel is kept");
        file.execute(
            "INSERT INTO reopened (work, at, why) VALUES (11, 2, 'wanted again')",
            [],
        )
        .expect("its reopen is kept");
        // 14 was added depending on 13, gave it up, and came to depend on it again after 15 was added depending on it.
        kept_at_37(&file, 13, "one", 130, &[]);
        kept_at_37(&file, 14, "one", 140, &[13]);
        changed_at_37(&file, 14, 1, 150, r#"{"NoLongerDependsOn":13}"#);
        kept_at_37(&file, 15, "one", 160, &[13]);
        changed_at_37(&file, 14, 2, 170, r#"{"DependsOn":13}"#);
        // Nothing depends on 16.
        kept_at_37(&file, 16, "one", 180, &[]);
    }
    let store = db.open();
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    let cancelled = BTreeSet::from([2, 8, 9]);
    let mains = checked(&store, &cancelled, "as the file was opened");
    let expected: Mains = [(1, 3), (4, 6), (7, 8), (10, 11), (13, 15)]
        .into_iter()
        .map(|(on, main)| (ItemNumber(on), ItemNumber(main)))
        .collect();
    assert_eq!(mains, expected);
    // Each was marked once, with no time, which the store did not keep.
    let file = Connection::open(&db.path).expect("the database opens");
    let rows: Vec<(u64, Option<u64>, Option<u64>)> = (file.prepare("SELECT on_item, item, at FROM mains ORDER BY n"))
        .expect("the main dependencies are read")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("the main dependencies are read")
        .collect::<Result<_, _>>()
        .expect("the main dependencies are read");
    let marked: Vec<(u64, Option<u64>, Option<u64>)> =
        (expected.iter()).map(|(on, main)| (on.0, Some(main.0), None)).collect();
    assert_eq!(rows, marked);
}

// A change that makes a dependency the main one, or comes to depend on an item, names an item the store has.
#[test]
fn a_dependency_made_the_main_one_of_an_item_the_store_does_not_have_is_refused_and_nothing_is_kept() {
    let mut store = Store::in_memory().expect("the store opens");
    let nothing = Work::default();
    store
        .add(&first("first", "2", &[]), "judged", Time(10), &nothing)
        .unwrap();
    store
        .add(&first("second", "2", &[ItemNumber(1)]), "judged", Time(20), &nothing)
        .unwrap();
    let before = (store.items().unwrap(), store.mains().unwrap());
    assert_eq!(
        store.change(
            Time(30),
            Some("x"),
            &[(ItemNumber(2), ItemChange::Main(ItemNumber(9)))],
            &[]
        ),
        Err(store::Fault::NoSuchDependency(ItemNumber(9)))
    );
    assert_eq!((store.items().unwrap(), store.mains().unwrap()), before);
}
