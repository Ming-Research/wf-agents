//! The system prompt of the lines of the engineer's flow as the repository defines it (`flow/engineer`), made from the
//! files beside it, with the sample's settings.

use std::collections::BTreeSet;

use flow::{CellSpec, Compiled, Has};

use super::*;
use crate::permissions::Permissions;

/// The directory that holds the repository's flows.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

fn beside() -> Agents {
    flow::agents::read(&root()).expect("the files beside the flows are read")
}

/// The engineer's flow, compiled against what the engine has for it.
fn engineer() -> Compiled {
    let source = flow::read(&root().join("engineer")).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "handed_in",
            "main",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

/// The sample's settings on GitHub with a fast CI, whose checks by name the words of `run_tests` name.
fn settings() -> Settings {
    let mut settings = Settings::read(include_str!("../../install.sample.toml")).expect("the sample reads");
    let github = "repository = \"owner/name\"\n";
    settings.projects.get_mut("example").unwrap().github = Some(toml::from_str(github).unwrap());
    settings
}

/// The project's files, by their paths in `project.toml`.
fn project(path: &str) -> std::io::Result<String> {
    match path {
        "AGENTS.md" => Ok("# Working here\n\nThe guide.\n".to_string()),
        "design/constitution.md" => Ok("# The constitution\n\nArticle 1.\n".to_string()),
        "design/README.md" => Ok("# The design tree\n\nIts rules.\n".to_string()),
        _ => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
    }
}

/// The lines of the flow's agent cells, each once, in the order of their first cell.
fn lines(flow: &Compiled) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in flow.cells.iter().filter_map(|c| c.line.clone()) {
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines
}

/// The first agent cell of the line.
fn cell<'a>(flow: &'a Compiled, line: &str) -> &'a CellSpec {
    (flow.cells.iter())
        .find(|c| c.line.as_deref() == Some(line))
        .expect("a cell of the line")
}

fn of(line: &str) -> String {
    let (settings, beside, flow) = (settings(), beside(), engineer());
    let cell = cell(&flow, line);
    let protected = &beside.project.protected.paths;
    let tests = cell.tests_only.then(|| crate::tests_only::paths(&beside.project));
    let permissions = Permissions::of(cell.reads_only, &[], &[], &Default::default());
    let your_line = permissions.told(&beside.your_line, protected, tests);
    of_line(&beside, &filling(&settings, &beside), &flow, cell, &your_line, project).expect("it is made")
}

// The system prompt holds, in the order `session.toml` names them, the project's files each headed by its path, the
// line's own sets of rules, the engine's words with what the line may do filled in, and how a cell returns; each
// setting they name, but in the project's files, with its value from the settings or from `project.toml`.
#[test]
fn a_lines_system_prompt_holds_its_parts_in_their_order_with_the_settings_they_name() {
    let engineer = of("engineer");
    let at = |text: &str| engineer.find(text).unwrap_or_else(|| panic!("{text}:\n{engineer}"));
    let order = [
        "# `AGENTS.md`\n\n# Working here\n\nThe guide.",
        "# `design/constitution.md`\n\n# The constitution",
        "# `design/README.md`\n\n# The design tree",
        "# How every agent works\n\n- Check each fact when you write it",
        "# How an engineer works",
        "# How you work here",
        "## How you return",
    ];
    let places: Vec<usize> = order.iter().map(|text| at(text)).collect();
    assert!(places.windows(2).all(|w| w[0] < w[1]), "{engineer}");
    assert!(engineer.starts_with(order[0]), "{engineer}");
    assert!(
        engineer.contains(
            "worktree of the work's own branch. Make your changes there and commit them there. The project protects \
             `AGENTS.md`, "
        ),
        "{engineer}"
    );
    assert!(engineer.contains("`.github/workflows/`: change what is there only where the work needs it. Never"));
    assert!(
        engineer.contains("\n- `protected_changes`: for each file the project protects"),
        "{engineer}"
    );
    for named in [
        "`agents.subagents.most` (3)",
        "`project.research` (research/)",
        "`owner.language` (Chinese)",
        "and checks among `lint`;",
        "`project.test_names` (To name a test to `run_tests`, an agent gives the test's name",
    ] {
        assert!(engineer.contains(named), "{named}:\n{engineer}");
    }
    assert!(
        !engineer.contains("{your_line}") && !engineer.contains("{store}"),
        "{engineer}"
    );
    let reviewer = of("code_reviewer");
    assert!(reviewer.contains("You only read it, and change no file."), "{reviewer}");
    assert!(!reviewer.contains("Make your changes there"), "{reviewer}");
}

// The tester's line changes only the project's tests, as its file says: its system prompt tells it so, with the paths
// `project.toml` names for them, and the paths the project protects after them.
#[test]
fn the_testers_system_prompt_says_it_changes_only_the_projects_tests() {
    let tester = of("tester");
    let told = "Make your changes there and commit them there. Change only the project's tests, the files that \
                `:(glob)crates/**/tests/**`, ";
    assert!(tester.contains(told), "{tester}");
    assert!(
        tester.contains(
            "`:(glob)client/**/fixtures.ts` match. If a test must go into another file, say so in what you hand in. \
             The project protects `AGENTS.md`, "
        ),
        "{tester}"
    );
    assert!(!of("engineer").contains("Change only the project's tests"));
}

// A line that changes only the project's tests is held to the paths its project's declaration names for them: a
// declaration that names none keeps the engine from starting, said of the line.
#[test]
fn a_line_that_changes_only_the_tests_of_a_project_that_names_none_is_refused_as_the_engine_starts() {
    let (beside, flow) = (beside(), engineer());
    let fault = "flow engineer: the line tester changes only the project's tests, and project.toml names none in \
                 [tests] paths";
    for declared in [None, Some(Vec::new())] {
        let mut beside = beside.clone();
        match declared {
            None => beside.project.tests = None,
            Some(paths) => {
                beside
                    .project
                    .tests
                    .as_mut()
                    .expect("the project declares its tests")
                    .paths = paths
            }
        }
        assert_eq!(faults(&settings(), &beside, &flow), [fault]);
    }
}

// What the run of a line that changes only the project's tests changed is read from the version of the work's head it
// began on: a flow with no block of the work's head keeps the engine from starting, said of the line.
#[test]
fn a_line_that_changes_only_the_tests_of_a_flow_with_no_head_is_refused_as_the_engine_starts() {
    let mut flow = engineer();
    assert!(faults(&settings(), &beside(), &flow).is_empty());
    flow.blocks
        .retain(|_, block| block.writers.outside.as_deref() != Some(crate::head::HEAD));
    let fault = "flow engineer: the line tester changes only the project's tests, and the flow has no block of the \
                 work's head (`head`), from which what its run changed is read";
    assert_eq!(faults(&settings(), &beside(), &flow), [fault]);
}

// The guide is a file of the project like the constitution and the tree's rules (design document `data-and-context`,
// 24; decision the-projects-files-as-the-work-has-them): a session begins with the file as the work's worktree has it
// as the session begins, a change the work made to it before then included.
#[test]
fn a_session_begins_with_the_guide_the_worktree_has_as_it_begins() {
    let (settings, beside, flow) = (settings(), beside(), engineer());
    let filling = filling(&settings, &beside);
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::create_dir_all(dir.path().join("design")).unwrap();
    for path in ["AGENTS.md", "design/constitution.md", "design/README.md"] {
        std::fs::write(dir.path().join(path), project(path).unwrap()).unwrap();
    }
    std::fs::write(dir.path().join("AGENTS.md"), "# Working here\n\nChanged by the work.\n").unwrap();
    let read = |path: &str| read_within(dir.path(), path);
    let began = of_line(&beside, &filling, &flow, cell(&flow, "engineer"), "", read).expect("it is made");
    assert!(
        began.starts_with("# `AGENTS.md`\n\n# Working here\n\nChanged by the work."),
        "{began}"
    );
}

// A session keeps the system prompt and the form it began with, each alike: made and kept where it has none, and the
// kept one given whenever it has one, whatever it was made of became since and however the round begins in it.
#[test]
fn a_session_keeps_the_system_prompt_and_the_form_it_began_with() {
    let dir = tempfile::tempdir().expect("a directory");
    let made = |text: &'static str| move || Ok::<_, String>(text.to_string());
    for what in [Begun::System, Begun::Form] {
        let at = kept_at(dir.path(), Uuid::nil(), what);
        assert_eq!(of_session(&at, made("as it began")).unwrap(), "as it began", "{what:?}");
        assert_eq!(
            of_session(&at, made("changed since")).unwrap(),
            "as it began",
            "{what:?}"
        );
        // Another session, under an id of its own, is made anew.
        let other = kept_at(dir.path(), Uuid::max(), what);
        assert_eq!(of_session(&other, made("another")).unwrap(), "another", "{what:?}");
        assert_eq!(
            of_session(&at, made("changed again")).unwrap(),
            "as it began",
            "{what:?}"
        );
    }
    // Each is kept apart: the form the session began with is not its system prompt.
    let form = kept_at(dir.path(), Uuid::nil(), Begun::Form);
    assert_ne!(form, kept_at(dir.path(), Uuid::nil(), Begun::System));
}

// A session whose first round was cut off before the program began it is begun again under its id, and one the
// program lost is begun anew under it: either way it keeps the prompt it was first given, though the files changed.
#[test]
fn a_session_cut_off_and_begun_again_keeps_its_prompt() {
    let (settings, beside, flow) = (settings(), beside(), engineer());
    let filling = filling(&settings, &beside);
    let cell = cell(&flow, "engineer");
    let dir = tempfile::tempdir().expect("a directory");
    let worktree = dir.path().join("worktree");
    std::fs::create_dir_all(worktree.join("design")).unwrap();
    for path in ["AGENTS.md", "design/constitution.md", "design/README.md"] {
        std::fs::write(worktree.join(path), project(path).unwrap()).unwrap();
    }
    let make = || {
        let read = |path: &str| read_within(&worktree, path);
        of_line(&beside, &filling, &flow, cell, "", read)
    };
    let at = kept_at(&dir.path().join("data"), Uuid::nil(), Begun::System);
    // The first try begins the session, and is cut off before the program runs.
    let began = of_session(&at, make).unwrap();
    // The work changes its guide; the round is tried again, the session begun again under its id, then gone on in.
    std::fs::write(worktree.join("AGENTS.md"), "# Working here\n\nChanged.\n").unwrap();
    assert_eq!(of_session(&at, make).unwrap(), began);
    assert!(!of_session(&at, make).unwrap().contains("Changed."));
}

/// A session of the engineer's line of a work, in a directory of its own: the work's worktree, with the project's
/// files, and the data directory its session's system prompt and form are kept in.
struct Line {
    dir: tempfile::TempDir,
    of: crate::Of,
    flow: Compiled,
}

impl Line {
    fn new() -> Line {
        let dir = tempfile::tempdir().expect("a directory");
        std::fs::create_dir_all(dir.path().join("worktree/design")).unwrap();
        for path in ["AGENTS.md", "design/constitution.md", "design/README.md"] {
            std::fs::write(dir.path().join("worktree").join(path), project(path).unwrap()).unwrap();
        }
        let settings = settings();
        let project = settings.projects["example"].clone();
        let of = crate::Of::new(&settings, project, beside(), "refs/heads/main".to_string());
        Line {
            dir,
            of,
            flow: engineer(),
        }
    }

    fn worktree(&self) -> PathBuf {
        self.dir.path().join("worktree")
    }

    /// Where the session of this id keeps what it began with.
    fn kept(&self, session: Uuid, what: Begun) -> PathBuf {
        kept_at(&self.dir.path().join("data"), session, what)
    }

    /// What a new session of the line begins with now, as a round makes it.
    fn now(&self, what: Begun) -> Result<String, String> {
        let cell = cell(&self.flow, "engineer");
        let permissions = Permissions::of(cell.reads_only, &[], &[], &Default::default());
        self.of
            .begins_with(&self.flow, cell, &permissions, &self.worktree(), what)
    }

    /// The session of this id begins, as its first round begins it: what it begins with is made and kept.
    fn begin(&self, session: Uuid) {
        for what in [Begun::System, Begun::Form] {
            of_session(&self.kept(session, what), || self.now(what)).expect("it is made");
        }
    }

    /// Whether the session of this id would be given otherwise than a new one would begin with now.
    fn changed(&self, session: Uuid) -> bool {
        changed(&self.dir.path().join("data"), session, |what| self.now(what))
    }
}

// A session is given otherwise than a new one would begin with now where the system prompt or the form it began with,
// either alone, is not what is made now, byte for byte, as a change of the project's files in the worktree makes it;
// it is not where each is, as for a session just begun from the same worktree and flow, which two makings give alike;
// and one that began with neither kept, as one whose first round has not begun, is given nothing else. Runs on
// temporary files in milliseconds; only this test makes the check of real prompts and forms, whose bytes must not
// differ from one making to the next, or every round of a line would begin a new session.
#[test]
fn a_session_is_given_otherwise_than_a_new_one_would_be_where_what_it_began_with_differs_from_what_is_made_now() {
    let line = Line::new();
    let session = Uuid::nil();
    assert!(!line.changed(session), "a session that has not begun");
    line.begin(session);
    for what in [Begun::System, Begun::Form] {
        let (once, again) = (line.now(what), line.now(what));
        assert_eq!(once, again, "{what:?} is made alike twice");
    }
    assert!(!line.changed(session), "a session just begun");
    for what in [Begun::System, Begun::Form] {
        let began = std::fs::read_to_string(line.kept(session, what)).unwrap();
        std::fs::write(
            line.kept(session, what),
            format!("{began}\nAs an earlier flow made it."),
        )
        .unwrap();
        assert!(line.changed(session), "the kept {what:?} differs alone");
        std::fs::write(line.kept(session, what), &began).unwrap();
        assert!(!line.changed(session), "the kept {what:?} is what is made now again");
    }
    // A session that keeps only its system prompt, its form made as it is given, is compared by that alone.
    std::fs::remove_file(line.kept(session, Begun::Form)).unwrap();
    assert!(!line.changed(session));
    std::fs::write(
        line.worktree().join("AGENTS.md"),
        "# Working here\n\nChanged by the work.\n",
    )
    .unwrap();
    assert!(line.changed(session), "the work changed its guide");
    // A new session, begun after the change, begins with what a new one does.
    line.begin(Uuid::max());
    assert!(!line.changed(Uuid::max()));
}

// A session whose kept system prompt or form can be read, where a new one cannot be made now, as for a file of the
// project the worktree has lost, is given otherwise than a new one: its line begins a new session, whose first round
// fails to make it and says why. That new session keeps nothing, so it is given nothing else, and its line begins no
// other for it. A kept one that cannot be read is no reason for one: the round that reads it fails, saying why. Runs on
// temporary files in milliseconds; only this test holds the check to what a round does with each fault.
#[test]
fn a_session_whose_new_prompt_cannot_be_made_is_given_otherwise_and_one_whose_kept_prompt_cannot_be_read_is_not() {
    let line = Line::new();
    let (session, renewed) = (Uuid::nil(), Uuid::max());
    line.begin(session);
    std::fs::remove_file(line.worktree().join("design/README.md")).unwrap();
    assert!(line.now(Begun::System).is_err(), "no system prompt can be made");
    assert!(line.changed(session));
    assert!(!line.changed(renewed), "the new session keeps nothing");
    // What its first round finds: no system prompt is made, and nothing is kept for it.
    assert!(of_session(&line.kept(renewed, Begun::System), || line.now(Begun::System)).is_err());
    assert!(!line.changed(renewed));
    // The first session's kept system prompt cannot be read, here a directory in its place: though no new one can be
    // made, its line begins no new session for it.
    std::fs::remove_file(line.kept(session, Begun::System)).unwrap();
    std::fs::create_dir_all(line.kept(session, Begun::System)).unwrap();
    assert!(!line.changed(session));
}

// A project's file that is not in the worktree makes no system prompt: it is said, with its path.
#[test]
fn a_projects_file_the_worktree_lacks_is_said() {
    let (beside, flow) = (beside(), engineer());
    let lacking = |path: &str| match path {
        "design/README.md" => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
        _ => project(path),
    };
    let cell = cell(&flow, "engineer");
    let filling = filling(&settings(), &beside);
    let why = of_line(&beside, &filling, &flow, cell, "", lacking).expect_err("no tree rules");
    assert!(
        why.starts_with("design/README.md cannot be read in the work's worktree"),
        "{why}"
    );
}

// No file of the project is read through a symbolic link in the worktree: neither a link that is the file, nor one that
// is a directory above it; nor a path that leaves the worktree.
#[test]
fn a_projects_file_is_never_read_through_a_link() {
    let dir = tempfile::tempdir().expect("a directory");
    let (worktree, elsewhere) = (dir.path().join("worktree"), dir.path().join("elsewhere"));
    std::fs::create_dir_all(worktree.join("design")).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("README.md"), "Rules from elsewhere.\n").unwrap();
    std::fs::write(worktree.join("design/constitution.md"), "Article 1.\n").unwrap();
    assert_eq!(
        read_within(&worktree, "design/constitution.md").unwrap(),
        "Article 1.\n"
    );
    std::os::unix::fs::symlink(elsewhere.join("README.md"), worktree.join("AGENTS.md")).unwrap();
    let file = read_within(&worktree, "AGENTS.md").expect_err("a linked file");
    assert!(file.to_string().contains("AGENTS.md is a symbolic link"), "{file}");
    std::os::unix::fs::symlink(&elsewhere, worktree.join("tree")).unwrap();
    let above = read_within(&worktree, "tree/README.md").expect_err("a linked directory");
    assert!(above.to_string().starts_with("tree is a symbolic link"), "{above}");
    let out = read_within(&worktree, "../elsewhere/README.md").expect_err("out of the worktree");
    assert!(out.to_string().contains("no path within the repository"), "{out}");
}

// Every fault of every text the flow's agents are sent is said as the engine starts, all at once, each against the
// file or the thing of the flow that holds the text; none where every setting has a value.
#[test]
fn every_text_sent_naming_a_setting_with_no_value_is_refused_as_the_engine_starts() {
    let (mut beside, flow) = (beside(), engineer());
    assert_eq!(faults(&settings(), &beside, &flow), Vec::<String>::new());
    // A setting the rules and two tasks name has no value.
    beside.project.project.remove("research");
    beside.installation.names.push("project.research".to_string());
    let lines = lines(&flow);
    assert_eq!(
        lines,
        [
            "engineer",
            "deep_researcher",
            "plan_reviewer",
            "code_reviewer",
            "tester"
        ]
    );
    let mut expected = Vec::new();
    for line in &lines {
        let line = format!("flow engineer: the system prompt of the line {line}");
        expected.push(format!(
            "{line}: flow/agents/rules/engineering.md: names the setting `project.research`, which has no value"
        ));
    }
    for cell in ["R1", "D1"] {
        expected.push(format!(
            "flow engineer: cell {cell}, its task: names the setting `project.research`, which has no value"
        ));
    }
    assert_eq!(faults(&settings(), &beside, &flow), expected);
}

// The checks an agent may name to `run_tests` are said as the project declares its fast workflow: the checks it runs by
// name, or that it runs none by name; with none declared, that nothing is run alone. No way of declaring it keeps the
// engine from starting.
#[test]
fn the_checks_an_agent_may_name_are_said_as_the_project_declares_its_fast_workflow() {
    let (beside, flow) = (beside(), engineer());
    let ways = &beside.tools["run_tests"].places["named_checks"];
    let told = |beside: &Agents| {
        assert_eq!(faults(&settings(), beside, &flow), Vec::<String>::new());
        let cell = cell(&flow, "engineer");
        of_line(beside, &filling(&settings(), beside), &flow, cell, "", project).expect("it is made")
    };
    let declared = |checks: &[&str]| {
        let mut declared = beside.clone();
        let ci = declared
            .project
            .ci
            .as_mut()
            .expect("the project declares its workflows");
        ci.fast.checks = checks.iter().map(|c| c.to_string()).collect();
        declared
    };
    let fast = told(&beside);
    assert!(fast.contains("and checks among `lint`;"), "{fast}");
    assert!(told(&declared(&["lint", "page"])).contains("and checks among `lint`, `page`;"));
    assert!(told(&declared(&[])).contains(&format!("and checks among {};", ways["none_named"])));
    // A project that declares no workflows, on GitHub or on the local repository alike.
    let mut no_fast = beside.clone();
    no_fast.project.ci = None;
    let whole = told(&no_fast);
    assert!(
        whole.contains(&format!("and checks among {};", ways["whole"])),
        "{whole}"
    );
    assert!(!whole.contains("{named_checks}"), "{whole}");
    let mut local = settings();
    local.projects.get_mut("example").unwrap().github = None;
    assert_eq!(faults(&local, &beside, &flow), Vec::<String>::new());
}

// A place the engine does not fill, in the heading a file of the project stands under or in the engine's words, and a
// set of rules or a tool nothing beside the flows defines, are said as the engine starts, every one of them.
#[test]
fn every_place_rule_set_and_tool_that_cannot_be_made_is_said_at_once() {
    let (mut beside, flow) = (beside(), engineer());
    beside.session.heading = "# `{path}` {where}\n\n{text}".to_string();
    beside.rules.remove("engineering");
    beside.tools.remove("run_tests");
    let words = beside.words.values_mut().next().expect("the engine's words");
    words.push_str("\n{unknown}\n");
    let faults = faults(&settings(), &beside, &flow);
    let engineer: Vec<&String> = (faults.iter())
        .filter(|f| f.starts_with("flow engineer: the system prompt of the line engineer: "))
        .collect();
    let said: Vec<&str> = (engineer.iter())
        .map(|f| f.trim_start_matches("flow engineer: the system prompt of the line engineer: "))
        .collect();
    assert_eq!(
        said,
        [
            "agents/rules/rules.toml has no set `engineering`, which a line takes",
            "the flow names the tool `run_tests`, which agents/tools/ does not have",
            "flow/agents/session.toml: the engine fills no place `{where}`",
            "flow/agents/how_you_work_here.md: the engine fills no place `{unknown}`",
        ]
    );
    // Every line is said, not only the first.
    assert!(faults.iter().any(|f| f.contains("of the line tester: ")), "{faults:?}");
}

// The engine does not start where main, as the host last gave it, lacks a file every system prompt holds, holds it
// through a symbolic link, or where a file is not named within the repository; each said with its name in
// `project.toml`.
#[test]
fn a_project_file_main_lacks_or_links_is_said() {
    let repo = crate::git::tests::Repo::new("true");
    let project = repo.project.clone();
    let mut beside = beside();
    let missing = project_files(&project, &beside);
    assert_eq!(missing.len(), 3, "{missing:?}");
    assert_eq!(
        missing[0],
        "project.toml: the file guide, AGENTS.md, is not on main, refs/heads/main, where works branch from"
    );
    let dir = &project.repo;
    std::fs::create_dir_all(dir.join("design")).unwrap();
    crate::git::tests::commit(dir, "AGENTS.md", "The guide.\n");
    crate::git::tests::commit(dir, "design/constitution.md", "The constitution.\n");
    assert_eq!(
        project_files(&project, &beside),
        [
            "project.toml: the file tree_rules, design/README.md, is not on main, refs/heads/main, where works branch from"
        ]
    );
    // A file in the checkout that main does not hold is missing all the same.
    std::fs::write(dir.join("design/README.md"), "Not committed.\n").unwrap();
    assert_eq!(project_files(&project, &beside).len(), 1);
    // A link on main, the file itself or a directory above it, is refused.
    std::fs::remove_file(dir.join("design/README.md")).unwrap();
    std::os::unix::fs::symlink("constitution.md", dir.join("design/README.md")).unwrap();
    crate::git::tests::run(dir, &["add", "design/README.md"]);
    crate::git::tests::run(dir, &["commit", "-q", "-m", "Link the tree's rules"]);
    assert_eq!(
        project_files(&project, &beside),
        [
            "project.toml: the file tree_rules, design/README.md: design/README.md is a symbolic link on main, \
             refs/heads/main, which no file of the project is read through"
        ]
    );
    std::fs::create_dir_all(dir.join("real")).unwrap();
    crate::git::tests::commit(dir, "real/rules.md", "The rules.\n");
    std::os::unix::fs::symlink("real", dir.join("linked")).unwrap();
    crate::git::tests::run(dir, &["add", "linked"]);
    crate::git::tests::run(dir, &["commit", "-q", "-m", "Link a directory"]);
    beside.project.files.get_mut("tree_rules").unwrap().path = "linked/rules.md".to_string();
    assert_eq!(
        project_files(&project, &beside),
        [
            "project.toml: the file tree_rules, linked/rules.md: linked is a symbolic link on main, refs/heads/main, \
             which no file of the project is read through"
        ]
    );
    // A file not named within the repository is refused before main is looked at.
    beside.project.files.get_mut("tree_rules").unwrap().path = "../outside.md".to_string();
    assert_eq!(
        project_files(&project, &beside),
        ["project.toml: the file tree_rules is named by its path within the repository, not \"../outside.md\""]
    );
}

// The files are looked for on main as the host last gave it: on GitHub, the reference main is fetched to, and not the
// local branch; where that reference is not there, that is said.
#[test]
fn the_project_files_are_looked_for_where_works_branch_from() {
    let repo = crate::git::tests::Repo::new("true");
    let mut project = repo.project.clone();
    project.github = settings().projects["example"].github.clone();
    let dir = &project.repo;
    std::fs::create_dir_all(dir.join("design")).unwrap();
    for path in ["AGENTS.md", "design/constitution.md", "design/README.md"] {
        crate::git::tests::commit(dir, path, "Some text.\n");
    }
    let remote = project.github.as_ref().unwrap().remote.clone();
    let fetched = format!("refs/remotes/{remote}/main");
    assert_eq!(
        project_files(&project, &beside()),
        [format!(
            "{fetched}, main's head where works branch from, is not in the project's repository: the files \
             project.toml names are not looked for there"
        )]
    );
    // Main as GitHub last gave it, before the files were committed on the local branch.
    let first = crate::git::tests::run(dir, &["rev-list", "--max-parents=0", "HEAD"]);
    crate::git::tests::run(dir, &["update-ref", &fetched, &first]);
    assert_eq!(project_files(&project, &beside()).len(), 3);
    crate::git::tests::run(dir, &["update-ref", &fetched, "HEAD"]);
    assert_eq!(project_files(&project, &beside()), Vec::<String>::new());
}

// The place `{store}` in the words of `query` is filled by `crate::store`: the file's words that the store is not open
// where the settings leave it closed, which they do unless the owner opens it, and what is given as the description
// where it is.
#[test]
fn the_store_the_words_of_query_name_is_what_the_settings_say_of_it() {
    let (beside, flow) = (beside(), engineer());
    let closed = &beside.tools["query"].texts["closed"];
    let cell = cell(&flow, "engineer");
    let prompt = |filling: &Filling| of_line(&beside, filling, &flow, cell, "", project).expect("it is made");
    let settings = settings();
    assert!(
        !settings.query.open,
        "the store is closed to agents unless the owner opens it"
    );
    assert_eq!(&crate::store(&settings, &beside).unwrap(), closed);
    let shut = prompt(&filling(&settings, &beside));
    assert!(shut.contains(&format!("{{\"sql\": \"...\"}}`. {closed}")), "{shut}");
    assert!(!shut.contains("{store}"), "{shut}");
    // What describes the store, as `crate::store` gives it where it is open, reaches the prompt in the same place.
    let mut open = filling(&settings, &beside);
    open.store = "### `things`\n- `id` INTEGER".to_string();
    let told = prompt(&open);
    assert!(told.contains("### `things`\n- `id` INTEGER"), "{told}");
    assert!(!told.contains(closed.as_str()), "{told}");
}

/// The five columns of the real schema that hold what agents may not read, each with its table.
const HIDDEN_COLUMNS: [(&str, &str); 5] = [
    ("ran", "directory"),
    ("backup_failures", "why"),
    ("results", "result"),
    ("back", "back"),
    ("calls", "what"),
];

// With the store open and the real schema, the description of the store reaches the system prompt, with each table the
// schema keeps and none of the columns it hides.
#[test]
fn the_real_store_opened_to_agents_is_described_in_the_system_prompt_without_the_columns_the_schema_hides() {
    let (beside, flow) = (beside(), engineer());
    let mut open = settings();
    open.query.open = true;
    assert_eq!(faults(&open, &beside, &flow), Vec::<String>::new());
    let cell = cell(&flow, "engineer");
    let told = of_line(&beside, &filling(&open, &beside), &flow, cell, "", project).expect("it is made");
    let described = crate::store(&open, &beside).expect("the schema hides what agents may not read");
    assert!(told.contains(&described), "{told}");
    assert!(
        !told.contains("{store}") && !told.contains("is not open to agents yet"),
        "{told}"
    );
    for (table, hidden) in HIDDEN_COLUMNS {
        let section = described
            .split("\n\n")
            .find(|s| s.starts_with(&format!("### `{table}`")))
            .unwrap_or_else(|| panic!("{table}:\n{described}"));
        assert!(
            section.lines().skip(1).any(|l| l.starts_with("- `")),
            "the table keeps its other columns: {section}"
        );
        assert!(
            !section.lines().any(|l| l.starts_with(&format!("- `{hidden}` "))),
            "{table}.{hidden}:\n{section}"
        );
    }
}

// The words of what a cell returns are sent in a message, which gives each setting they name with its value: one with no
// value is refused as the engine starts, as in any other text sent.
#[test]
fn the_words_of_what_a_cell_returns_naming_a_setting_with_no_value_are_refused_as_the_engine_starts() {
    let (mut beside, flow) = (beside(), engineer());
    beside.returns.outputs.none = "This cell writes no block; it reads `project.research`.".to_string();
    assert_eq!(faults(&settings(), &beside, &flow), Vec::<String>::new());
    beside.project.project.remove("research");
    beside.installation.names.push("project.research".to_string());
    let said = "flow engineer: flow/agents/return/return.toml, [outputs]: names the setting `project.research`, which \
                has no value";
    let faults = faults(&settings(), &beside, &flow);
    assert!(faults.iter().any(|f| f == said), "{faults:#?}");
}
