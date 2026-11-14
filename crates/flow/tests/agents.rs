//! What the engine says to every agent, read from its files beside the flows into their shapes.

use std::fs;
use std::path::{Path, PathBuf};

use flow::agents::{self, GivenTo, OfTheLine, ReturnKind, Sent, SessionPart, ToldAs};

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
    assert_eq!(read.project.installation.names, ["owner.language"]);
    assert_eq!(read.project.project["research"], "research/");

    let parts: Vec<String> = (read.session.parts.iter())
        .map(|part| match part {
            SessionPart::Project(p) => format!("project {}", p.project),
            SessionPart::Line(l) => format!("line {:?}", l.line),
            SessionPart::Words(w) => format!("words {}", w.words),
        })
        .collect();
    assert_eq!(
        parts,
        [
            "project guide",
            "project constitution",
            "line Rules",
            "words how_you_work_here.md"
        ]
    );
    assert!(matches!(&read.session.parts[2], SessionPart::Line(l) if l.line == OfTheLine::Rules));
    assert_eq!(read.session.heading, "# `{path}`\n\n{text}");
    assert_eq!(
        read.words["how_you_work_here.md"],
        "Work in the engine as these words say.\n"
    );

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

    assert_eq!(read.returns.form.calls, ["tools", "summary"]);
    assert_eq!(read.returns.told.listed, ["reply", "summary"]);
    assert_eq!(
        read.fields.keys().map(String::as_str).collect::<Vec<_>>(),
        ["dealt_with", "reply", "summary"]
    );
    let dealt_with = &read.fields["dealt_with"];
    assert_eq!(dealt_with.kinds, [ReturnKind::Outcome]);
    assert!(dealt_with.required);
    assert_eq!(dealt_with.given_to, GivenTo::Every);
    assert_eq!(dealt_with.told, ToldAs::Alone);
    assert_eq!(dealt_with.names.as_deref(), Some("runs_on"));
    assert_eq!(dealt_with.shape["type"].as_str(), Some("array"));
    let summary = &read.fields["summary"];
    assert_eq!(summary.kinds, [ReturnKind::Calls, ReturnKind::Outcome]);
    assert!(!summary.required);
    assert_eq!(read.fields["reply"].given_to, GivenTo::WritesThrough);

    assert_eq!(read.tools.keys().map(String::as_str).collect::<Vec<_>>(), ["query"]);
    assert_eq!(read.tools["query"].request["required"][0].as_str(), Some("sql"));

    let message: Vec<(&str, Sent)> = (read.message.parts.iter())
        .map(|part| (part.part.as_str(), part.sent))
        .collect();
    assert_eq!(
        message,
        [
            ("heading", Sent::EveryTime),
            ("context", Sent::ByKind),
            ("task", Sent::OnceASession),
            ("triggers", Sent::EveryTime),
        ]
    );
    assert_eq!(read.message.parts[3].does.as_deref(), Some(": {do}"));
    assert_eq!(read.given.choice, ", with the value `{value}`");
    assert!(read.given.last_and_latest.text.contains("{which_version}"));
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
    fs::write(&message, text.replace("sent = \"by_kind\"", "sent = \"now_and_then\"")).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&message.display().to_string()), "{refused}");
    assert!(refused.contains("now_and_then"), "{refused}");
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
