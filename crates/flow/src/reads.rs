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
//! - What a session holds: an agent's cell on a line resumes the session the line's earlier rounds left. An input is
//!   already in that session when, on every way to the cell, every place it reads was last replaced by a round of
//!   the line, or was given to one of its rounds since; a place nothing has written counts as held, there being
//!   nothing to give. The engine gives the rest. A rule is held when an earlier round of the line was given it.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::bits::Bits;
use crate::check::Analysis;
use crate::field::{Place, PlaceKind, by_id, head};
use crate::format::Src;
use crate::graph::{CellKind, Graph, POINTERS};
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

    pub fn sessions(&mut self) {
        let g = self.g;
        let n = self.states.len();
        for line in &g.lines {
            let boxes: BTreeSet<usize> = (0..g.cells.len())
                .filter(|i| g.cells[*i].line.as_ref() == Some(line))
                .collect();
            let at_boxes = |s: usize| boxes.contains(&self.states[s].at);
            let u: BTreeSet<Spot> = (0..n)
                .filter(|s| at_boxes(*s))
                .flat_map(|s| self.reads[s].iter().copied())
                .collect();
            let bit: HashMap<Spot, usize> = u.iter().enumerate().map(|(i, sp)| (*sp, i)).collect();
            let nu = u.len();
            let mut full = Bits::new(nu);
            for i in 0..nu {
                full.set(i);
            }
            let mut own: BTreeMap<usize, Vec<Spot>> = BTreeMap::new();
            for sp in &u {
                own.entry(sp.0).or_default().push(*sp);
            }
            let msk = |spots: &mut dyn Iterator<Item = Spot>| {
                let mut b = Bits::new(nu);
                for sp in spots {
                    if let Some(i) = bit.get(&sp) {
                        b.set(*i);
                    }
                }
                b
            };
            // Reading a place reads every place inside it.
            let inside: HashMap<Spot, Bits> = u
                .iter()
                .map(|&(c, k)| {
                    let key = &self.places[c][k].key;
                    let mut ins = own[&c]
                        .iter()
                        .copied()
                        .filter(|&(_, j)| under(&self.places[c][j].key, key));
                    ((c, k), msk(&mut ins))
                })
                .collect();
            let read: Vec<Bits> = (0..n)
                .map(|s| {
                    let mut b = Bits::new(nu);
                    if at_boxes(s) {
                        for sp in &self.reads[s] {
                            if let Some(x) = inside.get(sp) {
                                b.or(x);
                            }
                        }
                    }
                    b
                })
                .collect();
            // Of what the line reads, what each outcome replaces and writes.
            let mut masks: HashMap<(usize, usize), (Bits, Bits)> = HashMap::new();
            for m in 0..g.cells.len() {
                for o in 0..self.rets[m].outcomes.len() {
                    let pres = &self.rets[m].outcomes[o].pres;
                    let mine = own.get(&m).map(Vec::as_slice).unwrap_or_default();
                    let gone: Vec<Spot> = mine.iter().copied().filter(|&(_, k)| self.replaced(m, k, o)).collect();
                    let got: Vec<Spot> = gone.iter().copied().filter(|&(_, k)| pres.has(k)).collect();
                    masks.insert((m, o), (msk(&mut gone.into_iter()), msk(&mut got.into_iter())));
                }
            }
            // What the line holds after the work leaves a state, for every return that takes the row: a round of the
            // line holds what it read and what it returned; any other return or step replaces what the line held.
            let moved = |s: usize, t: usize, v: &Bits| {
                let m = self.states[s].at;
                let mut v = v.clone();
                if at_boxes(s) {
                    v.or(&read[s]);
                }
                let mut out = full.clone();
                for &o in &self.takes[t] {
                    let (gone, got) = &masks[&(m, o)];
                    let mut x = v.clone();
                    x.or(gone);
                    if !at_boxes(s) {
                        x.and_not(got);
                    }
                    out.and(&x);
                }
                out
            };
            let mut holds = vec![full.clone(); n];
            let mut todo: VecDeque<usize> = (0..n).collect();
            let mut queued = vec![true; n];
            while let Some(s) = todo.pop_front() {
                queued[s] = false;
                for &(_, s2, t) in &self.steps[s] {
                    let mut v = moved(s, t, &holds[s]);
                    v.and(&holds[s2]);
                    if v != holds[s2] {
                        holds[s2] = v;
                        if !queued[s2] {
                            queued[s2] = true;
                            todo.push_back(s2);
                        }
                    }
                }
            }
            for &m in &boxes {
                for (idx, x) in g.cells[m].inputs.iter().enumerate() {
                    let held = (0..n).filter(|s| self.states[*s].at == m).all(|s| {
                        let st = &self.states[s];
                        let mut b = msk(&mut x.from.iter().filter_map(|src| match src {
                            Src::Place(c, p) => self.spot(c, p, st),
                            _ => None,
                        }));
                        b.and_not(&holds[s]);
                        !b.any()
                    });
                    self.held.insert((m, idx), held);
                }
            }
            // A rule never changes: the line's session holds it when, on every way to the cell, an earlier round
            // of the line was given it. The line's session holds none at the start.
            let given = |c: usize| -> Vec<Option<String>> {
                let cell = &g.cells[c];
                let mut out = Vec::new();
                if cell.kind == CellKind::Agent {
                    out.push(None);
                }
                out.extend(cell.rules.iter().cloned().map(Some));
                out
            };
            let names: Vec<Option<String>> = boxes
                .iter()
                .flat_map(|c| given(*c))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if names.is_empty() {
                continue;
            }
            let rbit = |r: &Option<String>| names.iter().position(|x| x == r).unwrap_or(0);
            let nr = names.len();
            let mine: BTreeMap<usize, Bits> = boxes
                .iter()
                .map(|c| {
                    let mut b = Bits::new(nr);
                    for r in given(*c) {
                        b.set(rbit(&r));
                    }
                    (*c, b)
                })
                .collect();
            let mut all = Bits::new(nr);
            for i in 0..nr {
                all.set(i);
            }
            let mut rh = vec![all; n];
            rh[0] = Bits::new(nr);
            let mut todo: VecDeque<usize> = (0..n).collect();
            let mut queued = vec![true; n];
            while let Some(s) = todo.pop_front() {
                queued[s] = false;
                for &(_, s2, _) in &self.steps[s] {
                    let mut v = rh[s].clone();
                    if let Some(b) = mine.get(&self.states[s].at) {
                        v.or(b);
                    }
                    v.and(&rh[s2]);
                    if v != rh[s2] && s2 != 0 {
                        rh[s2] = v;
                        if !queued[s2] {
                            queued[s2] = true;
                            todo.push_back(s2);
                        }
                    }
                }
            }
            for &m in &boxes {
                for r in given(m) {
                    let b = rbit(&r);
                    let held = (0..n).filter(|s| self.states[*s].at == m).all(|s| rh[s].has(b));
                    self.rule_held.insert((m, r), held);
                }
            }
        }
    }
}
