//! Every situation an entry of an agent's cell can reach, as the flow's definition says, each with a name that says it,
//! for whoever reads what the cell is sent in each: the entry, what it watches that ran it, and how its session stands
//! with each datum of its context; and, apart, each other thing a round may be given.
//!
//! An entry is `first`, where the cell has worked from nothing; `later`, where the cell was entered before in the same
//! session, which holds every part of the message it keeps, the last return in it being that of the cell or of another
//! cell of its line entered since; `reopened`, where the cell ran before and its session was begun anew since, so that
//! it holds nothing; `compacted`, where the session was compacted as the last round of the line's session ended, that of
//! the cell or of another cell of its line, and holds what that return handed in and no part of the message; or `reopened_after_read`, where the session
//! was begun anew or compacted since the cell ran, and another cell of its line was entered in it first, so that it
//! holds what that cell was sent and what its return handed in, and of the message only what it has in common with
//! this cell. On a first entry the session holds nothing where the cell is the first of its line in the flow's order or
//! the session was begun anew; otherwise it goes on from the cell of the line before it, and holds each part of the
//! message kept alike for both, and what that cell's return handed in, where it writes any block and handed one in.
//!
//! What it watches that ran it is each datum or group alone, those whose blocks one cell's return writes together, and
//! all of them at once. On a first entry, where everything written is new to the cell, every datum it watches that the
//! work starts with runs it as well: what people
//! write that runs the flow's first agent's cell.
//!
//! A datum is certainly written when the cell runs where the work starts with it, or where the cells before it in the
//! order that must write it, and surely run, run whichever way the conditions go: one of them runs on no condition, or
//! between them they run on each value of a choice the cell itself does not run on one value of. A cell surely runs
//! where something it watches is certainly written before it. A datum is also certainly written where the cell runs
//! only because something it watches was written, that only cells write, and each of them ran. The flow says no more of
//! which blocks a cell writes each time than what it must write, so that a cell is taken to write no more.
//!
//! On a first entry, a datum only the cell itself or cells after it in the order write is not written yet, and what the
//! cell watches of such data runs no first entry.
//!
//! A datum the cell watches that ran it is new to it: the session lacks it, or holds it from a cell of the line that
//! is given it or writes it, as the version last worked from or the latest, or as one the cell never worked from; of a
//! group, one datum of the group at least is new, and each stands by its own state. A datum the cell writes as well
//! may stand with nothing new in it though it ran the cell: a version written by another while its run was under way
//! ran it, and the cell's own, after it, is the one it last worked from; not the record of its questions, whose versions
//! are the owner's. Any other datum the cell is given, the cell's own writes aside,
//! stands any way the entry allows, since a version of it may be written while the run is under way: unwritten where
//! it is not certainly written, lacking, held from a cell of the line, or, on a later entry, held as last worked from,
//! latest, or with nothing new; but a datum only people write that an earlier cell of the line watches is never held as
//! last worked from where it did not run the cell, since its new version runs that cell, whose round fills it first; and
//! the datum the work starts with is never held as one the first agent's cell never worked from where it did not run it,
//! though a later agent's cell may never have worked from it, its run having been stopped for the earlier cell that the
//! datum ran. A datum only the cell writes is never new to it: unwritten on a first entry, held on a
//! later one, held or, where its return handed in none, lacking on one compacted, lacking on one reopened, and lacking
//! or, held from another cell of the line, held on one reopened after another cell read, and unwritten on any where it
//! is not certainly written. A block the cell writes that nothing else gives it is given only where the session lacks
//! its latest: lacking or, held from a cell of the line, held on a first entry where it may be written before; held on
//! a later one, or lacking where another writes it too; held or lacking on one compacted; lacking on one reopened;
//! lacking or, held from another cell of the line, held on one reopened after another cell read; and unwritten on any
//! where it is not certainly written. A datum whose versions the carrier compares itself, given `last_and_latest`, that
//! stands lacking is unread: the session holds none of it, and on a first entry it may hold an earlier version, which
//! it is told what changed since.
//!
//! A datum's text is the builder's from its kind and from how the session stands with it alone, so a product of every
//! datum's own states repeats the same texts: it is counted ([`combinations`]), and the situations hold every state of
//! each datum: the data of what ran the cell and the rest each stand alike where they can, each state of either in
//! some situation, and each datum that stands in a state of its own goes through its states as the situations go on; a
//! datum that cannot stand as its class does is unwritten where it may be, else lacking. Besides, on a later entry run by the
//! first datum or group the cell watches, with the data of what ran it held as last worked from and the rest held: the
//! result of a call of each kind the engine gives a round and why a return was sent back, which come after a return that
//! handed in nothing, each come of taking main in where the cell takes it, new to the session, and an entry after a
//! return that handed in nothing; and, where the cell takes main in, one that holds what came of it.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Cell, Condition};
use flow::agents::{Agents, Part};
use flow::system::Settings;
use flow::{CellSpec, Compiled, Given, InputKind, Kind, Listed, Watched};

use crate::message::{CALLS, MAIN, Situation, Stands, Values, kept};

/// A situation, with a name that says it, its parts joined by `-`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub situation: Situation,
}

/// The entries of a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    First,
    Later,
    Reopened,
    /// The session was compacted as the cell's last round ended, and holds what its return handed in.
    Compacted,
    /// The session was begun anew since the cell ran, and another cell of its line was entered in it first.
    ReopenedAfterRead,
}

impl Entry {
    const ALL: [Entry; 5] = [
        Entry::First,
        Entry::Later,
        Entry::Reopened,
        Entry::Compacted,
        Entry::ReopenedAfterRead,
    ];

    fn name(self) -> &'static str {
        match self {
            Entry::First => "first",
            Entry::Later => "later",
            Entry::Reopened => "reopened",
            Entry::Compacted => "compacted",
            Entry::ReopenedAfterRead => "reopened_after_read",
        }
    }
}

/// How a datum stands to what ran the cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// A datum, no group's, that ran the cell.
    Fired,
    /// A datum of a group that ran the cell, one of which is new.
    Grouped,
    /// A datum only the cell writes, so never new to it.
    Never,
    /// A block the cell writes that nothing else gives it.
    Output,
    /// Any other.
    Rest,
}

/// The results of a call the engine never gives a round: of the engine's own wait before a round is tried again, which
/// is no result, and of a call the work does not hold, and the owner's word to a run at its limit, which the run goes
/// on after, in the same run, and so is given to no round of another.
const UNREACHED: [&str; 2] = ["unknown", "at_limit"];

/// A set of what the cell watches that ran it, with the name that says it.
struct Ran<'a> {
    name: String,
    watched: Vec<&'a Watched>,
}

/// What the flow says of where the cell stands, which decides the situations it can reach.
struct Reach<'a> {
    flow: &'a Compiled,
    words: &'a Agents,
    cell: &'a CellSpec,
    /// The blocks whose versions the carrier compares itself.
    compared: &'a BTreeSet<BlockId>,
    /// What a round of the cell is given, in order ([`crate::message::given`]).
    given: Vec<Given>,
    /// The data certainly written whenever the cell runs.
    certain: BTreeSet<BlockId>,
    /// Of those, what the work starts with.
    starts: BTreeSet<BlockId>,
    /// The cells of its line before it in the flow's order.
    before: Vec<&'a CellSpec>,
    /// The other cells of its line.
    others: Vec<&'a CellSpec>,
    /// Whether it is the last cell of the flow, whose run finishing ends the work, a run that only asked for calls
    /// included: it never worked from anything, is never entered again once a run of it finished, and is given the
    /// result of no call.
    last: bool,
}

/// What a session holds as an entry begins, with the name of the cell it goes on from where that tells the entries of
/// one kind apart: the parts of the message it keeps, and whether the last return in it handed in blocks.
struct Frame {
    holds: Vec<String>,
    after_hand_in: bool,
    after: Option<String>,
}

/// Every situation an entry of the agent's cell can reach, as the module says, each once, in order: the first entry,
/// the later, the reopened and the reopened after another cell read, each by the sets of what ran it, then the other
/// things a round may be given. `compared` is the blocks whose versions the carrier compares itself.
pub fn situations(flow: &Compiled, words: &Agents, cell: &CellSpec, compared: &BTreeSet<BlockId>) -> Vec<Named> {
    let reach = Reach::of(flow, words, cell, compared);
    let mut out: Vec<Named> = Vec::new();
    let mut add = |named: Named| {
        if !out.iter().any(|o| o.situation == named.situation) {
            out.push(named);
        }
    };
    for entry in Entry::ALL {
        if reach.last && !matches!(entry, Entry::First | Entry::Later) {
            continue;
        }
        for ran in reach.ran(entry) {
            let data = reach.classes(&ran.watched);
            // A class's state that none of its data can stand in gives nothing of its own.
            let states = |classes: &[Class], states: Vec<Stands>| {
                let of: Vec<(&BlockId, &Class)> = data.iter().filter(|(_, c)| classes.contains(c)).collect();
                let able = |s: &Stands| of.iter().any(|(b, c)| reach.allowed(entry, b, c).contains(s));
                let states: Vec<Option<Stands>> = states.into_iter().filter(able).map(Some).collect();
                if states.is_empty() { vec![None] } else { states }
            };
            let frames = reach.frames(entry);
            // A datum the cell watches and writes itself may stand with nothing new in it though it ran the cell.
            let mut new_states = reach.newly(entry, true);
            new_states.push(Stands::Holds);
            let news = states(&[Class::Fired, Class::Grouped], new_states);
            let rests = states(&[Class::Rest], reach.rest(entry, true, true));
            // Each state of what ran the cell and each state of the rest is written once, the shorter list going round
            // again beside the longer: the texts of a datum do not depend on how another stands.
            let pairs: Vec<(Option<Stands>, Option<Stands>)> = (0..news.len().max(rests.len()))
                .map(|i| (news[i % news.len()], rests[i % rests.len()]))
                .collect();
            // A datum that stands in a state of its own goes through them as the situations go on, so that each is
            // written with the cell: as many situations as the datum with most states needs.
            let most = (data.iter())
                .filter(|(_, c)| matches!(c, Class::Never | Class::Output))
                .map(|(b, c)| reach.allowed(entry, b, c).len())
                .max()
                .unwrap_or(1);
            for k in 0..pairs.len().max(most) {
                let (new, rest) = pairs[k % pairs.len()];
                for frame in &frames {
                    let mut name = vec![entry.name().to_string(), ran.name.clone()];
                    name.extend(frame.after.clone());
                    name.extend(new.map(|s| format!("ran_{}", s.name())));
                    name.extend(rest.map(|s| format!("rest_{}", s.name())));
                    if k >= pairs.len() {
                        name.push(format!("own_{}", k - pairs.len() + 1));
                    }
                    let stands = (data.iter())
                        .map(|(block, class)| {
                            let allowed = reach.allowed(entry, block, class);
                            let wanted = match class {
                                Class::Fired | Class::Grouped => new,
                                Class::Rest => rest,
                                Class::Never | Class::Output => allowed.get(k % allowed.len().max(1)).copied(),
                            };
                            let instead = match allowed.contains(&Stands::Unwritten) {
                                true => Stands::Unwritten,
                                false => Stands::Lacks,
                            };
                            let stands = wanted.filter(|s| allowed.contains(s)).unwrap_or(instead);
                            (block.to_string(), reach.unread(block, stands))
                        })
                        .collect();
                    add(Named {
                        name: name.join("-"),
                        situation: reach.situation(frame, &ran, stands),
                    });
                }
            }
        }
    }
    for named in reach.besides() {
        add(named);
    }
    out
}

/// How many combinations of the entry, what ran the cell and each datum's own state the cell can reach,
/// every datum apart, as the module says.
pub fn combinations(flow: &Compiled, words: &Agents, cell: &CellSpec, compared: &BTreeSet<BlockId>) -> u128 {
    let reach = Reach::of(flow, words, cell, compared);
    let mut count: u128 = 0;
    for entry in Entry::ALL {
        if reach.last && !matches!(entry, Entry::First | Entry::Later) {
            continue;
        }
        for ran in reach.ran(entry) {
            let data = reach.classes(&ran.watched);
            let mut product: u128 = 1;
            let mut grouped: Vec<&BlockId> = Vec::new();
            for (block, class) in &data {
                match class {
                    Class::Grouped => grouped.push(block),
                    _ => product *= reach.allowed(entry, block, class).len() as u128,
                }
            }
            // The data of the groups, each new or not, every group that ran the cell holding a new one.
            let groups: Vec<Vec<&BlockId>> = (ran.watched.iter())
                .filter(|t| t.is_group())
                .map(|t| t.blocks.iter().filter(|b| grouped.contains(b)).collect::<Vec<_>>())
                .filter(|g: &Vec<&BlockId>| !g.is_empty())
                .collect();
            let mut ways: u128 = 0;
            for mask in 0u64..(1 << grouped.len()) {
                let is_new = |b: &BlockId| {
                    (grouped.iter())
                        .position(|g| *g == b)
                        .is_some_and(|i| mask & (1 << i) != 0)
                };
                if groups.iter().all(|g| g.iter().any(|b| is_new(b))) {
                    ways += (grouped.iter())
                        .map(|b| {
                            let new = reach.allowed(entry, b, &Class::Fired).len() as u128;
                            let all = reach.allowed(entry, b, &Class::Rest).len() as u128;
                            if is_new(b) { new } else { all - new }
                        })
                        .product::<u128>();
                }
            }
            count += product * ways * reach.frames(entry).len() as u128;
        }
    }
    count
}

impl<'a> Reach<'a> {
    fn of(flow: &'a Compiled, words: &'a Agents, cell: &'a CellSpec, compared: &'a BTreeSet<BlockId>) -> Reach<'a> {
        let place = flow.flow.place(&cell.id).unwrap_or(0);
        let spec = |id: &decide::CellId| flow.cells.iter().find(|c| c.id == *id);
        let before: Vec<&CellSpec> = (flow.flow.cells()[..place].iter())
            .filter_map(|c| spec(&c.id))
            .filter(|c| c.kind == Kind::Agent && c.line.is_some() && c.line == cell.line)
            .collect();
        let others: Vec<&CellSpec> = (flow.cells.iter())
            .filter(|c| c.kind == Kind::Agent && c.line.is_some() && c.line == cell.line && c.id != cell.id)
            .collect();
        let mut reach = Reach {
            flow,
            words,
            cell,
            compared,
            given: crate::message::given(flow, words, cell),
            certain: BTreeSet::new(),
            starts: BTreeSet::new(),
            before,
            others,
            last: place + 1 == flow.flow.cells().len(),
        };
        let given: Vec<BlockId> = reach.given.iter().map(|g| g.block.clone()).collect();
        reach.starts = given.iter().filter(|b| reach.starts_with(b)).cloned().collect();
        let implied = reach.implied(cell, &mut Vec::new());
        let certain = given
            .iter()
            .filter(|b| reach.starts.contains(*b) || implied.contains(*b) || reach.written_before(place, b));
        reach.certain = certain.cloned().collect();
        reach
    }

    /// Whether the work starts with the datum: the first of what people write that runs the flow's first agent's cell.
    fn starts_with(&self, block: &BlockId) -> bool {
        let flow = self.flow;
        let first = (flow.flow.cells().iter())
            .filter_map(|c| flow.cells.iter().find(|s| s.id == c.id))
            .find(|c| c.kind == Kind::Agent);
        let people = |name: &str| flow.blocks.get(&BlockId::from(name)).is_some_and(|d| d.writers.people);
        let starting = first.and_then(|c| c.watches.iter().find(|t| !t.is_group() && people(&t.name)));
        starting.is_some_and(|t| t.name == block.as_str())
    }

    /// What is certainly written whenever the cell runs, because it ran: it runs where one of what it watches is
    /// written, and that is certainly written; and where only cells write that, one of them ran, and wrote what it
    /// must, and what is certainly written whenever it runs. What people or the outside write implies nothing more.
    fn implied(&self, cell: &CellSpec, seen: &mut Vec<decide::CellId>) -> BTreeSet<BlockId> {
        if seen.contains(&cell.id) {
            return BTreeSet::new();
        }
        seen.push(cell.id.clone());
        let mut common: Option<BTreeSet<BlockId>> = None;
        for datum in cell.watches.iter().flat_map(|t| t.blocks.iter()) {
            let mut set = BTreeSet::from([datum.clone()]);
            let writers = self.flow.blocks.get(datum).map(|d| &d.writers);
            if let Some(w) = writers.filter(|w| !w.people && w.outside.is_none() && !w.cells.is_empty()) {
                let mut some: Option<BTreeSet<BlockId>> = None;
                for id in &w.cells {
                    let spec = self.flow.cells.iter().find(|c| c.id == *id);
                    let mut by = spec.map(|c| self.implied(c, seen)).unwrap_or_default();
                    by.extend(spec.into_iter().flat_map(|c| c.must_write.iter().cloned()));
                    some = Some(match some {
                        None => by,
                        Some(known) => known.intersection(&by).cloned().collect(),
                    });
                }
                set.extend(some.unwrap_or_default());
            }
            common = Some(match common {
                None => set,
                Some(known) => known.intersection(&set).cloned().collect(),
            });
        }
        seen.pop();
        common.unwrap_or_default()
    }

    /// Whether the cells before the one at this place that write the datum on every run and surely run, do run whichever
    /// way the conditions go, as the module says. A cell surely runs where something it watches is certainly written
    /// before it.
    fn written_before(&self, place: usize, block: &BlockId) -> bool {
        let flow = self.flow;
        let cells = flow.flow.cells();
        let surely = |w: &Cell| {
            let Some(spec) = flow.cells.iter().find(|c| c.id == w.id) else {
                return false;
            };
            let at = flow.flow.place(&w.id).unwrap_or(0);
            let watched = |b: &BlockId| self.starts_with(b) || self.written_before(at, b);
            spec.must_write.contains(block) && (spec.watches.iter()).any(|t| t.blocks.iter().any(watched))
        };
        let writers: Vec<&Cell> = (cells[..place].iter())
            .filter(|c| c.outputs.contains(block) && surely(c))
            .collect();
        if writers.iter().any(|w| w.applies.is_empty()) {
            return true;
        }
        // Between them, the writers run on each value of a choice that the cell itself may run on.
        let own = &cells[place].applies;
        let single = |w: &Cell| match w.applies.as_slice() {
            [Condition::Is { block, value }] => Some((block.clone(), value.clone())),
            _ => None,
        };
        let choices: BTreeSet<BlockId> = writers.iter().filter_map(|w| single(w)).map(|(b, _)| b).collect();
        choices.iter().any(|choice| {
            let values = flow
                .blocks
                .get(choice)
                .and_then(|d| d.values.clone())
                .unwrap_or_default();
            let wanted = values.iter().filter(|v| {
                !own.iter()
                    .any(|c| matches!(c, Condition::Is { block, value } if block == choice && value != *v))
            });
            let covered: Vec<String> = (writers.iter())
                .filter_map(|w| single(w))
                .filter(|(b, _)| b == choice)
                .map(|(_, v)| v)
                .collect();
            wanted.into_iter().all(|v| covered.contains(v))
        })
    }

    /// Whether the session may hold the datum as the entry begins: on a first entry, a cell of its line before it is
    /// given it or writes it; on one reopened after another cell read, another cell of its line is.
    fn holdable(&self, entry: Entry, block: &BlockId) -> bool {
        let writes = |c: &CellSpec| {
            let place = self.flow.flow.place(&c.id);
            place.is_some_and(|p| self.flow.flow.cells()[p].outputs.contains(block))
        };
        let cells = match entry {
            Entry::ReopenedAfterRead => &self.others,
            _ => &self.before,
        };
        cells
            .iter()
            .any(|c| c.given.iter().any(|g| g.block == *block) || writes(c))
    }

    /// The states of a datum new to the cell, held from another cell of the line where it may be.
    fn newly(&self, entry: Entry, may_hold: bool) -> Vec<Stands> {
        match entry {
            Entry::First if may_hold => vec![Stands::Lacks, Stands::HoldsLast, Stands::HoldsNew],
            Entry::First | Entry::Reopened | Entry::Compacted => vec![Stands::Lacks],
            Entry::Later => vec![Stands::Lacks, Stands::HoldsLast, Stands::HoldsLatest, Stands::HoldsNew],
            Entry::ReopenedAfterRead if may_hold => {
                vec![Stands::Lacks, Stands::HoldsLast, Stands::HoldsLatest, Stands::HoldsNew]
            }
            Entry::ReopenedAfterRead => vec![Stands::Lacks],
        }
    }

    /// The states of any datum, unwritten where it may be and held from another cell of the line where it may be.
    fn rest(&self, entry: Entry, unwritten: bool, may_hold: bool) -> Vec<Stands> {
        let mut states: Vec<Stands> = unwritten.then_some(Stands::Unwritten).into_iter().collect();
        match entry {
            Entry::First => {
                states.push(Stands::Lacks);
                if may_hold {
                    states.extend([Stands::HoldsLast, Stands::HoldsNew]);
                }
            }
            Entry::Later => states.extend([
                Stands::Lacks,
                Stands::HoldsLast,
                Stands::HoldsLatest,
                Stands::HoldsNew,
                Stands::Holds,
            ]),
            Entry::Reopened | Entry::Compacted => states.push(Stands::Lacks),
            Entry::ReopenedAfterRead => {
                states.push(Stands::Lacks);
                if may_hold {
                    states.extend([Stands::HoldsLast, Stands::HoldsLatest, Stands::HoldsNew, Stands::Holds]);
                }
            }
        }
        states
    }

    /// The states the datum can stand in on the entry, in its class, the one it stands in most first. On a first entry,
    /// a session that holds an earlier version of a datum whose versions the carrier compares, given `last_and_latest`,
    /// is told what changed since, though the cell worked from none: that is the one datum that stands held as last
    /// worked from.
    fn allowed(&self, entry: Entry, block: &BlockId, class: &Class) -> Vec<Stands> {
        let mut states = self.allowed_in(entry, block, class);
        let compares = self.compared.contains(block)
            && (self.given.iter())
                .any(|g| &g.block == block && g.kind == InputKind::LastAndLatest && g.list != Listed::Outputs);
        if entry == Entry::First && !compares {
            states.retain(|s| *s != Stands::HoldsLast);
        }
        // The first run of a cell that watches the datum the work starts with was run by it, so that the cell worked
        // from it: where it did not run the cell now, the session never holds it as one the cell never worked from.
        let watches = self
            .cell
            .watches
            .iter()
            .any(|t| !t.is_group() && t.name == block.as_str());
        // A cell with an agent's cell before it may have had its run stopped for that cell, which a new version of what
        // people write ran, and so never worked from it.
        let place = self.flow.flow.place(&self.cell.id).unwrap_or(0);
        let first = (self.flow.flow.cells()[..place].iter())
            .filter_map(|c| self.flow.cells.iter().find(|s| s.id == c.id))
            .all(|c| c.kind != Kind::Agent);
        if self.starts.contains(block) && watches && first && *class == Class::Rest && entry != Entry::First {
            states.retain(|s| *s != Stands::HoldsNew);
        }
        // A datum the cell watches and writes itself: a version written by another while its run was under way ran it,
        // and the cell's own, after it, is the one it last worked from, so that nothing is new to it.
        let writes = self.flow.flow.cells()[place].outputs.contains(block);
        let holds_own = match entry {
            Entry::Later | Entry::Compacted => true,
            Entry::ReopenedAfterRead => self.holdable(entry, block),
            Entry::First | Entry::Reopened => false,
        };
        if *class == Class::Fired && writes && holds_own && !self.answered(block) && !states.contains(&Stands::Holds) {
            states.push(Stands::Holds);
        }
        // A new version of a datum only people write, as it is of what an earlier cell of the line watches, runs that
        // cell first, whose round fills it into the session: where it did not run this cell, the session never lacks
        // its latest while it holds the version this cell last worked from.
        let filled_early = (self.before.iter()).any(|c| c.watches.iter().any(|t| t.blocks.contains(block)))
            && (self.flow.blocks.get(block))
                .is_some_and(|d| d.writers.people && d.writers.cells.is_empty() && d.writers.outside.is_none());
        if filled_early && *class == Class::Rest && matches!(entry, Entry::Later | Entry::ReopenedAfterRead) {
            states.retain(|s| *s != Stands::HoldsLast);
        }
        // The last cell worked from nothing, so that the session holds what it was sent in an earlier round of its run,
        // never as the version it last worked from.
        if self.last && entry == Entry::Later {
            states.retain(|s| matches!(s, Stands::Unwritten | Stands::Lacks | Stands::HoldsNew));
            if states.is_empty() {
                states.push(Stands::Unwritten);
            }
        }
        states
    }

    fn allowed_in(&self, entry: Entry, block: &BlockId, class: &Class) -> Vec<Stands> {
        let unwritten = !self.certain.contains(block);
        if entry == Entry::First && !self.early(block) {
            return vec![Stands::Unwritten];
        }
        match class {
            Class::Fired => self.newly(entry, self.holdable(entry, block)),
            Class::Grouped | Class::Rest => self.rest(entry, unwritten, self.holdable(entry, block)),
            Class::Never => {
                let mut states = match entry {
                    Entry::First => vec![],
                    Entry::Later => vec![Stands::Holds],
                    Entry::Reopened => vec![Stands::Lacks],
                    Entry::Compacted => vec![Stands::Holds],
                    Entry::ReopenedAfterRead => {
                        let mut states = vec![Stands::Lacks];
                        states.extend(self.holdable(entry, block).then_some(Stands::Holds));
                        states
                    }
                };
                if unwritten || states.is_empty() {
                    states.push(Stands::Unwritten);
                }
                states
            }
            Class::Output => {
                let writers = self.flow.blocks.get(block).map(|d| &d.writers);
                let others = writers
                    .is_some_and(|w| w.people || w.outside.is_some() || w.cells.iter().any(|c| *c != self.cell.id));
                let mut states = match entry {
                    Entry::First => {
                        let mut states = vec![Stands::Lacks];
                        states.extend(self.holdable(entry, block).then_some(Stands::Holds));
                        states
                    }
                    Entry::Later => {
                        let mut states = vec![Stands::Holds];
                        states.extend(others.then_some(Stands::Lacks));
                        states
                    }
                    Entry::Reopened => vec![Stands::Lacks],
                    Entry::Compacted => vec![Stands::Holds, Stands::Lacks],
                    Entry::ReopenedAfterRead => {
                        let mut states = vec![Stands::Lacks];
                        states.extend(self.holdable(entry, block).then_some(Stands::Holds));
                        states
                    }
                };
                if unwritten {
                    states.push(Stands::Unwritten);
                }
                states
            }
        }
    }

    /// How the datum stands, said of one whose versions the carrier compares itself, given `last_and_latest`: lacking
    /// it, the session holds none of it, and it is unread.
    fn unread(&self, block: &BlockId, stands: Stands) -> Stands {
        let last_and_latest = (self.given.iter())
            .any(|g| &g.block == block && g.kind == InputKind::LastAndLatest && g.list != Listed::Outputs);
        match stands {
            Stands::Lacks if last_and_latest && self.compared.contains(block) => Stands::Unread,
            stands => stands,
        }
    }

    /// Whether the owner's answers write the datum: the record of a cell's questions, none of whose versions the cell
    /// wrote itself, so that its run on an answer always has something new in it.
    fn answered(&self, block: &BlockId) -> bool {
        (self.flow.blocks.get(block)).is_some_and(|d| d.writers.outside.as_deref() == Some(flow::ANSWER))
    }

    /// Whether the datum may be written before the cell's first entry: people or the outside write it, or a cell before
    /// it in the order may. One that only the cell itself or cells after it write is not written yet.
    fn early(&self, block: &BlockId) -> bool {
        let flow = self.flow;
        let place = flow.flow.place(&self.cell.id).unwrap_or(0);
        let outside = flow
            .blocks
            .get(block)
            .is_some_and(|d| d.writers.people || d.writers.outside.is_some());
        outside || flow.flow.cells()[..place].iter().any(|c| c.outputs.contains(block))
    }

    /// The sets of what the cell watches that can run an entry, as the module says, each once, in the order the cell
    /// names them.
    fn ran(&self, entry: Entry) -> Vec<Ran<'a>> {
        let (flow, cell) = (self.flow, self.cell);
        let starts = |t: &Watched| t.blocks.iter().any(|b| self.starts.contains(b));
        let mut sets: Vec<Ran<'a>> = (cell.watches.iter())
            .map(|t| Ran {
                name: t.name.clone(),
                watched: vec![t],
            })
            .collect();
        for writer in &flow.cells {
            let writes = |t: &&Watched| {
                let by = |b: &BlockId| flow.blocks.get(b).is_some_and(|d| d.writers.cells.contains(&writer.id));
                t.blocks.iter().any(by)
            };
            let together: Vec<&Watched> = cell.watches.iter().filter(writes).collect();
            if together.len() > 1 {
                sets.push(Ran {
                    name: format!("by_{}", writer.id.as_str()),
                    watched: together,
                });
            }
        }
        if cell.watches.len() > 1 {
            sets.push(Ran {
                name: "all".to_string(),
                watched: cell.watches.iter().collect(),
            });
        }
        let names = |watched: &[&Watched]| watched.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
        let mut out: Vec<Ran<'a>> = Vec::new();
        for mut set in sets {
            if entry == Entry::First {
                // Everything written is new to a cell that worked from nothing.
                let added: Vec<&Watched> = (cell.watches.iter())
                    .filter(|t| starts(t) && !set.watched.iter().any(|s| s.name == t.name))
                    .collect();
                if !added.is_empty() {
                    let mut all: Vec<&Watched> = set.watched.iter().chain(&added).copied().collect();
                    all.sort_by_key(|t| cell.watches.iter().position(|c| c.name == t.name));
                    let alone = set.watched.len() == 1;
                    set = Ran {
                        name: if alone { names(&all).join("+") } else { set.name },
                        watched: all,
                    };
                }
            }
            // A datum or group none of whose blocks is written yet runs no first entry.
            let unwritten = |t: &&Watched| !t.blocks.iter().any(|b| self.early(b));
            let early = entry != Entry::First || !set.watched.iter().any(unwritten);
            let again = out.iter().any(|o| names(&o.watched) == names(&set.watched));
            if !early || again {
                continue;
            }
            out.push(set);
        }
        out
    }

    /// The class of each datum of the cell's context, by its name, for what ran it.
    fn classes(&self, ran: &[&Watched]) -> BTreeMap<BlockId, Class> {
        let (flow, cell) = (self.flow, self.cell);
        let given: Vec<&BlockId> = self.given.iter().map(|g| &g.block).collect();
        let mut classes: BTreeMap<BlockId, Class> = BTreeMap::new();
        for g in self.given.iter().filter(|g| g.list == Listed::Outputs) {
            classes.insert(g.block.clone(), Class::Output);
        }
        for watched in ran.iter().filter(|t| !t.is_group()) {
            // What the agent reads for it is new with it: what the owner said, for words to the cell.
            for block in watched.blocks.iter() {
                if given.contains(&block) {
                    classes.insert(block.clone(), Class::Fired);
                }
            }
        }
        for watched in ran.iter().filter(|t| t.is_group()) {
            // A group with a datum new as another watched holds a new one already.
            let fired = watched.blocks.iter().any(|b| classes.get(b) == Some(&Class::Fired));
            for block in watched.blocks.iter().filter(|b| given.contains(b)) {
                let class = if fired { Class::Rest } else { Class::Grouped };
                classes.entry(block.clone()).or_insert(class);
            }
        }
        let alone = |b: &BlockId| {
            flow.blocks.get(b).is_some_and(|d| {
                let w = &d.writers;
                w.cells == [cell.id.clone()] && !w.people && w.outside.is_none()
            })
        };
        for block in given {
            if !classes.contains_key(block) {
                let class = if alone(block) { Class::Never } else { Class::Rest };
                classes.insert(block.clone(), class);
            }
        }
        classes
    }

    /// The parts of the message the session keeps, in the order of the message, but what came of taking main in, which
    /// is the run's, unless `main` says it is held.
    fn kept(&self, main: bool) -> Vec<String> {
        (self.words.message.parts.iter())
            .filter(|p| p.kind.is_some() && p.part != Part::Context && (main || p.part != Part::Main))
            .map(|p| p.part.name().to_string())
            .collect()
    }

    /// These parts of the message the session holds, and what came of taking main in, in the order of the message.
    fn with_main(&self, holds: &[String]) -> Vec<String> {
        let all = self.kept(true);
        all.into_iter()
            .filter(|p| p == Part::Main.name() || holds.contains(p))
            .collect()
    }

    /// Whether the cell writes any block, so that a return of it can hand something in.
    fn writes(&self) -> bool {
        self.writes_of(self.cell)
    }

    /// Whether the cell writes any block.
    fn writes_of(&self, cell: &CellSpec) -> bool {
        let place = self.flow.flow.place(&cell.id);
        place.is_some_and(|p| !self.flow.flow.cells()[p].outputs.is_empty())
    }

    /// What the session may hold as the entry begins, each way once, each with the part of the name that tells it
    /// from the others.
    fn frames(&self, entry: Entry) -> Vec<Frame> {
        let mut frames: Vec<Frame> = Vec::new();
        let mut add = |holds: Vec<String>, after_hand_in: bool, after: Option<String>| {
            if !frames
                .iter()
                .any(|f| f.holds == holds && f.after_hand_in == after_hand_in)
            {
                frames.push(Frame {
                    holds,
                    after_hand_in,
                    after,
                });
            }
        };
        match entry {
            // The session goes on from the cell of the line before it, whose last return handed in blocks or did not;
            // or holds nothing, as one begun anew or compacted during the round does.
            Entry::First => {
                let (alike, writes) = self.after_the_line();
                add(alike.clone(), writes, None);
                add(alike.clone(), false, Some("after_nothing_handed_in".to_string()));
                add(Vec::new(), false, Some("fresh".to_string()));
                // The cell before it took main in too, and was sent what came of it.
                if self.before.last().is_some_and(|b| b.takes_main) && self.cell.takes_main {
                    add(self.with_main(&alike), writes, Some("after_main".to_string()));
                }
            }
            Entry::Later => {
                // The last return in the session may be that of a cell of the line entered since this one, or of this.
                let handed = self.writes() || self.others.iter().any(|o| self.writes_of(o));
                add(self.kept(false), handed, None);
                // A cell that takes main in holds what came of it where its run's earlier round was sent it.
                if self.cell.takes_main {
                    add(self.kept(true), handed, Some("main_held".to_string()));
                }
            }
            Entry::Reopened => add(Vec::new(), false, None),
            // Its session holds what its return handed in, and none of the message.
            // Its session holds what the return of the last round in it handed in, that of any cell of the line, and none
            // of the message.
            Entry::Compacted if self.writes() || self.others.iter().any(|o| self.writes_of(o)) => {
                add(Vec::new(), true, None)
            }
            Entry::Compacted => {}
            Entry::ReopenedAfterRead => {
                for other in &self.others {
                    let (holds, after_hand_in) = self.after(other);
                    let id = other.id.as_str().to_lowercase();
                    add(holds.clone(), after_hand_in, Some(format!("after_{id}")));
                    add(holds.clone(), false, Some(format!("after_{id}_nothing_handed_in")));
                    // That cell took main in too, and was sent what came of it.
                    if other.takes_main && self.cell.takes_main {
                        let held = self.with_main(&holds);
                        add(held.clone(), after_hand_in, Some(format!("after_{id}_main")));
                        add(held, false, Some(format!("after_{id}_main_nothing_handed_in")));
                    }
                }
            }
        }
        frames
    }

    /// The situation of an entry run by what it watches of these, its data standing so, and nothing else given.
    fn situation(&self, frame: &Frame, ran: &Ran, data: BTreeMap<String, Stands>) -> Situation {
        Situation {
            holds: frame.holds.clone(),
            after_hand_in: frame.after_hand_in,
            new: (self.cell.watches.iter())
                .filter(|t| ran.watched.iter().any(|r| r.name == t.name))
                .map(|t| t.name.clone())
                .collect(),
            data,
            main: None,
            results: Vec::new(),
            sent_back: false,
        }
    }

    /// What the session holds of the message's kept parts on the cell's first entry, and whether the last return in it
    /// handed in blocks: nothing, where the cell is the first of its line; else what the cell of the line before it
    /// was sent alike, and whether that cell writes any block.
    fn after_the_line(&self) -> (Vec<String>, bool) {
        match self.before.last() {
            Some(before) => self.after(before),
            None => (Vec::new(), false),
        }
    }

    /// What the session holds of the message's kept parts on an entry that goes on from the cell of the line this is,
    /// and whether the last return in it handed in blocks.
    fn after(&self, before: &CellSpec) -> (Vec<String>, bool) {
        let as_written = Settings::default();
        let texts = |cell: &CellSpec| kept(self.flow, self.words, cell, Values::Slots(&as_written));
        let theirs = texts(before);
        let alike = (texts(self.cell).into_iter())
            .filter(|part| theirs.contains(part))
            .map(|(part, _)| part)
            .collect();
        let place = self.flow.flow.place(&before.id);
        let outputs = place.map_or(&[][..], |p| &self.flow.flow.cells()[p].outputs[..]);
        (alike, !outputs.is_empty())
    }

    /// The other things a round may be given, each alone, on a later entry run by the first of what the cell watches
    /// that runs one; and, where a later entry holds what came of taking main in, a return that handed nothing in.
    fn besides(&self) -> Vec<Named> {
        let entry = Entry::Later;
        let Some(ran) = self.ran(entry).into_iter().next() else {
            return Vec::new();
        };
        let data: BTreeMap<String, Stands> = (self.classes(&ran.watched).into_iter())
            .map(|(block, class)| {
                let stands = match (class, self.last) {
                    (Class::Fired | Class::Grouped | Class::Rest, true) => Stands::HoldsNew,
                    (Class::Never | Class::Output, true) => Stands::Unwritten,
                    (Class::Fired | Class::Grouped, false) => Stands::HoldsLast,
                    (Class::Rest | Class::Never | Class::Output, false) => Stands::Holds,
                };
                (block.to_string(), stands)
            })
            .collect();
        // A return that asked for calls, or was sent back, handed nothing in.
        let after_hand_in = false;
        let mut out = Vec::new();
        for (n, frame) in self.frames(entry).iter().enumerate() {
            let base = self.situation(frame, &ran, data.clone());
            let named = |what: String, situation: Situation| Named {
                name: format!(
                    "{}-{}-{}{what}",
                    entry.name(),
                    ran.name,
                    frame.after.as_deref().map_or(String::new(), |a| format!("{a}-"))
                ),
                situation,
            };
            if n == 0 {
                for key in CALLS.iter().filter(|key| !UNREACHED.contains(key) && !self.last) {
                    let results = vec![key.to_string()];
                    let situation = Situation {
                        results,
                        after_hand_in,
                        ..base.clone()
                    };
                    out.push(named(format!("results_{key}"), situation));
                }
                let sent_back = Situation {
                    sent_back: true,
                    after_hand_in,
                    ..base.clone()
                };
                out.push(named("sent_back".into(), sent_back));
                if self.cell.takes_main {
                    for key in MAIN {
                        let main = Some(key.to_string());
                        out.push(named(format!("main_{key}"), Situation { main, ..base.clone() }));
                    }
                }
            }
            out.push(named("nothing_handed_in".into(), Situation { after_hand_in, ..base }));
        }
        out
    }
}
