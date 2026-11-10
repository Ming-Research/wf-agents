//! The tool `read`: what an agent reads of its work's blocks, answered from the work as it is kept (decision
//! `agent-reads-what-it-needs`). Its input names what to read, several things at once, each apart from the next by a
//! space or a line: `<block>` for the current version, `<block>@<version>` for that version, and
//! `<block>@<a>..<b>` for what differs between two versions, as a plain diff of their lines. Each is answered with its
//! version, who wrote it, its value where it is a choice, and its content; what cannot be found is said so, and the
//! rest is answered all the same. It is a function of the flow, the work and the input, with no I/O.

use decide::{BlockId, Seq, VersionView};
use flow::Compiled;
use work::Work;

use crate::message::{version, writer};

/// The name of the tool.
pub const READ: &str = "read";

const NOTHING_NAMED: &str = "The input names nothing to read: name blocks, as `plan`, `plan@12` or `plan@12..15`.";
fn no_block(name: &str) -> String {
    format!("### `{name}`: not found: the flow has no block `{name}`.")
}
fn no_version(name: &str, block: &BlockId, at: u64) -> String {
    format!("### `{name}`: not found: `{block}` has no version {at}.")
}
fn not_a_name(name: &str) -> String {
    format!("### `{name}`: not found: a name is `<block>`, `<block>@<version>` or `<block>@<a>..<b>`.")
}
fn not_written(name: &str) -> String {
    format!("### `{name}`: not written yet.")
}
fn heading(block: &BlockId, at: Seq, by: &str, value: Option<&str>) -> String {
    let value = value.map(|v| format!(", with the value `{v}`")).unwrap_or_default();
    format!("### `{block}`, version {}, written by {by}{value}", at.0)
}
fn diff_heading(block: &BlockId, from: Seq, to: Seq) -> String {
    format!(
        "### `{block}`, what differs from version {} to version {}: `-` lines are only in the first, `+` lines only in \
         the second",
        from.0, to.0
    )
}
fn too_long(block: &BlockId, from: Seq, to: Seq) -> String {
    format!(
        "### `{block}`, version {} and version {}: too long to compare line by line, so both are given whole",
        from.0, to.0
    )
}
const SAME: &str = "(The two versions are the same.)";
/// How many lines each of two versions may have to be compared line by line: the comparison takes memory as the
/// product of the two, and past this each is given whole.
pub const COMPARED: usize = 4000;
const ELIDED: &str = "…";
/// How many lines that are the same are shown on each side of a change.
const CONTEXT: usize = 2;

/// What the tool answers to this input.
pub fn answer(flow: &Compiled, work: &Work, input: &str) -> String {
    let versions = decide::versions(&work.records);
    let names: Vec<&str> = input.split_whitespace().collect();
    if names.is_empty() {
        return NOTHING_NAMED.to_string();
    }
    let parts: Vec<String> = names.iter().map(|name| one(flow, work, &versions, name)).collect();
    let mut text = parts.join("\n\n");
    text.push('\n');
    text
}

/// What is answered for one name.
fn one(
    flow: &Compiled,
    work: &Work,
    versions: &std::collections::BTreeMap<BlockId, Vec<VersionView>>,
    name: &str,
) -> String {
    let (block, at) = match name.split_once('@') {
        Some((block, at)) => (block, Some(at)),
        None => (name, None),
    };
    let block = BlockId::from(block);
    if !flow.blocks.contains_key(&block) {
        return no_block(block.as_str());
    }
    let find = |at: &str| -> Result<&VersionView, String> {
        let number: u64 = at.parse().map_err(|_| not_a_name(name))?;
        version(versions, &block, Seq(number)).ok_or_else(|| no_version(name, &block, number))
    };
    let shown = |v: &VersionView| {
        let by = writer(flow, work, &block, Some(v));
        let head = heading(&block, v.at, &by, v.value.as_deref());
        format!("{head}\n\n{}", content(work, &block, v.at).trim_end())
    };
    match at {
        None => match versions.get(&block).and_then(|vs| vs.iter().max_by_key(|v| v.at)) {
            Some(v) => shown(v),
            None => not_written(name),
        },
        Some(at) => match at.split_once("..") {
            None => find(at).map_or_else(|why| why, shown),
            Some((from, to)) => match (find(from), find(to)) {
                (Ok(from), Ok(to)) => {
                    let (a, b) = (content(work, &block, from.at), content(work, &block, to.at));
                    if a.lines().count() > COMPARED || b.lines().count() > COMPARED {
                        format!(
                            "{}\n\n{}\n\n{}",
                            too_long(&block, from.at, to.at),
                            shown(from),
                            shown(to)
                        )
                    } else {
                        format!("{}\n\n{}", diff_heading(&block, from.at, to.at), diff(a, b))
                    }
                }
                (Err(why), _) | (_, Err(why)) => why,
            },
        },
    }
}

/// The content of the version.
fn content<'a>(work: &'a Work, block: &BlockId, at: Seq) -> &'a str {
    work.contents.get(&(block.clone(), at)).map_or("", String::as_str)
}

/// What differs between two texts, line by line: each line only in the first with `- `, each only in the second with
/// `+ `, and up to [`CONTEXT`] lines that are the same on each side of a change with two spaces; the same lines further
/// from a change are left out, [`ELIDED`] standing for them.
pub fn diff(a: &str, b: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    // The length of the longest run of lines the two have in common from each pair of places on.
    let mut common = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            common[i][j] = if a[i] == b[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    // Each line of the walk: '=' the same in both, '-' only in the first, '+' only in the second.
    let mut walk: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            walk.push(('=', a[i]));
            (i, j) = (i + 1, j + 1);
        } else if i < a.len() && (j == b.len() || common[i + 1][j] >= common[i][j + 1]) {
            walk.push(('-', a[i]));
            i += 1;
        } else {
            walk.push(('+', b[j]));
            j += 1;
        }
    }
    if walk.iter().all(|(kind, _)| *kind == '=') {
        return SAME.to_string();
    }
    let near = |k: usize| {
        let from = k.saturating_sub(CONTEXT);
        let to = (k + CONTEXT + 1).min(walk.len());
        walk[from..to].iter().any(|(kind, _)| *kind != '=')
    };
    let mut lines = Vec::new();
    let mut elided = false;
    for (k, (kind, line)) in walk.iter().enumerate() {
        match kind {
            '=' if !near(k) => {
                if !elided {
                    lines.push(ELIDED.to_string());
                    elided = true;
                }
                continue;
            }
            '=' => lines.push(format!("  {line}")),
            '-' => lines.push(format!("- {line}")),
            _ => lines.push(format!("+ {line}")),
        }
        elided = false;
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::tests::{Small, objected, objected_twice};

    #[test]
    fn a_diff_shows_what_changed_with_two_lines_around_it() {
        let a = "one\ntwo\nthree\nfour\nfive\nsix\nseven\n";
        let b = "one\ntwo\nthree\nFOUR\nfive\nsix\nseven\neight\n";
        assert_eq!(
            diff(a, b),
            "…\n  two\n  three\n- four\n+ FOUR\n  five\n  six\n  seven\n+ eight"
        );
        assert_eq!(diff(a, a), SAME);
        assert_eq!(diff("", "new\n"), "+ new");
    }

    #[test]
    fn several_blocks_are_read_at_once_each_with_its_version_and_writer() {
        let small = objected();
        let (request, plan, opinions) = (small.request(), small.wrote("plan"), small.wrote("review"));
        let expected = format!(
            "### `request`, version {request}, written by a person

Greet the user.

### `plan`, version {plan}, written by the cell `plan`

Greet by name.

### `verdict`, version {opinions}, written by the cell `review`, with the value `no`

Not yet.
"
        );
        assert_eq!(answer(&small.flow, &small.work, "request\nplan verdict"), expected);
    }

    #[test]
    fn a_version_and_a_difference_between_two_are_read() {
        let (mut small, _, first, _) = objected_twice();
        small.hands_in(&[
            ("opinions", "Name the error.\nAnd say it.", None),
            ("verdict", "Not yet.", Some("no")),
        ]);
        let second = small.wrote("review");
        let earlier = answer(&small.flow, &small.work, &format!("opinions@{first}"));
        assert!(earlier.ends_with("\n\nName the error.\n"), "{earlier}");
        let differs = answer(&small.flow, &small.work, &format!("opinions@{first}..{second}"));
        let heading = format!("### `opinions`, what differs from version {first} to version {second}");
        assert!(differs.starts_with(&heading), "{differs}");
        assert!(differs.ends_with("\n\n  Name the error.\n+ And say it.\n"), "{differs}");
    }

    #[test]
    fn versions_too_long_to_compare_are_given_whole() {
        let mut small = objected();
        let long = "a line\n".repeat(COMPARED + 1);
        small.hands_in(&[("code", long.as_str(), None)]);
        let (first, second) = (small.work.records.finished[1].at.0, small.wrote("build"));
        let said = answer(&small.flow, &small.work, &format!("code@{first}..{second}"));
        let heading = format!("### `code`, version {first} and version {second}: too long to compare");
        assert!(said.starts_with(&heading), "{}", &said[..200]);
        assert!(
            said.contains("\n\nA greeting.\n\n### `code`, version"),
            "the first is given whole"
        );
        assert!(said.ends_with(&long), "the second is given whole");
    }

    #[test]
    fn what_cannot_be_found_is_said_so_and_the_rest_answered() {
        let small = Small::new();
        let said = answer(&small.flow, &small.work, "nothing plan request@999 request@x request");
        let parts: Vec<&str> = said.split("\n\n### ").collect();
        assert_eq!(parts[0], "### `nothing`: not found: the flow has no block `nothing`.");
        assert_eq!(parts[1], "`plan`: not written yet.");
        assert_eq!(parts[2], "`request@999`: not found: `request` has no version 999.");
        assert!(parts[3].starts_with("`request@x`: not found: a name is"), "{said}");
        assert!(parts[4].ends_with("Greet the user.\n"), "{said}");
        assert_eq!(answer(&small.flow, &small.work, " "), NOTHING_NAMED);
    }
}
