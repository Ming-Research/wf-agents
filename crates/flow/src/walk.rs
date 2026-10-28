//! Every state the engine can be in, walking from the start.
//!
//! A state is the cell the work is at, where the work stopped, the cells that called and wait for the answer in the
//! order they called, the cell whose row brought the work here, and what rows recorded. Going into the asking part
//! from outside while no cell waits, the engine notes the cell the work leaves as where it stopped; it forgets it
//! when the work is out of the part and no cell waits. Going to a called cell, the engine adds the cell that calls to
//! those that wait; going back takes the last one off. An answer to an agent's cell outside the asking part, on the
//! line of the cell that receives it, comes in there; one to another agent's cell or to an engine step goes back to
//! that cell, which runs again. While a cell waits, the work only goes on by calls and their answers: an agent's cell
//! that waits is not called again, since its round is still open, so a row to the agent's cell that called goes back
//! to it; and a row that goes on otherwise leaves the cell without its answer, unless it says it drops the cells that
//! wait, which get their answer later another way. An engine step that waits may be called again: it keeps nothing
//! between runs, and each answer goes back to the last call.

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::check::Analysis;
use crate::format::Src;
use crate::graph::{CellKind, Graph, POINTERS, Row, Target, ensure};
use crate::returns::holds;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct State {
    pub at: usize,
    pub stopped: Option<usize>,
    pub callers: Vec<usize>,
    pub from: Option<usize>,
    pub recorded: Recorded,
}

/// The values rows recorded on the way, by name.
pub(crate) type Recorded = Vec<(String, String)>;

impl State {
    /// The cell a source naming a pointer points at in this state.
    pub fn ptr(&self, p: &str) -> Option<usize> {
        match p {
            "@stopped" => self.stopped,
            "@caller" => self.callers.last().copied(),
            "@from" => self.from,
            _ => None,
        }
    }
}

/// The rows a cell's returns take, each with the outcomes that take it.
pub(crate) type Taken = BTreeMap<usize, BTreeSet<usize>>;

pub(crate) enum Step {
    To(State),
    /// The row calls a cell that is still waiting for an answer.
    Again,
    /// The row goes on while a cell still waits for an answer.
    Left,
    /// The row points at no cell.
    Nowhere,
}

impl Graph<'_> {
    pub fn step(&self, s: &State, r: &Row) -> Step {
        let mut v: BTreeMap<String, String> = s.recorded.iter().cloned().collect();
        v.extend(r.sets.iter().map(|(k, x)| (k.clone(), x.clone())));
        let v: Recorded = v.into_iter().collect();
        let m = s.at;
        // A row that goes on without answering the cells that wait leaves them, and where the work stopped, behind.
        let dropped;
        let s = if r.drops {
            dropped = State {
                stopped: None,
                callers: Vec::new(),
                ..s.clone()
            };
            &dropped
        } else {
            s
        };
        if r.target == Target::Same {
            // A pause: the same cell goes on in the same round, with what brought the work here.
            return Step::To(State {
                recorded: v,
                ..s.clone()
            });
        }
        let to_caller = matches!(r.target, Target::Cell(t) if s.callers.last() == Some(&t) && self.cells[t].kind == CellKind::Agent);
        if matches!(r.target, Target::Caller | Target::Asker) || to_caller {
            let Some((&last, rest)) = s.callers.split_last() else {
                return Step::Nowhere;
            };
            let mut t = last;
            if r.target == Target::Asker
                && let Some(rc) = self.receive
                && !self.region.contains(&t)
                && self.cells[t].kind == CellKind::Agent
                && self.cells[t].line == self.cells[rc].line
            {
                t = rc;
            }
            let stopped = if self.region.contains(&t) || !rest.is_empty() {
                s.stopped
            } else {
                None
            };
            return Step::To(State {
                at: t,
                stopped,
                callers: rest.to_vec(),
                from: Some(m),
                recorded: v,
            });
        }
        let t = match r.target {
            Target::Stopped => s.stopped,
            Target::Cell(t) => Some(t),
            _ => None,
        };
        let Some(t) = t else { return Step::Nowhere };
        if self.called.contains(&t) {
            if t == m || (s.callers.contains(&t) && self.cells[t].kind == CellKind::Agent) {
                return Step::Again;
            }
            let stopped = if self.region.contains(&m) || !s.callers.is_empty() {
                s.stopped
            } else if self.region.contains(&t) {
                Some(m)
            } else {
                None
            };
            let mut callers = s.callers.clone();
            callers.push(m);
            return Step::To(State {
                at: t,
                stopped,
                callers,
                from: Some(m),
                recorded: v,
            });
        }
        if !s.callers.is_empty() {
            return Step::Left;
        }
        let stopped = if !self.region.contains(&t) {
            None
        } else if self.region.contains(&m) {
            s.stopped
        } else {
            Some(m)
        };
        Step::To(State {
            at: t,
            stopped,
            callers: Vec::new(),
            from: Some(m),
            recorded: v,
        })
    }
}

impl Analysis<'_, '_> {
    /// The rows the returns of a cell take when the engine has recorded `v`, each with the outcomes that take it;
    /// and the recorded values the cell reads that `v` does not hold.
    pub fn rows_at(&self, m: usize, v: &[(String, String)]) -> (Taken, Vec<String>) {
        let names = &self.inp_names[m];
        let fixed: BTreeMap<String, String> = v.iter().filter(|(k, _)| names.contains(k)).cloned().collect();
        let mut taken = Taken::new();
        for r in &self.rets[m].rets {
            for inp in self.assignments(m, &fixed) {
                let row = self.conds[m]
                    .iter()
                    .position(|cs| cs.iter().all(|x| holds(x, &r.env, &inp)));
                if let Some(row) = row {
                    taken.entry(row).or_default().insert(r.outcome);
                }
            }
        }
        let unset = names
            .iter()
            .filter(|k| self.set_names.contains(*k) && !fixed.contains_key(*k))
            .cloned()
            .collect();
        (taken, unset)
    }

    pub fn walk(&mut self) {
        let g = self.g;
        let s0 = State {
            at: g.start,
            stopped: None,
            callers: Vec::new(),
            from: None,
            recorded: Vec::new(),
        };
        let mut index: HashMap<State, usize> = HashMap::new();
        index.insert(s0.clone(), 0);
        self.states.push(s0);
        self.par.push(None);
        let mut cache: HashMap<(usize, Recorded), (Taken, Vec<String>)> = HashMap::new();
        let mut take_ids: HashMap<BTreeSet<usize>, usize> = HashMap::new();
        let mut next = 0;
        while next < self.states.len() {
            let i = next;
            next += 1;
            self.trans.push(Vec::new());
            self.steps.push(Vec::new());
            let s = self.states[i].clone();
            if s.at == g.end {
                continue;
            }
            let c = &g.cells[s.at];
            let names = &self.inp_names[s.at];
            let fixed: Recorded = s.recorded.iter().filter(|(k, _)| names.contains(k)).cloned().collect();
            let key = (s.at, fixed);
            let (taken, unset) = match cache.get(&key) {
                Some(x) => x.clone(),
                None => {
                    let x = self.rows_at(s.at, &s.recorded);
                    cache.insert(key, x.clone());
                    x
                }
            };
            for k in unset {
                let ex = format!("{} -> {}", self.way(i), c.id);
                self.findings.first("not recorded", &c.id, &k, ex);
            }
            for (ri, r) in c.rows.iter().enumerate() {
                let Some(outs) = taken.get(&ri) else { continue };
                let way = || format!("{} -> {}", self.way(i), c.id);
                match g.step(&s, r) {
                    Step::Again => {
                        let ex = way();
                        self.findings.first("asked again", &c.id, &r.name, ex);
                    }
                    Step::Left => {
                        let who = s.callers.last().map(|x| g.cells[*x].id.as_str()).unwrap_or_default();
                        let ex = format!("{who} still waits for an answer, after {}", way());
                        self.findings.first("not back", &c.id, &r.name, ex);
                    }
                    Step::Nowhere => {
                        let ex = way();
                        self.findings.first("nowhere", &c.id, &r.name, ex);
                    }
                    Step::To(s2) => {
                        let j = match index.get(&s2) {
                            Some(j) => *j,
                            None => {
                                let j = self.states.len();
                                index.insert(s2.clone(), j);
                                self.states.push(s2);
                                self.par.push(Some((i, ri)));
                                j
                            }
                        };
                        let t = match take_ids.get(outs) {
                            Some(t) => *t,
                            None => {
                                take_ids.insert(outs.clone(), self.takes.len());
                                self.takes.push(outs.clone());
                                self.takes.len() - 1
                            }
                        };
                        self.trans[i].push((ri, j));
                        self.steps[i].push((ri, j, t));
                    }
                }
            }
        }
    }

    /// Every cell is reached, every row taken somewhere, and every recorded value read.
    pub fn reached(&mut self) -> Result<(), String> {
        let g = self.g;
        let reached: BTreeSet<usize> = self.states.iter().map(|s| s.at).collect();
        let mut taken = BTreeSet::new();
        for (i, ts) in self.trans.iter().enumerate() {
            for (r, _) in ts {
                taken.insert((self.states[i].at, *r));
            }
        }
        for (ci, c) in g.cells.iter().enumerate() {
            if !reached.contains(&ci) {
                self.findings.first("unreached", &c.id, "", String::new());
                continue;
            }
            for (ri, r) in c.rows.iter().enumerate() {
                if !taken.contains(&(ci, ri)) && !self.findings.has("nowhere", &c.id, &r.name) {
                    self.findings.first("never taken", &c.id, &r.name, String::new());
                }
            }
        }
        let mut read = BTreeSet::new();
        for names in &self.inp_names {
            read.extend(names.iter().cloned());
        }
        let unread: Vec<String> = self.set_names.difference(&read).cloned().collect();
        for k in unread {
            self.findings.first("never read", "", &k, String::new());
        }
        for c in &g.cells {
            for r in &c.rows {
                for (k, v) in &r.sets {
                    for m in &g.cells {
                        let ok = m.inenums.get(k).is_none_or(|vs| vs.iter().any(|x| &x.0 == v));
                        ensure!(
                            ok,
                            "a row records a value the reader does not have: {} {} {k} {v}",
                            c.id,
                            r.name
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn free(&self, s: usize, r: usize) -> bool {
        let row = &self.g.cells[self.states[s].at].rows[r];
        row.counts.is_none() && !row.waits
    }

    /// Every loop passes a row that counts or waits: without those rows the states form no cycle. Each strongly
    /// connected part that is left is one finding, named by its rows, with one cycle through it.
    pub fn counters(&mut self) {
        let n = self.states.len();
        let succ: Vec<Vec<usize>> = (0..n)
            .map(|s| {
                self.trans[s]
                    .iter()
                    .filter(|(r, _)| self.free(s, *r))
                    .map(|x| x.1)
                    .collect()
            })
            .collect();
        let mut index = vec![usize::MAX; n];
        let mut low = vec![0; n];
        let mut on = vec![false; n];
        let mut stack = Vec::new();
        let mut parts = Vec::new();
        let mut count = 0;
        for root in 0..n {
            if index[root] != usize::MAX {
                continue;
            }
            index[root] = count;
            low[root] = count;
            count += 1;
            stack.push(root);
            on[root] = true;
            let mut work = vec![(root, 0)];
            while let Some(&(v, i)) = work.last() {
                if i < succ[v].len() {
                    if let Some(top) = work.last_mut() {
                        top.1 = i + 1;
                    }
                    let w = succ[v][i];
                    if index[w] == usize::MAX {
                        index[w] = count;
                        low[w] = count;
                        count += 1;
                        stack.push(w);
                        on[w] = true;
                        work.push((w, 0));
                    } else if on[w] {
                        low[v] = low[v].min(index[w]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(u, _)) = work.last() {
                    low[u] = low[u].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut part = Vec::new();
                    while let Some(w) = stack.pop() {
                        on[w] = false;
                        part.push(w);
                        if w == v {
                            break;
                        }
                    }
                    if part.len() > 1 || succ[v].contains(&v) {
                        parts.push(part);
                    }
                }
            }
        }
        let g = self.g;
        for part in parts {
            let inside: BTreeSet<usize> = part.iter().copied().collect();
            let mut rows: BTreeSet<(usize, usize)> = BTreeSet::new();
            for &s in &part {
                for &(r, s2) in &self.trans[s] {
                    if inside.contains(&s2) && self.free(s, r) {
                        rows.insert((self.states[s].at, r));
                    }
                }
            }
            let start = part[0];
            let mut par: HashMap<usize, (usize, usize)> = HashMap::new();
            let mut q = VecDeque::from([start]);
            let mut back = None;
            'search: while let Some(s) = q.pop_front() {
                for &(r, s2) in &self.trans[s] {
                    if inside.contains(&s2) && self.free(s, r) {
                        if s2 == start {
                            back = Some((s, r));
                            break 'search;
                        }
                        if let Entry::Vacant(e) = par.entry(s2) {
                            e.insert((s, r));
                            q.push_back(s2);
                        }
                    }
                }
            }
            let mut path = Vec::new();
            let mut at = back;
            while let Some((s, r)) = at {
                let c = &g.cells[self.states[s].at];
                path.push(format!("{} \"{}\"", c.id, c.rows[r].name));
                at = par.get(&s).copied();
            }
            path.reverse();
            let first = rows.iter().next().map(|x| g.cells[x.0].id.clone()).unwrap_or_default();
            let names: Vec<String> = rows
                .iter()
                .map(|(c, r)| format!("{}:{}", g.cells[*c].id, g.cells[*c].rows[*r].name))
                .collect();
            self.findings
                .first("no counter", &first, &names.join(" "), path.join(" -> "));
        }
    }

    /// Where each pointer turned out to point; a row into another block goes to one of its entries; and a source
    /// that names a pointer reaches a cell that has the place.
    pub fn pointed(&mut self) -> Result<(), String> {
        let g = self.g;
        for s in &self.states {
            for p in POINTERS {
                if let Some(x) = s.ptr(p) {
                    self.points.entry((s.at, p)).or_default().insert(x);
                }
            }
        }
        // A row back to a cell the engine keeps track of is no way in, and a row that takes an interjection to the
        // cell that receives it is the one other way in.
        for (ci, c) in g.cells.iter().enumerate() {
            for r in &c.rows {
                let Target::Cell(t) = r.target else { continue };
                let (bt, bc) = (g.block[t], g.block[ci]);
                if bt != bc && !g.entries[bt].contains(&t) && !r.interject {
                    let ex = format!(
                        "goes to {} in {}, not to an entry of it",
                        g.cells[t].id, g.block_names[bt]
                    );
                    self.findings.first("side entry", &c.id, &r.name, ex);
                }
            }
        }
        for (ci, c) in g.cells.iter().enumerate() {
            for x in &c.inputs {
                for src in &x.from {
                    let Src::Place(n, p) = src else { continue };
                    let Some(ptr) = POINTERS.iter().copied().find(|q| *q == n.as_str()) else {
                        continue;
                    };
                    if let Some(ns) = self.points.get(&(ci, ptr)) {
                        let ok = ns.iter().any(|m| g.out_key(*m, p).is_some());
                        ensure!(ok, "no cell this source points at has the place: {} {n} {p}", c.id);
                    }
                }
            }
        }
        Ok(())
    }
}
