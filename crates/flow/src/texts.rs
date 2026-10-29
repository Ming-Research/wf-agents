//! The cells the texts of the flow name. A text names a cell by the word cell and the cell's name in backticks, its
//! stage before a dot where its stage has others, as cell `ask.way_on`, or several after the word cells, as cells
//! `test.full_ci`, `merge.queue` and `watch_main`; a field or a value in backticks, with no such word before it, names
//! no cell. Every cell a text names is a cell of its graph: a text of a graph names that graph's cells, and a text
//! every graph shares, the cells of some graph.

use std::collections::BTreeSet;

use serde_json::Value;

/// What joins one name of a list to the next.
const JOINS: [&str; 5] = [", and `", ", or `", ", `", " and `", " or `"];

/// The names of the cells a text names, in the order it names them.
pub fn cells_named(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices(['c', 'C']) {
        if text[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        let after = &text[i + 1..];
        let Some(mut rest) = after.strip_prefix("ells `").or_else(|| after.strip_prefix("ell `")) else {
            continue;
        };
        while let Some(end) = rest.find('`') {
            let name = &rest[..end];
            let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
            if !name.starts_with(word) || !name.ends_with(word) || !name.chars().all(|c| word(c) || c == '.') {
                break;
            }
            out.push(name);
            rest = &rest[end + 1..];
            match JOINS.iter().find_map(|j| rest.strip_prefix(*j)) {
                Some(r) => rest = r,
                None => break,
            }
        }
    }
    out
}

/// Every text of the flow that names a cell its graph does not have: where the text is, and the name.
pub(crate) fn unknown_cells(flow: &Value) -> Vec<String> {
    let cells_of = |f: &Value| -> BTreeSet<String> {
        f["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|n| n["id"].as_str().map(str::to_string))
            .collect()
    };
    let graphs: Vec<(String, BTreeSet<String>)> = flow["flows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| (f["name"].as_str().unwrap_or_default().to_string(), cells_of(f)))
        .collect();
    let mut out = Vec::new();
    let Some(top) = flow.as_object() else { return out };
    for (key, v) in top {
        if key != "flows" {
            let mut texts = Vec::new();
            walk(v, key.clone(), &mut texts);
            for (at, name) in texts {
                if !graphs.iter().any(|(_, cells)| cells.contains(name)) {
                    out.push(format!("{at} names cell `{name}`, which no graph has"));
                }
            }
        }
    }
    for (f, (graph, cells)) in flow["flows"].as_array().into_iter().flatten().zip(&graphs) {
        let mut texts = Vec::new();
        walk(f, format!("flows.{graph}"), &mut texts);
        for (at, name) in texts {
            if !cells.contains(name) {
                out.push(format!("{graph}: {at} names cell `{name}`, which its graph has not"));
            }
        }
    }
    out
}

/// Each cell every text under `v` names, with where the text is: an item of a list by its id or name when it has one,
/// else by its place.
fn walk<'v>(v: &'v Value, at: String, out: &mut Vec<(String, &'v str)>) {
    match v {
        Value::String(s) => out.extend(cells_named(s).into_iter().map(|n| (at.clone(), n))),
        Value::Array(xs) => {
            for (i, x) in xs.iter().enumerate() {
                let key = ["id", "name"].iter().find_map(|k| x[*k].as_str());
                let at = match key {
                    Some(k) => format!("{at}.{k}"),
                    None => format!("{at}[{i}]"),
                };
                walk(x, at, out);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                walk(x, format!("{at}.{k}"), out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A cell is named after the word cell or cells, in backticks, alone or in a list; a field in backticks, a word
    // that holds cell, and a name that is not one are not.
    #[test]
    fn a_text_names_a_cell_after_the_word_cell() {
        assert_eq!(cells_named("goes to cell `ask.way_on` at once"), ["ask.way_on"]);
        assert_eq!(
            cells_named("Cells `test.full_ci`, `merge.queue` and `watch_main` count, as cell `a` or `b` does"),
            ["test.full_ci", "merge.queue", "watch_main", "a", "b"]
        );
        assert!(cells_named("hand in `tools`, without `progress`; the cell that asked").is_empty());
        assert!(cells_named("an excellent `x`, a subcell `y`, cell `format:common`, cell `.x`, cell 11").is_empty());
    }

    // A text of a graph names a cell of that graph, and a text every graph shares a cell of some graph; each that
    // does not is told with where it is.
    #[test]
    fn a_text_that_names_no_cell_of_its_graph_is_found() {
        let flow = serde_json::json!({
            "engine": { "steps": { "s": "Goes on at cell `b`, then cell `c`." } },
            "flows": [
                { "name": "one", "nodes": [{ "id": "a", "prompt": ["After cell `a`, cells `b` and `x`."] }] },
                { "name": "two", "nodes": [{ "id": "b" }] }
            ]
        });
        assert_eq!(
            unknown_cells(&flow),
            [
                "engine.steps.s names cell `c`, which no graph has",
                "one: flows.one.nodes.a.prompt[0] names cell `b`, which its graph has not",
                "one: flows.one.nodes.a.prompt[0] names cell `x`, which its graph has not",
            ]
        );
    }
}
