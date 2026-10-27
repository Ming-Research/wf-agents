//! The checks over a whole graph, in order: every return takes one row; every state the engine can be in is walked;
//! every loop counts; every value returned is read; and every input is there. A check that fails names the place and
//! an example; all failures are reported together.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::field::{Place, places};
use crate::graph::Graph;
use crate::returns::{Cond, Rets};
use crate::walk::State;

/// The kinds of finding, in the order they are reported.
const KINDS: [&str; 14] = [
    "overlap",
    "no row",
    "unreached",
    "never taken",
    "nowhere",
    "asked again",
    "not recorded",
    "never read",
    "side entry",
    "dropped",
    "not back",
    "missing",
    "marked maybe",
    "no counter",
];

#[derive(Default)]
pub(crate) struct Findings {
    order: Vec<(usize, String, String)>,
    example: HashMap<(usize, String, String), String>,
    dropped: Vec<Dropped>,
}

/// A cell, a row it takes, and the places of its return nobody reads after it, each with a way there.
type Dropped = (String, String, Vec<(String, String)>);

fn kind_at(kind: &str) -> usize {
    KINDS.iter().position(|k| *k == kind).unwrap_or(KINDS.len())
}

impl Findings {
    /// A finding, unless one is already there for the same place.
    pub fn first(&mut self, kind: &str, cell: &str, at: &str, ex: String) {
        let k = (kind_at(kind), cell.to_string(), at.to_string());
        if !self.example.contains_key(&k) {
            self.order.push(k.clone());
            self.example.insert(k, ex);
        }
    }

    /// A finding, keeping the shortest example for the same place.
    pub fn shorter(&mut self, kind: &str, cell: &str, at: &str, ex: String) {
        let k = (kind_at(kind), cell.to_string(), at.to_string());
        match self.example.get_mut(&k) {
            Some(old) if old.len() <= ex.len() => {}
            Some(old) => *old = ex,
            None => {
                self.order.push(k.clone());
                self.example.insert(k, ex);
            }
        }
    }

    pub fn has(&self, kind: &str, cell: &str, at: &str) -> bool {
        self.example
            .contains_key(&(kind_at(kind), cell.to_string(), at.to_string()))
    }

    /// A place of a cell's return that nobody reads after the row it takes.
    pub fn drop_place(&mut self, cell: &str, row: &str, place: &str, way: String) {
        let i = match self.dropped.iter().position(|(c, r, _)| c == cell && r == row) {
            Some(i) => i,
            None => {
                self.dropped.push((cell.to_string(), row.to_string(), Vec::new()));
                self.dropped.len() - 1
            }
        };
        let places = &mut self.dropped[i].2;
        if !places.iter().any(|(p, _)| p == place) {
            places.push((place.to_string(), way));
        }
    }

    pub fn lines(&self) -> Vec<String> {
        let mut all: Vec<(usize, usize, String)> = self
            .order
            .iter()
            .enumerate()
            .map(|(i, k)| (k.0, i, format!("{} {} {}: {}", KINDS[k.0], k.1, k.2, self.example[k])))
            .collect();
        let drop = kind_at("dropped");
        for (i, (c, r, ps)) in self.dropped.iter().enumerate() {
            let names: Vec<&str> = ps.iter().map(|(p, _)| p.as_str()).collect();
            let way = ps.first().map(|x| x.1.as_str()).unwrap_or_default();
            all.push((
                drop,
                i,
                format!("dropped {c} {r}: nobody reads {}, e.g. after {way}", names.join(", ")),
            ));
        }
        all.sort_by_key(|a| (a.0, a.1));
        all.into_iter().map(|x| x.2).collect()
    }
}

/// What the checks work out about one graph, kept for the compiled flow.
pub(crate) struct Analysis<'g, 's> {
    pub g: &'g Graph<'s>,
    /// The places of each cell's output.
    pub places: Vec<Vec<Place>>,
    pub place_at: Vec<HashMap<String, usize>>,
    pub conds: Vec<Vec<Vec<Cond>>>,
    pub inp_names: Vec<Vec<String>>,
    pub rets: Vec<Rets>,
    /// Every value a row records.
    pub set_names: BTreeSet<String>,
    pub findings: Findings,
    /// The states in the order the walk found them; the first is the start.
    pub states: Vec<State>,
    /// The state and row each state was first reached from.
    pub par: Vec<Option<(usize, usize)>>,
    /// The rows out of each state, and the states they lead to.
    pub trans: Vec<Vec<(usize, usize)>>,
    /// The same, with the set of outcomes that take the row there.
    pub steps: Vec<Vec<(usize, usize, usize)>>,
    pub takes: Vec<BTreeSet<usize>>,
    /// The cells a pointer turned out to point at, by the cell it is read at.
    pub points: BTreeMap<(usize, &'static str), BTreeSet<usize>>,
    /// What the cell at each state reads of earlier returns, as (cell, place).
    pub reads: Vec<BTreeSet<(usize, usize)>>,
}

impl<'g, 's> Analysis<'g, 's> {
    pub fn run(g: &'g Graph<'s>) -> Result<Analysis<'g, 's>, String> {
        let mut a = Analysis {
            g,
            places: Vec::new(),
            place_at: Vec::new(),
            conds: Vec::new(),
            inp_names: Vec::new(),
            rets: Vec::new(),
            set_names: g
                .cells
                .iter()
                .flat_map(|c| c.rows.iter().flat_map(|r| r.sets.keys().cloned()))
                .collect(),
            findings: Findings::default(),
            states: Vec::new(),
            par: Vec::new(),
            trans: Vec::new(),
            steps: Vec::new(),
            takes: Vec::new(),
            points: BTreeMap::new(),
            reads: Vec::new(),
        };
        for c in &g.cells {
            let mut ps = Vec::new();
            places(&c.out, "", &mut ps);
            a.place_at
                .push(ps.iter().enumerate().map(|(i, p)| (p.key.clone(), i)).collect());
            a.places.push(ps);
        }
        for ci in 0..g.cells.len() {
            let names = a.inp_names_of(ci)?;
            a.inp_names.push(names);
            let conds = a.conds_of(ci);
            a.conds.push(conds);
        }
        for ci in 0..g.cells.len() {
            let r = if g.cells[ci].rows.is_empty() {
                Rets::default()
            } else {
                a.returns(ci)?
            };
            a.rets.push(r);
        }
        a.tables();
        a.walk();
        a.reached()?;
        a.counters();
        a.pointed()?;
        a.reads = (0..a.states.len()).map(|s| a.reads_at(s)).collect();
        a.dropped();
        a.missing();
        Ok(a)
    }

    /// The way from the start to a state, by the cells and rows on it.
    pub fn way(&self, s: usize) -> String {
        let mut out = Vec::new();
        let mut at = s;
        while let Some((p, r)) = self.par[at] {
            let c = &self.g.cells[self.states[p].at];
            out.push(format!("{} \"{}\"", c.id, c.rows[r].name));
            at = p;
        }
        out.reverse();
        if out.is_empty() {
            "the start".into()
        } else {
            out.join(" -> ")
        }
    }
}
