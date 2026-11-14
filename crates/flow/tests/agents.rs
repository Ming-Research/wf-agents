//! What the engine says to every agent, read from its files beside the flows into their shapes.

use std::fs;
use std::path::{Path, PathBuf};

use flow::InputKind;
use flow::agents::{self, Group, InForm, OfTheLine, OfTheReturn, Only, Part, ReturnKind, SessionPart, ToldAs};

/// The directory of the tests' flows, beside which the files are.
fn beside() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/laid_out")
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
        read.returns.form.calls,
        [InForm::Group(Group::Tools), InForm::Field("summary".to_string())]
    );
    assert_eq!(read.returns.form.outcome[4], InForm::Field(agents::OUTPUTS.to_string()));
    assert_eq!(read.returns.how_you_return.listed, ["reply", "summary"]);
    assert_eq!(read.returns.how_you_return.reads, ["reply", "summary"]);
    assert_eq!(read.returns.outputs.none, "This cell writes no block.");
    assert_eq!(read.sent_back.heading, "## Sent back");
    assert_eq!(read.sent_back.item, "- {text}");
    assert_eq!(read.sent_back.faults.not_a_trigger, "`{name}` does not run this cell.");
    assert_eq!(read.sent_back.unread.requests_not_a_list, "`{name}` has no list");
    assert_eq!(read.returns.asked.recommended, "{number}. {does}, {follows} (yes)");
    assert_eq!(
        read.fields.keys().map(String::as_str).collect::<Vec<_>>(),
        ["dealt_with", "meaning_unchanged", "outputs", "reply", "summary"]
    );
    let dealt_with = &read.fields["dealt_with"];
    assert_eq!(dealt_with.kinds, [ReturnKind::Outcome]);
    assert!(dealt_with.required);
    assert_eq!(dealt_with.told, ToldAs::Alone);
    assert_eq!(dealt_with.shape["type"].as_str(), Some("array"));
    let summary = &read.fields["summary"];
    assert_eq!(summary.kinds, [ReturnKind::Calls, ReturnKind::Outcome]);
    assert!(!summary.required);
    assert_eq!(read.fields["meaning_unchanged"].told, ToldAs::WithOutputs);

    assert_eq!(
        read.tools.keys().map(String::as_str).collect::<Vec<_>>(),
        ["answers", "query"]
    );
    assert_eq!(read.tools["query"].request["required"][0].as_str(), Some("sql"));
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
            (Part::Triggers, None, None),
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
fn each_field_the_form_and_told_name_is_a_field_of_the_return_and_the_form_s_groups_are_its_own() {
    for (name, from, to) in [
        ("form", "\"reply\", \"outputs\"", "\"answer\", \"outputs\""),
        (
            "listed",
            "listed = [\"reply\", \"summary\"]",
            "listed = [\"reply\", \"summary\", \"notes\"]",
        ),
        (
            "reads",
            "reads = [\"reply\", \"summary\"]",
            "reads = [\"reply\", \"words\"]",
        ),
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
