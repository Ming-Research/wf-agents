//! What a later version of an input changes in the version a session holds, which a message gives instead of the
//! later version whole: for a text, the lines that differ as a unified diff; for an object, the entries that differ.

use serde_json::{Map, Value};

/// How many unchanged lines a hunk shows around what changed.
const CONTEXT: usize = 2;

/// The most cells the table that finds the lines two texts share may have; beyond it, the lines between their common
/// start and end are given as all removed and all added, which is still what changed.
const MOST: usize = 4_000_000;

/// The entries of a later object that differ from an earlier one's: each entry the later one changed or added, an
/// object inside both written the same way, and null for an entry the later one no longer has. Empty when they are
/// the same.
pub fn entries(earlier: &Value, later: &Value) -> Value {
    let none = Map::new();
    let was = earlier.as_object().unwrap_or(&none);
    let is = later.as_object().unwrap_or(&none);
    let mut out = Map::new();
    for (k, v) in is {
        match was.get(k) {
            Some(w) if w == v => {}
            Some(w) if w.is_object() && v.is_object() => {
                out.insert(k.clone(), entries(w, v));
            }
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    for k in was.keys().filter(|k| !is.contains_key(*k)) {
        out.insert(k.clone(), Value::Null);
    }
    Value::Object(out)
}

/// The lines of a later text that differ from an earlier one's, as a unified diff: each hunk headed by where it is in
/// both, `-` before a line removed, `+` before a line added, and a space before a line around them that stayed.
pub fn lines(earlier: &str, later: &str) -> String {
    let a: Vec<&str> = earlier.lines().collect();
    let b: Vec<&str> = later.lines().collect();
    // Each line as the diff has it: ' ', '-' or '+', and its number in each text, counted from 0.
    let mut ops: Vec<(char, usize, usize)> = Vec::new();
    let start = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let end = a[start..]
        .iter()
        .rev()
        .zip(b[start..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[start..a.len() - end], &b[start..b.len() - end]);
    ops.extend((0..start).map(|i| (' ', i, i)));
    let (n, m) = (am.len(), bm.len());
    if (n + 1).saturating_mul(m + 1) <= MOST {
        // The longest run of lines both share, from each pair of positions to the end.
        let w = m + 1;
        let mut t = vec![0u32; (n + 1) * w];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * w + j] = if am[i] == bm[j] {
                    t[(i + 1) * w + j + 1] + 1
                } else {
                    t[(i + 1) * w + j].max(t[i * w + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            // A line removed goes before a line added where either keeps as many shared lines.
            if i < n && j < m && am[i] == bm[j] {
                ops.push((' ', start + i, start + j));
                i += 1;
                j += 1;
            } else if i < n && (j == m || t[(i + 1) * w + j] >= t[i * w + j + 1]) {
                ops.push(('-', start + i, start + j));
                i += 1;
            } else {
                ops.push(('+', start + i, start + j));
                j += 1;
            }
        }
    } else {
        ops.extend((0..n).map(|i| ('-', start + i, start)));
        ops.extend((0..m).map(|j| ('+', start + n, start + j)));
    }
    ops.extend((0..end).map(|k| (' ', a.len() - end + k, b.len() - end + k)));
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k].0 != ' ').collect();
    let mut out = String::new();
    let mut k = 0;
    while k < changed.len() {
        // A hunk runs from the context before its first change to the context after its last, taking in each change
        // whose context touches the one before.
        let mut last = k;
        while last + 1 < changed.len() && changed[last + 1] - changed[last] <= 2 * CONTEXT + 1 {
            last += 1;
        }
        let from = changed[k].saturating_sub(CONTEXT);
        let to = (changed[last] + CONTEXT + 1).min(ops.len());
        let hunk = &ops[from..to];
        let olds = hunk.iter().filter(|o| o.0 != '+').count();
        let news = hunk.iter().filter(|o| o.0 != '-').count();
        let at = |c: usize| if c == 0 { 0 } else { 1 };
        out.push_str(&format!(
            "@@ -{},{olds} +{},{news} @@\n",
            hunk[0].1 + at(olds),
            hunk[0].2 + at(news)
        ));
        for &(op, i, j) in hunk {
            let line = if op == '+' { b[j] } else { a[i] };
            out.push(op);
            out.push_str(line);
            out.push('\n');
        }
        k = last + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // A text's change is its lines that differ with two lines around them, in hunks a unified diff reads.
    #[test]
    fn a_text_changes_by_the_lines_that_differ() {
        let earlier = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let later = "a\nb\nC\nd\ne\nf\ng\nh\ni\nj\nk\n";
        assert_eq!(
            lines(earlier, later),
            "@@ -1,5 +1,5 @@\n a\n b\n-c\n+C\n d\n e\n@@ -9,2 +9,3 @@\n i\n j\n+k\n"
        );
        assert_eq!(lines("x\ny\n", "y\n"), "@@ -1,2 +1,1 @@\n-x\n y\n");
        assert_eq!(lines("same\n", "same\n"), "");
        assert_eq!(lines("", "new\n"), "@@ -0,0 +1,1 @@\n+new\n");
    }

    // An object's change is the entries that differ, inside the objects both have, and null for one gone.
    #[test]
    fn an_object_changes_by_the_entries_that_differ() {
        let earlier = json!({ "agents_md": "A", "design_tree": { "a.md": "A", "b.md": "B" }, "cited_nodes": {} });
        let later = json!({ "agents_md": "A", "design_tree": { "a.md": "A2", "c.md": "C" }, "cited_nodes": {} });
        assert_eq!(
            entries(&earlier, &later),
            json!({ "design_tree": { "a.md": "A2", "c.md": "C", "b.md": null } })
        );
        assert_eq!(entries(&later, &later), json!({}));
    }
}
