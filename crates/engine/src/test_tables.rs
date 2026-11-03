//! The tables of the engineer's tests with their two results: the new-behaviour tests and the old-behaviour tests, one
//! row for each, `{ name, by, on_head, on_base }` as the tables of adversarial testing have them.
//!
//! The engineer writes a test once, as a row of the test changes of a return (`added`, or `extended` for a test that
//! was there before), with its kind and its two results. The tables are read from those rows here, so that no second
//! list of the same tests has to agree with the first. A return of a flow version from before holds the tables itself,
//! and they are read as they are. Nothing is written back to a return: a return stays as the engineer handed it in.

use serde_json::{Map, Value, json};

/// The cells of the engineer whose test changes make up the tables, in the order of the work.
pub const CELLS: [&str; 2] = ["I1", "T6"];

/// The engineer's two tables: new-behaviour tests and old-behaviour tests.
#[derive(Debug, Default, PartialEq)]
pub struct Tables {
    /// Tests of the new behaviour the change brings (the kind `new_behaviour`).
    pub new: Vec<Value>,
    /// Tests that guard old behaviour the change must not alter (the kind `old_behaviour`).
    pub guard: Vec<Value>,
}

impl Tables {
    /// The table of a kind, as the tools and checks name them: `new_tests` or `guard_tests`.
    pub fn table(&self, name: &str) -> &[Value] {
        if name == "new_tests" { &self.new } else { &self.guard }
    }

    /// The kind of each test in the tables, its name and `new_behaviour` or `old_behaviour`.
    pub fn kinds(&self) -> Vec<(String, String)> {
        let of = |rows: &[Value], kind: &str| -> Vec<(String, String)> {
            rows.iter()
                .map(|r| (r["name"].as_str().unwrap_or_default().to_string(), kind.to_string()))
                .collect()
        };
        let mut all = of(&self.new, "new_behaviour");
        all.extend(of(&self.guard, "old_behaviour"));
        all
    }

    fn put(&mut self, kind: &str, row: Value) {
        let name = row["name"].clone();
        // A test written again takes its later row, in the table of the kind that row gives.
        self.new.retain(|r| r["name"] != name);
        self.guard.retain(|r| r["name"] != name);
        match kind {
            "new_behaviour" => self.new.push(row),
            "old_behaviour" => self.guard.push(row),
            _ => {}
        }
    }
}

/// The rows of a field of a return, where it has the field: of the flat return, or of a return of a flow version from
/// before the answer was flat, which holds its fields under `end`.
fn field<'a>(ret: &'a Value, name: &str) -> Option<&'a Vec<Value>> {
    ret.get(name)
        .or_else(|| ret.get("end").and_then(|e| e.get(name)))
        .and_then(Value::as_array)
}

/// The tables one return gives: the tables it holds itself, when it is from a flow version that had them, and else
/// the tests its test changes add or extend, each in the table of its kind with the results the row gives (a result
/// the row leaves out is left out of the table's row too). A row of a test changed in another way, or of no kind, gives
/// none.
pub fn of_return(ret: &Value) -> Tables {
    let (new, guard) = (field(ret, "new_tests"), field(ret, "guard_tests"));
    if new.is_some() || guard.is_some() {
        return Tables {
            new: new.cloned().unwrap_or_default(),
            guard: guard.cloned().unwrap_or_default(),
        };
    }
    let mut tables = Tables::default();
    for change in field(ret, "test_changes").into_iter().flatten() {
        if !matches!(change["change"].as_str(), Some("added" | "extended")) {
            continue;
        }
        let mut row = Map::new();
        row.insert("name".into(), change["test"].clone());
        row.insert("by".into(), json!("engineer"));
        for k in ["on_head", "on_base"] {
            if let Some(v) = change.get(k).filter(|v| !v.is_null()) {
                row.insert(k.into(), v.clone());
            }
        }
        tables.put(change["kind"].as_str().unwrap_or_default(), Value::Object(row));
    }
    tables
}

/// The tables the returns give together, the oldest first: each test once, with the latest row any return gave it. A
/// return of a flow version with tables of its own adds their rows the same way, to the table they are in.
pub fn accumulated<'a>(returns: impl IntoIterator<Item = &'a Value>) -> Tables {
    let mut all = Tables::default();
    for ret in returns {
        let one = of_return(ret);
        for (kind, rows) in [("new_behaviour", one.new), ("old_behaviour", one.guard)] {
            for row in rows {
                all.put(kind, row);
            }
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn added(test: &str, kind: &str, head: &str, base: &str) -> Value {
        json!({ "change": "added", "test": test, "catches": "c", "kind": kind, "on_head": head, "on_base": base,
                "time": "1s", "worth": "w" })
    }

    fn names(rows: &[Value]) -> Vec<&str> {
        rows.iter().map(|r| r["name"].as_str().unwrap()).collect()
    }

    // Item #217, R241-28: a test is written once, as a row of the test changes, and is in the table of its kind with
    // its two results, whether it was added or extended; rows of other changes give none.
    #[test]
    fn each_test_is_in_the_table_of_its_kind_with_the_results_its_row_gives() {
        let mut extended = added("every_sample_takes_its_row", "new_behaviour", "green", "red");
        extended["change"] = json!("extended");
        let ret = json!({ "test_changes": [
            added("a", "new_behaviour", "green", "cannot_run"),
            added("b", "old_behaviour", "green", "green"),
            extended,
            { "change": "removed", "test": "gone", "reason": "r" },
            { "change": "relaxed", "test": "loose", "reason": "r" },
        ] });
        let t = of_return(&ret);
        assert_eq!(names(&t.new), ["a", "every_sample_takes_its_row"]);
        assert_eq!(names(&t.guard), ["b"]);
        assert_eq!(
            t.new[1],
            json!({ "name": "every_sample_takes_its_row", "by": "engineer", "on_head": "green", "on_base": "red" })
        );
        assert_eq!(t.table("new_tests"), t.new);
        assert_eq!(t.table("guard_tests"), t.guard);
    }

    // A result the row leaves out (the design chose adversarial testing, which runs the tests) is left out of the
    // table's row, not written as a value.
    #[test]
    fn a_result_the_row_leaves_out_is_left_out_of_the_table() {
        let ret = json!({ "test_changes": [
            { "change": "added", "test": "a", "catches": "c", "kind": "new_behaviour", "on_head": null,
              "time": "1s", "worth": "w" },
        ] });
        assert_eq!(of_return(&ret).new, [json!({ "name": "a", "by": "engineer" })]);
    }

    // A return of a flow version that had the tables gives them as they are, whether it is flat or holds its fields
    // under `end`, and its test changes add nothing to them.
    #[test]
    fn a_return_of_an_older_flow_version_gives_its_own_tables() {
        let own = json!([{ "name": "t", "by": "engineer", "on_head": "green", "on_base": "red" }]);
        for ret in [
            json!({ "new_tests": own, "test_changes": [added("x", "new_behaviour", "green", "red")] }),
            json!({ "end": { "new_tests": own, "test_changes": [added("x", "new_behaviour", "green", "red")] } }),
        ] {
            let t = of_return(&ret);
            assert_eq!(t.new, own.as_array().unwrap().clone());
            assert!(t.guard.is_empty());
        }
    }

    // The tables of several returns hold each test once, with its latest row, in the table of the kind that row gives:
    // a test the fix wrote again with another result, or as another kind, is not in both.
    #[test]
    fn a_test_written_again_has_its_later_row_only() {
        let first = json!({ "test_changes": [
            added("a", "new_behaviour", "red", "red"),
            added("b", "new_behaviour", "green", "red"),
        ] });
        let fix = json!({ "test_changes": [
            added("a", "new_behaviour", "green", "red"),
            added("b", "old_behaviour", "green", "green"),
        ] });
        let t = accumulated([&first, &fix]);
        assert_eq!(
            t.new,
            [json!({ "name": "a", "by": "engineer", "on_head": "green", "on_base": "red" })]
        );
        assert_eq!(names(&t.guard), ["b"]);
        // The order of the returns is the order of the work: the same returns the other way give the other rows.
        let t = accumulated([&fix, &first]);
        assert_eq!(names(&t.new), ["a", "b"]);
        assert!(t.guard.is_empty());
    }

    // R267-85: the kind the table review holds a test to is the kind of its latest row in any round, as the tables
    // have it, not that of the first round that wrote it (a fix that re-recorded a guard test as new behaviour).
    #[test]
    fn the_kind_of_a_test_is_that_of_its_latest_row() {
        let first = json!({ "test_changes": [added("a", "old_behaviour", "green", "cannot_run")] });
        let fix = json!({ "test_changes": [added("a", "new_behaviour", "green", "cannot_run")] });
        assert_eq!(
            accumulated([&first, &fix]).kinds(),
            [("a".to_string(), "new_behaviour".to_string())]
        );
        assert_eq!(
            accumulated([&first]).kinds(),
            [("a".to_string(), "old_behaviour".to_string())]
        );
    }
}
