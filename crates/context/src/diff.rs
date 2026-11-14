//! What changed between two versions of a text, as a cell given a datum `last_and_latest` is sent it: each changed
//! line with two lines on each side, in hunks as a unified diff writes them, so that the latest is rebuilt exactly from
//! the version the session holds. A line is what stands between two line breaks, the text after the last one
//! included, so that a line break at the end is a change like any other.

/// How many lines that are the same are shown on each side of a change.
const AROUND: usize = 2;

/// How many pairs of lines may be compared to find what two texts share between what begins and what ends them alike:
/// the comparison takes memory as their product, and past this the change is not worked out.
const COMPARED: usize = 16_000_000;

/// What changed from the first text to the second, in hunks; nothing where they are the same; none where they are too
/// long to compare.
pub fn diff(a: &str, b: &str) -> Option<String> {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.split('\n').collect(), b.split('\n').collect());
    let walk = walk(&a, &b)?;
    let mut out: Vec<String> = Vec::new();
    // The steps that are changes, by their place in the walk.
    let changes: Vec<usize> = (0..walk.len()).filter(|&k| walk[k].0 != Step::Same).collect();
    let mut k = 0;
    while k < changes.len() {
        // A hunk runs from two lines before its first change to two after its last, taking in each change whose lines
        // around it meet those of the one before.
        let first = changes[k];
        let mut last = first;
        while k + 1 < changes.len() && changes[k + 1] - last <= 2 * AROUND + 1 {
            k += 1;
            last = changes[k];
        }
        k += 1;
        let from = first.saturating_sub(AROUND);
        let to = (last + AROUND + 1).min(walk.len());
        let (old_at, new_at) = (walk[from].1, walk[from].2);
        let hunk = &walk[from..to];
        let old_len = hunk.iter().filter(|s| s.0 != Step::Added).count();
        let new_len = hunk.iter().filter(|s| s.0 != Step::Removed).count();
        // A side with no line in the hunk is placed after the line before it, as a unified diff places it.
        let start = |at: usize, len: usize| if len == 0 { at } else { at + 1 };
        out.push(format!(
            "@@ -{},{old_len} +{},{new_len} @@",
            start(old_at, old_len),
            start(new_at, new_len)
        ));
        for (step, _, _, line) in hunk {
            let mark = match step {
                Step::Same => ' ',
                Step::Removed => '-',
                Step::Added => '+',
            };
            out.push(format!("{mark}{line}"));
        }
    }
    Some(out.join("\n"))
}

/// The second text, rebuilt from the first and what changed from it to the second, as [`diff`] gives it; none where
/// the change does not fit the first.
pub fn apply(a: &str, change: &str) -> Option<String> {
    let old: Vec<&str> = a.split('\n').collect();
    let mut out: Vec<&str> = Vec::new();
    let mut at = 0;
    let mut lines = change.split('\n').peekable();
    if change.is_empty() {
        return Some(a.to_string());
    }
    while let Some(header) = lines.next() {
        let ranges = header.strip_prefix("@@ -")?.strip_suffix(" @@")?;
        let (old_range, _) = ranges.split_once(" +")?;
        let (start, len) = old_range.split_once(',')?;
        let (start, len): (usize, usize) = (start.parse().ok()?, len.parse().ok()?);
        let begins = if len == 0 { start } else { start.checked_sub(1)? };
        if begins < at || begins > old.len() {
            return None;
        }
        out.extend(&old[at..begins]);
        at = begins;
        while let Some(line) = lines.next_if(|l| !l.starts_with("@@ -")) {
            let (mark, text) = line.split_at(line.char_indices().nth(1).map_or(line.len(), |(i, _)| i));
            match mark {
                " " if old.get(at) == Some(&text) => {
                    out.push(text);
                    at += 1;
                }
                "-" if old.get(at) == Some(&text) => at += 1,
                "+" => out.push(text),
                _ => return None,
            }
        }
    }
    out.extend(&old[at..]);
    Some(out.join("\n"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Same,
    Removed,
    Added,
}

/// The walk from the first lines to the second: each step with how many lines of each it has passed before it, and
/// its line; none where what lies between what begins and ends them alike is too long to compare.
fn walk<'a>(a: &[&'a str], b: &[&'a str]) -> Option<Vec<(Step, usize, usize, &'a str)>> {
    let head = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let tail = (a[head..].iter().rev())
        .zip(b[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    if (am.len() + 1).saturating_mul(bm.len() + 1) > COMPARED {
        return None;
    }
    // The length of the longest run of lines the two middles have in common from each pair of places on.
    let width = bm.len() + 1;
    let mut common = vec![0u32; (am.len() + 1) * width];
    for i in (0..am.len()).rev() {
        for j in (0..bm.len()).rev() {
            common[i * width + j] = if am[i] == bm[j] {
                common[(i + 1) * width + j + 1] + 1
            } else {
                common[(i + 1) * width + j].max(common[i * width + j + 1])
            };
        }
    }
    let mut steps: Vec<(Step, usize, usize, &str)> = Vec::new();
    for (k, line) in a.iter().enumerate().take(head) {
        steps.push((Step::Same, k, k, line));
    }
    let (mut i, mut j) = (0, 0);
    while i < am.len() || j < bm.len() {
        let (x, y) = (head + i, head + j);
        if i < am.len() && j < bm.len() && am[i] == bm[j] {
            steps.push((Step::Same, x, y, am[i]));
            (i, j) = (i + 1, j + 1);
        } else if i < am.len() && (j == bm.len() || common[(i + 1) * width + j] >= common[i * width + j + 1]) {
            steps.push((Step::Removed, x, y, am[i]));
            i += 1;
        } else {
            steps.push((Step::Added, x, y, bm[j]));
            j += 1;
        }
    }
    for k in 0..tail {
        let (x, y) = (a.len() - tail + k, b.len() - tail + k);
        steps.push((Step::Same, x, y, a[x]));
    }
    Some(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_each_changed_line_with_two_on_each_side_in_hunks() {
        let a = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n";
        let b = "one\ntwo\nthree\nFOUR\nfive\nsix\nseven\neight\nnine\nten\neleven\n";
        assert_eq!(
            diff(a, b).unwrap(),
            "@@ -2,5 +2,5 @@\n two\n three\n-four\n+FOUR\n five\n six\n@@ -9,3 +9,4 @@\n nine\n ten\n+eleven\n "
        );
        assert_eq!(diff(a, a).unwrap(), "");
        assert_eq!(diff("", "new").unwrap(), "@@ -1,1 +1,1 @@\n-\n+new");
    }

    #[test]
    fn the_second_is_rebuilt_exactly_from_the_first_and_the_change() {
        let texts = [
            "",
            "\n",
            "a",
            "a\n",
            "a\nb\nc\nd\ne\nf\ng\nh\n",
            "a\nB\nc\nd\ne\nf\nG\nh",
            "x\na\nb\nc\nd\ne\nf\ng\nh\ny\n",
            "a\na\na\nb\na\na\n",
            "  indented\n+plus\n-minus\n@@ -1,1 +1,1 @@\n",
        ];
        for a in texts {
            for b in texts {
                let change = diff(a, b).unwrap();
                assert_eq!(apply(a, &change).as_deref(), Some(b), "{a:?} to {b:?}: {change}");
            }
        }
    }

    #[test]
    fn texts_too_long_to_compare_have_no_change() {
        let a = "a line\n".repeat(5000);
        let b = "another\n".repeat(5000);
        assert_eq!(diff(&a, &b), None);
        // What begins and ends them alike is not compared.
        let c = format!("{a}one more\n{a}");
        assert!(diff(&format!("{a}{a}"), &c).is_some());
    }
}
