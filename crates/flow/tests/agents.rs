//! What the engine says to every agent, read from its files beside the flows into their shapes.

use std::fs;
use std::path::{Path, PathBuf};

use flow::InputKind;
use flow::agents::{self, OfTheLine, OfTheReturn, Only, Part, SessionPart, ToldAs};

/// The directory of the tests' flows, beside which the files are.
fn beside() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/project")
}

/// A copy of the files beside the flows, in a directory of the test's own, which the test may change.
fn copy(name: &str) -> PathBuf {
    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let path = entry.unwrap().path();
            let target = to.join(path.file_name().unwrap());
            if path.is_dir() {
                copy_dir(&path, &target);
            } else {
                fs::copy(&path, &target).unwrap();
            }
        }
    }
    let dir = std::env::temp_dir().join(format!("flow-agents-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    copy_dir(&beside(), &dir);
    dir
}

#[test]
fn what_the_engine_says_to_every_agent_is_read_from_its_files_beside_the_flows() {
    let read = agents::read(&beside()).unwrap();
    assert_eq!(read.root, beside());
    assert_eq!(read.project.files["guide"].path, "AGENTS.md");
    assert_eq!(read.project.files["constitution"].path, "design/constitution.md");
    assert_eq!(read.installation.names, ["owner.language"]);
    assert_eq!(read.project.project["research"], "research/");

    let parts: Vec<String> = (read.session.parts.iter())
        .map(|part| match part {
            SessionPart::Project(p) => format!("project {}", p.project),
            SessionPart::Line(l) => format!("line {:?}", l.line),
            SessionPart::Words(w) => format!("words {}", w.words),
            SessionPart::Returns(r) => format!("returns {:?}", r.returns),
        })
        .collect();
    assert_eq!(
        parts,
        [
            "project guide",
            "project constitution",
            "line Rules",
            "words how_you_work_here.md",
            "returns HowYouReturn",
        ]
    );
    assert!(matches!(&read.session.parts[4], SessionPart::Returns(r) if r.returns == OfTheReturn::HowYouReturn));
    assert!(matches!(&read.session.parts[2], SessionPart::Line(l) if l.line == OfTheLine::Rules));
    assert_eq!(read.session.heading, "# `{path}`\n\n{text}");
    assert_eq!(
        read.words["how_you_work_here.md"],
        "Work in the engine as these words say.\n"
    );
    assert_eq!(read.your_line.reads_only, "You only read.");
    assert_eq!(read.your_line.locks, " Lock with {commands}.");
    assert_eq!(read.your_line.tests_only, " Tests only: {paths}.");
    assert_eq!(read.your_line.protected_read, " Read protected: {paths}.");

    let rules: Vec<(&str, &str, &str)> = (read.rules.iter())
        .map(|(name, set)| (name.as_str(), set.file.as_str(), set.text.as_str()))
        .collect();
    assert_eq!(
        rules,
        [
            ("engineering", "engineering.md", "Test what you change.\n"),
            ("general", "general.md", "Say one thing per sentence.\n"),
        ]
    );

    assert_eq!(
        read.returns.form.outcome,
        ["dealt_with", "summary", "meaning_unchanged", agents::OUTPUTS]
    );
    assert_eq!(read.returns.how_you_return.listed, ["summary"]);
    assert_eq!(read.returns.how_you_return.reads, ["summary"]);
    assert_eq!(read.returns.how_you_return.tools, "## What you may ask the engine for");
    assert_eq!(read.returns.outputs.none, "This cell writes no block.");
    assert_eq!(read.sent_back.heading, "## Sent back");
    assert_eq!(read.sent_back.item, "- {text}");
    assert_eq!(read.sent_back.faults.not_watched, "`{name}` does not run this cell.");
    assert_eq!(read.sent_back.unread.requests_not_a_list, "`{name}` has no list");
    assert_eq!(
        read.sent_back.faults.not_answerable,
        "`{name}` cannot be answered here."
    );
    assert_eq!(read.project.protected.paths, ["AGENTS.md", "flow/"]);
    assert_eq!(read.asks.protected.name, "protected_files");
    assert_eq!(read.asks.protected.options.len(), 2);
    assert_eq!(
        read.fields.keys().map(String::as_str).collect::<Vec<_>>(),
        ["dealt_with", "meaning_unchanged", "outputs", "summary"]
    );
    let dealt_with = &read.fields["dealt_with"];
    assert!(dealt_with.required);
    assert_eq!(dealt_with.told, ToldAs::Alone);
    assert_eq!(dealt_with.shape["type"].as_str(), Some("array"));
    let summary = &read.fields["summary"];
    assert!(!summary.required);
    assert_eq!(read.fields["meaning_unchanged"].told, ToldAs::WithOutputs);

    assert_eq!(
        read.tools.keys().map(String::as_str).collect::<Vec<_>>(),
        ["answers", "query"]
    );
    assert_eq!(read.tools["query"].request["required"][0].as_str(), Some("sql"));
    assert_eq!(read.tools["query"].defines, ["rows", "cut", "substr"]);
    assert!(read.tools["answers"].defines.is_empty());
    let given = read.tools["answers"]
        .given
        .as_ref()
        .expect("how much of a result is given");
    assert_eq!(
        (given.bound, given.cut.as_str()),
        (40, "(Cut: {given} of {size} bytes.)")
    );
    assert_eq!(read.tools["answers"].unread, None);

    let message: Vec<(Part, Option<InputKind>, Option<Only>)> = (read.message.parts.iter())
        .map(|part| (part.part, part.kind, part.only))
        .collect();
    assert_eq!(
        message,
        [
            (Part::Taken, None, Some(Only::AfterHandIn)),
            (Part::Heading, None, None),
            (Part::Main, Some(InputKind::Latest), Some(Only::TakesMain)),
            (Part::Context, None, None),
            (Part::Outputs, Some(InputKind::Latest), None),
            (Part::Task, Some(InputKind::Latest), None),
            (Part::Results, None, None),
            (Part::SentBack, None, None),
            (Part::New, None, None),
        ]
    );
    assert_eq!(read.message.parts[8].does.as_deref(), Some(": {do}"));
    assert_eq!(read.message.parts[0].between.as_deref(), Some(", "));
    assert_eq!(read.message.compacted, "Compacted: the round again.");
    assert_eq!(read.results.none, "(none)");
    let main = read
        .message
        .parts
        .iter()
        .find(|p| p.part == Part::Main)
        .expect("the part main");
    assert!(main.text.as_deref().is_some_and(|t| t.contains("{merge}")));
    assert_eq!(read.given.choice, ", with the value `{value}`");
    assert!(read.given.last_and_latest.text.contains("{diff}"));
    assert!(
        read.given
            .last_and_latest
            .whole
            .as_deref()
            .is_some_and(|w| w.contains("{content}"))
    );
    assert_eq!(read.given.latest.whole, None);

    assert_eq!(read.acts.check.values.passed, "yes");
    assert!(read.acts.merge.text.contains("{branch}"));
    assert_eq!(read.commits.name, "wf-agents");
}

#[test]
fn what_the_engine_writes_as_it_works_is_read_alone_as_read_reads_it() {
    let read = agents::read(&beside()).unwrap();
    let writes = agents::writes(&beside()).unwrap();
    assert_eq!(writes.commits, read.commits);
    assert_eq!(writes.acts.start.text, read.acts.start.text);
    assert_eq!(writes.acts.start.continued, read.acts.start.continued);
    assert!(read.acts.start.continued.contains("{continues}"));
    assert_eq!(writes.acts.deliver.between, read.acts.deliver.between);
}

#[test]
fn a_text_is_filled_by_the_names_it_is_given_and_keeps_those_it_is_not() {
    assert_eq!(
        agents::fill(
            "Merge {main} at {head} as run {run}",
            &[("main", "main"), ("head", "c0ffee")]
        ),
        "Merge main at c0ffee as run {run}"
    );
}

#[test]
fn what_fills_a_place_is_not_filled_again_and_what_opens_no_place_is_kept() {
    assert_eq!(
        agents::fill("by {by} at {head}", &[("by", "{head}"), ("head", "c0ffee")]),
        "by {head} at c0ffee"
    );
    assert_eq!(
        agents::fill("ask {{q}} of {x} {\"a\": 1} {Y} {", &[("q", "no"), ("x", "it")]),
        "ask {{q}} of it {\"a\": 1} {Y} {"
    );
}

#[test]
fn a_field_a_file_does_not_know_is_refused_with_the_path_of_its_file() {
    let dir = copy("unknown");
    let query = dir.join("agents/tools/query.toml");
    let text = fs::read_to_string(&query).unwrap();
    fs::write(&query, text.replace("words = ", "colour = \"red\"\nwords = ")).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&query.display().to_string()), "{refused}");
    assert!(refused.contains("colour"), "{refused}");
}

// A project that declares `[ci]` declares both workflows: `run_tests` runs only the fast one and the check before the
// merge only the full ones, so a declaration with one of them leaves one of the two with nothing to run.
#[test]
fn a_declaration_of_ci_that_lacks_one_of_its_two_workflows_is_refused_with_the_path_of_its_file() {
    let dir = copy("ci");
    let project = dir.join("project.toml");
    let text = fs::read_to_string(&project).unwrap();
    let fast = "\n[ci.fast]\nworkflow = \"fast.yml\"\nrunner = \"github\"\n";
    let full = "\n[ci.full]\nworkflows = [\"ci.yml\"]\nrunner = \"github\"\ntoo_long = \"2h\"\n";
    fs::write(&project, format!("{text}{fast}")).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    assert!(refused.contains(&project.display().to_string()), "{refused}");
    assert!(refused.contains("missing field `full`"), "{refused}");
    fs::write(&project, format!("{text}{full}")).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    assert!(refused.contains("missing field `fast`"), "{refused}");
    fs::write(&project, format!("{text}{fast}{full}")).unwrap();
    let read = agents::read(&dir).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    let ci = read.project.ci.expect("it declares both");
    assert_eq!(
        (ci.fast.workflow.as_str(), ci.full.workflows.as_slice()),
        ("fast.yml", &["ci.yml".to_string()][..])
    );
}

// How an entry of the design tree is found is the project's: a `[tree_format]` that names no kind, or whose heading or
// reference lacks the place of the kind or of the name, could find no entry, and is refused as the declaration is
// read, beside the flows or on the project's main, with where it was read; one that holds both is taken as written.
#[test]
fn a_format_of_the_tree_that_could_find_no_entry_is_refused_as_the_declaration_is_read_with_where_it_was_read() {
    let dir = copy("tree-format");
    let project = dir.join("project.toml");
    let text = fs::read_to_string(&project).unwrap();
    let wrong = "\n[tree_format]\nkinds = []\nheading = \"## {kind}\"\nreference = \"{name}\"\n";
    fs::write(&project, format!("{text}{wrong}")).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    assert!(refused.contains(&project.display().to_string()), "{refused}");
    for fault in [
        "tree_format.kinds names no kind",
        "tree_format.heading must hold {name}",
        "tree_format.reference must hold {kind}",
    ] {
        assert!(refused.contains(fault), "{fault} is not in: {refused}");
    }
    let empty = "\n[tree_format]\nkinds = [{ kind = \"rule\", dir = \"\" }]\nheading = \"## {kind}: {name}\"\nreference = \"{kind} {name}\"\n";
    let declared = agents::read(&beside())
        .unwrap()
        .declared(&format!("{text}{empty}"), "main:flow/project.toml");
    let refused = declared.expect_err("a kind with no directory is refused");
    assert!(refused.starts_with("main:flow/project.toml: "), "{refused}");
    assert!(
        refused.contains("tree_format.kinds[0] must give a kind and a directory"),
        "{refused}"
    );
    let right = "\n[tree_format]\nkinds = [{ kind = \"rule\", dir = \"rules/\" }]\nheading = \"## {kind}: {name}\"\nreference = \"{kind} {name}\"\n";
    fs::write(&project, format!("{text}{right}")).unwrap();
    let read = agents::read(&dir).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    let format = read.project.tree_format.expect("the format is read");
    assert_eq!(format.heading_of("## rule: keep-it  "), Some(("rule", "keep-it")));
    assert_eq!(format.heading_of("### rule: keep-it"), None);
    assert_eq!(format.referred("  rule keep-it "), Some(("rule", "keep-it")));
    assert_eq!(format.referred("rule keep it"), None, "a name holds no white space");
    assert_eq!(format.referred("rule "), None, "a name is not empty");
    assert_eq!(format.referred("decision keep-it"), None, "no kind of the format");
}

#[test]
fn a_value_no_kind_of_it_has_is_refused_with_the_path_of_its_file() {
    let dir = copy("value");
    let message = dir.join("agents/message/message.toml");
    let text = fs::read_to_string(&message).unwrap();
    fs::write(
        &message,
        text.replace("only = \"takes_main\"", "only = \"now_and_then\""),
    )
    .unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&message.display().to_string()), "{refused}");
    assert!(refused.contains("now_and_then"), "{refused}");
}

#[test]
fn the_words_of_a_return_sent_back_lacking_those_of_one_thing_wrong_are_refused_with_the_path_of_their_file() {
    let dir = copy("sent-back");
    let sent_back = dir.join("agents/return/sent_back.toml");
    let text = fs::read_to_string(&sent_back).unwrap();
    let lacking: String = (text.lines())
        .filter(|line| !line.starts_with("not_a_tool"))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::write(&sent_back, lacking).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&sent_back.display().to_string()), "{refused}");
    assert!(refused.contains("not_a_tool"), "{refused}");
}

#[test]
fn a_file_that_is_named_and_not_there_is_refused_with_its_path() {
    let dir = copy("missing");
    let general = dir.join("agents/rules/general.md");
    fs::remove_file(&general).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&general.display().to_string()), "{refused}");
}

#[test]
fn a_file_of_the_project_the_session_names_is_one_project_toml_has() {
    let dir = copy("project");
    let session = dir.join("agents/session.toml");
    let text = fs::read_to_string(&session).unwrap();
    fs::write(
        &session,
        text.replace("{ project = \"constitution\" }", "{ project = \"charter\" }"),
    )
    .unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&session.display().to_string()), "{refused}");
    assert!(refused.contains("charter"), "{refused}");
}

#[test]
fn each_field_the_form_and_told_name_is_a_field_of_the_return() {
    for (name, from, to) in [
        ("form", "\"meaning_unchanged\", \"outputs\"", "\"answer\", \"outputs\""),
        ("listed", "listed = [\"summary\"]", "listed = [\"summary\", \"notes\"]"),
        ("reads", "reads = [\"summary\"]", "reads = [\"words\"]"),
    ] {
        let dir = copy(name);
        let returns = dir.join("agents/return/return.toml");
        let text = fs::read_to_string(&returns).unwrap();
        assert!(text.contains(from), "{name}");
        fs::write(&returns, text.replace(from, to)).unwrap();
        let refused = agents::read(&dir).unwrap_err();
        fs::remove_dir_all(&dir).unwrap();
        assert!(refused.contains(&returns.display().to_string()), "{name}: {refused}");
    }
}

#[test]
fn a_tool_file_that_lacks_a_text_the_engine_says_or_names_one_it_does_not_is_refused() {
    let dir = copy("texts");
    let query = dir.join("agents/tools/query.toml");
    let text = fs::read_to_string(&query).unwrap();
    fs::write(&query, text.replace("timeout = ", "timeoutt = ")).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&query.display().to_string()), "{refused}");
    assert!(
        refused.contains("lacks: timeout") && refused.contains("does not say: timeoutt"),
        "{refused}"
    );
}

#[test]
fn the_files_of_the_real_flow_hold_every_text_the_engine_says_of_each_tool() {
    let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let read = agents::read(&real).expect("the real files beside the flows are read");
    for (tool, known) in agents::TOOL_TEXTS {
        let mut held: Vec<&str> = read.tools[*tool].texts.keys().map(String::as_str).collect();
        let mut known = known.to_vec();
        held.sort_unstable();
        known.sort_unstable();
        assert_eq!(held, known, "{tool}");
    }
    assert!(read.tools["query"].given_bytes.is_some());
}

#[test]
fn a_reason_is_a_sentence_ended_as_one_in_english_or_in_chinese_and_given_a_full_stop_where_it_has_none() {
    assert_eq!(
        agents::sentence("The agent could not start"),
        "The agent could not start."
    );
    assert_eq!(agents::sentence("Which way? "), "Which way?");
    assert_eq!(agents::sentence("It spent $4."), "It spent $4.");
    assert_eq!(agents::sentence("选哪一种？"), "选哪一种？");
    assert_eq!(agents::sentence("先做这个。"), "先做这个。");
    assert_eq!(agents::sentence("快！"), "快！");
    assert_eq!(agents::sentence(""), "");
}

#[test]
fn the_context_part_orders_what_a_round_is_given_by_naming_each_list_of_a_cell_once() {
    // The words with the context part given this order, or none, read or refused.
    let read = |name: &str, order: Option<&str>| {
        let dir = copy(name);
        let message = dir.join("agents/message/message.toml");
        let text = fs::read_to_string(&message).unwrap();
        let kept: String = (text.lines())
            .filter(|line| !line.starts_with("order = "))
            .map(|line| format!("{line}\n"))
            .collect();
        let ordered = match order {
            Some(order) => kept.replace(
                "part = \"context\"\n",
                &format!("part = \"context\"\norder = {order}\n"),
            ),
            None => kept,
        };
        fs::write(&message, ordered).unwrap();
        let read = agents::read(&dir);
        fs::remove_dir_all(&dir).unwrap();
        read.map(|_| ())
            .map_err(|refused| (refused, message.display().to_string()))
    };
    let whole = r#"["every_agent", "reads", "watches", "outputs"]"#;
    assert_eq!(read("order-whole", Some(whole)), Ok(()));
    let other = r#"["outputs", "watches", "reads", "every_agent"]"#;
    assert_eq!(read("order-other", Some(other)), Ok(()));
    for (name, order, named) in [
        (
            "order-lacking",
            Some(r#"["every_agent", "reads", "watches"]"#),
            "outputs",
        ),
        (
            "order-twice",
            Some(r#"["every_agent", "reads", "reads", "watches", "outputs"]"#),
            "reads",
        ),
        ("order-none", None, "every_agent, reads, watches, outputs"),
    ] {
        let (refused, file) = read(name, order).unwrap_err();
        assert!(refused.contains(&file), "{name}: {refused}");
        assert!(refused.contains(named), "{name}: {refused}");
    }
}

// The first message every fork of a round is sent is not blank, in the repository's own words as the engine reads them:
// a blank message is no message, and the fork would then be sent its question alone. A few files read: milliseconds.
#[test]
fn the_first_message_of_a_fork_in_the_repositorys_flow_is_not_blank() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let read = agents::read(&root).expect("the files beside the repository's flows are read");
    let first = &read.cost.fork.first;
    assert!(!first.trim().is_empty(), "a blank message is sent first");
}
