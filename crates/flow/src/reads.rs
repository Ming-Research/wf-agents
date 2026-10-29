//! What is read of every return, and what is there to read.
//!
//! - Nothing dropped: every value of a return is read on some way on from the row it takes, before the cell
//!   answers again: by the row itself (what it switches on or does), by what the cell always does or checks, by its
//!   handoff, or by a cell on the way. A return of only a common form does not replace the cell's answer: what is
//!   read after it still counts for the answer, but not for the common fields, which belong to the one return that
//!   holds them.
//! - Nothing missing: on every way from the start to a cell, each input it reads has been written, unless the input
//!   is marked maybe; and an input marked maybe is missing on some way. An engine step that switches on an input it
//!   reads by id takes a row only on what that input can hold: what was not written, it cannot see.

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::bits::Bits;
use crate::check::Analysis;
use crate::field::{Place, PlaceKind, by_id, head};
use crate::format::Src;
use crate::graph::{CellKind, Graph, POINTERS, Target};
use crate::walk::State;

/// A place of a cell's output: the cell, and the place's number among its places.
type Spot = (usize, usize);

/// Whether `x` is `u` or inside it.
fn under(x: &str, u: &str) -> bool {
    x == u || (x.starts_with(u) && matches!(x.as_bytes().get(u.len()), Some(b'.' | b'[')))
}

/// A leaf is read when it or a field around it is read. A union's marker is part of its branches: it counts as
/// read when any branch is; a branch with no fields, when its marker is.
fn covers(p: &Place, u: &str) -> bool {
    match &p.kind {
        PlaceKind::Field => under(&p.key, u),
        PlaceKind::Disc(name) => {
            let union = &p.key[..p.key.len() - name.len() - 1];
            under(&p.key, u) || (u.starts_with(union) && u.as_bytes().get(union.len()) == Some(&b'['))
        }
        PlaceKind::Branch(dk) => under(&p.key, u) || under(dk, u),
    }
}

/// The least fixed point of: a state's own part, joined with the answer at each state it leads to.
fn fixed_point(mut val: Vec<Bits>, succ: &[Vec<usize>]) -> Vec<Bits> {
    let n = val.len();
    let mut preds = vec![Vec::new(); n];
    for (s, ts) in succ.iter().enumerate() {
        for &t in ts {
            preds[t].push(s);
        }
    }
    let mut todo: VecDeque<usize> = (0..n).collect();
    let mut queued = vec![true; n];
    while let Some(t) = todo.pop_front() {
        queued[t] = false;
        for &s in &preds[t] {
            let mut v = val[s].clone();
            v.or(&val[t]);
            if v != val[s] {
                val[s] = v;
                if !queued[s] {
                    queued[s] = true;
                    todo.push_back(s);
                }
            }
        }
    }
    val
}

/// One way of the search for a missing input: the state, and which of the input's places are written.
type At = (usize, Bits);

impl Analysis<'_, '_> {
    fn point(&self, n: &str, s: &State) -> Option<usize> {
        if Graph::is_pointer(n) {
            s.ptr(n)
        } else {
            self.g.at.get(n).copied()
        }
    }

    fn spot(&self, n: &str, p: &str, s: &State) -> Option<Spot> {
        let m = self.point(n, s)?;
        let k = self.g.out_key(m, p)?;
        self.place_at[m].get(&k).map(|i| (m, *i))
    }

    /// Whether the place's field is one every agent's return may hold, and not one of those the outcome leaves.
    fn replaced(&self, cell: usize, place: usize, outcome: usize) -> bool {
        let o = &self.rets[cell].outcomes[outcome];
        let top = head(&self.places[cell][place].key);
        o.answered || (self.g.common_names.contains(top) && !o.keeps.iter().any(|k| k.as_str() == top))
    }

    /// What the cell at a state reads of earlier returns: a cell with an output reads all its inputs to make it; an
    /// engine step with none reads its inputs only where its rows, actions and checks name them by id. A condition
    /// on whether a place is there reads none of it.
    pub fn reads_at(&self, si: usize) -> BTreeSet<Spot> {
        let s = &self.states[si];
        let c = &self.g.cells[s.at];
        let mut srcs: Vec<(String, String)> = Vec::new();
        if c.reads_all {
            for x in &c.inputs {
                for src in &x.from {
                    if let Src::Place(n, p) = src {
                        srcs.push((n.clone(), p.clone()));
                    }
                }
            }
        } else {
            for p in c.paths_named(true) {
                let Some((iid, rest)) = by_id(&p) else { continue };
                let Some(j) = c.by_id.get(iid) else { continue };
                if let [Src::Place(n, q)] = c.inputs[*j].from.as_slice() {
                    srcs.push((n.clone(), format!("{q}{rest}")));
                }
            }
        }
        srcs.iter().filter_map(|(n, p)| self.spot(n, p, s)).collect()
    }

    pub fn dropped(&mut self) {
        let g = self.g;
        let n = self.states.len();
        let all_succ: Vec<Vec<usize>> = self.trans.iter().map(|ts| ts.iter().map(|x| x.1).collect()).collect();
        for ci in 0..g.cells.len() {
            let c = &g.cells[ci];
            if c.out.is_empty() || ci == g.end {
                continue;
            }
            let km = &self.places[ci];
            let nk = km.len();
            // For each leaf, the places whose reading reads it.
            let leaves: Vec<(usize, Bits, bool)> = km
                .iter()
                .enumerate()
                .filter(|(_, p)| p.leaf)
                .map(|(i, p)| {
                    let mut by = Bits::new(nk);
                    for (j, u) in km.iter().enumerate() {
                        if covers(p, &u.key) {
                            by.set(j);
                        }
                    }
                    (i, by, g.common_names.contains(head(&p.key)))
                })
                .collect();
            let reads: Vec<Bits> = (0..n)
                .map(|s| {
                    let mut b = Bits::new(nk);
                    for &(m, k) in &self.reads[s] {
                        if m == ci {
                            b.set(k);
                        }
                    }
                    b
                })
                .collect();
            let here = |s: usize| self.states[s].at == ci;
            let common = |s: usize| -> Vec<usize> {
                self.trans[s]
                    .iter()
                    .filter(|(r, _)| c.rows[*r].common)
                    .map(|x| x.1)
                    .collect()
            };
            // Read from a state on before the cell returns again; after a return of the cell that does not replace
            // its answer; and from a state on that is already after one.
            let succ: Vec<Vec<usize>> = (0..n)
                .map(|s| if here(s) { common(s) } else { all_succ[s].clone() })
                .collect();
            let after = fixed_point(reads.clone(), &succ);
            let succ: Vec<Vec<usize>> = (0..n)
                .map(|s| if here(s) { Vec::new() } else { all_succ[s].clone() })
                .collect();
            let first = fixed_point(reads, &succ);
            let base: Vec<Bits> = (0..n)
                .map(|s| {
                    let mut b = Bits::new(nk);
                    if here(s) {
                        for t in common(s) {
                            b.or(&after[t]);
                        }
                    }
                    b
                })
                .collect();
            let later = fixed_point(base, &succ);
            let own: Vec<Bits> = c
                .rows
                .iter()
                .map(|r| {
                    let mut b = Bits::new(nk);
                    for p in c.row_paths(r).iter().filter(|p| !p.starts_with('@')) {
                        if let Some(i) = self.place_at[ci].get(g.akey(ci, p)) {
                            b.set(*i);
                        }
                    }
                    b
                })
                .collect();
            let mut found = Vec::new();
            for s in (0..n).filter(|s| here(*s)) {
                for &(r, s2, t) in &self.steps[s] {
                    // A row that waits where the cell is, as a work behind another in the merge queue does, is taken
                    // before the cell's step hands anything in; and an engine step's row that takes an interjection
                    // sets aside what the step handed in, which it works out again if the way on comes back to it:
                    // neither drops anything.
                    let row = &c.rows[r];
                    if (row.waits && row.target == Target::Cell(ci)) || (row.interject && c.kind != CellKind::Agent) {
                        continue;
                    }
                    let mut held = Bits::new(nk);
                    for o in &self.takes[t] {
                        held.or(&self.rets[ci].outcomes[*o].pres);
                    }
                    let mut read = own[r].clone();
                    read.or(&first[s2]);
                    let mut read_later = read.clone();
                    read_later.or(&later[s2]);
                    for (k, by, com) in &leaves {
                        if held.has(*k) && !by.meets(if *com { &read } else { &read_later }) {
                            found.push((r, *k, s));
                        }
                    }
                }
            }
            for (r, k, s) in found {
                let way = format!("{} -> {}", self.way(s), c.id);
                self.findings.drop_place(&c.id, &c.rows[r].name, &km[k].key, way);
            }
        }
    }

    pub fn missing(&mut self) {
        let g = self.g;
        let n = self.states.len();
        let checked: Vec<(usize, usize)> = g
            .cells
            .iter()
            .enumerate()
            .flat_map(|(m, c)| {
                c.inputs
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| !x.from.is_empty() && x.from.iter().all(|s| matches!(s, Src::Place(..))))
                    .map(move |(i, _)| (m, i))
            })
            .collect();
        // The cells a source of a cell can come from.
        let from = |m: usize, src: &Src| -> Vec<Spot> {
            let Src::Place(c, p) = src else { return Vec::new() };
            let cands: Vec<usize> = match POINTERS.iter().copied().find(|q| *q == c.as_str()) {
                Some(ptr) => self.points.get(&(m, ptr)).into_iter().flatten().copied().collect(),
                None => g.at.get(c).copied().into_iter().collect(),
            };
            cands
                .into_iter()
                .filter_map(|x| g.out_key(x, p).and_then(|k| self.place_at[x].get(&k).map(|i| (x, *i))))
                .collect()
        };
        // One bit per place some input reads.
        let mut bit: HashMap<Spot, usize> = HashMap::new();
        for &(m, i) in &checked {
            for src in &g.cells[m].inputs[i].from {
                for sp in from(m, src) {
                    let len = bit.len();
                    bit.entry(sp).or_insert(len);
                }
            }
        }
        let nb = bit.len();
        let mask = |spots: &mut dyn Iterator<Item = Spot>| {
            let mut b = Bits::new(nb);
            for sp in spots {
                if let Some(i) = bit.get(&sp) {
                    b.set(*i);
                }
            }
            b
        };
        // What each outcome of each cell replaces and writes of those places.
        let rep: Vec<Vec<(Bits, Bits)>> = (0..g.cells.len())
            .map(|ci| {
                (0..self.rets[ci].outcomes.len())
                    .map(|o| {
                        let pres = &self.rets[ci].outcomes[o].pres;
                        let ks = self.places[ci].len();
                        let gone = mask(&mut (0..ks).filter(|k| self.replaced(ci, *k, o)).map(|k| (ci, k)));
                        let got = mask(&mut (0..ks).filter(|k| pres.has(*k)).map(|k| (ci, k)));
                        (gone, got)
                    })
                    .collect()
            })
            .collect();
        // The places a return of the cell at a state sees: it can only see what was written.
        let seen_at = |s: &State, sees: &BTreeSet<String>| {
            let c = &g.cells[s.at];
            mask(&mut sees.iter().filter_map(|k| {
                let (iid, rest) = by_id(k)?;
                let j = c.by_id.get(iid)?;
                let Src::Place(n, q) = c.inputs[*j].from.first()? else {
                    return None;
                };
                self.spot(n, &format!("{q}{rest}"), s)
            }))
        };
        // For each state: (row, the state it leads to, {(replaced, written, seen)} of the returns that take the row).
        type Outs = Vec<(Bits, Bits, Bits)>;
        let go: Vec<Vec<(usize, usize, Outs)>> = (0..n)
            .map(|si| {
                let s = &self.states[si];
                self.steps[si]
                    .iter()
                    .map(|&(r, j, t)| {
                        let mut outs: Outs = Vec::new();
                        for &o in &self.takes[t] {
                            let (gone, got) = rep[s.at][o].clone();
                            let x = (gone, got, seen_at(s, &self.rets[s.at].outcomes[o].sees));
                            if !outs.contains(&x) {
                                outs.push(x);
                            }
                        }
                        (r, j, outs)
                    })
                    .collect()
            })
            .collect();
        let mut seen_any = Bits::new(nb);
        let mut before: Vec<Vec<(usize, Bits)>> = vec![Vec::new(); n];
        for (i, steps) in go.iter().enumerate() {
            for (_, j, outs) in steps {
                let mut gone = Bits::new(nb);
                for b in 0..nb {
                    gone.set(b);
                }
                for (g0, _, v) in outs {
                    gone.and(g0);
                    seen_any.or(v);
                }
                before[*j].push((i, gone));
            }
        }
        let mut found = Vec::new();
        for &(m, idx) in &checked {
            let x = &g.cells[m].inputs[idx];
            let mut ks: Vec<usize> = Vec::new();
            for src in &x.from {
                for sp in from(m, src) {
                    let b = bit[&sp];
                    if !ks.contains(&b) {
                        ks.push(b);
                    }
                }
            }
            // The search tracks only the places of this input, one bit each.
            let nl = ks.len();
            let loc = |v: &Bits| {
                let mut b = Bits::new(nl);
                for (i, k) in ks.iter().enumerate() {
                    if v.has(*k) {
                        b.set(i);
                    }
                }
                b
            };
            // The places of which one must be written when the work is at the cell.
            let mut need: HashMap<usize, Bits> = HashMap::new();
            for (i, s) in self.states.iter().enumerate().filter(|(_, s)| s.at == m) {
                let mut b = Bits::new(nl);
                for src in &x.from {
                    if let Src::Place(c, p) = src
                        && let Some(sp) = self.spot(c, p, s)
                        && let Some(gb) = bit.get(&sp)
                    {
                        let mut one = Bits::new(nb);
                        one.set(*gb);
                        b.or(&loc(&one));
                    }
                }
                need.insert(i, b);
            }
            // A place matters at a state only when some way on reaches the cell before every return on it replaces
            // the place; the search keeps only those, so ways that differ in places that no longer matter are one.
            let mut live = vec![Bits::new(nl); n];
            let mut todo: VecDeque<usize> = VecDeque::new();
            let mut queued = vec![false; n];
            for (i, v) in &need {
                if v.any() {
                    live[*i] = v.clone();
                    todo.push_back(*i);
                    queued[*i] = true;
                }
            }
            while let Some(t) = todo.pop_front() {
                queued[t] = false;
                for (i, gone) in &before[t] {
                    let mut v = live[t].clone();
                    v.and_not(&loc(gone));
                    v.or(&live[*i]);
                    if v != live[*i] {
                        live[*i] = v;
                        if !queued[*i] {
                            queued[*i] = true;
                            todo.push_back(*i);
                        }
                    }
                }
            }
            // A way that has written less finds the input missing at least as often as one that has written more,
            // and goes wherever it goes, as long as the two hold the same of the places an engine step can see. So
            // at each state, for each set of those places, only the least sets of the others are kept.
            let sees = loc(&seen_any);
            let mut on: HashMap<usize, Vec<(usize, usize, Outs)>> = HashMap::new();
            let start: At = (0, Bits::new(nl));
            let mut par: HashMap<At, Option<(At, usize)>> = HashMap::from([(start.clone(), None)]);
            let mut least: HashMap<At, Vec<Bits>> = HashMap::from([(start.clone(), vec![Bits::new(nl)])]);
            let mut q = VecDeque::from([start]);
            let mut absent = None;
            while let Some(st) = q.pop_front() {
                let (i, av) = &st;
                if let Some(nd) = need.get(i)
                    && !av.meets(nd)
                {
                    absent = Some(st);
                    break;
                }
                let steps = on.entry(*i).or_insert_with(|| {
                    go[*i]
                        .iter()
                        .map(|(r, j, outs)| (*r, *j, outs.iter().map(|(a, b, c)| (loc(a), loc(b), loc(c))).collect()))
                        .collect()
                });
                for (r, j, outs) in steps.iter() {
                    for (gone, got, seen) in outs {
                        // A place that still matters and is not written cannot be seen.
                        let mut blind = seen.clone();
                        blind.and(&live[*i]);
                        blind.and_not(av);
                        if blind.any() {
                            continue;
                        }
                        let mut a2 = av.clone();
                        a2.and_not(gone);
                        a2.or(got);
                        a2.and(&live[*j]);
                        let mut rest = a2.clone();
                        rest.and_not(&sees);
                        let mut kept = a2.clone();
                        kept.and(&sees);
                        let st2: At = (*j, a2);
                        let lot = least.entry((*j, kept)).or_default();
                        if par.contains_key(&st2) || lot.iter().any(|a| a.within(&rest)) {
                            continue;
                        }
                        lot.push(rest);
                        par.insert(st2.clone(), Some((st.clone(), *r)));
                        q.push_back(st2);
                    }
                }
            }
            let at = (idx + 1).to_string();
            let what: String = x.what.chars().take(40).collect();
            match (absent, x.maybe) {
                (Some(st), false) => {
                    let mut way = Vec::new();
                    let mut cur = st;
                    while let Some(Some((prev, r))) = par.get(&cur) {
                        let c = &g.cells[self.states[prev.0].at];
                        way.push(format!("{} \"{}\"", c.id, c.rows[*r].name));
                        cur = prev.clone();
                    }
                    way.reverse();
                    let way = if way.is_empty() {
                        "the start".to_string()
                    } else {
                        way.join(" -> ")
                    };
                    found.push((
                        "missing",
                        m,
                        at,
                        format!("\"{what}\" not written on the way {way} -> {}", g.cells[m].id),
                    ));
                }
                (None, true) => found.push(("marked maybe", m, at, format!("\"{what}\" written on every way"))),
                _ => {}
            }
        }
        for (kind, m, at, ex) in found {
            self.findings.first(kind, &g.cells[m].id, &at, ex);
        }
    }
}
