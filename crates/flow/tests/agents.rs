//! What the engine says to every agent, read from its files beside the flows into their shapes.

use std::fs;
use std::path::{Path, PathBuf};

use flow::InputKind;
use flow::agents::{self, Declaration, OfTheLine, OfTheReturn, Only, Part, SessionPart, ToldAs};

/// The directory of the tests' flows, beside which the files are.
fn beside() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/project")
}

/// The text of the declaration of the tests' project.
fn declaration_text() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/declaration/wf-agents.toml")).unwrap()
}

/// A declaration as the engine reads a project's: from its text, against the files beside the tests' flows.
fn declared(text: &str) -> Result<Declaration, String> {
    Declaration::read(text, "main:wf-agents.toml", &agents::read(&beside()).unwrap())
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
    assert_eq!(read.installation.names, ["owner.language"]);

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
    assert_eq!(read.refused.commands, ["git push", agents::PROGRAM_SLOT]);
    assert_eq!(
        read.allowed.may,
        [
            agents::Ability::Shell,
            agents::Ability::Files,
            agents::Ability::TempFiles,
            agents::Ability::Web,
            agents::Ability::Subagents
        ]
    );
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
    // What the line of a datum new to the cell says of its versions: the one last consumed, and each that changed its
    // meaning, one or several.
    let new = &read.message.parts[8];
    assert_eq!(new.since.as_deref(), Some(", new since version {last}"));
    assert_eq!(
        new.changed_one.as_deref(),
        Some(", version {changed} changed its meaning")
    );
    assert_eq!(
        new.changed_many.as_deref(),
        Some(", versions {changed} changed their meaning")
    );
    assert_eq!(new.between.as_deref(), Some(", "));
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
fn a_project_declares_its_files_settings_protection_and_heavy_commands_in_its_own_file() {
    let declaration = declared(&declaration_text()).unwrap();
    assert_eq!(declaration.files["guide"].path, "AGENTS.md");
    assert_eq!(declaration.files["constitution"].path, "design/constitution.md");
    assert_eq!(declaration.project["research"], "research/");
    assert_eq!(declaration.protected.paths, ["AGENTS.md", "flow/"]);
    assert_eq!(declaration.heavy.commands, ["cargo", "npm"]);
    assert!(declaration.local.is_none() && declaration.pull_request.is_none() && declaration.ci.is_none());
}

// What the installation held of a project is the project's now: the command that checks it where its repository is the
// host, and how its pull requests are merged where GitHub is.
#[test]
fn what_the_installation_once_held_of_a_project_is_read_from_its_declaration() {
    let text = declaration_text();
    let declaration = declared(&format!(
        "{text}\n[local]\ncheck = \"./test.sh\"\n\n[pull_request]\nmerge_method = \"rebase\"\ndelete_branch = true\n"
    ))
    .unwrap();
    assert_eq!(declaration.local.expect("the check").check, "./test.sh");
    let pull = declaration.pull_request.expect("how it merges");
    assert_eq!((pull.merge_method.as_str(), pull.delete_branch), ("rebase", true));
}

// A merge method the host does not take, a check that names no command, a key the declaration does not know and a
// declaration that lacks its heavy commands are refused, each said with where the declaration was read.
#[test]
fn a_declaration_that_the_host_could_not_use_is_refused_with_where_it_was_read() {
    let text = declaration_text();
    for (what, wrong, said) in [
        (
            "merge method",
            format!("{text}\n[pull_request]\nmerge_method = \"fast\"\ndelete_branch = false\n"),
            "pull_request.merge_method: one of squash, merge, rebase, not \"fast\"",
        ),
        (
            "check",
            format!("{text}\n[local]\ncheck = \" \"\n"),
            "local.check: names a command",
        ),
        (
            "unknown key",
            format!("{text}\n[pull_request]\nmerge_method = \"squash\"\ndelete_branch = false\nlabel = \"x\"\n"),
            "unknown field `label`",
        ),
        (
            "heavy",
            text.replace("\n[heavy]\ncommands = [\"cargo\", \"npm\"]\n", ""),
            "missing field `heavy`",
        ),
    ] {
        let refused = declared(&wrong).expect_err(what);
        assert!(refused.starts_with("main:wf-agents.toml: "), "{what}: {refused}");
        assert!(refused.contains(said), "{what}: {refused}");
    }
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

// What every agent may do is a closed list of words that name no program: a word it does not know, a word twice, a
// key it does not know, and a refused entry with a slot other than the agents' program are each refused, said with the
// path of the file. Reference: the file's own vocabulary (`Ability`). Milliseconds.
#[test]
fn what_agents_may_do_and_are_refused_is_read_from_words_and_one_slot_and_nothing_else() {
    let wrong = |file: &str, from: &str, to: &str| {
        let dir = copy("allowed");
        let path = dir.join("agents").join(file);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{file} has no {from:?}");
        fs::write(&path, text.replacen(from, to, 1)).unwrap();
        let refused = agents::read(&dir).unwrap_err();
        fs::remove_dir_all(&dir).unwrap();
        (path.display().to_string(), refused)
    };
    for (file, from, to, said) in [
        ("allowed.toml", "\"web\"", "\"claude\"", "claude"),
        ("allowed.toml", "\"web\"", "\"files\"", "`may` names files twice"),
        ("allowed.toml", "may = ", "colour = \"red\"\nmay = ", "colour"),
        (
            "refused.toml",
            "\"{agents.command}\"",
            "\"{agents.cmd}\"",
            "the only slot is {agents.command}",
        ),
        (
            "refused.toml",
            "\"{agents.command}\"",
            "\"{agents.command} {x}\"",
            "the only slot is {agents.command}",
        ),
    ] {
        let (path, refused) = wrong(file, from, to);
        assert!(refused.contains(&path), "{refused}");
        assert!(refused.contains(said), "{refused}");
    }
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

// Every commit of a work ends with these two lines, so that `git log --grep='^Item: 36$'` finds a work's commits: the
// item's number alone, which the host would link as a pull request's with a `#`, and the work's as it is written
// everywhere.
#[test]
fn the_lines_every_commit_ends_with_name_their_numbers_after_a_blank_line() {
    let commits = agents::read(&beside()).unwrap().commits;
    assert_eq!(
        commits.lines("36", "W12"),
        ("Item: 36".to_string(), "Work: W12".to_string())
    );
    assert_eq!(
        commits.ending("Merge main at abc into the work's branch\n", "36", "W12"),
        "Merge main at abc into the work's branch\n\nItem: 36\nWork: W12"
    );
    // Either line of any item or work is one of the lines, by what the caller says a number is; a line that only looks
    // like one is not.
    let digits = |number: &str| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit());
    let written = |number: &str| number.strip_prefix('W').is_some_and(digits);
    for line in ["Item: 20", "Work: W3", "Item: 36", "Work: W12"] {
        assert!(commits.is_one_of_the_lines(line, digits, written), "{line}");
    }
    for line in [
        "Item: ",
        "Item: x",
        "Item: #20",
        "Item: 20 and more",
        "Work: 3",
        "Work: W",
        "Work: W3 and more",
        "Items: 20",
        "item: 20",
    ] {
        assert!(!commits.is_one_of_the_lines(line, digits, written), "{line}");
    }
    // A line is of the work's kind only where what stands in its place is a work's number: an item's line with a work's
    // number, or a work's with an item's, is neither.
    assert!(!commits.is_one_of_the_lines("Item: W3", digits, written));
    assert!(!commits.is_one_of_the_lines("Work: 3", digits, written));
}

// A line with no place for its number would end every commit with a line that names nothing.
#[test]
fn a_line_every_commit_ends_with_that_names_no_number_is_refused_with_the_path_of_its_file() {
    for (key, line, place, wanted) in [
        ("item_line", "Item: {item_number}", "Item: none", "{item_number}"),
        ("work_line", "Work: {work}", "Work: {item_number}", "{work}"),
    ] {
        let dir = copy(key);
        let file = dir.join("agents/commits.toml");
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains(line), "{key}");
        fs::write(&file, text.replace(line, place)).unwrap();
        let refused = agents::read(&dir).unwrap_err();
        let alone = agents::writes(&dir).unwrap_err();
        fs::remove_dir_all(&dir).unwrap();
        for said in [refused, alone] {
            assert!(said.contains(&file.display().to_string()), "{said}");
            assert!(said.contains(&format!("{key} does not name {wanted}")), "{said}");
        }
    }
}

// A line that names its number but runs over two lines would make the trailer block of every commit hold a stray line.
#[test]
fn a_line_every_commit_ends_with_that_runs_over_two_lines_is_refused_with_the_path_of_its_file() {
    for (key, line, place) in [
        (
            "item_line",
            "Item: {item_number}",
            "Item: {item_number}\\nSee {item_number}",
        ),
        ("work_line", "Work: {work}", "Work: {work}\\nSee {work}"),
    ] {
        let dir = copy(key);
        let file = dir.join("agents/commits.toml");
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains(line), "{key}");
        fs::write(&file, text.replace(line, place)).unwrap();
        let refused = agents::read(&dir).unwrap_err();
        fs::remove_dir_all(&dir).unwrap();
        assert!(refused.contains(&file.display().to_string()), "{refused}");
        assert!(refused.contains(&format!("{key} is not one line")), "{refused}");
        assert!(!refused.contains("does not name"), "{refused}");
    }
}

// A declaration names the variables its toolchain reads, none where it needs none, as it names its heavy commands: one
// that leaves `[environment]` out is refused with where it was read, rather than taken to need none; one that names
// them is read. Well under a millisecond alone, and no other test holds that the table is required.
#[test]
fn a_declaration_that_lacks_its_environment_is_refused_with_where_it_was_read() {
    let text = declaration_text();
    let (before, _) = text
        .split_once("\n[environment]\n")
        .expect("the fixture names its environment");
    let refused = declared(before).unwrap_err();
    assert!(refused.starts_with("main:wf-agents.toml: "), "{refused}");
    assert!(refused.contains("missing field `environment`"), "{refused}");
    let none = declared(&format!("{before}\n[environment]\n")).unwrap();
    assert!(none.environment.is_empty());
    let read = declared(&text).unwrap();
    assert_eq!(
        read.environment.get("RUSTUP_AUTO_INSTALL").map(String::as_str),
        Some("0")
    );
}

// A time a declaration gives, as a workflow's limit, is read in seconds, minutes, hours or days. It moved here from the
// host of GitHub, where the declaration's limit is read now by the one function that reads it for both the fast and
// the full workflows.
#[test]
fn a_time_is_read_in_seconds_minutes_hours_or_days() {
    use std::time::Duration;
    assert_eq!(agents::span("30s").unwrap(), Duration::from_secs(30));
    assert_eq!(agents::span("90m").unwrap(), Duration::from_secs(5400));
    assert_eq!(agents::span("2h").unwrap(), Duration::from_secs(7200));
    assert_eq!(agents::span("1d").unwrap(), Duration::from_secs(86_400));
    for wrong in ["", "h", "2", "2 weeks", "-1h"] {
        assert!(agents::span(wrong).is_err(), "{wrong:?}");
    }
}

// A project that declares `[ci]` declares both workflows: `run_tests` runs only the fast one and the check before the
// merge only the full ones, so a declaration with one of them leaves one of the two with nothing to run; and each has
// the time it may take.
#[test]
fn a_declaration_of_ci_that_lacks_one_of_its_two_workflows_or_a_time_limit_is_refused_with_where_it_was_read() {
    let text = declaration_text();
    let fast = "\n[ci.fast]\nworkflow = \"fast.yml\"\nrunner = \"github\"\ntoo_long = \"2h\"\n";
    let full = "\n[ci.full]\nworkflows = [\"ci.yml\"]\nrunner = \"github\"\ntoo_long = \"2h\"\n";
    let refused = declared(&format!("{text}{fast}")).unwrap_err();
    assert!(refused.starts_with("main:wf-agents.toml: "), "{refused}");
    assert!(refused.contains("missing field `full`"), "{refused}");
    let refused = declared(&format!("{text}{full}")).unwrap_err();
    assert!(refused.contains("missing field `fast`"), "{refused}");
    let unlimited = fast.replace("too_long = \"2h\"\n", "");
    let refused = declared(&format!("{text}{unlimited}{full}")).unwrap_err();
    assert!(refused.contains("missing field `too_long`"), "{refused}");
    let ci = declared(&format!("{text}{fast}{full}"))
        .unwrap()
        .ci
        .expect("it declares both");
    assert_eq!(
        (
            ci.fast.workflow.as_str(),
            ci.fast.too_long.as_str(),
            ci.full.workflows.as_slice()
        ),
        ("fast.yml", "2h", &["ci.yml".to_string()][..])
    );
}

// A project that measures declares a measure workflow beside the fast and the full one, which it must still have: one that
// declares none is read with none, one that declares it is read with the names of the inputs the engine gives it, and one
// that does not say how long a run may take, or declares it with no other workflow, is refused. Well under a millisecond
// alone; no other test holds the measure workflow's declaration.
#[test]
fn a_declaration_may_name_a_measure_workflow_which_says_how_long_a_run_may_take() {
    let text = declaration_text();
    let fast = "\n[ci.fast]\nworkflow = \"fast.yml\"\nrunner = \"github\"\ntoo_long = \"2h\"\n";
    let full = "\n[ci.full]\nworkflows = [\"ci.yml\"]\nrunner = \"github\"\ntoo_long = \"2h\"\n";
    let measure = "\n[ci.measure]\nworkflow = \"measure.yml\"\nrunner = \"14900k\"\ntoo_long = \"30m\"\n";
    assert_eq!(
        declared(&format!("{text}{fast}{full}"))
            .unwrap()
            .ci
            .expect("both")
            .measure,
        None
    );
    let named = declared(&format!("{text}{fast}{full}{measure}"))
        .unwrap()
        .ci
        .expect("all three")
        .measure;
    let text_of = |s: &str| s.to_string();
    assert_eq!(
        named,
        Some(agents::CiMeasure {
            workflow: text_of("measure.yml"),
            runner: text_of("14900k"),
            too_long: text_of("30m"),
            commit_input: text_of("commit"),
            base_input: text_of("base"),
            call_input: text_of("call"),
            measure_input: text_of("measure"),
            results: text_of("test-results"),
        })
    );
    let lacking = "\n[ci.measure]\nworkflow = \"measure.yml\"\nrunner = \"14900k\"\n";
    let refused = declared(&format!("{text}{fast}{full}{lacking}")).unwrap_err();
    assert!(refused.contains("missing field `too_long`"), "{refused}");
    let refused = declared(&format!("{text}{measure}")).unwrap_err();
    assert!(refused.contains("missing field `fast`"), "{refused}");
    let unreadable = measure.replace("30m", "soon");
    let refused = declared(&format!("{text}{fast}{full}{unreadable}")).unwrap_err();
    assert!(refused.contains("ci.measure.too_long"), "{refused}");
}

// How an entry of the design tree is found is the tree's rules', the same for every project: a `[format]` in
// `agents/tree.toml` that names no kind, or whose heading or reference lacks the place of the kind or of the name,
// could find no entry, and one that lacks a kind the forms are checked for gives the check nothing to check by: each is
// refused as the files are read, with where it was read; one that holds all is taken as written.
#[test]
fn a_format_of_the_tree_that_could_find_no_entry_is_refused_as_the_files_are_read_with_where_they_were_read() {
    let dir = copy("tree-format");
    let file = dir.join("agents/tree.toml");
    let text = fs::read_to_string(&file).unwrap();
    let format = "[format]\nkinds = [{ kind = \"principle\", dir = \"principles/\" }, { kind = \"decision\", dir = \"decisions/\" }]\nheading = \"### {kind} {name}\"\nreference = \"{kind} {name}\"\n";
    assert!(
        text.contains(format),
        "the file of the tests holds the format this test changes"
    );
    let wrong = "[format]\nkinds = []\nheading = \"## {kind}\"\nreference = \"{name}\"\n";
    fs::write(&file, text.replace(format, wrong)).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    assert!(refused.contains(&file.display().to_string()), "{refused}");
    for fault in [
        "format.kinds names no kind",
        "format.heading must hold {name}",
        "format.reference must hold {kind}",
        "format.kinds lacks the kind principle",
        "format.kinds lacks the kind decision",
    ] {
        assert!(refused.contains(fault), "{fault} is not in: {refused}");
    }
    let empty = "[format]\nkinds = [{ kind = \"principle\", dir = \"\" }, { kind = \"decision\", dir = \"decisions/\" }]\nheading = \"## {kind}: {name}\"\nreference = \"{kind} {name}\"\n";
    fs::write(&file, text.replace(format, empty)).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    assert!(
        refused.contains("format.kinds[0] must give a kind and a directory"),
        "{refused}"
    );
    let lacking = "[format]\nkinds = [{ kind = \"principle\", dir = \"principles/\" }]\nheading = \"### {kind} {name}\"\nreference = \"{kind} {name}\"\n";
    fs::write(&file, text.replace(format, lacking)).unwrap();
    let refused = agents::read(&dir).unwrap_err();
    assert!(refused.contains("format.kinds lacks the kind decision"), "{refused}");
    assert!(!refused.contains("lacks the kind principle"), "{refused}");
    let right = "[format]\nkinds = [{ kind = \"decision\", dir = \"decisions/\" }, { kind = \"principle\", dir = \"principles/\" }]\nheading = \"## {kind}: {name}\"\nreference = \"{kind} {name}\"\n";
    fs::write(&file, text.replace(format, right)).unwrap();
    let read = agents::read(&dir).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    let format = read.acts.tree.format;
    assert_eq!(
        format.heading_of("## decision: keep-it  "),
        Some(("decision", "keep-it"))
    );
    assert_eq!(format.heading_of("### decision: keep-it"), None);
    assert_eq!(format.referred("  principle keep-it "), Some(("principle", "keep-it")));
    assert_eq!(
        format.referred("principle keep it"),
        None,
        "a name holds no white space"
    );
    assert_eq!(format.referred("principle "), None, "a name is not empty");
    assert_eq!(format.referred("rule keep-it"), None, "no kind of the format");
}

// The tree's format is the flow's, not the project's. A declaration written before that still gives one, which an
// engine installed before reads: it is accepted where it is the flow's own format, so that it is no second one, and
// refused, with where it was read, where it differs.
#[test]
fn a_declaration_that_gives_a_format_of_the_tree_is_accepted_where_it_is_the_flows_and_refused_where_it_differs() {
    let text = declaration_text();
    let same = "\n[tree_format]\nkinds = [{ kind = \"principle\", dir = \"principles/\" }, { kind = \"decision\", dir = \"decisions/\" }]\nheading = \"### {kind} {name}\"\nreference = \"{kind} {name}\"\n";
    let flows = agents::read(&beside()).unwrap().acts.tree.format;
    assert_eq!(
        flows.heading, "### {kind} {name}",
        "the format this test gives is the flow's"
    );
    declared(&format!("{text}{same}")).expect("a format that is the flow's is taken");
    let other = same.replace("### {kind} {name}", "## {kind}: {name}");
    let refused = declared(&format!("{text}{other}")).expect_err("a format that is not the flow's is refused");
    assert!(refused.starts_with("main:wf-agents.toml: "), "{refused}");
    assert!(refused.contains("tree_format"), "{refused}");
}

// This repository's own declaration, which still holds the items the installed engine reads, is read with the flow's
// files and has no fault.
#[test]
fn the_declaration_of_this_repository_is_read_with_the_files_beside_its_flows() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = fs::read_to_string(root.join("wf-agents.toml")).unwrap();
    let beside = agents::read(&root.join("flow")).expect("the real files beside the flows are read");
    let read = Declaration::read(&text, "main:wf-agents.toml", &beside).expect("the declaration is read");
    assert!(read.tree_format.is_some() && read.files.contains_key("tree_rules"));
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
fn a_file_of_the_project_the_session_names_is_one_its_declaration_has() {
    let dir = copy("project");
    let session = dir.join("agents/session.toml");
    let text = fs::read_to_string(&session).unwrap();
    fs::write(
        &session,
        text.replace("{ project = \"constitution\" }", "{ project = \"charter\" }"),
    )
    .unwrap();
    let read = agents::read(&dir).expect("the files beside the flows do not know the project's files");
    fs::remove_dir_all(&dir).unwrap();
    let refused = Declaration::read(&declaration_text(), "main:wf-agents.toml", &read).unwrap_err();
    assert!(refused.starts_with("main:wf-agents.toml: "), "{refused}");
    assert!(refused.contains("has no file charter"), "{refused}");
    assert!(refused.contains(&session.display().to_string()), "{refused}");
}

// A part of a line's session is a file of the project, the line's rules, words or the return's form: what the guide says
// about a datum is no part of it.
#[test]
fn a_part_of_the_session_that_names_a_datum_is_refused() {
    let dir = copy("datum");
    let session = dir.join("agents/session.toml");
    let text = fs::read_to_string(&session).unwrap();
    fs::write(
        &session,
        text.replace("{ project = \"constitution\" }", "{ datum = \"constitution\" }"),
    )
    .unwrap();
    let refused = agents::read(&dir).unwrap_err();
    fs::remove_dir_all(&dir).unwrap();
    assert!(refused.contains(&session.display().to_string()), "{refused}");
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

// A part of the message other than the context that names an `order` is refused, saying which part and that only the
// context has one: it would order what nothing of it lists. A few files read: milliseconds.
#[test]
fn a_part_of_the_message_other_than_the_context_that_names_an_order_is_refused() {
    let dir = copy("order-on-task");
    let message = dir.join("agents/message/message.toml");
    let text = fs::read_to_string(&message).unwrap();
    let ordered = text.replace(
        "part = \"task\"\n",
        "part = \"task\"\norder = [\"every_agent\", \"reads\", \"watches\", \"outputs\"]\n",
    );
    assert_ne!(ordered, text, "the words have a task part");
    fs::write(&message, ordered).unwrap();
    let read = agents::read(&dir);
    fs::remove_dir_all(&dir).unwrap();
    let refused = read.map(|_| ()).unwrap_err();
    assert!(refused.contains(&message.display().to_string()), "{refused}");
    assert!(
        refused.contains("the part task has an `order`, which only context has"),
        "{refused}"
    );
}

/// The text of `file` under `agents/` in a copy of the files with `key` of `table` taken out, or set to `to`.
fn with_key(dir: &Path, file: &str, table: &[&str], key: &str, to: Option<&str>) -> (PathBuf, String) {
    let path = dir.join("agents").join(file);
    let original = fs::read_to_string(&path).unwrap();
    let mut whole: toml::Table = original.parse().unwrap();
    let mut at = &mut whole;
    for name in table {
        at = at.get_mut(*name).and_then(toml::Value::as_table_mut).unwrap();
    }
    match to {
        Some(text) => assert!(
            at.insert(key.to_string(), text.into()).is_some(),
            "{file}: {key} is there"
        ),
        None => assert!(at.remove(key).is_some(), "{file}: {key} is there"),
    }
    fs::write(&path, toml::to_string(&whole).unwrap()).unwrap();
    (path, original)
}

// A start where the file of the words of the pull request, of the calls the engine ends, or of the program's results
// lacks a text, or holds an empty one, is refused naming the file and the key: the engine would say nothing, or what
// it has no words for, where it speaks. Each key is taken out and then emptied, in a copy of the files.
#[test]
fn a_file_of_the_engines_words_that_lacks_a_text_or_holds_an_empty_one_is_refused_naming_its_file_and_key() {
    let dir = copy("every-word");
    let cases: &[(&str, &[&str], &[&str])] = &[
        (
            "pull_request.toml",
            &[],
            &["about", "body", "cut", "merged", "closed", "superseded"],
        ),
        ("pull_request.toml", &["withheld"], &["data", "repository", "home"]),
        ("calls.toml", &[], &["withdrawn", "waited", "no_such_tool"]),
        (
            "cost.toml",
            &["program"],
            &[
                "no_stream_result",
                "unreadable_result",
                "no_return",
                "error_kind",
                "error",
                "last_attempt",
                "refused",
                "too_long",
                "lines_too_long",
                "not_a_result",
                "no_reason",
            ],
        ),
        ("cost.toml", &["check"], &["request"]),
        ("cost.toml", &["work"], &["failed"]),
    ];
    assert!(agents::read(&dir).is_ok(), "the copy reads before it is changed");
    for (file, table, keys) in cases {
        for key in *keys {
            for to in [None, Some(" ")] {
                let (path, original) = with_key(&dir, file, table, key, to);
                let refused = agents::read(&dir).expect_err(&format!("{file} {key} {to:?}"));
                fs::write(&path, original).unwrap();
                assert!(refused.contains(&path.display().to_string()), "{file} {key}: {refused}");
                assert!(refused.contains(key), "{file} {key} {to:?}: {refused}");
            }
        }
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// The places of `text` that write a number's letter, `W` or `#`, by hand before a place the engine fills with a
/// number already written (`{work}`, `{item}`, `{items}`, `{last}`), whose lines are not comments.
fn hand_written_numbers(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in text.lines().filter(|line| !line.trim_start().starts_with('#')) {
        for place in agents::NUMBER_PLACES {
            let place = format!("{{{place}}}");
            for (at, _) in line.match_indices(&place) {
                if line[..at].ends_with(['W', '#']) {
                    found.push(line.trim().to_string());
                }
            }
        }
    }
    found
}

/// Every `.toml` file under `dir`, with its text.
fn toml_files(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(toml_files(&path));
        } else if path.extension().is_some_and(|e| e == "toml") {
            let text = fs::read_to_string(&path).unwrap();
            files.push((path, text));
        }
    }
    files
}

// The engine fills a number written as its kind writes it (`W43`, `#79`), once: a text that writes the letter before
// the place as well would say it twice.
#[test]
fn no_text_of_the_flows_files_writes_a_numbers_letter_before_the_place_the_engine_fills_with_one() {
    for text in ["Work W{work}", "the item #{item}", "the items #{items} and #{last}"] {
        assert!(!hand_written_numbers(text).is_empty(), "the scan misses {text:?}");
    }
    assert!(hand_written_numbers("Work {work}, on the item {item}.\n# W{work} in a comment").is_empty());
    let flow = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let mut read = 0;
    for dir in ["agents", "data"] {
        for (path, text) in toml_files(&flow.join(dir)) {
            read += 1;
            let found = hand_written_numbers(&text);
            assert!(found.is_empty(), "{}: {found:?}", path.display());
        }
    }
    assert!(read > 20, "the scan reads the files: {read}");
}

/// The lines of `text`, not comments, that put `{why}` where nothing before it on the line ends in a colon.
fn causes_without_a_colon(text: &str) -> Vec<String> {
    (text.lines())
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter(|line| line.split("{why}").count() > 1)
        .filter(|line| {
            let before = line.split("{why}").next().unwrap_or_default();
            !before.trim_end().ends_with(':')
        })
        .map(|line| line.trim().to_string())
        .collect()
}

// A cause someone else wrote, `{why}`, is set off after a colon, so that whatever it begins with, a lower case word, a
// path or a quote, the sentence that holds it reads whole. These two files frame the reasons the owner reads on the
// pause and wait cards.
#[test]
fn a_cause_is_set_off_after_a_colon_in_the_words_of_the_pause_and_wait_cards_and_of_the_program() {
    assert!(!causes_without_a_colon("failed = \"The round failed. {why}\"").is_empty());
    assert!(causes_without_a_colon("failed = \"The round failed. The cause: {why}\"").is_empty());
    let flow = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/agents");
    for file in ["waits.toml", "cost.toml"] {
        let text = fs::read_to_string(flow.join(file)).unwrap();
        assert!(text.contains("{why}"), "{file} frames a cause");
        let found = causes_without_a_colon(&text);
        assert!(found.is_empty(), "{file}: {found:?}");
    }
}

/// Every string a TOML value holds, at any depth.
fn strings(value: &toml::Value, out: &mut Vec<String>) {
    match value {
        toml::Value::String(text) => out.push(text.clone()),
        toml::Value::Array(values) => values.iter().for_each(|value| strings(value, out)),
        toml::Value::Table(table) => table.values().for_each(|value| strings(value, out)),
        _ => {}
    }
}

/// The places of `text` where a number's letter, `W` or `#`, stands right before a place the engine fills with a
/// number already written.
fn lettered(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for place in agents::NUMBER_PLACES {
        let place = format!("{{{place}}}");
        for (at, _) in text.match_indices(&place) {
            if text[..at].ends_with(['W', '#']) {
                found.push(format!("{}{place}", &text[at - 1..at]));
            }
        }
    }
    found
}

// A text of a file is what its TOML says it is, not what its lines look like: a line of a multi-line text that begins
// with `#` is part of the text, and is sent as it is, whatever a reader of the file takes it for. The letter of a
// number is written before a place of the engine's by no text, at any depth of any file. Reference: the TOML
// specification, by which a `#` inside a string is no comment, read by the parser the engine reads the files with.
#[test]
fn no_string_of_the_flows_files_writes_a_numbers_letter_before_the_place_the_engine_fills_with_one_wherever_it_stands()
{
    let hidden: toml::Table = "text = '''\nIn it:\n#{item} is done\n'''\nlast = \"W{work}\"\n"
        .parse()
        .unwrap();
    let hidden = toml::Value::Table(hidden);
    let mut found = Vec::new();
    strings(&hidden, &mut found);
    let found: Vec<String> = found.iter().flat_map(|text| lettered(text)).collect();
    assert_eq!(
        found,
        ["#{item}", "W{work}"],
        "the check finds a line that begins with a hash"
    );
    let flow = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let mut read = 0;
    for dir in ["agents", "data"] {
        for (path, text) in toml_files(&flow.join(dir)) {
            let table: toml::Table = text.parse().unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let value = toml::Value::Table(table);
            let mut all = Vec::new();
            strings(&value, &mut all);
            read += all.len();
            let found: Vec<String> = all.iter().flat_map(|text| lettered(text)).collect();
            assert!(found.is_empty(), "{}: {found:?}", path.display());
        }
    }
    assert!(read > 100, "the check reads the texts: {read}");
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

// A time is the seconds its number and its unit make, as arithmetic gives them. A number of days, hours or minutes whose
// seconds do not fit in 64 bits is no time the engine can wait for: it is refused, where it would stop the engine with
// an overflow (a debug build) or wrap to a short time (a release build). A project's declaration is written by agents,
// and one that cannot be used holds its project; it never stops the engine. Reference: u64::MAX is 18446744073709551615,
// and 213503982334602 days, 5124095576030432 hours and 307445734561825861 minutes each make more seconds than that;
// the next lower number of each makes fewer. Milliseconds.
#[test]
fn a_time_whose_seconds_do_not_fit_is_refused_and_one_that_just_fits_is_read() {
    use std::time::Duration;
    assert_eq!(
        agents::span("213503982334601d").unwrap(),
        Duration::from_secs(213_503_982_334_601 * 86_400)
    );
    assert_eq!(
        agents::span("5124095576030431h").unwrap(),
        Duration::from_secs(5_124_095_576_030_431 * 3600)
    );
    assert_eq!(
        agents::span("307445734561825860m").unwrap(),
        Duration::from_secs(307_445_734_561_825_860 * 60)
    );
    for wrong in [
        "213503982334602d",
        "5124095576030432h",
        "307445734561825861m",
        "18446744073709551616s",
    ] {
        assert!(agents::span(wrong).is_err(), "{wrong:?} is no time");
    }
    // Through the declaration: the fast workflow's limit that is none is refused with its key, and one that is a time
    // is read.
    let text = declaration_text();
    let ci = |fast: &str| {
        format!(
            "{text}\n[ci.fast]\nworkflow = \"fast.yml\"\nrunner = \"github\"\ntoo_long = \"{fast}\"\n\n[ci.full]\n\
             workflows = [\"ci.yml\"]\nrunner = \"github\"\ntoo_long = \"2h\"\n"
        )
    };
    assert!(declared(&ci("2h")).is_ok());
    let refused = declared(&ci("213503982334602d")).unwrap_err();
    assert!(refused.contains("ci.fast.too_long"), "{refused}");
}

// A declaration that names lock commands is refused, as no declaration has them since agents are allowed every shell
// command and a refusal beats an allow; and so is a merge method or branch deletion of the wrong spelling or type, which
// would be taken for another. Reference: the declaration's own table of keys (`Heavy`, `Merging`), and TOML's types.
// Milliseconds.
#[test]
fn a_lock_command_or_a_misspelled_or_mistyped_merge_or_deletion_setting_is_refused_not_taken_for_none() {
    let text = declaration_text();
    let heavy = "commands = [\"cargo\", \"npm\"]";
    let pull = "\n[pull_request]\n";
    for (what, wrong, said) in [
        (
            "lock commands",
            text.replace(heavy, &format!("{heavy}\nlocks = [\"cargo update\"]")),
            "unknown field `locks`",
        ),
        (
            "uses of lock commands refused",
            text.replace(heavy, &format!("{heavy}\nlocks_except = [\"cargo update *\"]")),
            "unknown field `locks_except`",
        ),
        (
            "merge method of another case",
            format!("{text}{pull}merge_method = \"Squash\"\ndelete_branch = false\n"),
            "pull_request.merge_method: one of squash, merge, rebase, not \"Squash\"",
        ),
        (
            "merge method empty",
            format!("{text}{pull}merge_method = \"\"\ndelete_branch = false\n"),
            "pull_request.merge_method: one of squash, merge, rebase, not \"\"",
        ),
        (
            "deletion a string",
            format!("{text}{pull}merge_method = \"squash\"\ndelete_branch = \"false\"\n"),
            "invalid type",
        ),
        (
            "deletion left out",
            format!("{text}{pull}merge_method = \"squash\"\n"),
            "missing field `delete_branch`",
        ),
        (
            "merge method left out",
            format!("{text}{pull}delete_branch = false\n"),
            "missing field `merge_method`",
        ),
        ("check left out", format!("{text}\n[local]\n"), "missing field `check`"),
    ] {
        let refused = declared(&wrong).expect_err(what);
        assert!(refused.starts_with("main:wf-agents.toml: "), "{what}: {refused}");
        assert!(refused.contains(said), "{what}: {refused}");
    }
}
