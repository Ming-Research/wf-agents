//! What the command line is told of a return: the shape of a part of a cell's return schema as a typed value, the
//! shapes of the same part in several cells taken together, and the schema the command line is given from the result.
//!
//! A [`Shape`] keeps the type of each value, each object with its fields, those it requires and whether it takes no
//! other, each list with its items, and, for a union whose branches all fix one field to a different value (the
//! flow's unions, told apart by `kind`, `verdict`, `by` and the like), a table of the branches by that value
//! ([`Split`]). It keeps no `enum` and no description. The command line's schema is one for every cell of a line, so
//! the shapes of the cells are merged ([`Shape::merged`]) and the merge refuses nothing any cell takes: types and
//! fields are the union of both, what is required is what both require, no other field is refused only when both
//! refuse it, and the branches of one value are merged branch by branch. The schema generated from a shape
//! ([`Shape::schema`]) lists the fields of all branches together and, for each branch, an `if` that a value is that
//! branch with a `then` that requires what the branch requires and takes no field it does not take, in an `allOf`
//! beside the fields, so that the command line refuses a field written in the branch it does not belong to, naming it.

use serde_json::{Map, Value, json};

/// A part of a cell's return schema as far as the command line is told it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shape {
    /// The types a value may be; none when it may be anything.
    pub types: Option<Vec<String>>,
    /// The fields of an object in the order its schema lists them; none when the schema lists none.
    pub fields: Option<Vec<(String, Shape)>>,
    /// The fields an object requires.
    pub required: Vec<String>,
    /// Whether an object takes no field beyond those it lists.
    pub closed: bool,
    /// The items of a list.
    pub items: Option<Box<Shape>>,
    /// The branches of a union by the field they fix; none for any other value.
    pub split: Option<Split>,
}

/// The branches of a union whose branches are objects that all fix the same field to a value of its own: the field,
/// and for each value the branch's shape, in the order the schema lists them.
#[derive(Debug, Clone, PartialEq)]
pub struct Split {
    pub by: String,
    pub branches: Vec<(Value, Shape)>,
}

impl Shape {
    /// The shape of a part of a cell's schema. A value with no type of its own takes that of its fixed value or of all
    /// its values; a field its object may leave out may also be null, which the engine takes as the field left out;
    /// alternatives come together as one ([`Shape::merged`]), and a union whose alternatives are told apart by a fixed
    /// field keeps its branches ([`Split`]).
    pub fn of(s: &Value) -> Shape {
        let Some(o) = s.as_object() else { return Shape::default() };
        let mut out = Shape {
            types: type_of(o),
            closed: s.get("additionalProperties") == Some(&Value::Bool(false)),
            ..Shape::default()
        };
        if let Some(ps) = o.get("properties").and_then(Value::as_object) {
            let required = required_of(s);
            out.fields = Some(
                ps.iter()
                    .map(|(k, p)| {
                        let mut x = Shape::of(p);
                        if !required.contains(&k.as_str()) {
                            x.or_null();
                        }
                        (k.clone(), x)
                    })
                    .collect(),
            );
            out.required = ps.keys().filter(|k| required.contains(&k.as_str())).cloned().collect();
        }
        out.items = o.get("items").map(|i| Box::new(Shape::of(i)));
        let alts: Vec<&Value> = ["anyOf", "oneOf"]
            .iter()
            .filter_map(|k| o.get(*k)?.as_array())
            .flatten()
            .collect();
        let shapes: Vec<Shape> = alts.iter().map(|a| Shape::of(a)).collect();
        let Some(all) = shapes.iter().cloned().reduce(|a, b| a.merged(&b)) else {
            return out;
        };
        let split = Split::of(&alts, &shapes);
        let mut out = if out == Shape::default() { all } else { out.merged(&all) };
        out.split = split;
        out
    }

    /// Whether the shape may be an object: it names that type, or none, as it may be anything.
    pub fn takes_object(&self) -> bool {
        self.types.as_ref().is_none_or(|t| t.iter().any(|t| t == "object"))
    }

    /// The names of the fields the shape lists, in its order.
    pub fn field_names(&self) -> Vec<&str> {
        self.fields
            .iter()
            .flatten()
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// The field of that name.
    pub fn field(&self, name: &str) -> Option<&Shape> {
        self.fields.iter().flatten().find(|(k, _)| k == name).map(|(_, s)| s)
    }

    /// The shape may also be null; one of no type is left as it is, as it may be anything.
    fn or_null(&mut self) {
        if let Some(ts) = &mut self.types
            && !ts.iter().any(|t| t == "null")
        {
            ts.push("null".into());
        }
    }

    /// Two shapes taken together, as alternatives of one value: the types of both, the fields of both, a field both
    /// have taken together too, the items of both lists taken together, and the branches of both unions that fix the
    /// same field, branch by branch; of no type when either is, as it may be anything, and of no branches when either
    /// has none. Of the shapes that may be an object, the fields all of them require are required, and no other field
    /// is refused only when none of them takes one.
    pub fn merged(&self, b: &Shape) -> Shape {
        let a = self;
        let types = match (&a.types, &b.types) {
            (Some(x), Some(y)) => {
                let mut ts = x.clone();
                for t in y {
                    if !ts.contains(t) {
                        ts.push(t.clone());
                    }
                }
                Some(ts)
            }
            _ => None,
        };
        let fields = if a.fields.is_some() || b.fields.is_some() {
            let mut fs = a.fields.clone().unwrap_or_default();
            for (k, v) in b.fields.iter().flatten() {
                match fs.iter_mut().find(|(n, _)| n == k) {
                    Some((_, x)) => *x = x.merged(v),
                    None => fs.push((k.clone(), v.clone())),
                }
            }
            Some(fs)
        } else {
            None
        };
        let objects: Vec<&Shape> = [a, b].into_iter().filter(|s| s.takes_object()).collect();
        let (required, closed) = match objects.first() {
            Some(first) => (
                first
                    .required
                    .iter()
                    .filter(|k| objects.iter().all(|s| s.required.contains(k)))
                    .cloned()
                    .collect(),
                objects.iter().all(|s| s.closed),
            ),
            None => (Vec::new(), false),
        };
        let items = match (&a.items, &b.items) {
            (Some(x), Some(y)) => Some(Box::new(x.merged(y))),
            (Some(x), None) | (None, Some(x)) => Some(x.clone()),
            (None, None) => None,
        };
        let split = match (&a.split, &b.split) {
            (Some(x), Some(y)) if x.by == y.by => Some(x.merged(y)),
            _ => None,
        };
        Shape {
            types,
            fields,
            required,
            closed,
            items,
            split,
        }
    }

    /// The schema the command line is given for the shape.
    pub fn schema(&self) -> Value {
        let mut o = Map::new();
        if let Some(t) = &self.types {
            o.insert("type".into(), types_schema(t));
        }
        if let Some(fs) = &self.fields {
            o.insert(
                "properties".into(),
                Value::Object(fs.iter().map(|(k, v)| (k.clone(), v.schema())).collect()),
            );
        }
        if !self.required.is_empty() {
            o.insert("required".into(), json!(self.required));
        }
        if self.closed {
            o.insert("additionalProperties".into(), json!(false));
        }
        if let Some(i) = &self.items {
            o.insert("items".into(), i.schema());
        }
        if let Some(rules) = self.split.as_ref().map(|s| s.rules(self)).filter(|r| !r.is_empty()) {
            o.insert("allOf".into(), Value::Array(rules));
        }
        Value::Object(o)
    }
}

impl Split {
    /// The branches of a union, when every alternative is an object that fixes one and the same field, to a value no
    /// other alternative fixes it to; the field is the first of the first alternative that does. `shapes` are the
    /// alternatives' shapes, in the order of `alts`.
    fn of(alts: &[&Value], shapes: &[Shape]) -> Option<Split> {
        let object = |s: &Shape| s.types.as_ref().is_some_and(|t| t.len() == 1 && t[0] == "object");
        if alts.is_empty() || !shapes.iter().all(object) {
            return None;
        }
        let fixed = |a: &Value, k: &str| a.get("properties")?.get(k)?.get("const").cloned();
        let first = alts[0].get("properties")?.as_object()?;
        let by = first.keys().find(|k| {
            let values: Vec<Option<Value>> = alts.iter().map(|a| fixed(a, k)).collect();
            values.iter().all(Option::is_some)
                && values
                    .iter()
                    .enumerate()
                    .all(|(i, v)| values.iter().take(i).all(|w| w != v))
        })?;
        let branches = alts
            .iter()
            .zip(shapes)
            .filter_map(|(a, s)| Some((fixed(a, by)?, s.clone())))
            .collect();
        Some(Split {
            by: by.clone(),
            branches,
        })
    }

    /// The branches of both, a branch of a value both have taken together.
    fn merged(&self, other: &Split) -> Split {
        let mut branches = self.branches.clone();
        for (v, b) in &other.branches {
            match branches.iter_mut().find(|(w, _)| w == v) {
                Some((_, x)) => *x = x.merged(b),
                None => branches.push((v.clone(), b.clone())),
            }
        }
        Split {
            by: self.by.clone(),
            branches,
        }
    }

    /// For each branch the rule that a value that fixes the field to the branch's value requires what the branch
    /// requires and, when the branch takes no other field, takes none: an `if` and a `then`. A field of the branch of
    /// a type other than the field has beside the branches (as a field the branch requires that another branch may
    /// leave out) is given its type there. An object a keyword reads says it is one, which the command line's strict
    /// check otherwise warns of.
    fn rules(&self, whole: &Shape) -> Vec<Value> {
        let mut rules = Vec::new();
        for (value, branch) in &self.branches {
            let mut then = Map::new();
            then.insert("type".into(), json!("object"));
            if !branch.required.is_empty() {
                then.insert("required".into(), json!(branch.required));
            }
            let retyped = |k: &str, x: &Shape| match (&x.types, whole.field(k).and_then(|w| w.types.as_ref())) {
                (Some(t), Some(w)) if t != w => Some(types_schema(t)),
                _ => None,
            };
            let props: Map<String, Value> = branch
                .fields
                .iter()
                .flatten()
                .filter_map(|(k, x)| match retyped(k, x) {
                    Some(t) => Some((k.clone(), json!({ "type": t }))),
                    None if branch.closed => Some((k.clone(), json!({}))),
                    None => None,
                })
                .collect();
            if !props.is_empty() {
                then.insert("properties".into(), Value::Object(props));
            }
            if branch.closed {
                then.insert("additionalProperties".into(), json!(false));
            }
            if then.len() > 1 {
                let mut fixes = Map::new();
                fixes.insert(self.by.clone(), json!({ "const": value }));
                let when = json!({ "type": "object", "properties": fixes, "required": [self.by.clone()] });
                rules.push(json!({ "if": when, "then": then }));
            }
        }
        rules
    }
}

/// A schema's own type, or that of its fixed value or of all its values when they share one.
fn type_of(o: &Map<String, Value>) -> Option<Vec<String>> {
    match o.get("type") {
        Some(Value::String(t)) => return Some(vec![t.clone()]),
        Some(Value::Array(ts)) => return Some(ts.iter().filter_map(Value::as_str).map(str::to_string).collect()),
        Some(_) => return None,
        None => {}
    }
    let values = o
        .get("const")
        .into_iter()
        .chain(o.get("enum").and_then(Value::as_array).into_iter().flatten());
    let mut types: Vec<&str> = Vec::new();
    for v in values {
        let t = match v {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        if !types.contains(&t) {
            types.push(t);
        }
    }
    match types.as_slice() {
        [one] => Some(vec![one.to_string()]),
        _ => None,
    }
}

/// A list of types as a schema says it: the type alone for one.
fn types_schema(ts: &[String]) -> Value {
    match ts {
        [one] => json!(one),
        _ => json!(ts),
    }
}

/// The fields a schema requires of an object.
pub fn required_of(s: &Value) -> Vec<&str> {
    s.get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A shape keeps only the types of the values, each object with its fields, those it requires and whether it takes
    // no other, and each list with its items: no value is fixed, and a field its object may leave out may also be null.
    // Alternatives come together as one: an object requires what all of them require and takes no other field when
    // none of them does, an alternative that is no object leaving that to the others; a value of no type of its own
    // takes that of its values. A union told apart by a fixed field keeps its branches by value.
    #[test]
    fn a_shape_keeps_only_the_types_and_the_fields_of_the_values() {
        let s = json!({
            "description": "What this round comes to",
            "anyOf": [
                { "type": "object", "required": ["reading"], "additionalProperties": false, "properties": {
                    "reading": { "type": "object", "required": ["direction"], "additionalProperties": false,
                                 "properties": {
                        "direction": { "type": "string", "description": "The direction" },
                        "asks": { "type": "array", "minItems": 1, "items": { "type": "string" } },
                    } },
                } },
                { "type": "object", "required": ["verdict"], "properties": {
                    "verdict": { "oneOf": [
                        { "type": "object", "required": ["verdict"], "additionalProperties": false,
                          "properties": { "verdict": { "const": "holds" } } },
                        { "type": "object", "required": ["verdict", "kind"], "additionalProperties": false,
                          "properties": {
                            "verdict": { "const": "fails" },
                            "kind": { "enum": ["a", "b"] },
                        } },
                    ] },
                } },
                { "type": "object", "required": ["reading"], "additionalProperties": false, "properties": {
                    "reading": { "type": "string" },
                } },
            ],
        });
        assert_eq!(
            Shape::of(&s).schema(),
            json!({ "type": "object", "properties": {
                "reading": { "type": ["object", "string"], "properties": {
                    "direction": { "type": "string" },
                    "asks": { "type": ["array", "null"], "items": { "type": "string" } },
                }, "required": ["direction"], "additionalProperties": false },
                "verdict": { "type": "object", "properties": {
                    "verdict": { "type": "string" },
                    "kind": { "type": "string" },
                }, "required": ["verdict"], "additionalProperties": false, "allOf": [
                    { "if": { "type": "object", "properties": { "verdict": { "const": "holds" } },
                              "required": ["verdict"] },
                      "then": { "type": "object", "required": ["verdict"],
                                "properties": { "verdict": {} }, "additionalProperties": false } },
                    { "if": { "type": "object", "properties": { "verdict": { "const": "fails" } },
                              "required": ["verdict"] },
                      "then": { "type": "object", "required": ["verdict", "kind"],
                                "properties": { "verdict": {}, "kind": {} }, "additionalProperties": false } },
                ] },
            } })
        );
    }

    /// An object that fixes `kind` to a value, with other fields, those it requires and whether it takes no other.
    fn branch(kind: &str, fields: Value, required: Value, closed: bool) -> Value {
        let mut props = json!({ "kind": { "const": kind } });
        props.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
        let mut b = json!({ "type": "object", "required": required, "properties": props });
        if closed {
            b["additionalProperties"] = json!(false);
        }
        b
    }

    /// The `then` of the rule for the branch of a value, in a schema generated from a union.
    fn then_of<'a>(schema: &'a Value, kind: &str) -> &'a Value {
        let rules = schema["allOf"].as_array().unwrap();
        &rules
            .iter()
            .find(|r| r["if"]["properties"]["kind"]["const"] == kind)
            .unwrap()["then"]
    }

    // Two unions that fix the same field are merged branch by branch, by the value each fixes: a value both have takes the
    // fields of both and requires what both require, and takes no other field only when both take none; a value only
    // one has is kept as it is. A union taken together with a value that is no such union, or one that fixes another
    // field, keeps no branches, since it must take what either does.
    #[test]
    fn the_branches_of_two_unions_are_merged_by_the_value_they_fix() {
        let a = json!({ "oneOf": [
            branch("x", json!({ "p": { "type": "string" } }), json!(["kind", "p"]), true),
            branch("y", json!({ "q": { "type": "string" } }), json!(["kind", "q"]), true),
        ] });
        let b = json!({ "oneOf": [
            branch("x", json!({ "p": { "type": "string" }, "r": { "type": "integer" } }), json!(["kind", "p"]), false),
            branch("z", json!({ "s": { "type": "string" } }), json!(["kind", "s"]), true),
        ] });
        let both = Shape::of(&a).merged(&Shape::of(&b)).schema();
        assert_eq!(both["allOf"].as_array().unwrap().len(), 3);
        // The fields of every branch, each as its branch has it (null-able only where its branch may leave it out), the
        // fields all require, and no other field refused only if no branch takes one.
        assert_eq!(
            both["properties"],
            json!({
                "kind": { "type": "string" },
                "p": { "type": "string" },
                "q": { "type": "string" },
                "r": { "type": ["integer", "null"] },
                "s": { "type": "string" },
            })
        );
        assert_eq!(both["required"], json!(["kind"]));
        assert!(both.get("additionalProperties").is_none());
        // x is in both: it takes `r` of the one and requires `p` of both, and, one of them taking other fields, refuses
        // none; y and z are in one each, and are as that one has them.
        assert_eq!(then_of(&both, "x"), &json!({ "type": "object", "required": ["kind", "p"] }));
        assert_eq!(
            then_of(&both, "y"),
            &json!({ "type": "object", "required": ["kind", "q"],
                     "properties": { "kind": {}, "q": {} }, "additionalProperties": false })
        );
        assert_eq!(
            then_of(&both, "z"),
            &json!({ "type": "object", "required": ["kind", "s"],
                     "properties": { "kind": {}, "s": {} }, "additionalProperties": false })
        );
        // A union taken with a plain object, or with one fixing another field, keeps no branches.
        let plain = json!({ "type": "object", "properties": { "kind": { "type": "string" } } });
        assert!(Shape::of(&a).merged(&Shape::of(&plain)).schema().get("allOf").is_none());
        let other = json!({ "oneOf": [{ "type": "object", "required": ["by"], "properties": { "by": { "const": "k" } } }] });
        assert!(Shape::of(&a).merged(&Shape::of(&other)).schema().get("allOf").is_none());
        // Branches that do not all fix the field, or fix it to the same value, are no union of branches.
        let same = json!({ "oneOf": [
            branch("x", json!({}), json!(["kind"]), true),
            branch("x", json!({ "p": { "type": "string" } }), json!(["kind"]), true),
        ] });
        assert!(Shape::of(&same).schema().get("allOf").is_none());
    }

    // A field a branch requires that another branch may leave out is null-able beside the branches, since it may be left
    // out in one; its branch gives it its own type, so that the command line refuses a null where the branch requires
    // a value, which the engine would send back.
    #[test]
    fn a_field_a_branch_requires_is_given_its_type_in_the_branch() {
        let u = json!({ "oneOf": [
            branch("w", json!({ "t": { "type": "string" } }), json!(["kind", "t"]), true),
            branch("v", json!({ "t": { "type": "string" } }), json!(["kind"]), true),
        ] });
        let schema = Shape::of(&u).schema();
        assert_eq!(schema["properties"]["t"], json!({ "type": ["string", "null"] }));
        assert_eq!(then_of(&schema, "w")["properties"]["t"], json!({ "type": "string" }));
        assert_eq!(then_of(&schema, "v")["properties"]["t"], json!({}));
    }
}
