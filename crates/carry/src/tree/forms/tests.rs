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

// What the script this check replaced (`design/lint.py` at the commit before this work, still in the repository while
// the installed engine needs it) says of the same files, one line for each fault as it printed them: each case below
// is a tree that script was run on, and what a case expects is the script's output on it, line for line, not what the
// port is thought to do. Each case is a small tree in memory; all of them together take well under a millisecond.

const HEAD: &str = "# T\n\nCovers.\n\n";
const REJ: &str = "- rejected: an agent proposed x; the owner rejected it, because y.";
const OPENS: &str = "a file opens with `# <title>`, a blank line and one line of what it covers";
const BLANK: &str = "two entries are parted by one blank line, and no blank line ends the file";
const NOT_A_LINE: &str = "not a line in the form of a `rejected` or a `replaced` line";
const NOT_A_HEADING: &str = "an entry of decisions/ opens with `### decision <name>`";

fn dec(name: &str, first: &str, under: &[&str]) -> String {
    let lines: String = under.iter().map(|line| format!("{line}\n")).collect();
    format!("### decision {name}\n{first}\n{lines}")
}

fn prin(name: &str) -> String {
    format!("### principle {name}\nRule one.\nWhy: because.\n")
}

/// A line the script printed, of a file named from the design directory.
fn said(file: &str, line: usize, text: &str) -> String {
    format!("design/{file}:{line}: {text}")
}

/// What the check says of exactly these files, named from the design directory, where the repository holds exactly
/// `held` besides them, each as a line of its words.
fn told_of(files: &[(&str, String)], held: &[&str]) -> Vec<String> {
    let rules = rules();
    let sources: Vec<Source> = files
        .iter()
        .map(|(path, text)| Source {
            path: format!("design/{path}"),
            kind: if path.starts_with("principles/") {
                "principle"
            } else {
                "decision"
            }
            .to_string(),
            text: Some(text.clone()),
        })
        .collect();
    words(&check(&sources, &rules.format, held), &rules.faults)
}

/// A case: its name, its files by their paths from the design directory with their texts, the paths the repository holds
/// besides, and the lines the script printed.
type Case<'a> = (&'a str, Vec<(&'a str, String)>, Vec<&'a str>, Vec<String>);

/// Each case whose told lines are not the script's, with both, so that one run shows every difference.
fn assert_as_the_script_said(cases: Vec<Case>) {
    let mut differing = Vec::new();
    for (name, files, held, expected) in cases {
        let told = told_of(&files, &held);
        if told != expected {
            differing.push(format!("{name}:\n  told:     {told:#?}\n  expected: {expected:#?}"));
        }
    }
    assert!(differing.is_empty(), "{}", differing.join("\n"));
}

#[test]
fn the_lines_of_a_decision_are_in_form_where_the_script_said_so() {
    let case = |name, lines: &[&str], expected: Vec<usize>| {
        let told: Vec<String> = expected
            .iter()
            .map(|n| said("decisions/b.md", *n, &format!("decision three-four: {NOT_A_LINE}")))
            .collect();
        let text = format!("{HEAD}{}", dec("three-four", "It holds.", lines));
        (name, vec![("decisions/b.md", text)], Vec::new(), told)
    };
    assert_as_the_script_said(vec![
        case(
            "a reason of one full stop and one more",
            &["- rejected: an agent proposed x; the owner rejected it, because .."],
            vec![],
        ),
        case(
            "a reason of a full stop alone",
            &["- rejected: an agent proposed x; the owner rejected it, because ."],
            vec![7],
        ),
        case(
            "a reason with no full stop",
            &["- rejected: an agent proposed x; the owner rejected it, because y"],
            vec![7],
        ),
        case(
            "a full stop and then a space",
            &["- rejected: an agent proposed x; the owner rejected it, because y. "],
            vec![7],
        ),
        case(
            "an approach of no characters",
            &["- rejected: an agent proposed ; the owner rejected it, because y."],
            vec![7],
        ),
        case(
            "an approach of one space",
            &["- rejected: an agent proposed  ; the owner rejected it, because y."],
            vec![],
        ),
        case(
            "the words between twice",
            &["- rejected: an agent proposed a; the owner rejected it, because ; the owner rejected it, because y."],
            vec![],
        ),
        case(
            "who proposed, not of the three",
            &[
                "- rejected: the reviewer proposed x; the owner rejected it, because y.",
                "- rejected: a reviewer agent proposed x; the owner rejected it, because y.",
                "- rejected: an agentx proposed x; the owner rejected it, because y.",
            ],
            vec![7, 9],
        ),
        case(
            "a replaced line with nothing before or after its words",
            &[
                "- replaced:  was on main; the owner changed it, because y.",
                "- replaced: x was on main; the owner changed it, because .",
                "- replaced: x was on main; the owner changed it, because yy",
            ],
            vec![7, 8, 9],
        ),
        case(
            "a replaced line with no approach",
            &[
                "- replaced: was on main; the owner changed it, because y.",
                "- replaced:x was on main; the owner changed it, because y.",
            ],
            vec![7, 8],
        ),
    ]);
}

#[test]
fn the_opening_of_a_file_and_the_blank_lines_in_it_are_as_the_script_said() {
    let one = dec("three-four", "It holds.", &[REJ]);
    let two = dec("one-three", "It holds.", &[REJ]);
    let only = |text: String| vec![("decisions/b.md", text)];
    let crlf = |text: String| text.replace('\n', "\r\n");
    assert_as_the_script_said(vec![
        (
            "a tree in the line breaks of a system that ends each line with two characters",
            vec![
                ("principles/a.md", crlf(format!("{HEAD}{}", prin("one-two")))),
                ("decisions/b.md", crlf(format!("{HEAD}{one}"))),
            ],
            vec![],
            vec![],
        ),
        (
            "a mark of byte order before the title",
            only(format!("\u{feff}{HEAD}{one}")),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "a file of nothing",
            only(String::new()),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "a title and no more",
            only("# T\n".to_string()),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "a title and a line of what it covers, no entry",
            only("# T\n\nCovers.".to_string()),
            vec![],
            vec![],
        ),
        (
            "the same ended by a line break",
            only("# T\n\nCovers.\n".to_string()),
            vec![],
            vec![],
        ),
        ("the same and a blank line", only(HEAD.to_string()), vec![], vec![]),
        (
            "a line of what it covers that is a heading",
            only(format!("# T\n\n# Covers\n\n{one}")),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "a title with no space after its mark",
            only(format!("#T\n\nCovers.\n\n{one}")),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "a line of what it covers that begins with a mark",
            only(format!("# T\n\n#x\n\n{one}")),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "a second line of spaces",
            only(format!("# T\n \nCovers.\n\n{one}")),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "no title, and two entries after what it covers",
            only(format!(
                "T\n\nCovers.\n\n{one}\n{}",
                dec("five-six", "It holds.", &[REJ])
            )),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "no title and no entry",
            only("T\nCovers.\n\nplain text\n".to_string()),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
        (
            "two blank lines after what a file covers",
            only(format!("# T\n\nCovers.\n\n\n{one}")),
            vec![],
            vec![said("decisions/b.md", 5, BLANK)],
        ),
        (
            "two blank lines between entries",
            only(format!("{HEAD}{one}\n\n{two}")),
            vec![],
            vec![said("decisions/b.md", 9, BLANK)],
        ),
        (
            "three blank lines at the end, the last said twice as the script said it",
            only(format!("{HEAD}{one}\n\n\n")),
            vec![],
            vec![
                said("decisions/b.md", 9, BLANK),
                said("decisions/b.md", 10, BLANK),
                said("decisions/b.md", 10, BLANK),
            ],
        ),
        (
            "a line of spaces between entries is no blank line",
            only(format!("{HEAD}{one}  \n{two}")),
            vec![],
            vec![
                said("decisions/b.md", 8, &format!("decision three-four: {NOT_A_LINE}")),
                said("decisions/b.md", 9, &format!("decision three-four: {NOT_A_LINE}")),
                said(
                    "decisions/b.md",
                    9,
                    "decision three-four: names decision one-three, which no file holds",
                ),
                said("decisions/b.md", 10, &format!("decision three-four: {NOT_A_LINE}")),
            ],
        ),
        (
            "no line break at the end of the last line",
            only(format!("{HEAD}{one}").trim_end_matches('\n').to_string()),
            vec![],
            vec![],
        ),
        (
            "a line break of two characters inside an entry",
            only(format!("{HEAD}### decision three-four\nIt holds.\r\n{REJ}\r\n")),
            vec![],
            vec![],
        ),
    ]);
}

#[test]
fn headings_are_read_as_the_script_read_them() {
    let after = |heading: &str| format!("{HEAD}{heading}\nIt holds.\n{REJ}\n");
    let only = |text: String| vec![("decisions/b.md", text)];
    let not_a_name = |name: &str| {
        said(
            "decisions/b.md",
            5,
            &format!("decision {name}: the name is not lowercase words joined by hyphens"),
        )
    };
    let heading = said("decisions/b.md", 5, NOT_A_HEADING);
    assert_as_the_script_said(vec![
        (
            "a tab in the heading",
            only(after("### decision\tthree-four")),
            vec![],
            vec![heading.clone()],
        ),
        (
            "a no-break space at the end of the heading",
            only(after("### decision three-four\u{a0}")),
            vec![],
            vec![heading.clone()],
        ),
        (
            "a space at the end of the heading",
            only(after("### decision three-four ")),
            vec![],
            vec![heading.clone()],
        ),
        (
            "a heading of another case",
            only(after("### Decision three-four")),
            vec![],
            vec![heading.clone()],
        ),
        (
            "a heading of four marks",
            only(after("#### decision three-four")),
            vec![],
            vec![heading.clone()],
        ),
        (
            "a heading with no name",
            only(after("### decision ")),
            vec![],
            vec![heading.clone()],
        ),
        (
            "a name of one word",
            only(after("### decision three")),
            vec![],
            vec![not_a_name("three")],
        ),
        (
            "a name of two hyphens in a row",
            only(after("### decision three--four")),
            vec![],
            vec![not_a_name("three--four")],
        ),
        (
            "a name that ends with a hyphen",
            only(after("### decision three-four-")),
            vec![],
            vec![not_a_name("three-four-")],
        ),
        (
            "a name of an upper case letter",
            only(after("### decision Three-4")),
            vec![],
            vec![not_a_name("Three-4")],
        ),
        (
            "an entry that opens in the middle of another",
            only(format!(
                "{HEAD}### decision one-two\nIt holds.\n### decision three-four\nIt holds.\n{REJ}\n"
            )),
            vec![],
            vec![
                said("decisions/b.md", 7, &format!("decision one-two: {NOT_A_LINE}")),
                said(
                    "decisions/b.md",
                    7,
                    "decision one-two: names decision three-four, which no file holds",
                ),
                said("decisions/b.md", 8, &format!("decision one-two: {NOT_A_LINE}")),
            ],
        ),
        (
            "a heading before the line of what the file covers",
            only(format!("# T\n\n{}", dec("one-two", "It holds.", &[REJ]))),
            vec![],
            vec![said("decisions/b.md", 1, OPENS)],
        ),
    ]);
}

#[test]
fn the_names_taken_and_named_are_judged_as_the_script_judged_them() {
    let at = |file: &str, line: usize, text: &str| said(file, line, text);
    let first = |name: &str| dec(name, "It holds.", &[REJ]);
    assert_as_the_script_said(vec![
        (
            "a name taken by the later of two files, as the files are ordered by their parts and not their paths",
            vec![
                ("decisions/a-b.md", format!("{HEAD}{}", first("same-name"))),
                ("decisions/a/x.md", format!("{HEAD}{}", first("same-name"))),
                (
                    "decisions/a.md",
                    format!("{HEAD}{}", dec("other-name", "See decision same-name.", &[REJ])),
                ),
            ],
            vec![],
            vec![at(
                "decisions/a-b.md",
                5,
                "decision same-name: the name is taken by design/decisions/a/x.md:5",
            )],
        ),
        (
            "a name a principle has and two decisions take",
            vec![
                ("decisions/b.md", format!("{HEAD}{}", first("same-name"))),
                ("decisions/a.md", format!("{HEAD}{}", first("same-name"))),
                ("principles/z.md", format!("{HEAD}{}", prin("same-name"))),
            ],
            vec![],
            vec![
                at(
                    "decisions/a.md",
                    5,
                    "decision same-name: the name is taken by design/principles/z.md:5",
                ),
                at(
                    "decisions/b.md",
                    5,
                    "decision same-name: the name is taken by design/principles/z.md:5",
                ),
            ],
        ),
        (
            "a name taken in the files of a concept, under it and beside it",
            vec![
                ("decisions/a/b.md", format!("{HEAD}{}", first("same-name"))),
                ("decisions/a.md", format!("{HEAD}{}", first("same-name"))),
                ("decisions/a-.md", format!("{HEAD}{}", first("same-name"))),
            ],
            vec![],
            vec![
                at(
                    "decisions/a/b.md",
                    5,
                    "decision same-name: the name is taken by design/decisions/a.md:5",
                ),
                at(
                    "decisions/a-.md",
                    5,
                    "decision same-name: the name is taken by design/decisions/a.md:5",
                ),
            ],
        ),
        (
            "a name taken in one file",
            vec![(
                "decisions/b.md",
                format!("{HEAD}{}\n{}", first("one-two"), first("one-two")),
            )],
            vec![],
            vec![at(
                "decisions/b.md",
                9,
                "decision one-two: the name is taken by design/decisions/b.md:5",
            )],
        ),
        (
            "a decision that takes a principle's name and names itself as a decision",
            vec![
                ("principles/a.md", format!("{HEAD}{}", prin("one-two"))),
                (
                    "decisions/b.md",
                    format!(
                        "{HEAD}{}",
                        dec("one-two", "See principle one-two and decision one-two.", &[REJ])
                    ),
                ),
            ],
            vec![],
            vec![
                at(
                    "decisions/b.md",
                    5,
                    "decision one-two: the name is taken by design/principles/a.md:5",
                ),
                at(
                    "decisions/b.md",
                    6,
                    "decision one-two: names decision one-two, which is a principle",
                ),
            ],
        ),
        (
            "a line that names one of the wrong kind, one of its own wrong kind and one no file holds",
            vec![
                ("principles/p.md", format!("{HEAD}{}", prin("one-two"))),
                (
                    "decisions/d.md",
                    format!(
                        "{HEAD}{}",
                        dec(
                            "three-four",
                            "decision one-two, principle three-four, principle none-here.",
                            &[REJ]
                        )
                    ),
                ),
            ],
            vec![],
            vec![
                at(
                    "decisions/d.md",
                    6,
                    "decision three-four: names decision one-two, which is a principle",
                ),
                at(
                    "decisions/d.md",
                    6,
                    "decision three-four: names principle three-four, which is a decision",
                ),
                at(
                    "decisions/d.md",
                    6,
                    "decision three-four: names principle none-here, which no file holds",
                ),
            ],
        ),
        (
            "names that end in a hyphen, a longer name, a word run together and two spaces",
            vec![
                ("principles/a.md", format!("{HEAD}{}", prin("one-two"))),
                (
                    "decisions/b.md",
                    format!(
                        "{HEAD}{}",
                        dec(
                            "three-four",
                            "Per principle one-two, principle one-two-, decision three-four; principle one-two-three. \
                             principleone-two decision  three-four.",
                            &[REJ]
                        )
                    ),
                ),
            ],
            vec![],
            vec![at(
                "decisions/b.md",
                6,
                "decision three-four: names principle one-two-three, which no file holds",
            )],
        ),
        (
            "a name run into an underscore, a full stop and a bracket",
            vec![(
                "decisions/b.md",
                format!(
                    "{HEAD}{}",
                    dec(
                        "three-four",
                        "Per principle one-two_x and principle one-two.x and (principle one-two)",
                        &[REJ]
                    )
                ),
            )],
            vec![],
            vec![
                at(
                    "decisions/b.md",
                    6,
                    "decision three-four: names principle one-two, which no file holds",
                ),
                at(
                    "decisions/b.md",
                    6,
                    "decision three-four: names principle one-two, which no file holds",
                ),
            ],
        ),
        (
            "names that begin with a digit",
            vec![
                ("principles/a.md", format!("{HEAD}{}", prin("1-2"))),
                (
                    "decisions/b.md",
                    format!(
                        "{HEAD}{}",
                        dec("three-four", "Per principle 1-2 and principle 1-2-x.", &[REJ])
                    ),
                ),
            ],
            vec![],
            vec![at(
                "decisions/b.md",
                6,
                "decision three-four: names principle 1-2-x, which no file holds",
            )],
        ),
        (
            "a name that runs into a capital letter, and one that runs into a hyphen and a letter that is no word of a name",
            vec![(
                "decisions/b.md",
                format!(
                    "{HEAD}{}",
                    dec(
                        "three-four",
                        "See principle foo-bar\u{c9} and decision foo-bar-\u{e9}.",
                        &[REJ]
                    )
                ),
            )],
            vec![],
            vec![at(
                "decisions/b.md",
                6,
                "decision three-four: names decision foo-bar, which no file holds",
            )],
        ),
        (
            "names in marks that are no word characters, and kinds run into a word",
            vec![
                ("principles/p.md", format!("{HEAD}{}", prin("one-two"))),
                (
                    "decisions/d.md",
                    format!(
                        "{HEAD}{}",
                        dec(
                            "three-four",
                            "Per `principle one-two`, \"decision three-four\", *principle one-two*, principle one-two's \
                             and x1principle one-two and 9decision three-four.",
                            &[REJ]
                        )
                    ),
                ),
            ],
            vec![],
            vec![],
        ),
        (
            "names in the lines of a principle",
            vec![(
                "principles/p.md",
                format!("{HEAD}### principle one-two\nSee decision nowhere-here.\nWhy: see principle nowhere-else.\n"),
            )],
            vec![],
            vec![
                at(
                    "principles/p.md",
                    6,
                    "principle one-two: names decision nowhere-here, which no file holds",
                ),
                at(
                    "principles/p.md",
                    7,
                    "principle one-two: names principle nowhere-else, which no file holds",
                ),
            ],
        ),
    ]);
}

#[test]
fn the_limits_the_moments_and_the_data_are_judged_as_the_script_judged_them() {
    let at = |line: usize, text: &str| said("decisions/b.md", line, &format!("decision one-two: {text}"));
    let moment = |what: &str| format!("names a work, an item or a date: {what}");
    let only =
        |first: &str, under: &[&str]| vec![("decisions/b.md", format!("{HEAD}{}", dec("one-two", first, under)))];
    let missing = |data: &str| format!("the data `{data}` is not in the repository");
    assert_as_the_script_said(vec![
        (
            "a work, an item and a date in many forms, and one past the first on the next line",
            only(
                "Work 12, work 12, W3, item #4, Item 5, item 6, items 7, #99999, #100000, 2026-1-1, 12026-01-01, \
                 2026-01-011.",
                &["- rejected: an agent proposed x W9; the owner rejected it, because see 2020-02-30."],
            ),
            vec![],
            vec![at(6, &moment("Work 12")), at(7, &moment("W9"))],
        ),
        (
            "a number of five digits after a number of six",
            only("Fix #123456 and #12345 and C#7.", &[REJ]),
            vec![],
            vec![at(6, &moment("#12345"))],
        ),
        (
            "a work in the words of the other lines",
            only(
                "It holds.",
                &[
                    "- rejected: an agent proposed x; the owner rejected it, because y at #1.",
                    "- replaced: Work 3 was on main; the owner changed it, because y.",
                ],
            ),
            vec![],
            vec![at(7, &moment("#1")), at(8, &moment("Work 3"))],
        ),
        (
            "a work number set off by hyphens, and words that are no work",
            only("a-W12 and W12-b and x#12-y and w12.", &[REJ]),
            vec![],
            vec![at(6, &moment("W12"))],
        ),
        (
            "numbers and letters that run into a work's number",
            only("Fix \u{e9}W12 and W12\u{e9} and W12_ and _W12.", &[REJ]),
            vec![],
            vec![],
        ),
        (
            "a date in the name of data",
            only(
                "It holds.",
                &[
                    "- rejected: an agent proposed x; the owner rejected it, because y (data: `research/2026-01-01-foo.md`).",
                ],
            ),
            vec!["research/2026-01-01-foo.md"],
            vec![at(7, &moment("2026-01-01"))],
        ),
        (
            "a first line of 200 characters of two bytes each",
            only(&"\u{e9}".repeat(200), &[REJ]),
            vec![],
            vec![],
        ),
        (
            "a first line of 201 characters of two bytes each",
            only(&"\u{e9}".repeat(201), &[REJ]),
            vec![],
            vec![at(6, "the first line is 201 characters, over 200")],
        ),
        (
            "a first line of 200 characters of three bytes each",
            only(&"\u{65e5}".repeat(200), &[REJ]),
            vec![],
            vec![],
        ),
        (
            "a rule and a reason each of 301 characters",
            vec![(
                "principles/p.md",
                format!(
                    "{HEAD}### principle one-two\n{}\nWhy: {}\n",
                    "a".repeat(301),
                    "b".repeat(301)
                ),
            )],
            vec![],
            vec![
                said(
                    "principles/p.md",
                    6,
                    "principle one-two: the rule is 301 characters, over 300",
                ),
                said(
                    "principles/p.md",
                    7,
                    "principle one-two: the reason is 301 characters, over 300",
                ),
            ],
        ),
        (
            "a principle that names a work, an item and a date, which only a decision is held from",
            vec![(
                "principles/p.md",
                format!("{HEAD}### principle one-two\nRule at W12 and 2026-01-01.\nWhy: see #12.\n"),
            )],
            vec![],
            vec![],
        ),
        (
            "a reason of spaces",
            vec![(
                "principles/p.md",
                format!("{HEAD}### principle one-two\nRule.\nWhy:    \n"),
            )],
            vec![],
            vec![said(
                "principles/p.md",
                7,
                "principle one-two: the reason after `Why: ` is empty",
            )],
        ),
        (
            "a reason with no space after its mark",
            vec![(
                "principles/p.md",
                format!("{HEAD}### principle one-two\nRule.\nWhy:x\n"),
            )],
            vec![],
            vec![said(
                "principles/p.md",
                6,
                "principle one-two: a principle is its rule on one line and its reason on one line after `Why: `",
            )],
        ),
        (
            "data named in many ways in two lines",
            only(
                "x (data: `a/b`) and (data: `c d`) and (data: ``) and (data: `x`",
                &["- rejected: an agent proposed x; the owner rejected it, because y (data: `p/q`)(data: `r`)."],
            ),
            vec![],
            vec![
                at(6, &missing("a/b")),
                at(6, &missing("c d")),
                at(7, &missing("p/q")),
                at(7, &missing("r")),
            ],
        ),
    ]);
}
