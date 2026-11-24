//! The check of the design tree's forms on small trees made in memory, each in well under a millisecond. What a test
//! expects is taken from the rules of `flow/agents/tree_rules.md`, not from the check's code: each fault is named by its
//! file and line, as the rules say where it is, and a tree in the rules' forms has none. Each test is on a tree of its
//! own, so that its fault is the only one the tree has, and reads on its own what it catches.

use std::path::Path;

use super::*;

/// The files of the tree every test starts from, by their paths from the root of the repository: in the forms of the
/// rules, with principles in one file, decisions in a concept's file and in the file of a concept under it, a line of
/// each kind under a decision, names of entries in other files, and a file of data in the repository.
const TREE: [(&str, &str); 3] = [
    (
        "design/principles/common.md",
        "# Common\n\nWhat holds for every part of the system.\n\n\
         ### principle one-thing-one-place\n\
         Each thing is said in one place, and every other place points to it.\n\
         Why: Two copies of one thing drift apart, and a reader cannot tell which one holds.\n\n\
         ### principle state-from-records\n\
         The engine works out its state from its records, never from what it holds in memory.\n\
         Why: Records outlive a stopped engine, and what was only in memory is lost with it.\n",
    ),
    (
        "design/decisions/engine.md",
        "# Engine\n\nWhat is true of the engine as a whole.\n\n\
         ### decision the-engine-reads-every-record-at-start\n\
         The engine reads every record when it starts, as principle state-from-records asks.\n\
         - rejected: an agent proposed keeping the state in a cache file; the owner rejected it, because the cache \
         drifted from the records (data: `measurements/cache-drift.csv`).\n\
         - replaced: Reading only the last record was on main; the owner changed it, because a stop between two \
         writes lost a step.\n",
    ),
    (
        "design/decisions/engine/loop.md",
        "# Loop\n\nWhat is true of the engine's loop.\n\n\
         ### decision one-round-at-a-time\n\
         The loop runs one round of a work at a time, after decision the-engine-reads-every-record-at-start.\n\
         - rejected: the owner proposed running two rounds side by side; the owner rejected it, because both wrote \
         the same record.\n\
         - rejected: a reviewer agent proposed a lock on each record; the owner rejected it, because the lock added a \
         state no test reached.\n",
    ),
];

/// The paths of the repository that hold data, which the tests' trees may name.
const HELD: [&str; 1] = ["measurements/cache-drift.csv"];

/// The lines a test's own file opens with: its first entry is on line 5.
const OPENING: [&str; 4] = ["# Extra", "", "What the tests add to the tree.", ""];
const RULE: &str = "Each part is built so that it can be tested alone.";
const WHY: &str = "Why: A part tested only with the others hides the fault of each.";
const FIRST: &str = "The engine keeps one log for each work.";
const REJECTED: &str = "- rejected: an agent proposed one log for all works; the owner rejected it, because a work's log \
                        could not be read alone.";
const REPLACED: &str = "- replaced: A log for each round was on main; the owner changed it, because the logs of one \
                        work were read together.";

/// The format and the words of the tree's rules as this repository gives them.
fn rules() -> flow::agents::Tree {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    flow::agents::acts(&root).expect("the tree's rules are read").tree
}

fn principle(name: &str, rule: &str, why: &str) -> Vec<String> {
    vec![format!("### principle {name}"), rule.to_string(), why.to_string()]
}

fn principle_of(name: &str) -> Vec<String> {
    principle(name, RULE, WHY)
}

fn decision(name: &str, first: &str, under: &[&str]) -> Vec<String> {
    let mut lines = vec![format!("### decision {name}"), first.to_string()];
    lines.extend(under.iter().map(|line| line.to_string()));
    lines
}

fn decision_of(name: &str) -> Vec<String> {
    decision(name, FIRST, &[REJECTED])
}

/// The lines of a file that opens as the rules say, with these entries, one blank line between each two.
fn entries(found: &[Vec<String>]) -> Vec<String> {
    let mut lines: Vec<String> = OPENING.iter().map(|line| line.to_string()).collect();
    for (number, entry) in found.iter().enumerate() {
        if number > 0 {
            lines.push(String::new());
        }
        lines.extend(entry.iter().cloned());
    }
    lines
}

/// A line of exactly `n` characters: `start`, then `fill` repeated, then `end`.
fn sized(n: usize, start: &str, end: &str, fill: &str) -> String {
    let fills = n - start.chars().count() - end.chars().count();
    let line = format!("{start}{}{end}", fill.repeat(fills));
    assert_eq!(line.chars().count(), n);
    line
}

/// A file of the tree, named from the design directory, with each line ended by a line break.
fn file(name: &str, lines: &[String]) -> (String, String) {
    (
        format!("design/{name}"),
        lines.iter().map(|line| format!("{line}\n")).collect(),
    )
}

/// The faults of the tree the tests start from with these files added, each as its file named from the design
/// directory and its line, in the order they are told.
fn faults_with(files: &[(String, String)], held: &[&str]) -> Vec<(String, usize)> {
    found_with(files, held)
        .into_iter()
        .map(|fault| (fault.path.trim_start_matches("design/").to_string(), fault.line))
        .collect()
}

fn found_with(files: &[(String, String)], held: &[&str]) -> Vec<Fault> {
    let format = rules().format;
    let mut sources: Vec<Source> = (TREE.iter().map(|(path, text)| (path.to_string(), text.to_string())))
        .chain(files.iter().cloned())
        .map(|(path, text)| Source {
            kind: if path.starts_with("design/principles/") {
                "principle"
            } else {
                "decision"
            }
            .to_string(),
            path,
            text: Some(text),
        })
        .collect();
    sources.reverse();
    let mut held: Vec<&str> = held.to_vec();
    held.extend(HELD);
    check(&sources, &format, &held)
}

/// The tree has exactly these faults, each as a file named from the design directory and a line, in the order the tree
/// is read.
fn assert_faults(files: &[(String, String)], expected: &[(&str, usize)]) {
    let expected: Vec<(String, usize)> = expected.iter().map(|(name, line)| (name.to_string(), *line)).collect();
    assert_eq!(faults_with(files, &[]), expected);
}

fn assert_no_faults(files: &[(String, String)]) {
    assert_faults(files, &[]);
}

#[test]
fn a_tree_in_the_rules_forms_has_no_fault() {
    assert_no_faults(&[]);
}

#[test]
fn every_fault_is_found_not_only_the_first_and_in_the_order_the_tree_is_read() {
    let long = principle("spoken-words", &sized(301, "Each part ", ".", "a"), WHY);
    let against = REJECTED.replace("one log", "one log, against principle no-such-rule,");
    let files = [
        // The decisions are given first: the order of the tree puts the principles before them.
        file(
            "decisions/extra.md",
            &entries(&[decision("Spoken-Log", FIRST, &[&against])]),
        ),
        file("principles/extra.md", &entries(&[long])),
    ];
    assert_faults(
        &files,
        &[
            ("principles/extra.md", 6),
            ("decisions/extra.md", 5),
            ("decisions/extra.md", 7),
        ],
    );
}

#[test]
fn a_file_that_is_not_text_is_a_fault_at_its_first_line() {
    let format = rules().format;
    let sources = [Source {
        path: "design/decisions/binary.md".to_string(),
        kind: "decision".to_string(),
        text: None,
    }];
    let faults = check(&sources, &format, &[]);
    assert_eq!(faults.len(), 1);
    assert_eq!((faults[0].line, &faults[0].what), (1, &What::NotText));
}

// Each file has a title and one line saying what its concept covers, before its entries.
#[test]
fn a_file_that_does_not_open_with_its_title_a_blank_line_and_one_line_of_what_it_covers_is_a_fault() {
    let entry = principle_of("spoken-words");
    let mut no_title = vec!["What the tests add to the tree.".to_string(), String::new()];
    no_title.extend(entry.clone());
    let mut not_a_title = ["Extra", "", "What the tests add to the tree.", ""]
        .map(String::from)
        .to_vec();
    not_a_title.extend(entry.clone());
    let mut no_cover = vec!["# Extra".to_string(), String::new()];
    no_cover.extend(entry.clone());
    for (case, lines) in [
        ("no title", no_title),
        ("not a title", not_a_title),
        ("no line of what it covers", no_cover),
    ] {
        let faults = faults_with(&[file("principles/extra.md", &lines)], &[]);
        assert_eq!(faults, [("principles/extra.md".to_string(), 1)], "{case}");
    }
}

#[test]
fn a_line_of_what_a_file_covers_written_as_two_lines_is_a_fault_at_the_fourth() {
    let mut lines: Vec<String> = ["# Extra", "", "What the tests add", "to the tree.", ""]
        .map(String::from)
        .to_vec();
    lines.extend(decision_of("spoken-log"));
    assert_faults(&[file("decisions/extra.md", &lines)], &[("decisions/extra.md", 4)]);
}

// One blank line between entries, and none at the end of a file.
#[test]
fn two_blank_lines_between_entries_are_a_fault_at_the_second_blank_line() {
    let mut lines = entries(&[principle_of("spoken-words")]);
    lines.extend(["".to_string(), "".to_string()]);
    lines.extend(principle_of("written-words"));
    assert_faults(&[file("principles/extra.md", &lines)], &[("principles/extra.md", 9)]);
}

#[test]
fn entries_with_no_blank_line_between_them_are_one_entry_and_a_fault() {
    let mut principles = entries(&[principle_of("spoken-words")]);
    principles.extend(principle_of("written-words"));
    // The two are one entry of six lines: it is not a rule and a reason, at its rule, and its second heading line, which
    // no blank line parts from the entry, is a line of it that names an entry no file holds.
    let faults = faults_with(&[file("principles/extra.md", &principles)], &[]);
    let expected = [6, 8].map(|line| ("principles/extra.md".to_string(), line));
    assert_eq!(faults, expected);
    let mut decisions = entries(&[decision_of("spoken-log")]);
    decisions.extend(decision_of("written-log"));
    let faults = faults_with(&[file("decisions/extra.md", &decisions)], &[]);
    assert!(
        !faults.is_empty()
            && faults
                .iter()
                .all(|(name, line)| name == "decisions/extra.md" && (5..=10).contains(line))
    );
}

#[test]
fn a_blank_line_ending_a_file_is_a_fault_at_it() {
    let mut lines = entries(&[principle_of("spoken-words")]);
    lines.push(String::new());
    let last = lines.len();
    assert_faults(&[file("principles/extra.md", &lines)], &[("principles/extra.md", last)]);
}

// An entry opens with `### principle <name>` in principles/ and `### decision <name>` in decisions/.
#[test]
fn an_entry_of_the_other_kind_is_a_fault_at_its_heading() {
    let files = [file("principles/extra.md", &entries(&[decision_of("spoken-log")]))];
    assert_faults(&files, &[("principles/extra.md", 5)]);
    let files = [file("decisions/extra.md", &entries(&[principle_of("spoken-words")]))];
    assert_faults(&files, &[("decisions/extra.md", 5)]);
}

#[test]
fn a_heading_of_another_level_or_with_no_name_is_a_fault() {
    for heading in [
        "## principle spoken-words",
        "### principle",
        "### principle spoken words",
        "### principle spoken-words ",
    ] {
        let entry = vec![heading.to_string(), RULE.to_string(), WHY.to_string()];
        assert_faults(
            &[file("principles/extra.md", &entries(&[entry]))],
            &[("principles/extra.md", 5)],
        );
    }
}

// A name is lowercase words joined by hyphens, and no two entries in the tree have the same name.
#[test]
fn lowercase_words_joined_by_hyphens_are_a_name_and_any_other_is_a_fault() {
    let files = [file(
        "principles/extra.md",
        &entries(&[principle_of("spoken-words"), principle_of("a-cell-knows-only-its-data")]),
    )];
    assert_no_faults(&files);
    for name in [
        "Spoken-Words",
        "spoken_words",
        "spoken--words",
        "-spoken-words",
        "spoken-words-",
        "spoken",
    ] {
        let files = [file("principles/extra.md", &entries(&[principle_of(name)]))];
        assert_faults(&files, &[("principles/extra.md", 5)]);
    }
}

#[test]
fn an_entry_whose_name_another_entry_has_is_a_fault_at_the_later_one_whichever_kind_they_are() {
    let files = [file(
        "principles/extra.md",
        &entries(&[principle_of("state-from-records")]),
    )];
    assert_faults(&files, &[("principles/extra.md", 5)]);
    let files = [file(
        "principles/extra.md",
        &entries(&[principle_of("spoken-words"), principle_of("spoken-words")]),
    )];
    assert_faults(&files, &[("principles/extra.md", 9)]);
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision_of("one-round-at-a-time")]),
    )];
    assert_faults(&files, &[("decisions/extra.md", 5)]);
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision_of("one-thing-one-place")]),
    )];
    assert_faults(&files, &[("decisions/extra.md", 5)]);
    let fault = &found_with(&files, &[])[0];
    assert_eq!(
        fault.what,
        What::NameTaken {
            file: "design/principles/common.md".to_string(),
            line: 5
        }
    );
}

// A principle is its rule on one line of at most 300 characters, then `Why: ` and its reason on one line of at most 300.
#[test]
fn a_principle_not_of_a_rule_line_and_a_why_line_is_a_fault_at_its_rule() {
    let cases: [(&str, Vec<&str>); 5] = [
        ("no reason", vec![RULE]),
        (
            "the rule on two lines",
            vec!["Each part is built so that", "it can be tested alone.", WHY],
        ),
        ("the reason first", vec![WHY, RULE]),
        (
            "a reason without `Why: `",
            vec![
                RULE,
                "Because a part tested only with the others hides the fault of each.",
            ],
        ),
        (
            "the reason on two lines",
            vec![
                RULE,
                "Why: A part tested only with the others",
                "hides the fault of each.",
            ],
        ),
    ];
    for (case, body) in cases {
        let mut entry = vec!["### principle spoken-words".to_string()];
        entry.extend(body.iter().map(|line| line.to_string()));
        let faults = faults_with(&[file("principles/extra.md", &entries(&[entry]))], &[]);
        assert_eq!(faults, [("principles/extra.md".to_string(), 6)], "{case}");
    }
}

#[test]
fn an_empty_reason_is_a_fault_at_the_reason() {
    let entry = principle("spoken-words", RULE, "Why: ");
    assert_faults(
        &[file("principles/extra.md", &entries(&[entry]))],
        &[("principles/extra.md", 7)],
    );
}

#[test]
fn a_rule_and_a_reason_of_300_characters_hold_and_of_301_are_a_fault_each_at_its_line() {
    for fill in ["a", "é"] {
        let rule = sized(300, "Each part ", ".", fill);
        let why = format!("Why: {}", sized(300, "A part ", ".", fill));
        assert_no_faults(&[file(
            "principles/extra.md",
            &entries(&[principle("spoken-words", &rule, &why)]),
        )]);
    }
    let rule = sized(301, "Each part ", ".", "a");
    assert_faults(
        &[file(
            "principles/extra.md",
            &entries(&[principle("spoken-words", &rule, WHY)]),
        )],
        &[("principles/extra.md", 6)],
    );
    let why = format!("Why: {}", sized(301, "A part ", ".", "a"));
    assert_faults(
        &[file(
            "principles/extra.md",
            &entries(&[principle("spoken-words", RULE, &why)]),
        )],
        &[("principles/extra.md", 7)],
    );
}

// A decision's first line says what is true now, in at most 200 characters.
#[test]
fn a_first_line_of_200_characters_holds_and_of_201_is_a_fault() {
    let first = sized(200, "The engine ", ".", "a");
    assert_no_faults(&[file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", &first, &[REJECTED])]),
    )]);
    let first = sized(201, "The engine ", ".", "a");
    assert_faults(
        &[file(
            "decisions/extra.md",
            &entries(&[decision("spoken-log", &first, &[REJECTED])]),
        )],
        &[("decisions/extra.md", 6)],
    );
}

#[test]
fn a_first_line_that_is_a_line_of_the_list_is_a_fault() {
    let entry = vec![
        "### decision spoken-log".to_string(),
        REJECTED.to_string(),
        REPLACED.to_string(),
    ];
    assert_faults(
        &[file("decisions/extra.md", &entries(&[entry]))],
        &[("decisions/extra.md", 6)],
    );
}

// Under the first line come one or more lines, each at most 300 characters, each in the exact form of a `rejected` or a
// `replaced` line.
#[test]
fn a_decision_with_no_line_under_its_first_is_a_fault() {
    let entry = decision("spoken-log", FIRST, &[]);
    assert_faults(
        &[file("decisions/extra.md", &entries(&[entry]))],
        &[("decisions/extra.md", 6)],
    );
}

#[test]
fn the_owner_an_agent_and_a_reviewer_agent_may_propose() {
    for who in ["the owner", "an agent", "a reviewer agent"] {
        let line = REJECTED.replace("an agent proposed", &format!("{who} proposed"));
        assert_no_faults(&[file(
            "decisions/extra.md",
            &entries(&[decision("spoken-log", FIRST, &[&line, REPLACED])]),
        )]);
    }
}

#[test]
fn a_line_not_in_the_exact_form_is_a_fault_at_it() {
    let approach = "one log for all works";
    let because = "because a work's log could not be read alone.";
    let lines = [
        format!("- rejected: a user proposed {approach}; the owner rejected it, {because}"),
        format!("- rejected: an agent proposed {approach}; an agent rejected it, {because}"),
        format!("- rejected: an agent proposed {approach}; the owner rejected it."),
        format!(
            "- rejected: an agent proposed {approach}; the owner rejected it, {}",
            &because[..because.len() - 1]
        ),
        format!("- rejected: an agent suggested {approach}; the owner rejected it, {because}"),
        format!("- Rejected: an agent proposed {approach}; the owner rejected it, {because}"),
        format!("rejected: an agent proposed {approach}; the owner rejected it, {because}"),
        format!("* rejected: an agent proposed {approach}; the owner rejected it, {because}"),
        format!("- replaced: {approach} was on main; a reviewer agent changed it, {because}"),
        format!("- replaced: {approach} was tried; the owner changed it, {because}"),
        format!("- replaced: {approach} was on main; the owner changed it."),
        format!("- deferred: an agent proposed {approach}; the owner put it off, {because}"),
        "and the engine reads the log when the work ends.".to_string(),
    ];
    for line in lines {
        let files = [file(
            "decisions/extra.md",
            &entries(&[decision("spoken-log", FIRST, &[REJECTED, &line])]),
        )];
        assert_faults(&files, &[("decisions/extra.md", 8)]);
    }
}

#[test]
fn the_first_line_carried_onto_a_second_line_is_a_fault_at_the_second() {
    let entry = decision("spoken-log", FIRST, &["and reads it when the work ends.", REJECTED]);
    assert_faults(
        &[file("decisions/extra.md", &entries(&[entry]))],
        &[("decisions/extra.md", 7)],
    );
}

#[test]
fn a_line_of_300_characters_holds_and_of_301_is_a_fault() {
    let ends = [
        "; the owner rejected it, because it was slower.",
        " was on main; the owner changed it, because it was slower.",
    ];
    let starts = ["- rejected: an agent proposed ", "- replaced: "];
    for (start, end) in starts.into_iter().zip(ends) {
        let line = sized(300, start, end, "a");
        assert_no_faults(&[file(
            "decisions/extra.md",
            &entries(&[decision("spoken-log", FIRST, &[&line])]),
        )]);
        let line = sized(301, start, end, "a");
        let files = [file(
            "decisions/extra.md",
            &entries(&[decision("spoken-log", FIRST, &[&line])]),
        )];
        assert_faults(&files, &[("decisions/extra.md", 7)]);
    }
}

// A decision names no work, item or date: the file's history keeps those.
#[test]
fn a_decision_naming_a_work_an_item_or_a_date_is_a_fault_at_the_line_that_names_it() {
    let cases: [(&str, Vec<String>, usize); 7] = [
        ("a work", vec![REJECTED.replace("could not", "in W10 could not")], 7),
        ("an item", vec![REJECTED.replace("could not", "of #367 could not")], 7),
        (
            "a date",
            vec![REJECTED.replace("could not", "could not, on 2026-11-08,")],
            7,
        ),
        (
            "a date in a replaced line",
            vec![REPLACED.replace("were read", "were read on 2026-11-08")],
            7,
        ),
        (
            "a work in the second line under",
            vec![REJECTED.to_string(), REPLACED.replace("one work", "W10")],
            8,
        ),
        (
            "a work by its word",
            vec![REJECTED.replace("could not", "in work 11 could not")],
            7,
        ),
        (
            "an item by its word",
            vec![REJECTED.replace("could not", "of item 5 could not")],
            7,
        ),
    ];
    for (case, under, line) in cases {
        let under: Vec<&str> = under.iter().map(String::as_str).collect();
        let files = [file(
            "decisions/extra.md",
            &entries(&[decision("spoken-log", FIRST, &under)]),
        )];
        let faults = faults_with(&files, &[]);
        assert_eq!(faults, [("decisions/extra.md".to_string(), line)], "{case}");
    }
}

#[test]
fn a_first_line_naming_a_date_is_a_fault() {
    let first = "The engine keeps one log for each work since 2026-11-08.";
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", first, &[REJECTED])]),
    )];
    assert_faults(&files, &[("decisions/extra.md", 6)]);
}

#[test]
fn what_names_a_moment_is_told_as_the_words_it_matched() {
    let line = REJECTED.replace("could not", "of item #5 could not");
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &[&line])]),
    )];
    assert_eq!(
        found_with(&files, &[])[0].what,
        What::NamesAMoment {
            what: "item #5".to_string()
        }
    );
    for (line, what) in [
        ("in W10 here", Some("W10")),
        ("in W10a here", None),
        ("in AW10 here", None),
        ("of #12345 here", Some("#12345")),
        ("of #123456 here", None),
        ("on 2026-11-08, here", Some("2026-11-08")),
        ("on 12026-11-08 here", None),
        ("in Work 4 here", Some("Work 4")),
        ("in homework 4 here", None),
    ] {
        assert_eq!(moment(line).as_deref(), what, "{line}");
    }
}

#[test]
fn other_numbers_name_no_work_item_or_date() {
    let line = "- rejected: an agent proposed one log for all works; the owner rejected it, because Article 3 asks for \
                it, a round cost 300 tokens, and the page's grey is #333333.";
    assert_no_faults(&[file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &[line])]),
    )]);
}

// An entry is named as "principle <name>" or "decision <name>", and the entry named is in the tree, of that kind.
#[test]
fn entries_named_from_other_files_hold() {
    let first = "The engine keeps one log for each work, as principle one-thing-one-place asks.";
    let line = REJECTED.replace("alone.", "alone, after decision one-round-at-a-time.");
    assert_no_faults(&[file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", first, &[&line])]),
    )]);
}

#[test]
fn a_named_entry_no_file_holds_is_a_fault_at_the_line_that_names_it() {
    let line = REJECTED.replace("one log", "one log, against principle no-such-rule,");
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &[&line])]),
    )];
    assert_faults(&files, &[("decisions/extra.md", 7)]);
    assert_eq!(
        found_with(&files, &[])[0].what,
        What::Unknown {
            kind: "principle".to_string(),
            name: "no-such-rule".to_string()
        }
    );
    let why = "Why: A part tested only with the others hides the fault of each, as decision no-such-choice found.";
    let files = [file(
        "principles/extra.md",
        &entries(&[principle("spoken-words", RULE, why)]),
    )];
    assert_faults(&files, &[("principles/extra.md", 7)]);
}

#[test]
fn an_entry_named_as_the_other_kind_is_a_fault() {
    let line = REJECTED.replace("one log", "one log, against decision state-from-records,");
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &[&line])]),
    )];
    assert_faults(&files, &[("decisions/extra.md", 7)]);
    assert_eq!(
        found_with(&files, &[])[0].what,
        What::WrongKind {
            kind: "decision".to_string(),
            name: "state-from-records".to_string(),
            is: "principle".to_string()
        }
    );
    let first = "The engine keeps one log for each work, as principle one-round-at-a-time asks.";
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", first, &[REJECTED])]),
    )];
    assert_faults(&files, &[("decisions/extra.md", 6)]);
}

#[test]
fn a_name_that_runs_into_a_word_character_is_named_as_far_as_a_boundary_allows() {
    let kinds = ["principle".to_string(), "decision".to_string()];
    let named = |line: &str| named(line, &kinds);
    let both = |kind: &str, name: &str| (kind.to_string(), name.to_string());
    assert_eq!(named("see principle a-b-cX here"), [both("principle", "a-b")]);
    assert!(named("see principle a-bX here").is_empty());
    assert_eq!(
        named("see principle a-b-, and decision c-d."),
        [both("principle", "a-b"), both("decision", "c-d")]
    );
    assert!(named("see xprinciple a-b here").is_empty());
    assert!(named("see principle a here").is_empty());
}

// A reason that rests on data ends with the repository file that holds it, as "(data: `<path>`)".
#[test]
fn data_the_repository_holds_holds_and_data_it_lacks_is_a_fault_at_its_line() {
    let line = |path: &str| REJECTED.replace("alone.", &format!("alone (data: `{path}`)."));
    let held = ["measurements/rounds.csv"];
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &[&line("measurements/rounds.csv")])]),
    )];
    assert!(faults_with(&files, &held).is_empty());
    assert_eq!(faults_with(&files, &[]), [("decisions/extra.md".to_string(), 7)]);
    assert_eq!(
        found_with(&files, &[])[0].what,
        What::DataMissing {
            data: "measurements/rounds.csv".to_string()
        }
    );
}

#[test]
fn the_path_of_data_is_taken_from_the_repositorys_root() {
    let line = |path: &str| REJECTED.replace("alone.", &format!("alone (data: `{path}`)."));
    let held = ["design/rounds.csv"];
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &[&line("rounds.csv")])]),
    )];
    assert_eq!(faults_with(&files, &held), [("decisions/extra.md".to_string(), 7)]);
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &[&line("design/rounds.csv")])]),
    )];
    assert!(faults_with(&files, &held).is_empty());
}

#[test]
fn the_data_a_tree_names_is_each_path_once_in_the_order_it_is_read() {
    let line = |path: &str| REJECTED.replace("alone.", &format!("alone (data: `{path}`)."));
    let named = [line("b.csv"), line("a.csv"), line("b.csv")];
    let under: Vec<&str> = named.iter().map(String::as_str).collect();
    let files = [file(
        "decisions/extra.md",
        &entries(&[decision("spoken-log", FIRST, &under)]),
    )];
    let sources: Vec<Source> = (files.iter())
        .map(|(path, text)| Source {
            path: path.clone(),
            kind: "decision".to_string(),
            text: Some(text.clone()),
        })
        .collect();
    assert_eq!(read(&sources, &rules().format).data(), ["b.csv", "a.csv"]);
}

// The numbers of the rules' words are the check's: the words an agent reads and the lengths the check holds it to are
// two statements of one limit, and are kept alike here.
#[test]
fn the_limits_the_check_holds_are_the_ones_the_rules_text_gives() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/agents/tree_rules.md");
    let text = std::fs::read_to_string(root).expect("the rules are read");
    for (what, most) in [
        ("rule", RULE_MOST),
        ("reason", WHY_MOST),
        ("first", FIRST_LINE_MOST),
        ("line", LINE_MOST),
    ] {
        let said = format!("at most {most} characters");
        assert!(text.contains(&said), "the rules say {said} of the {what}");
    }
}

// Each fault is told in the flow's words, with every place of them filled and none left open.
#[test]
fn each_fault_is_told_in_words_whose_places_are_all_filled() {
    let rules = rules();
    let text = |s: &str| s.to_string();
    let all = [
        What::NotText,
        What::Opens,
        What::Covers,
        What::BlankLines,
        What::Heading {
            dir: text("principles/"),
            heading: text("### principle <name>"),
        },
        What::NameShape,
        What::PrincipleForm,
        What::RuleLong { count: 301, most: 300 },
        What::WhyEmpty,
        What::WhyLong { count: 301, most: 300 },
        What::DecisionFirst,
        What::FirstLong { count: 201, most: 200 },
        What::NoLines,
        What::LineForm,
        What::LineLong { count: 301, most: 300 },
        What::NamesAMoment { what: text("W10") },
        What::DataMissing { data: text("a/b.csv") },
        What::NameTaken {
            file: text("design/a.md"),
            line: 5,
        },
        What::Unknown {
            kind: text("principle"),
            name: text("no-such-rule"),
        },
        What::WrongKind {
            kind: text("decision"),
            name: text("a-b"),
            is: text("principle"),
        },
    ];
    for what in all {
        let fault = Fault {
            path: text("design/decisions/a.md"),
            line: 7,
            entry: Some((text("decision"), text("keep-it"))),
            what,
        };
        let lines = words(std::slice::from_ref(&fault), &rules.faults);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].starts_with("design/decisions/a.md:7: decision keep-it: ") && !lines[0].contains(['{', '}']),
            "{}",
            lines[0]
        );
        let alone = words(&[Fault { entry: None, ..fault }], &rules.faults);
        assert!(alone[0].starts_with("design/decisions/a.md:7: ") && !alone[0].contains("decision keep-it"));
    }
    let said = words(
        &[Fault {
            path: text("design/principles/a.md"),
            line: 6,
            entry: Some((text("principle"), text("spoken-words"))),
            what: What::RuleLong { count: 301, most: 300 },
        }],
        &rules.faults,
    );
    assert_eq!(
        said,
        ["design/principles/a.md:6: principle spoken-words: the rule is 301 characters, over 300"]
    );
}

// This repository's own tree is in the forms of its rules, as the files are on disk. Edits of the tree made outside the
// flow meet no check of the engine's, so this is the one that keeps them in the forms. It reads the files of
// `design/principles` and `design/decisions`, about sixty, and holds a data path by its being a file or directory of
// the repository: a few milliseconds alone, and it grows with the tree; it cannot be smaller, since a fault may be in
// any file or in how any two of them name each other.
#[test]
fn this_repository_s_own_design_tree_is_in_the_forms_of_its_rules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rules = flow::agents::acts(&root.join("flow"))
        .expect("the tree's rules are read")
        .tree;
    let mut sources = Vec::new();
    for kind in &rules.format.kinds {
        let dir = Path::new("design").join(&kind.dir);
        let mut paths = Vec::new();
        markdown_under(&root, &dir, &mut paths);
        assert!(!paths.is_empty(), "the tree holds files of the kind {}", kind.kind);
        for path in paths {
            let text = std::fs::read_to_string(root.join(&path)).ok();
            sources.push(Source {
                path: path.to_string_lossy().replace('\\', "/"),
                kind: kind.kind.clone(),
                text,
            });
        }
    }
    let faults = read(&sources, &rules.format).finish(&|path| root.join(path).exists());
    assert_eq!(words(&faults, &rules.faults), Vec::<String>::new());
}

/// The markdown files under a directory of the repository, by their paths from its root.
fn markdown_under(root: &Path, dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(root.join(dir)).expect("the directory is read") {
        let path = dir.join(entry.expect("an entry").file_name());
        if root.join(&path).is_dir() {
            markdown_under(root, &path, into);
        } else if path.extension().is_some_and(|e| e == "md") {
            into.push(path);
        }
    }
}
