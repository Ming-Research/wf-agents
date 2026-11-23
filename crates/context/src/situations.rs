//! Every situation an entry of an agent's cell can reach, as the flow's definition says, each with a name that says it,
//! for whoever reads what the cell is sent in each: the entry, what it watches that ran it, and how its session stands
//! with each datum of its context; and, apart, each other thing a round may be given.
//!
//! An entry is `first`, where the cell has worked from nothing; `later`, where the cell ran before in the same
//! session, which holds every part of the message it keeps, and whose last return handed in blocks; or `reopened`,
//! where the cell ran before and its session was begun anew, or compacted, since, and the cell is the first entered in
//! it, so that it holds nothing. On a first entry the session holds nothing where the cell is the first of its line in
//! the flow's order; otherwise it goes on from the cell of the line before it, and holds each part of the message kept
//! alike for both, the work's line, and what that cell's return handed in, where it writes any block.
//!
//! What it watches that ran it is each datum or group alone, those whose blocks one cell's return writes together, and
//! all of them at once. On a first entry, where everything written is new to the cell, every datum it watches that the work starts with runs it as well: what people
//! write that runs the flow's first agent's cell.
//!
//! A datum is certainly written when the cell runs where the work starts with it, or where the cells before it in the
//! order that may write it run whichever way the conditions go: one of them runs on no condition, or between them they
//! run on each value of a choice the cell itself does not run on one value of. A cell that runs is taken to write what
//! it may write; the flow says no more of which blocks a cell writes each time than what it must write.
//!
//! On a first entry, a datum only the cell itself or cells after it in the order write is not written yet, and what the
//! cell watches of such data runs no first entry.
//!
//! A datum the cell watches that ran it is new to it: the session lacks it, or holds it from a cell of the line that
//! is given it or writes it, as the version last worked from or the latest, or as one the cell never worked from; of a
//! group, one datum of the group at least is new. A datum it watches that did not run the cell, or one the
//! cell alone writes, is new to it never: unwritten on a first entry, held on a later one, lacking on one reopened, and
//! unwritten on either where it is not certainly written. A block the cell writes that nothing else gives it is given
//! only where the session lacks its latest: lacking or, held from a cell of the line, held on a first entry where it
//! may be written before; held on a later one, or lacking where another writes it too; lacking on one reopened; and
//! unwritten on any where it is not certainly written. Any other datum stands any way the entry allows: unwritten
//! where it is not certainly written, lacking, held from a cell of the line, or, on a later entry, held as last worked
//! from, latest, or with nothing new. A datum whose versions the carrier compares itself, given `last_and_latest`, that
//! stands lacking is unread: the session holds none of it.
//!
//! A datum's text is the builder's from its kind and from how the session stands with it alone, so a product of every
//! datum's own states repeats the same texts: it is counted ([`combinations`]), and the situations hold every state of
//! each datum with the data in two classes, each standing alike where it can: those of what ran the cell, and the rest;
//! a datum that cannot stand as its class does is unwritten where it may be, else lacking. Besides, on a later entry
//! run by the first datum or group the cell watches, with the data of what ran it held as last worked from and the rest
//! held: the result of a call of each kind and why a return was sent back, which come after a return that handed in
//! nothing, each come of taking main in where the cell takes it, new to the session, and an entry after a return that
//! handed in nothing.

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
}

impl Entry {
    const ALL: [Entry; 3] = [Entry::First, Entry::Later, Entry::Reopened];

    fn name(self) -> &'static str {
        match self {
            Entry::First => "first",
            Entry::Later => "later",
            Entry::Reopened => "reopened",
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
    /// A datum never new to the cell.
    Never,
    /// A block the cell writes that nothing else gives it.
    Output,
    /// Any other.
    Rest,
}

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
}

/// Every situation an entry of the agent's cell can reach, as the module says, each once, in order: the first entry,
/// the later and the reopened, each by the sets of what ran it, then the other things a round may be given. `compared`
/// is the blocks whose versions the carrier compares itself.
pub fn situations(flow: &Compiled, words: &Agents, cell: &CellSpec, compared: &BTreeSet<BlockId>) -> Vec<Named> {
    let reach = Reach::of(flow, words, cell, compared);
    let mut out: Vec<Named> = Vec::new();
    let mut add = |named: Named| {
        if !out.iter().any(|o| o.situation == named.situation) {
            out.push(named);
        }
    };
    for entry in Entry::ALL {
        for ran in reach.ran(entry) {
            let data = reach.classes(&ran.watched);
            // A class's state that none of its data can stand in gives nothing of its own.
            let states = |classes: &[Class], states: Vec<Stands>| {
                let of: Vec<(&BlockId, &Class)> = data.iter().filter(|(_, c)| classes.contains(c)).collect();
                let able = |s: &Stands| of.iter().any(|(b, c)| reach.allowed(entry, b, c).contains(s));
                let states: Vec<Option<Stands>> = states.into_iter().filter(able).map(Some).collect();
                if states.is_empty() { vec![None] } else { states }
            };
            for new in states(&[Class::Fired, Class::Grouped], reach.newly(entry, true)) {
                for rest in states(&[Class::Rest], reach.rest(entry, true, true)) {
                    let mut name = vec![entry.name().to_string(), ran.name.clone()];
                    name.extend(new.map(|s| format!("ran_{}", s.name())));
                    name.extend(rest.map(|s| format!("rest_{}", s.name())));
                    let stands = (data.iter())
                        .map(|(block, class)| {
                            let allowed = reach.allowed(entry, block, class);
                            let wanted = match class {
                                Class::Fired | Class::Grouped => new,
                                Class::Rest => rest,
                                Class::Never | Class::Output => allowed.first().copied(),
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
                        situation: reach.situation(entry, &ran, stands),
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
                            let new = reach.newly(entry, reach.may_hold(b)).len() as u128;
                            let all = reach.allowed(entry, b, &Class::Rest).len() as u128;
                            if is_new(b) { new } else { all - new }
                        })
                        .product::<u128>();
                }
            }
            count += product * ways;
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
        let mut reach = Reach {
            flow,
            words,
            cell,
            compared,
            given: crate::message::given(flow, words, cell),
            certain: BTreeSet::new(),
            starts: BTreeSet::new(),
            before,
        };
        let given: Vec<BlockId> = reach.given.iter().map(|g| g.block.clone()).collect();
        reach.starts = given.iter().filter(|b| reach.starts_with(b)).cloned().collect();
        let certain = given
            .iter()
            .filter(|b| reach.starts.contains(*b) || reach.written_before(place, b));
        reach.certain = certain.cloned().collect();
        reach
    }

    /// Whether the work starts with the datum: people write it, and it runs the flow's first agent's cell.
    fn starts_with(&self, block: &BlockId) -> bool {
        let flow = self.flow;
        let first = (flow.flow.cells().iter())
            .filter_map(|c| flow.cells.iter().find(|s| s.id == c.id))
            .find(|c| c.kind == Kind::Agent);
        let people = flow.blocks.get(block).is_some_and(|d| d.writers.people);
        people && first.is_some_and(|c| (c.watches.iter()).any(|t| !t.is_group() && t.name == block.as_str()))
    }

    /// Whether the cells before the one at this place that may write the datum run whichever way the conditions go, as
    /// the module says.
    fn written_before(&self, place: usize, block: &BlockId) -> bool {
        let flow = self.flow;
        let cells = flow.flow.cells();
        let writers: Vec<&Cell> = cells[..place].iter().filter(|c| c.outputs.contains(block)).collect();
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

    /// Whether the session may hold the datum on the cell's first entry: a cell of its line before it is given it or
    /// writes it.
    fn may_hold(&self, block: &BlockId) -> bool {
        let writes = |c: &CellSpec| {
            let place = self.flow.flow.place(&c.id);
            place.is_some_and(|p| self.flow.flow.cells()[p].outputs.contains(block))
        };
        (self.before.iter()).any(|c| c.given.iter().any(|g| g.block == *block) || writes(c))
    }

    /// The states of a datum new to the cell, held from another cell of the line where it may be.
    fn newly(&self, entry: Entry, may_hold: bool) -> Vec<Stands> {
        match entry {
            Entry::First if may_hold => vec![Stands::Lacks, Stands::HoldsNew],
            Entry::First | Entry::Reopened => vec![Stands::Lacks],
            Entry::Later => vec![Stands::Lacks, Stands::HoldsLast, Stands::HoldsLatest, Stands::HoldsNew],
        }
    }

    /// The states of any datum, unwritten where it may be and held from another cell of the line where it may be.
    fn rest(&self, entry: Entry, unwritten: bool, may_hold: bool) -> Vec<Stands> {
        let mut states: Vec<Stands> = unwritten.then_some(Stands::Unwritten).into_iter().collect();
        match entry {
            Entry::First => {
                states.push(Stands::Lacks);
                states.extend(may_hold.then_some(Stands::HoldsNew));
            }
            Entry::Later => states.extend([
                Stands::Lacks,
                Stands::HoldsLast,
                Stands::HoldsLatest,
                Stands::HoldsNew,
                Stands::Holds,
            ]),
            Entry::Reopened => states.push(Stands::Lacks),
        }
        states
    }

    /// The states the datum can stand in on the entry, in its class, the one it stands in most first.
    fn allowed(&self, entry: Entry, block: &BlockId, class: &Class) -> Vec<Stands> {
        let unwritten = !self.certain.contains(block);
        if entry == Entry::First && !self.early(block) {
            return vec![Stands::Unwritten];
        }
        match class {
            Class::Fired => self.newly(entry, self.may_hold(block)),
            Class::Grouped | Class::Rest => self.rest(entry, unwritten, self.may_hold(block)),
            Class::Never => {
                let mut states = match entry {
                    Entry::First => vec![],
                    Entry::Later => vec![Stands::Holds],
                    Entry::Reopened => vec![Stands::Lacks],
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
                        states.extend(self.may_hold(block).then_some(Stands::Holds));
                        states
                    }
                    Entry::Later => {
                        let mut states = vec![Stands::Holds];
                        states.extend(others.then_some(Stands::Lacks));
                        states
                    }
                    Entry::Reopened => vec![Stands::Lacks],
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
        let of_others = |b: &BlockId| {
            (cell.watches.iter())
                .filter(|t| !ran.iter().any(|r| r.name == t.name))
                .any(|t| t.blocks.contains(b))
        };
        let alone = |b: &BlockId| {
            flow.blocks.get(b).is_some_and(|d| {
                let w = &d.writers;
                w.cells == [cell.id.clone()] && !w.people && w.outside.is_none()
            })
        };
        for block in given {
            if !classes.contains_key(block) {
                let class = if of_others(block) || alone(block) {
                    Class::Never
                } else {
                    Class::Rest
                };
                classes.insert(block.clone(), class);
            }
        }
        classes
    }

    /// The parts of the message the session keeps, but what came of taking main in, which is the run's.
    fn kept(&self) -> Vec<String> {
        (self.words.message.parts.iter())
            .filter(|p| p.kind.is_some() && p.part != Part::Context && p.part != Part::Main)
            .map(|p| p.part.name().to_string())
            .collect()
    }

    /// The situation of an entry run by what it watches of these, its data standing so, and nothing else given.
    fn situation(&self, entry: Entry, ran: &Ran, data: BTreeMap<String, Stands>) -> Situation {
        let (holds, after_hand_in) = match entry {
            Entry::First => self.after_the_line(),
            Entry::Later => (self.kept(), true),
            Entry::Reopened => (Vec::new(), false),
        };
        Situation {
            holds,
            after_hand_in,
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
        let Some(before) = self.before.last() else {
            return (Vec::new(), false);
        };
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
    /// that runs one.
    fn besides(&self) -> Vec<Named> {
        let entry = Entry::Later;
        let Some(ran) = self.ran(entry).into_iter().next() else {
            return Vec::new();
        };
        let data = (self.classes(&ran.watched).into_iter())
            .map(|(block, class)| {
                let stands = match class {
                    Class::Fired | Class::Grouped => Stands::HoldsLast,
                    Class::Rest | Class::Never | Class::Output => Stands::Holds,
                };
                (block.to_string(), stands)
            })
            .collect();
        let base = self.situation(entry, &ran, data);
        let named = |what: String, situation: Situation| Named {
            name: format!("{}-{}-{what}", entry.name(), ran.name),
            situation,
        };
        // A return that asked for calls, or was sent back, handed nothing in.
        let after_hand_in = false;
        let mut out = Vec::new();
        for key in CALLS {
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
        out.push(named("nothing_handed_in".into(), Situation { after_hand_in, ..base }));
        out
    }
}
