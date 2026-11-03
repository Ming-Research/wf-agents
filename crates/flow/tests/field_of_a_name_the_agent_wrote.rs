//! The field of a place of a return that does not fit, when the place is a name the agent wrote: a field the schema does
//! not have is told with the name as the agent wrote it, whatever characters it holds. An agent's name is an object's
//! key, not an item of a list, so it is not written `[]` where it looks like an index; a dot in it is part of it.

use serde_json::{Value, json};

/// A schema written here: an object of closed fields, one of which holds a list of closed objects.
fn schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["label", "items"],
        "properties": {
            "label": { "type": "string" },
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["a"],
                    "properties": { "a": { "type": "string" } }
                }
            }
        }
    })
}

// A field the schema does not have is told with its name as it was written: the path of the field is the path of its
// parent and the name. The reference is the definition of `Misfit::field`: the path of the field that is wrong, a
// list's items written `[]`; a key that reads like `[3]` is not an item of a list. The same name in two lists' items is
// one field, since the lists' indexes are written `[]` but the name is not.
#[test]
fn a_name_the_agent_wrote_is_the_field_as_it_wrote_it() {
    let schema = schema();
    let cases: Vec<(Value, Vec<Option<&str>>)> = vec![
        // A name that ends like an index.
        (json!({ "label": "l", "items": [], "x[3]": 1 }), vec![Some("x[3]")]),
        // A name that is an index and nothing else.
        (json!({ "label": "l", "items": [], "[7]": 1 }), vec![Some("[7]")]),
        // A name with a dot: its parent's path and its own name, not told as two fields.
        (json!({ "label": "l", "items": [], "a.b": 1 }), vec![Some("a.b")]),
        // In an item of a list: the item's index is written `[]`, the name is not.
        (
            json!({ "label": "l", "items": [{ "a": "s", "k[12]": 1 }, { "a": "s", "k[12]": 2 }] }),
            vec![Some("items[].k[12]"), Some("items[].k[12]")],
        ),
        // A name with a bracket that is not closed.
        (json!({ "label": "l", "items": [], "m[": 1 }), vec![Some("m[")]),
    ];
    for (value, fields) in cases {
        let places = flow::misfit_places(&schema, &value, "");
        let told: Vec<Option<&str>> = places.iter().map(|p| p.field.as_deref()).collect();
        assert_eq!(told, fields, "{value}\n{places:?}");
    }
}
