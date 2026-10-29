//! Fields as the compiler works with them, every shape and named set of branches or values resolved; and the places
//! of an output, each named by a key.
//!
//! A key names one place: `stands`, `stands.verdict` (the marker that tells a union's branches apart),
//! `stands[<value>]` (one branch), `stands[<value>].not_standing`. A list is transparent: its item's fields follow
//! the list's name. In a cell whose rows read an input by its id, a path `@<id>...` starts in that input.

use std::collections::BTreeMap;

use crate::format::{Branches, Kind, RawField, Role, Tool, Values};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    pub name: String,
    /// What the owner's page heads the field with; no agent is sent it.
    pub title: String,
    pub kind: Kind,
    pub desc: String,
    pub optional: bool,
    /// An enum's values with what each means.
    pub values: Vec<(String, String)>,
    pub fields: Vec<Field>,
    /// An object with no fields listed, which holds whatever fields its value has.
    pub open: bool,
    pub item: Option<Box<Field>>,
    pub branches: Vec<Branch>,
    /// The fields every branch of a union has.
    pub common: Vec<Field>,
    /// The field that tells a union's branches apart.
    pub by: String,
    pub min: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Branch {
    pub value: String,
    pub desc: String,
    pub fields: Vec<Field>,
}

/// What resolving fields needs from the flow.
pub(crate) struct Shapes<'s> {
    pub shapes: &'s BTreeMap<String, RawField>,
    /// The flow's tools, in the order the source lists them.
    pub tools: Vec<&'s Tool>,
    pub ask_kinds: Vec<(String, String)>,
    pub interject_from: Vec<(String, String)>,
}

impl Shapes<'_> {
    /// The field with its shape's keys under its own.
    pub fn merged(&self, x: &RawField) -> Result<RawField, String> {
        let mut x = x.clone();
        for _ in 0..32 {
            let Some(name) = x.shape.take() else { return Ok(x) };
            let b = self
                .shapes
                .get(&name)
                .ok_or_else(|| format!("no such shape: {name}"))?
                .clone();
            x = RawField {
                shape: b.shape,
                name: x.name.or(b.name),
                title: x.title.or(b.title),
                kind: x.kind.or(b.kind),
                desc: x.desc.or(b.desc),
                optional: x.optional.or(b.optional),
                values: x.values.or(b.values),
                fields: x.fields.or(b.fields),
                item: x.item.or(b.item),
                branches: x.branches.or(b.branches),
                common: x.common.or(b.common),
                by: x.by.or(b.by),
                min: x.min.or(b.min),
            };
        }
        Err("shapes that are each other".into())
    }

    pub fn enum_values(&self, v: &Values) -> Result<Vec<(String, String)>, String> {
        match v {
            Values::List(xs) => Ok(xs.clone()),
            Values::Named(n) if n == "askKinds" => Ok(self.ask_kinds.clone()),
            Values::Named(n) if n == "interjectFrom" => Ok(self.interject_from.clone()),
            Values::Named(n) => match self.shapes.get(n) {
                Some(RawField {
                    kind: Some(Kind::Enum),
                    values: Some(v),
                    ..
                }) => self.enum_values(v),
                _ => Err(format!("no enum shape of that name: {n}")),
            },
        }
    }

    pub fn tool(&self, name: &str) -> Result<&Tool, String> {
        self.tools
            .iter()
            .copied()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("no such tool: {name}"))
    }

    /// `tools` and `roles`: the tools the cell the field belongs to may ask for and the roles it may ask, for a
    /// union with a branch for each.
    pub fn field(&self, raw: &RawField, tools: &[String], roles: &[&Role]) -> Result<Field, String> {
        let x = self.merged(raw)?;
        let name = x.name.clone().unwrap_or_default();
        let kind = x.kind.ok_or_else(|| format!("a field with no type: {name}"))?;
        let all = |fs: &[RawField]| {
            fs.iter()
                .map(|y| self.field(y, tools, roles))
                .collect::<Result<Vec<_>, _>>()
        };
        let branch = |value: &str, desc: &str, fs: &[RawField]| -> Result<Branch, String> {
            Ok(Branch {
                value: value.to_string(),
                desc: desc.to_string(),
                fields: all(fs)?,
            })
        };
        let branches = match &x.branches {
            None => Vec::new(),
            Some(Branches::Named(n)) if n == "tools" => tools
                .iter()
                .map(|t| {
                    self.tool(t)
                        .and_then(|tool| branch(t.as_str(), &tool.desc, &tool.input))
                })
                .collect::<Result<_, _>>()?,
            Some(Branches::Named(n)) if n == "toolResults" => self
                .tools
                .iter()
                .map(|t| branch(t.name.as_str(), &t.runs, &t.out))
                .collect::<Result<_, _>>()?,
            Some(Branches::Named(n)) if n == "askRoles" => roles
                .iter()
                .map(|r| branch(r.name.as_str(), &r.desc, &r.fields))
                .collect::<Result<_, _>>()?,
            Some(Branches::Named(n)) => return Err(format!("no such set of branches: {n}")),
            Some(Branches::List(bs)) => bs
                .iter()
                .map(|b| branch(b.value.as_str(), b.desc.as_deref().unwrap_or_default(), &b.fields))
                .collect::<Result<_, _>>()?,
        };
        let f = Field {
            title: x.title.clone().unwrap_or_default(),
            kind,
            desc: x.desc.clone().unwrap_or_default(),
            optional: x.optional.unwrap_or(false),
            values: match &x.values {
                Some(v) => self.enum_values(v)?,
                None => Vec::new(),
            },
            fields: all(x.fields.as_deref().unwrap_or_default())?,
            open: kind == Kind::Obj && x.fields.is_none(),
            item: match &x.item {
                Some(i) => Some(Box::new(self.field(i, tools, roles)?)),
                None => None,
            },
            branches,
            common: all(x.common.as_deref().unwrap_or_default())?,
            by: x.by.clone().unwrap_or_default(),
            min: x.min.unwrap_or(0),
            name,
        };
        if kind == Kind::Union && f.by.is_empty() {
            return Err(format!(
                "a union names no field that tells its branches apart: {}",
                f.name
            ));
        }
        if kind == Kind::List && f.item.is_none() {
            return Err(format!("a list with no item: {}", f.name));
        }
        let clash = f
            .common
            .iter()
            .any(|c| c.name == f.by || f.branches.iter().any(|b| b.fields.iter().any(|y| y.name == c.name)));
        if clash {
            return Err(format!("a field every branch has is also in a branch: {}", f.name));
        }
        if f.branches.iter().any(|b| b.fields.iter().any(|y| y.name == f.by)) {
            return Err(format!(
                "a branch has a field named like the union's marker {}: {}",
                f.by, f.name
            ));
        }
        Ok(f)
    }
}

/// What a list holds, or the field itself.
/// A list's item, or the field itself.
fn inner_mut(f: &mut Field) -> &mut Field {
    if f.kind == Kind::List && f.item.is_some() {
        f.item.as_deref_mut().expect("a list's item")
    } else {
        f
    }
}

/// Adds a sentence to the description of the field, or the branch, at path `p` of `fields`: a list's item is
/// followed, and below a union, the fields every branch has and those of the branch named. False when the path names
/// nothing.
pub(crate) fn add_to_desc(fields: &mut [Field], p: &str, say: &str) -> bool {
    let (seg, rest) = match p.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (p, None),
    };
    let Some((name, branch)) = segment(seg) else {
        return false;
    };
    let Some(f) = fields.iter_mut().find(|x| x.name == name) else {
        return false;
    };
    let said = |d: &str| {
        if d.is_empty() {
            say.to_string()
        } else if d.ends_with('.') {
            format!("{d} {say}")
        } else {
            format!("{d}. {say}")
        }
    };
    let Some(rest) = rest else {
        if let Some(b) = branch {
            let g = inner_mut(f);
            return match g.branches.iter_mut().find(|x| x.value == b) {
                Some(x) => {
                    x.desc = said(&x.desc);
                    true
                }
                None => false,
            };
        }
        f.desc = said(&f.desc);
        return true;
    };
    let g = inner_mut(f);
    let mut found = add_to_desc(&mut g.common, rest, say);
    match branch {
        Some(b) => {
            if let Some(x) = g.branches.iter_mut().find(|x| x.value == b) {
                found |= add_to_desc(&mut x.fields, rest, say);
            }
        }
        None => {
            found |= add_to_desc(&mut g.fields, rest, say);
            for x in &mut g.branches {
                found |= add_to_desc(&mut x.fields, rest, say);
            }
        }
    }
    found
}

pub(crate) fn inner(f: &Field) -> &Field {
    match &f.item {
        Some(i) if f.kind == Kind::List => i,
        _ => f,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlaceKind {
    Field,
    /// The marker that tells a union's branches apart, by the marker's name.
    Disc(String),
    /// A branch, with the key of its union's marker.
    Branch(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Place {
    pub key: String,
    pub kind: PlaceKind,
    /// Nothing is inside it: a value, a marker, or a branch with no fields.
    pub leaf: bool,
}

/// Every place of `fields`, in order, each key starting with `prefix`.
pub(crate) fn places(fields: &[Field], prefix: &str, out: &mut Vec<Place>) {
    for f in fields {
        let k = format!("{prefix}{}", f.name);
        let g = inner(f);
        let leaf = matches!(g.kind, Kind::Text | Kind::Int | Kind::Bool | Kind::Enum) || g.open;
        out.push(Place {
            key: k.clone(),
            kind: PlaceKind::Field,
            leaf,
        });
        match g.kind {
            Kind::Obj => places(&g.fields, &format!("{k}."), out),
            Kind::Union => {
                let dk = format!("{k}.{}", g.by);
                out.push(Place {
                    key: dk.clone(),
                    kind: PlaceKind::Disc(g.by.clone()),
                    leaf: true,
                });
                places(&g.common, &format!("{k}."), out);
                for b in &g.branches {
                    let bk = format!("{k}[{}]", b.value);
                    let leaf = b.fields.is_empty();
                    out.push(Place {
                        key: bk.clone(),
                        kind: PlaceKind::Branch(dk.clone()),
                        leaf,
                    });
                    places(&b.fields, &format!("{bk}."), out);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn keys(fields: &[Field], prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    places(fields, prefix, &mut out);
    out.into_iter().map(|p| p.key).collect()
}

fn word(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// A segment of a path: a name, `@` and a name, either with a branch `[value]`.
pub(crate) fn segment(s: &str) -> Option<(&str, Option<&str>)> {
    let (name, rest) = s.split_at(s.find('[').unwrap_or(s.len()));
    if !word(name.strip_prefix('@').unwrap_or(name)) {
        return None;
    }
    if rest.is_empty() {
        return Some((name, None));
    }
    let b = rest.strip_prefix('[')?.strip_suffix(']')?;
    if b.is_empty() || b.contains(']') {
        return None;
    }
    Some((name, Some(b)))
}

/// Whether `p` is a path of names, each with a branch or not.
pub(crate) fn is_path(p: &str) -> bool {
    p.split('.')
        .all(|s| segment(s).is_some_and(|(n, _)| !n.starts_with('@')))
}

/// The id of a path that starts in an input read by id, and the rest of the path.
pub(crate) fn by_id(p: &str) -> Option<(&str, &str)> {
    let s = p.strip_prefix('@')?;
    let (id, rest) = s.split_at(s.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(s.len()));
    let ok = word(id) && (rest.is_empty() || rest.starts_with('.') || rest.starts_with('['));
    ok.then_some((id, rest))
}

/// The field a path starts from, with its `@` if it has one.
pub(crate) fn head(p: &str) -> &str {
    let s = p.strip_prefix('@').unwrap_or(p);
    let end = s.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(s.len());
    &p[..p.len() - s.len() + end]
}

/// What a path names: a field, or a union's marker with the values it takes.
#[derive(Debug, Clone)]
pub(crate) enum Resolved {
    Field(Field),
    Disc(Vec<String>),
}

impl Resolved {
    /// The values a condition on the place can switch on.
    pub fn values(&self) -> Option<Vec<String>> {
        match self {
            Resolved::Disc(vs) => Some(vs.clone()),
            Resolved::Field(f) if f.kind == Kind::Enum => Some(f.values.iter().map(|(v, _)| v.clone()).collect()),
            Resolved::Field(_) => None,
        }
    }
}

/// The key of path `p` in `fields`, and what it names.
pub(crate) fn resolve(fields: &[Field], p: &str) -> Result<(String, Resolved), String> {
    let segs: Vec<&str> = p.split('.').collect();
    let mut key = String::new();
    let mut base = String::new();
    // Where the next segment is looked up: fields, or a union with the branch chosen, if any; neither below a value.
    let mut in_fields: Option<&[Field]> = Some(fields);
    let mut in_union: Option<(&Field, Option<String>)> = None;
    let mut last: Option<&Field> = None;
    for (i, seg) in segs.iter().enumerate() {
        let (name, b) = segment(seg).ok_or_else(|| format!("not a path: {p}"))?;
        let f: &Field = if let Some(fs) = in_fields {
            let f = fs
                .iter()
                .find(|x| x.name == name)
                .ok_or_else(|| format!("no such field: {p}"))?;
            if !key.is_empty() {
                key.push('.');
            }
            key.push_str(name);
            f
        } else if let Some((u, sel)) = in_union.take() {
            if name == u.by {
                if b.is_some() || i + 1 != segs.len() {
                    return Err(format!("nothing is below a union's marker: {p}"));
                }
                let vs = u.branches.iter().map(|x| x.value.clone()).collect();
                return Ok((format!("{base}.{name}"), Resolved::Disc(vs)));
            }
            if let Some(c) = u.common.iter().find(|x| x.name == name) {
                key = format!("{base}.{name}");
                c
            } else {
                let found: Vec<(&Branch, &Field)> = u
                    .branches
                    .iter()
                    .filter(|bb| sel.as_deref().is_none_or(|s| s == bb.value))
                    .flat_map(|bb| bb.fields.iter().filter(|x| x.name == name).map(move |x| (bb, x)))
                    .collect();
                let &[(bb, x)] = found.as_slice() else {
                    return Err(format!("no such field, or one in two branches: {p}"));
                };
                if sel.is_none() {
                    key = format!("{key}[{}]", bb.value);
                }
                key = format!("{key}.{name}");
                x
            }
        } else {
            return Err(format!("no field below a value: {p}"));
        };
        let g = inner(f);
        base = key.clone();
        if let Some(b) = b {
            if g.kind != Kind::Union || !g.branches.iter().any(|x| x.value == b) {
                return Err(format!("no such branch: {p}"));
            }
            key = format!("{key}[{b}]");
        }
        (in_fields, in_union) = match g.kind {
            Kind::Obj => (Some(g.fields.as_slice()), None),
            Kind::Union => (None, Some((g, b.map(str::to_string)))),
            _ => (None, None),
        };
        last = Some(f);
    }
    let f = last.ok_or_else(|| format!("not a path: {p}"))?;
    Ok((key, Resolved::Field(f.clone())))
}
