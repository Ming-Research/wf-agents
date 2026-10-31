//! Every return a cell's output allows, as far as its rows can tell returns apart, and which row each takes.
//!
//! A return is told apart by its values, the places it holds and whether it is the cell's own answer. The values of
//! a place split into classes, each holding the values every condition on the place treats alike; a list holds any
//! non-empty set of classes, or nothing. Lists, branches and enums no row switches on stand as one return holding
//! everything they could hold. Whether a place is there is kept under `?<key>`, whether the return is the own
//! answer under `!own`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::bits::Bits;
use crate::check::Analysis;
use crate::field::{Field, inner, keys, places, resolve};
use crate::format::{Atom, Kind};
use crate::graph::{CellKind, ensure, flat};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Val {
    One(String),
    Many(BTreeSet<String>),
    Flag(bool),
}

pub(crate) type Env = BTreeMap<String, Val>;

/// What a return leaves for later cells: the places of the output it holds, whether it is the cell's own answer,
/// the places of inputs read by id it switches on, and the fields that come every time that it leaves as they were.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Outcome {
    pub pres: Bits,
    pub answered: bool,
    pub sees: BTreeSet<String>,
    pub keeps: Vec<String>,
}

pub(crate) struct Ret {
    pub env: Env,
    pub outcome: usize,
}

#[derive(Default)]
pub(crate) struct Rets {
    pub rets: Vec<Ret>,
    pub outcomes: Vec<Outcome>,
}

/// A row's condition with its paths as keys.
#[derive(Debug, Clone)]
pub(crate) enum Cond {
    All,
    Own,
    Or(Vec<Cond>),
    Has(String),
    Is(String, BTreeSet<String>),
    AnyOf(String, BTreeSet<String>),
    NoneOf(String, BTreeSet<String>),
    Inp(String, BTreeSet<String>),
}

pub(crate) fn holds(c: &Cond, env: &Env, inp: &BTreeMap<String, String>) -> bool {
    let hit = |k: &String, vs: &BTreeSet<String>| match env.get(k) {
        Some(Val::Many(s)) => !s.is_disjoint(vs),
        Some(Val::One(x)) => vs.contains(x),
        _ => false,
    };
    match c {
        Cond::All => true,
        Cond::Own => matches!(env.get("!own"), Some(Val::Flag(true))),
        Cond::Or(xs) => xs.iter().any(|x| holds(x, env, inp)),
        Cond::Inp(n, vs) => inp.get(n).is_some_and(|v| vs.contains(v)),
        Cond::Has(k) => matches!(env.get(k), Some(Val::Flag(true))),
        Cond::Is(k, vs) | Cond::AnyOf(k, vs) => hit(k, vs),
        Cond::NoneOf(k, vs) => !hit(k, vs),
    }
}

type Part = Vec<(Env, Bits)>;

/// Making the returns of one cell: the keys of what it switches on, numbered, and what its rows switch on.
struct Rx<'a> {
    cell: &'a str,
    index: &'a HashMap<String, usize>,
    n: usize,
    has: &'a BTreeSet<String>,
    cls: &'a BTreeMap<String, Vec<BTreeSet<String>>>,
}

fn joined(e1: &Env, e2: &Env) -> Env {
    let mut e = e1.clone();
    e.extend(e2.iter().map(|(k, v)| (k.clone(), v.clone())));
    e
}

/// Every non-empty union of the classes.
fn unions(cls: &[BTreeSet<String>]) -> Vec<BTreeSet<String>> {
    let n = cls.len();
    let mut out = Vec::new();
    for size in 1..=n {
        for mask in 1u64..(1u64 << n) {
            if mask.count_ones() as usize != size {
                continue;
            }
            let picked = cls.iter().enumerate().filter(|(i, _)| mask & (1u64 << i) != 0);
            out.push(picked.flat_map(|(_, c)| c.iter().cloned()).collect());
        }
    }
    out
}

impl Rx<'_> {
    fn bit(&self, k: &str) -> Bits {
        let mut b = Bits::new(self.n);
        b.set(self.index[k]);
        b
    }

    fn every(&self, fs: &[Field], prefix: &str) -> Bits {
        let mut b = Bits::new(self.n);
        for k in keys(fs, prefix) {
            b.set(self.index[&k]);
        }
        b
    }

    fn fields(&self, fs: &[Field], prefix: &str) -> Result<Part, String> {
        let mut out: Part = vec![(Env::new(), Bits::new(self.n))];
        for f in fs {
            let p = self.field(f, prefix)?;
            let mut next = Vec::with_capacity(out.len() * p.len());
            for (e1, s1) in &out {
                for (e2, s2) in &p {
                    let mut s = s1.clone();
                    s.or(s2);
                    next.push((joined(e1, e2), s));
                }
            }
            out = next;
        }
        Ok(out)
    }

    fn field(&self, f: &Field, prefix: &str) -> Result<Part, String> {
        let k = format!("{prefix}{}", f.name);
        let mut out: Part = match f.kind {
            Kind::List => self.listed(f, &k, prefix)?,
            Kind::Union => {
                let dk = format!("{k}.{}", f.by);
                let common = self.fields(&f.common, &format!("{k}."))?;
                let mut out = Vec::new();
                for b in &f.branches {
                    let bk = format!("{k}[{}]", b.value);
                    let mut here = self.bit(&k);
                    here.set(self.index[&dk]);
                    here.set(self.index[&bk]);
                    for (e, s) in self.fields(&b.fields, &format!("{bk}."))? {
                        for (ec, sc) in &common {
                            let mut env = joined(&e, ec);
                            if self.cls.contains_key(&dk) {
                                env.insert(dk.clone(), Val::One(b.value.clone()));
                            }
                            let mut s = s.clone();
                            s.or(sc);
                            s.or(&here);
                            out.push((env, s));
                        }
                    }
                }
                out
            }
            Kind::Obj => {
                let mut out = self.fields(&f.fields, &format!("{k}."))?;
                for (_, s) in &mut out {
                    s.set(self.index[&k]);
                }
                out
            }
            Kind::Enum if self.cls.contains_key(&k) => self.cls[&k]
                .iter()
                .map(|c| {
                    let first = c.iter().next().cloned().unwrap_or_default();
                    (Env::from([(k.clone(), Val::One(first))]), self.bit(&k))
                })
                .collect(),
            _ => vec![(Env::new(), self.bit(&k))],
        };
        if self.has.contains(&k) && f.kind != Kind::List {
            for (e, _) in &mut out {
                e.insert(format!("?{k}"), Val::Flag(true));
            }
        }
        if f.optional {
            out.push((Env::new(), Bits::new(self.n)));
        }
        Ok(out)
    }

    fn listed(&self, f: &Field, k: &str, prefix: &str) -> Result<Part, String> {
        let g = inner(f);
        let (dot, br) = (format!("{k}."), format!("{k}["));
        let inside: BTreeSet<&String> = self
            .has
            .iter()
            .chain(self.cls.keys())
            .filter(|x| x.starts_with(&dot) || x.starts_with(&br))
            .collect();
        for x in &inside {
            let rest = x.strip_prefix(&dot).unwrap_or("");
            let word = rest.strip_prefix('@').unwrap_or(rest);
            let ok = !word.is_empty() && word.chars().all(|c| c.is_alphanumeric() || c == '_');
            ensure!(
                ok && self.cls.contains_key(*x),
                "a row switches on a place in a list this check cannot tell apart: {} {x}",
                self.cell
            );
        }
        let q = format!("?{k}");
        let has = self.has.contains(k);
        let here = |v: bool| {
            if has {
                Env::from([(q.clone(), Val::Flag(v))])
            } else {
                Env::new()
            }
        };
        let empty_too = f.min < 1;
        let all = self.every(std::slice::from_ref(f), prefix);
        let mut out = Vec::new();
        if inside.is_empty() {
            if empty_too {
                out.push((here(false), self.bit(k)));
            }
            out.push((here(true), all));
            return Ok(out);
        }
        if empty_too {
            let mut env: Env = inside
                .iter()
                .map(|x| ((*x).clone(), Val::Many(BTreeSet::new())))
                .collect();
            env.extend(here(false));
            out.push((env, self.bit(k)));
        }
        let choices: Vec<Vec<BTreeSet<String>>> = inside.iter().map(|x| unions(&self.cls[*x])).collect();
        let mut combos: Vec<Vec<&BTreeSet<String>>> = vec![Vec::new()];
        for ch in &choices {
            combos = combos
                .iter()
                .flat_map(|o| ch.iter().map(move |c| [o.as_slice(), &[c]].concat()))
                .collect();
        }
        let dk = (g.kind == Kind::Union).then(|| format!("{k}.{}", g.by));
        let mut with_marker = self.bit(k);
        let mut branch_bits = Vec::new();
        if let Some(dk) = &dk {
            with_marker.set(self.index[dk]);
            with_marker.or(&self.every(&g.common, &dot));
            for b in &g.branches {
                let bk = format!("{k}[{}]", b.value);
                let mut bb = self.every(&b.fields, &format!("{bk}."));
                bb.set(self.index[&bk]);
                branch_bits.push((b.value.clone(), bb));
            }
        }
        for combo in combos {
            let mut env: Env = inside
                .iter()
                .zip(combo)
                .map(|(x, c)| ((*x).clone(), Val::Many(c.clone())))
                .collect();
            env.extend(here(true));
            let pres = match dk.as_ref().and_then(|dk| env.get(dk)) {
                Some(Val::Many(sel)) => {
                    let mut p = with_marker.clone();
                    for (v, bb) in &branch_bits {
                        if sel.contains(v) {
                            p.or(bb);
                        }
                    }
                    p
                }
                _ => all.clone(),
            };
            out.push((env, pres));
        }
        Ok(out)
    }
}

impl Analysis<'_, '_> {
    /// The rows' conditions of a cell, with keys for paths.
    pub fn conds_of(&self, ci: usize) -> Vec<Vec<Cond>> {
        fn conv(a: &Atom, an: &Analysis, ci: usize) -> Cond {
            let k = |p: &str| an.g.akey(ci, p).to_string();
            match a {
                Atom::All => Cond::All,
                Atom::Own => Cond::Own,
                Atom::Or(xs) => Cond::Or(xs.iter().map(|x| conv(x, an, ci)).collect()),
                Atom::Has(p) => Cond::Has(format!("?{}", k(p))),
                Atom::Is(p, vs) => Cond::Is(k(p), vs.clone()),
                Atom::AnyOf(p, vs) => Cond::AnyOf(k(p), vs.clone()),
                Atom::NoneOf(p, vs) => Cond::NoneOf(k(p), vs.clone()),
                Atom::Inp(n, vs) => Cond::Inp(n.clone(), vs.clone()),
                // No return meets it; the flow's own rows never have it.
                Atom::SentBack(_) | Atom::Failed(_) | Atom::Engine(_) => Cond::Or(Vec::new()),
            }
        }
        self.g.cells[ci]
            .rows
            .iter()
            .map(|r| r.atoms.iter().map(|a| conv(a, self, ci)).collect())
            .collect()
    }

    /// The values the engine knows or rows recorded that a cell's rows read, checked against what the cell names.
    pub fn inp_names_of(&self, ci: usize) -> Result<Vec<String>, String> {
        let c = &self.g.cells[ci];
        let mut atoms = Vec::new();
        for r in &c.rows {
            flat(&r.atoms, &mut atoms);
        }
        let mut names = BTreeSet::new();
        for a in &atoms {
            if let Atom::Inp(n, vs) = a {
                let Some(known) = c.inenums.get(n) else {
                    return Err(format!(
                        "a row reads a value the engine knows that the cell does not name: {} {n}",
                        c.id
                    ));
                };
                let ok = vs.iter().all(|v| known.iter().any(|x| &x.0 == v));
                ensure!(ok, "no such value: {} {n} {vs:?}", c.id);
                names.insert(n.clone());
            }
        }
        Ok(names.into_iter().collect())
    }

    /// Every assignment of the values the cell's rows read, with those recorded fixed.
    pub fn assignments(&self, ci: usize, fixed: &BTreeMap<String, String>) -> Vec<BTreeMap<String, String>> {
        let c = &self.g.cells[ci];
        let mut out = vec![BTreeMap::new()];
        for n in &self.inp_names[ci] {
            let vals: Vec<String> = match fixed.get(n) {
                Some(v) => vec![v.clone()],
                None => c.inenums[n].iter().map(|x| x.0.clone()).collect(),
            };
            out = out
                .into_iter()
                .flat_map(|a| {
                    vals.iter().map(move |v| {
                        let mut a = a.clone();
                        a.insert(n.clone(), v.clone());
                        a
                    })
                })
                .collect();
        }
        out
    }

    pub fn returns(&self, ci: usize) -> Result<Rets, String> {
        let g = self.g;
        let c = &g.cells[ci];
        let mut sw_places = Vec::new();
        places(&c.sw, "", &mut sw_places);
        let index: HashMap<String, usize> = sw_places.iter().enumerate().map(|(i, p)| (p.key.clone(), i)).collect();
        let km = self.places[ci].len();
        let mut atoms = Vec::new();
        for r in &c.rows {
            flat(&r.atoms, &mut atoms);
        }
        let mut has = BTreeSet::new();
        let mut sets: BTreeMap<String, (Vec<String>, Vec<BTreeSet<String>>)> = BTreeMap::new();
        let mut is_keys = BTreeSet::new();
        for a in &atoms {
            match a {
                Atom::Has(p) => {
                    has.insert(g.akey(ci, p).to_string());
                }
                Atom::Is(p, vs) | Atom::AnyOf(p, vs) | Atom::NoneOf(p, vs) => {
                    let (k, f) = resolve(&c.sw, p)?;
                    let values = f
                        .values()
                        .ok_or_else(|| format!("not a place to switch on: {} {p}", c.id))?;
                    let extra: Vec<&String> = vs.iter().filter(|v| !values.contains(v)).collect();
                    ensure!(
                        extra.is_empty(),
                        "values the place does not have: {} {p} {extra:?}",
                        c.id
                    );
                    if matches!(a, Atom::Is(..)) {
                        is_keys.insert(k.clone());
                    }
                    sets.entry(k).or_insert_with(|| (values, Vec::new())).1.push(vs.clone());
                }
                _ => {}
            }
        }
        let mut cls: BTreeMap<String, Vec<BTreeSet<String>>> = BTreeMap::new();
        for (k, (values, ss)) in &sets {
            let mut sig: Vec<(Vec<bool>, BTreeSet<String>)> = Vec::new();
            for v in values {
                let s: Vec<bool> = ss.iter().map(|x| x.contains(v)).collect();
                match sig.iter_mut().find(|x| x.0 == s) {
                    Some(x) => {
                        x.1.insert(v.clone());
                    }
                    None => sig.push((s, BTreeSet::from([v.clone()]))),
                }
            }
            cls.insert(k.clone(), sig.into_iter().map(|x| x.1).collect());
        }
        let rx = Rx {
            cell: &c.id,
            index: &index,
            n: sw_places.len(),
            has: &has,
            cls: &cls,
        };
        let mut rs: Vec<(Env, Bits, bool, Vec<String>)> = Vec::new();
        if c.kind != CellKind::Agent {
            rs.extend(rx.fields(&c.sw, "")?.into_iter().map(|(e, s)| (e, s, true, Vec::new())));
        } else {
            let com: HashMap<&str, &Field> = c
                .out
                .iter()
                .filter(|f| g.common_names.contains(&f.name))
                .map(|f| (f.name.as_str(), f))
                .collect();
            let field_of = |n: &str| {
                com.get(n)
                    .copied()
                    .ok_or_else(|| format!("no common field {n}: {}", c.id))
            };
            let pseudo = c.sw.iter().filter(|f| f.name.starts_with('@')).cloned();
            for form in &c.forms {
                let mut fs = Vec::new();
                // A field every return holds that this cell's line does not hold is not in its output.
                let held = |n: &&String| c.out.iter().any(|f| &f.name == *n);
                for n in g.every.iter().filter(held).filter(|n| !form.without.contains(n)) {
                    fs.push(field_of(n)?.clone());
                }
                fs.extend(pseudo.clone());
                let always = rx.fields(&fs, "")?;
                let part: Vec<(Env, Bits)> = if form.own {
                    rx.fields(&c.main, "")?
                } else {
                    // Alone, the form's field is there.
                    let f = field_of(form.field.as_deref().unwrap_or_default())?;
                    rx.field(f, "")?.into_iter().filter(|(_, s)| s.any()).collect()
                };
                for (e1, s1) in &part {
                    for (e2, s2) in &always {
                        let mut e = joined(e1, e2);
                        e.insert("!own".into(), Val::Flag(form.own));
                        let mut s = s1.clone();
                        s.or(s2);
                        rs.push((e, s, form.own, form.without.clone()));
                    }
                }
            }
        }
        let switched: BTreeSet<&String> = rs.iter().flat_map(|r| r.0.keys()).collect();
        let want: Vec<String> = has
            .iter()
            .map(|k| format!("?{k}"))
            .chain(cls.keys().cloned())
            .filter(|k| !switched.contains(k))
            .collect();
        ensure!(
            want.is_empty(),
            "a row switches on what no return carries: {} {want:?}",
            c.id
        );
        for k in &is_keys {
            let list = rs.iter().any(|r| matches!(r.0.get(k), Some(Val::Many(_))));
            ensure!(!list, "a row reads a list with is: {} {k}", c.id);
        }
        // The places of the inputs read by id that a return switches on are what the cell sees, not what it writes.
        let mut out = Rets::default();
        let mut ids: HashMap<Outcome, usize> = HashMap::new();
        let mut seen: HashSet<(Env, usize)> = HashSet::new();
        for (env, s, answered, keeps) in rs {
            let mut pres = Bits::new(km);
            let mut sees = BTreeSet::new();
            for i in s.ones() {
                if i < km {
                    pres.set(i);
                } else {
                    sees.insert(sw_places[i].key.clone());
                }
            }
            let o = Outcome {
                pres,
                answered,
                sees,
                keeps,
            };
            let id = match ids.get(&o) {
                Some(i) => *i,
                None => {
                    ids.insert(o.clone(), out.outcomes.len());
                    out.outcomes.push(o);
                    out.outcomes.len() - 1
                }
            };
            if seen.insert((env.clone(), id)) {
                out.rets.push(Ret { env, outcome: id });
            }
        }
        Ok(out)
    }

    /// How a return is shown in a finding.
    pub fn show(env: &Env, answered: bool, inp: &BTreeMap<String, String>) -> String {
        let mut parts = Vec::new();
        if !answered {
            parts.push("no answer".to_string());
        }
        for (k, v) in env {
            if k == "!own" {
                continue;
            }
            parts.push(match (k.strip_prefix('?'), v) {
                (Some(x), Val::Flag(true)) => x.to_string(),
                (Some(x), _) => format!("no {x}"),
                (None, Val::Many(s)) if s.is_empty() => format!("{k} empty"),
                (None, Val::Many(s)) => format!("{k} has {}", s.iter().cloned().collect::<Vec<_>>().join("/")),
                (None, Val::One(x)) => format!("{k}={x}"),
                (None, Val::Flag(x)) => format!("{k}={x}"),
            });
        }
        parts.extend(inp.iter().map(|(k, v)| format!("{k}={v} (engine)")));
        parts.join(", ")
    }

    /// Every return and every assignment of what the engine knows takes exactly one row.
    pub fn tables(&mut self) {
        for ci in 0..self.g.cells.len() {
            let c = &self.g.cells[ci];
            if c.rows.is_empty() {
                continue;
            }
            for r in &self.rets[ci].rets {
                for inp in self.assignments(ci, &BTreeMap::new()) {
                    let hits: Vec<&str> = self.conds[ci]
                        .iter()
                        .zip(&c.rows)
                        .filter(|(cs, _)| cs.iter().all(|x| holds(x, &r.env, &inp)))
                        .map(|(_, row)| row.name.as_str())
                        .collect();
                    if hits.len() != 1 {
                        let kind = if hits.is_empty() { "no row" } else { "overlap" };
                        let answered = self.rets[ci].outcomes[r.outcome].answered;
                        let ex = Self::show(&r.env, answered, &inp);
                        self.findings.shorter(kind, &c.id, &hits.join(" + "), ex);
                    }
                }
            }
        }
    }
}
