//! The forms of the design tree's files, as the tree's rules write them: what a machine can tell of the files under
//! each kind's directory, checked on their texts alone. Each file opens with its title, a blank line and one line of
//! what its concept covers; its entries are one blank line apart; each entry opens with its kind's heading and a name
//! of lowercase words joined by hyphens, and holds the lines of its kind: a principle its rule and its reason, a
//! decision its first line and a line for each approach not taken, none of them longer than the limits; no line of a
//! decision names a work, an item or a date; no name is taken twice; each entry another names is in the tree, of the
//! kind it is named as; and each file a reason names for its data is held. Whether a line says what is true is for the
//! one who reads it.
//!
//! The check finds every fault in one pass ([`read`] and [`Read::finish`]), as faults of plain values; their words are
//! the flow's (`agents/tree.toml`), which [`words`] fills in. The texts are read from git by the caller
//! (`super::forms`), so that the check itself reads nothing.

use std::collections::BTreeMap;

use flow::agents::{TreeFaults, TreeFormat, fill};

/// The most characters a principle's rule may hold.
pub(crate) const RULE_MOST: usize = 300;
/// The most characters the reason after `Why: ` may hold.
pub(crate) const WHY_MOST: usize = 300;
/// The most characters a decision's first line may hold.
pub(crate) const FIRST_LINE_MOST: usize = 200;
/// The most characters any other line of a decision may hold.
pub(crate) const LINE_MOST: usize = 300;

/// The kinds of entry the check has the form of by their names (`flow::agents::FORMED_KINDS`).
const PRINCIPLE: &str = "principle";
const DECISION: &str = "decision";
/// What a principle's reason begins with.
const WHY: &str = "Why: ";

/// A file of the tree: its path from the root of the repository, the kind of the directory it is under, and its text,
/// none where it is not text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub path: String,
    pub kind: String,
    pub text: Option<String>,
}

/// What is wrong, with what its words need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum What {
    NotText,
    Opens,
    Covers,
    BlankLines,
    /// An entry that does not begin as its directory's kind does: the directory, and the heading as it is written.
    Heading {
        dir: String,
        heading: String,
    },
    NameShape,
    PrincipleForm,
    RuleLong {
        count: usize,
        most: usize,
    },
    WhyEmpty,
    WhyLong {
        count: usize,
        most: usize,
    },
    DecisionFirst,
    FirstLong {
        count: usize,
        most: usize,
    },
    NoLines,
    LineForm,
    LineLong {
        count: usize,
        most: usize,
    },
    NamesAMoment {
        what: String,
    },
    DataMissing {
        data: String,
    },
    NameTaken {
        file: String,
        line: usize,
    },
    Unknown {
        kind: String,
        name: String,
    },
    WrongKind {
        kind: String,
        name: String,
        is: String,
    },
}

/// A fault: the file and the line it is at, the kind and the name of the entry it is in where it is in one, and what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fault {
    pub path: String,
    pub line: usize,
    pub entry: Option<(String, String)>,
    pub what: What,
}

/// An entry whose heading was read: where it is and its lines after the heading, with the faults found in it.
struct Entry {
    kind: String,
    name: String,
    file: String,
    first: usize,
    body: Vec<String>,
    faults: Vec<(usize, What)>,
}

impl Entry {
    fn fault(&mut self, line: usize, what: What) {
        self.faults.push((line, what));
    }
}

/// The files read: the faults of their own lines, and their entries.
pub(crate) struct Read {
    /// Where a file comes in the order the tree is read: its kind's place in the format, then its path by its parts.
    order: BTreeMap<String, (usize, Vec<String>)>,
    faults: Vec<Fault>,
    entries: Vec<Entry>,
    kinds: Vec<String>,
}

/// Reads the files of the tree by the format's kinds, finding the faults in each file's own lines and in each entry by
/// itself.
pub(crate) fn read(sources: &[Source], format: &TreeFormat) -> Read {
    let kinds: Vec<String> = format.kinds.iter().map(|k| k.kind.clone()).collect();
    let mut files: Vec<(&Source, (usize, Vec<String>))> = sources
        .iter()
        .map(|source| {
            let place = kinds.iter().position(|k| *k == source.kind).unwrap_or(kinds.len());
            (source, (place, parts(&source.path)))
        })
        .collect();
    files.sort_by(|a, b| a.1.cmp(&b.1));
    let mut read = Read {
        order: BTreeMap::new(),
        faults: Vec::new(),
        entries: Vec::new(),
        kinds,
    };
    for (source, place) in files {
        read.order.insert(source.path.clone(), place);
        read.file(source, format);
    }
    read
}

/// A path by its parts, the last without its suffix, so that a concept's file comes before the files of the concepts
/// under it.
fn parts(path: &str) -> Vec<String> {
    let mut parts: Vec<String> = path.split('/').map(str::to_string).collect();
    if let Some(last) = parts.last_mut()
        && let Some((stem, _)) = last.rsplit_once('.')
        && !stem.is_empty()
    {
        *last = stem.to_string();
    }
    parts
}

impl Read {
    fn file(&mut self, source: &Source, format: &TreeFormat) {
        let path = &source.path;
        let file_fault = |faults: &mut Vec<Fault>, line: usize, what: What| {
            faults.push(Fault {
                path: path.clone(),
                line,
                entry: None,
                what,
            })
        };
        let Some(text) = &source.text else {
            file_fault(&mut self.faults, 1, What::NotText);
            return;
        };
        let lines: Vec<&str> = text.lines().collect();
        // Where a file does not open as it should, its entries are read from its first heading on.
        let first_heading = || {
            lines
                .iter()
                .position(|line| line.starts_with("### "))
                .unwrap_or(lines.len())
        };
        let mut skip = 4;
        if lines.len() < 3
            || !lines[0].starts_with("# ")
            || !lines[1].is_empty()
            || lines[2].is_empty()
            || lines[2].starts_with('#')
        {
            file_fault(&mut self.faults, 1, What::Opens);
            skip = first_heading();
        } else if lines.len() > 3 && !lines[3].is_empty() {
            file_fault(&mut self.faults, 4, What::Covers);
            skip = first_heading();
        }
        let (entries, extra) = blocks(&lines, skip);
        for number in extra {
            file_fault(&mut self.faults, number, What::BlankLines);
        }
        let dir = format
            .kinds
            .iter()
            .find(|k| k.kind == source.kind)
            .map(|k| k.dir.clone())
            .unwrap_or_default();
        for (first, block) in entries {
            let named = (block[0] == block[0].trim_end())
                .then(|| format.heading_of(block[0]))
                .flatten()
                .filter(|(kind, _)| *kind == source.kind);
            let Some((kind, name)) = named else {
                let heading = format
                    .heading
                    .replace("{kind}", &source.kind)
                    .replace("{name}", "<name>");
                let what = What::Heading {
                    dir: dir.clone(),
                    heading,
                };
                file_fault(&mut self.faults, first, what);
                continue;
            };
            let mut entry = Entry {
                kind: kind.to_string(),
                name: name.to_string(),
                file: path.clone(),
                first,
                body: block[1..].iter().map(|line| line.to_string()).collect(),
                faults: Vec::new(),
            };
            if !is_a_name(&entry.name) {
                entry.fault(first, What::NameShape);
            }
            match entry.kind.as_str() {
                PRINCIPLE => check_principle(&mut entry),
                DECISION => check_decision(&mut entry),
                _ => {}
            }
            self.entries.push(entry);
        }
    }

    /// Each path the entries give as their data, once, in the order the tree is read.
    pub(crate) fn data(&self) -> Vec<String> {
        let mut data: Vec<String> = Vec::new();
        for entry in &self.entries {
            for line in &entry.body {
                for path in data_of(line) {
                    if !data.contains(&path) {
                        data.push(path);
                    }
                }
            }
        }
        data
    }

    /// Every fault found, as the tree is read: those of each file's own lines and of each entry by itself, then, with
    /// the data that is held, the data that is not, the names that are taken and the entries named that are not there.
    pub(crate) fn finish(mut self, holds: &dyn Fn(&str) -> bool) -> Vec<Fault> {
        for entry in &mut self.entries {
            let body = entry.body.clone();
            for (n, line) in body.iter().enumerate() {
                for data in data_of(line) {
                    if !holds(&data) {
                        entry.fault(entry.first + 1 + n, What::DataMissing { data });
                    }
                }
            }
        }
        // The entry that has a name first, in the order the tree is read.
        let mut names: BTreeMap<String, (String, String, usize)> = BTreeMap::new();
        for entry in &mut self.entries {
            match names.get(&entry.name).cloned() {
                Some((_, file, line)) => entry.fault(entry.first, What::NameTaken { file, line }),
                None => {
                    names.insert(
                        entry.name.clone(),
                        (entry.kind.clone(), entry.file.clone(), entry.first),
                    );
                }
            }
        }
        for entry in &mut self.entries {
            let body = entry.body.clone();
            for (n, line) in body.iter().enumerate() {
                for (kind, name) in named(line, &self.kinds) {
                    let line = entry.first + 1 + n;
                    match names.get(&name) {
                        None => entry.fault(line, What::Unknown { kind, name }),
                        Some((is, ..)) if *is != kind => {
                            let is = is.clone();
                            entry.fault(line, What::WrongKind { kind, name, is })
                        }
                        Some(_) => {}
                    }
                }
            }
        }
        let mut faults = self.faults;
        for entry in self.entries {
            for (line, what) in entry.faults {
                faults.push(Fault {
                    path: entry.file.clone(),
                    line,
                    entry: Some((entry.kind.clone(), entry.name.clone())),
                    what,
                });
            }
        }
        let order = self.order;
        faults.sort_by(|a, b| (order.get(&a.path), a.line).cmp(&(order.get(&b.path), b.line)));
        faults
    }
}

/// The faults of the files, where each path in `held` is one the repository holds.
#[cfg(test)]
pub(crate) fn check(sources: &[Source], format: &TreeFormat, held: &[&str]) -> Vec<Fault> {
    read(sources, format).finish(&|path| held.contains(&path))
}

/// The entries of the lines after the first `skip`, each as the number of its first line and its lines, and the numbers
/// of the blank lines too many.
fn blocks<'a>(lines: &[&'a str], skip: usize) -> (Vec<(usize, Vec<&'a str>)>, Vec<usize>) {
    let (mut found, mut extra, mut block, mut start) = (Vec::new(), Vec::new(), Vec::new(), 0);
    for (index, line) in lines.iter().enumerate().skip(skip) {
        let number = index + 1;
        if !line.is_empty() {
            if block.is_empty() {
                start = number;
            }
            block.push(*line);
        } else if !block.is_empty() {
            found.push((start, std::mem::take(&mut block)));
        } else {
            extra.push(number);
        }
    }
    if !block.is_empty() {
        found.push((start, block));
    } else if lines.len() > skip {
        extra.push(lines.len());
    }
    (found, extra)
}

/// Whether a name is two or more words of lowercase letters and digits joined by hyphens.
fn is_a_name(name: &str) -> bool {
    let words: Vec<&str> = name.split('-').collect();
    words.len() >= 2
        && words
            .iter()
            .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

fn check_principle(entry: &mut Entry) {
    let first = entry.first;
    let body = entry.body.clone();
    if body.len() != 2 || body[0].starts_with(WHY) || !body[1].starts_with(WHY) {
        entry.fault(first + 1, What::PrincipleForm);
        return;
    }
    let rule = body[0].chars().count();
    if rule > RULE_MOST {
        entry.fault(
            first + 1,
            What::RuleLong {
                count: rule,
                most: RULE_MOST,
            },
        );
    }
    let why = &body[1][WHY.len()..];
    if why.trim().is_empty() {
        entry.fault(first + 2, What::WhyEmpty);
    }
    let count = why.chars().count();
    if count > WHY_MOST {
        entry.fault(first + 2, What::WhyLong { count, most: WHY_MOST });
    }
}

fn check_decision(entry: &mut Entry) {
    let first = entry.first;
    let body = entry.body.clone();
    if body.is_empty() || body[0].starts_with("- ") {
        entry.fault(first + 1, What::DecisionFirst);
        return;
    }
    let count = body[0].chars().count();
    if count > FIRST_LINE_MOST {
        entry.fault(
            first + 1,
            What::FirstLong {
                count,
                most: FIRST_LINE_MOST,
            },
        );
    }
    if body.len() == 1 {
        entry.fault(first + 1, What::NoLines);
    }
    for (n, line) in body.iter().enumerate().skip(1) {
        let number = first + 1 + n;
        if !(rejected(line) || replaced(line)) {
            entry.fault(number, What::LineForm);
        }
        let count = line.chars().count();
        if count > LINE_MOST {
            entry.fault(number, What::LineLong { count, most: LINE_MOST });
        }
    }
    for (n, line) in body.iter().enumerate() {
        if let Some(what) = moment(line) {
            entry.fault(first + 1 + n, What::NamesAMoment { what });
        }
    }
}

/// Whether a line is `- rejected: <who> proposed <approach>; the owner rejected it, because <reason>.`, where who is the
/// owner, an agent or a reviewer agent.
fn rejected(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("- rejected: ") else {
        return false;
    };
    ["the owner", "an agent", "a reviewer agent"].iter().any(|who| {
        (rest.strip_prefix(who).and_then(|r| r.strip_prefix(" proposed ")))
            .is_some_and(|r| joined(r, "; the owner rejected it, because "))
    })
}

/// Whether a line is `- replaced: <approach> was on main; the owner changed it, because <reason>.`
fn replaced(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("- replaced: ") else {
        return false;
    };
    joined(rest, " was on main; the owner changed it, because ")
}

/// Whether the text is something, then `between`, then something that ends with a full stop and holds a character
/// before it: some place of `between` in it makes it so.
fn joined(text: &str, between: &str) -> bool {
    text.match_indices(between)
        .any(|(at, _)| at >= 1 && text[at + between.len()..].len() >= 2 && text.ends_with('.'))
}

/// Whether a character is one a word is made of, as a word boundary is judged.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the text begins a word at its start, taking `before` as the character before it.
fn boundary_before(before: Option<char>) -> bool {
    !before.is_some_and(is_word)
}

/// The digits that begin the text, and what follows them.
fn digits(text: &str) -> (usize, &str) {
    let count = text.chars().take_while(char::is_ascii_digit).count();
    (count, &text[count..])
}

/// The first place in a line that names a work (`W12`, `work 12`), an item (`#123`, `item 12`) or a date
/// (`2026-01-31`), as the words it matches.
fn moment(line: &str) -> Option<String> {
    let mut before: Option<char> = None;
    for (at, c) in line.char_indices() {
        let rest = &line[at..];
        let found = matched(rest, boundary_before(before));
        if let Some(length) = found {
            return Some(rest[..length].to_string());
        }
        before = Some(c);
    }
    None
}

/// The length of the match that begins the text, where one does: `at_start` says that a word may begin here.
fn matched(rest: &str, at_start: bool) -> Option<usize> {
    let ends = |after: &str| after.chars().next().is_none_or(|c| !is_word(c));
    if at_start {
        // `W` and digits.
        if let Some(after) = rest.strip_prefix('W') {
            let (count, tail) = digits(after);
            if count >= 1 && ends(tail) {
                return Some(rest.len() - tail.len());
            }
        }
        // `work ` and digits, with either case of its first letter.
        for head in ["Work ", "work "] {
            if let Some(after) = rest.strip_prefix(head) {
                let (count, tail) = digits(after);
                if count >= 1 && ends(tail) {
                    return Some(rest.len() - tail.len());
                }
            }
        }
    }
    // `#` and up to five digits.
    if let Some(after) = rest.strip_prefix('#') {
        let (count, tail) = digits(after);
        if (1..=5).contains(&count) && ends(tail) {
            return Some(rest.len() - tail.len());
        }
    }
    if at_start {
        for head in ["Item ", "item "] {
            if let Some(after) = rest.strip_prefix(head) {
                let after = after.strip_prefix('#').unwrap_or(after);
                let (count, tail) = digits(after);
                if count >= 1 && ends(tail) {
                    return Some(rest.len() - tail.len());
                }
            }
        }
        // A date, `0000-00-00`.
        let date: Vec<char> = rest.chars().take(10).collect();
        let shape = "dddd-dd-dd";
        if date.len() == 10
            && date.iter().zip(shape.chars()).all(|(c, s)| match s {
                'd' => c.is_ascii_digit(),
                _ => *c == s,
            })
            && ends(&rest[10..])
        {
            return Some(10);
        }
    }
    None
}

/// Each path a line gives as its data, as `(data: `path`)`.
fn data_of(line: &str) -> Vec<String> {
    const OPEN: &str = "(data: `";
    let mut paths = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find(OPEN).map(|a| a + from) {
        let start = at + OPEN.len();
        match line[start..].find('`') {
            Some(length) if length >= 1 && line[start + length + 1..].starts_with(')') => {
                paths.push(line[start..start + length].to_string());
                from = start + length + 2;
            }
            _ => from = at + 1,
        }
    }
    paths
}

/// Each entry a line names, as `<kind> <name>`, in the order it does, for the kinds the tree has.
fn named(line: &str, kinds: &[String]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut before: Option<char> = None;
    let mut skip_to = 0;
    for (at, c) in line.char_indices() {
        if at >= skip_to && boundary_before(before) {
            let rest = &line[at..];
            let hit = kinds.iter().find_map(|kind| {
                let after = rest.strip_prefix(kind.as_str())?.strip_prefix(' ')?;
                let name = name_at(after)?;
                Some((kind.clone(), name.to_string(), kind.len() + 1 + name.len()))
            });
            if let Some((kind, name, length)) = hit {
                found.push((kind, name));
                skip_to = at + length;
            }
        }
        before = Some(c);
    }
    found
}

/// The name that begins the text, as the longest run of two or more words of lowercase letters and digits joined by
/// hyphens that a word boundary follows, where one does.
fn name_at(text: &str) -> Option<&str> {
    let mut ends: Vec<usize> = Vec::new();
    let mut length = 0;
    let mut rest = text;
    loop {
        let word = rest
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            .count();
        if word == 0 {
            break;
        }
        length += word;
        rest = &rest[word..];
        ends.push(length);
        match rest.strip_prefix('-') {
            Some(after) => {
                length += 1;
                rest = after;
            }
            None => break,
        }
    }
    // The fewest words is two; each shorter run than the longest is followed by a hyphen, which is not a word.
    (1..ends.len()).rev().find_map(|n| {
        let end = ends[n];
        let after = text[end..].chars().next();
        (n < ends.len() - 1 || after.is_none_or(|c| !is_word(c))).then_some(&text[..end])
    })
}

/// Each fault as a line of what a round is told, in the flow's words: `<file>:<line>: <entry><what>`.
pub(crate) fn words(faults: &[Fault], words: &TreeFaults) -> Vec<String> {
    faults
        .iter()
        .map(|fault| {
            let entry = match &fault.entry {
                Some((kind, name)) => fill(&words.entry, &[("kind", kind), ("name", name)]),
                None => String::new(),
            };
            let line = fault.line.to_string();
            let text = what(&fault.what, words);
            fill(
                &words.line,
                &[
                    ("path", &fault.path),
                    ("line", &line),
                    ("entry", &entry),
                    ("text", &text),
                ],
            )
        })
        .collect()
}

/// The words of what is wrong.
fn what(what: &What, w: &TreeFaults) -> String {
    let n = |n: &usize| n.to_string();
    match what {
        What::NotText => w.not_text.clone(),
        What::Opens => w.opens.clone(),
        What::Covers => w.covers.clone(),
        What::BlankLines => w.blank_lines.clone(),
        What::Heading { dir, heading } => fill(&w.heading, &[("dir", dir), ("heading", heading)]),
        What::NameShape => w.name_shape.clone(),
        What::PrincipleForm => w.principle_form.clone(),
        What::RuleLong { count, most } => fill(&w.rule_long, &[("count", &n(count)), ("most", &n(most))]),
        What::WhyEmpty => w.why_empty.clone(),
        What::WhyLong { count, most } => fill(&w.why_long, &[("count", &n(count)), ("most", &n(most))]),
        What::DecisionFirst => w.decision_first.clone(),
        What::FirstLong { count, most } => fill(&w.first_long, &[("count", &n(count)), ("most", &n(most))]),
        What::NoLines => w.no_lines.clone(),
        What::LineForm => w.line_form.clone(),
        What::LineLong { count, most } => fill(&w.line_long, &[("count", &n(count)), ("most", &n(most))]),
        What::NamesAMoment { what } => fill(&w.names_a_moment, &[("what", what)]),
        What::DataMissing { data } => fill(&w.data_missing, &[("data", data)]),
        What::NameTaken { file, line } => fill(&w.name_taken, &[("file", file), ("line", &n(line))]),
        What::Unknown { kind, name } => fill(&w.unknown, &[("kind", kind), ("name", name)]),
        What::WrongKind { kind, name, is } => fill(&w.wrong_kind, &[("kind", kind), ("name", name), ("is", is)]),
    }
}

#[cfg(test)]
mod tests;
