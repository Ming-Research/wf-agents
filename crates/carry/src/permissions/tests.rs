use super::*;
use flow::agents::Ability;

/// What the place `{your_line}` says, as the files beside the repository's flows write it.
fn words() -> YourLine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/agents/your_line.toml");
    toml::from_str(&std::fs::read_to_string(&path).expect("the file is read")).expect("it reads")
}

/// The commands a round of the tests' project is refused, as the files beside the flows and its declaration give them:
/// the heavy commands of a project on many toolchains, and pushing, which every agent is refused.
const REFUSED: [&str; 29] = [
    "cargo",
    "rustc",
    "rustup",
    "make",
    "cmake",
    "ninja",
    "bazel",
    "tsc",
    "npx",
    "pnpm",
    "yarn",
    "bun",
    "deno",
    "npm",
    "node --test",
    "vitest",
    "jest",
    "go build",
    "go test",
    "go run",
    "pytest",
    "python -m pytest",
    "mvn",
    "gradle",
    "swift build",
    "swift test",
    "xcodebuild",
    "hyperfine",
    "git push",
];

/// Those, as a round is given them.
fn commands() -> Vec<String> {
    REFUSED.map(str::to_string).to_vec()
}

/// The files beside the repository's flows.
fn beside() -> Agents {
    flow::agents::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow"))
        .expect("what is written beside the flows is read")
}

/// The temporary directory the tests hand the program, as `$TMPDIR` writes it on some machines: with a trailing `/`.
const TEMP: &str = "/x/T/";

/// The worktree the tests hand the program, with no trailing slash.
const WORKTREE: &str = "/x/W";

/// The permissions of a line, for what every agent may do as the repository's flows say, in the test's worktree and
/// temporary directory.
fn of(reads_only: bool) -> Permissions {
    Permissions::of(
        reads_only,
        &beside().allowed.may,
        Path::new(WORKTREE),
        Path::new(TEMP),
        &commands(),
    )
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| a.to_string()).collect()
}

// A line that only reads is refused every tool that edits a file and every git command that changes the repository,
// and may still read with git; a line that writes is refused neither.
#[test]
fn a_line_that_only_reads_edits_nothing_and_changes_no_repository() {
    let refused = of(true).refused;
    for t in EDIT_TOOLS {
        assert!(refused.iter().any(|r| r == t), "{t}");
    }
    for g in GIT_CHANGES {
        assert!(refused.contains(&format!("Bash(git {g} *)")), "git {g}");
    }
    for g in ["show", "diff", "log", "fetch", "grep"] {
        assert!(!refused.contains(&format!("Bash(git {g} *)")), "git {g}");
    }
    let writes = of(false).refused;
    assert!(!writes.iter().any(|r| r == "Edit" || r == "Bash(git commit *)"));
}

// Every line is refused every heavy command and pushing whole, though the words let it run shell commands: the command
// line refuses what any rule refuses even where another allows it, so `Bash` allowed whole takes none of them back.
// Reference: the command line's own rule, that a refusal beats an allow.
#[test]
fn every_heavy_command_is_refused_whole_though_shell_commands_are_allowed() {
    for reads_only in [false, true] {
        let permissions = of(reads_only);
        assert!(
            permissions.allowed.iter().any(|r| r == "Bash"),
            "shell is allowed whole"
        );
        for command in REFUSED {
            assert!(
                permissions.refused.contains(&format!("Bash({command} *)")),
                "{command} on a line with reads_only {reads_only}"
            );
        }
        let given = permissions.arguments();
        let at = given.iter().position(|a| a == "--disallowedTools").expect("refused");
        assert_eq!(given[at + 1..], permissions.refused[..]);
    }
}

// The words of what agents may do are turned into what the program's own flags take: the tools it is given, the rules
// that allow them, and the directories it may reach. The real words give exactly what the installation gave before it
// was wf-agents' to give: the same tools and the same allow rules, none twice, and the engine's temporary directory
// with no trailing slash, as the rule and the flag write it. Reference: the installation's own former arguments.
#[test]
fn the_flows_words_are_turned_into_the_programs_tools_rules_and_directories() {
    let permissions = of(false);
    let mut tools = permissions.tools.clone();
    tools.sort();
    let mut wanted = strings(&[
        "Bash",
        "Read",
        "Edit",
        "Write",
        "Agent",
        "Grep",
        "Glob",
        "WebSearch",
        "WebFetch",
    ]);
    wanted.sort();
    assert_eq!(tools, wanted, "each tool once");
    let mut allowed = permissions.allowed.clone();
    allowed.sort();
    let mut wanted = strings(&[
        "Bash",
        "Grep",
        "Glob",
        "WebSearch",
        "WebFetch",
        "Edit(//x/W/**)",
        "Edit(//x/T/**)",
    ]);
    wanted.sort();
    assert_eq!(allowed, wanted, "each rule once");
    assert_eq!(permissions.directories, ["/x/W", "/x/T"]);
    // The same directories with or without their trailing slash give the same.
    let without = Permissions::of(
        false,
        &beside().allowed.may,
        Path::new("/x/W/"),
        Path::new("/x/T"),
        &commands(),
    );
    assert_eq!(without, permissions);
}

// Each word carries its own part and no other: `web` is the two web tools and no directory, `temp_files` is the
// directory and its edit rule, `shell` the shell, and a list of no word gives the program no tool and no rule.
#[test]
fn each_word_gives_its_own_tools_and_rules() {
    let only = |word| Permissions::of(false, &[word], Path::new(WORKTREE), Path::new(TEMP), &[]);
    let web = only(Ability::Web);
    assert_eq!(
        (web.tools, web.allowed),
        (strings(&["WebSearch", "WebFetch"]), strings(&["WebSearch", "WebFetch"]))
    );
    assert!(web.directories.is_empty());
    let temp = only(Ability::TempFiles);
    assert_eq!(temp.tools, ["Read", "Edit", "Write"]);
    assert_eq!(
        (temp.allowed, temp.directories),
        (strings(&["Edit(//x/T/**)"]), strings(&["/x/T"]))
    );
    let files = only(Ability::Files);
    assert_eq!(files.tools, ["Read", "Grep", "Glob", "Edit", "Write"]);
    assert_eq!(files.allowed, ["Grep", "Glob", "Edit(//x/W/**)"]);
    assert_eq!(files.directories, ["/x/W"]);
    let shell = only(Ability::Shell);
    assert_eq!((shell.tools, shell.allowed), (strings(&["Bash"]), strings(&["Bash"])));
    let subagents = only(Ability::Subagents);
    assert_eq!(subagents.tools, ["Agent"]);
    assert!(subagents.allowed.is_empty() && subagents.directories.is_empty());
    let none = Permissions::of(false, &[], Path::new(WORKTREE), Path::new(TEMP), &[]);
    assert_eq!(none.arguments(), ["--tools", ""], "no tool, no rule, no directory");
}

// The permissions reach the command line as its flags: the tools, then the allowed rules, then each directory, then
// the refused rules; a list of nothing is not given as an empty flag, but the tools always are, since none given
// would leave the program every tool it has.
#[test]
fn the_permissions_are_given_as_the_command_lines_flags() {
    let writes = of(false);
    let given = writes.arguments();
    let allowed = given.iter().position(|a| a == "--allowedTools").expect("allowed");
    let directory = given.iter().position(|a| a == "--add-dir").expect("a directory");
    let refused = given.iter().position(|a| a == "--disallowedTools").expect("refused");
    assert_eq!(given[0], "--tools");
    assert_eq!(given[1], writes.tools.join(","));
    assert!(allowed < directory && directory < refused);
    assert_eq!(given[allowed + 1..directory], writes.allowed[..]);
    // Each directory after a flag of its own, the worktree and then the temporary directory.
    assert_eq!(given[directory..refused], ["--add-dir", "/x/W", "--add-dir", "/x/T"]);
    assert_eq!(given[refused + 1..], writes.refused[..]);
    let nothing = Permissions::of(true, &[], Path::new(WORKTREE), Path::new(TEMP), &[]).arguments();
    assert!(
        !nothing.iter().any(|a| a == "--allowedTools" || a == "--add-dir"),
        "{nothing:?}"
    );
}

// An agent is told what its line may do in the worktree that no rule says by itself: that it changes no file, or that
// it commits there.
#[test]
fn an_agent_is_told_what_its_line_may_do() {
    assert_eq!(
        of(true).told(&words(), &[], None),
        "You only read it, and change no file."
    );
    assert_eq!(
        of(false).told(&words(), &[], None),
        "Make your changes there and commit them there."
    );
}

// A line that writes is told the paths the project protects, after what else it may do, and one that only reads is
// told them too, after that it changes no file, as it may judge a change of them.
#[test]
fn a_line_that_writes_is_told_the_paths_the_project_protects() {
    let protected = [
        "AGENTS.md".to_string(),
        "flow/".to_string(),
        ".github/workflows/".to_string(),
    ];
    let writes = of(false).told(&words(), &protected, None);
    assert_eq!(
        writes,
        "Make your changes there and commit them there. The project protects `AGENTS.md`, `flow/` and \
         `.github/workflows/`: change what is there only where the work needs it."
    );
    let one = of(false).told(&words(), &protected[..1], None);
    assert!(
        one.ends_with(" The project protects `AGENTS.md`: change what is there only where the work needs it."),
        "{one}"
    );
    assert_eq!(
        of(true).told(&words(), &protected, None),
        "You only read it, and change no file. The project protects `AGENTS.md`, `flow/` and `.github/workflows/`: the \
         work may change what is there only where it needs to."
    );
}

// A line that changes only the project's tests is told their paths, and the paths the project protects after them; one
// that only reads is told it changes no file, and the paths the project protects after that.
#[test]
fn a_line_that_changes_only_the_tests_is_told_their_paths() {
    let tests = strings(&[":(glob)crates/**/tests/**", ":(glob)client/**/*.test.ts"]);
    let protected = strings(&["AGENTS.md"]);
    assert_eq!(
        of(false).told(&words(), &protected, Some(&tests[..])),
        "Make your changes there and commit them there. Change only the project's tests, the files that \
         `:(glob)crates/**/tests/**` or `:(glob)client/**/*.test.ts` match. If a test must go into another file, say \
         so in what you hand in. The project protects `AGENTS.md`: change \
         what is there only where the work needs it."
    );
    assert_eq!(
        of(true).told(&words(), &protected, Some(&tests[..])),
        "You only read it, and change no file. The project protects `AGENTS.md`: the work may change what is there \
         only where it needs to."
    );
}

// The agents' program is the installation's, not the flow's: the file beside the flows holds a slot for it, filled
// with what the settings name, so that no program is assumed. `claude` is refused where it is named, `codex` where that
// is, and a path as it is written; one program is never refused for another. A program that is blank or holds a `*` or
// a parenthesis would make a rule that matches more than it names, and is a fault naming the setting. Reference: the
// file beside the flows, which names `git push` and the slot.
#[test]
fn the_agents_program_the_installation_names_is_refused_and_no_other() {
    let beside = beside();
    let declaration = declaration(&beside);
    for (program, other) in [("claude", "codex"), ("codex", "claude"), ("/opt/ai/claude", "claude")] {
        let refused = refused_commands(&beside, &declaration, program);
        assert!(refused.iter().any(|c| c == "git push"), "{program}");
        assert!(refused.iter().any(|c| c == program), "{program}: {refused:?}");
        assert!(
            !refused.iter().any(|c| c == other),
            "{program} is not {other}: {refused:?}"
        );
        assert!(
            refused.iter().any(|c| c == "cargo"),
            "the project's heavy commands follow"
        );
        assert_eq!(faults(&beside, program), Vec::<String>::new(), "{program}");
    }
    for program in ["co*dex", " ", "co(dex", ""] {
        let faults = faults(&beside, program);
        assert_eq!(faults.len(), 1, "{program:?}: {faults:?}");
        assert!(faults[0].starts_with("agents.command: "), "{faults:?}");
    }
}

/// The declaration of the repository, read against the files beside its flows.
fn declaration(beside: &Agents) -> Declaration {
    let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wf-agents.toml"))
        .expect("the declaration is read");
    Declaration::read(&text, "wf-agents.toml", beside).expect("it is a declaration")
}

// What every agent is refused, beside the flows, and a project's heavy commands, in its declaration, are each held to
// the shape of a command: one that names nothing, or holds a `*` or a parenthesis, is said with where it is written.
// The repository's own files hold none of these.
#[test]
fn what_is_refused_is_held_to_the_shape_of_a_command() {
    let mut beside = beside();
    let mut declaration = declaration(&beside);
    assert_eq!(faults(&beside, "claude"), Vec::<String>::new());
    assert_eq!(heavy_faults(&declaration), Vec::<String>::new());
    declaration.heavy.commands.push("uv*".to_string());
    declaration.heavy.commands.push(" ".to_string());
    assert_eq!(
        heavy_faults(&declaration),
        [
            "wf-agents.toml [heavy]: each entry names a command".to_string(),
            "wf-agents.toml [heavy]: \"uv*\" holds a `*` or a parenthesis".to_string(),
        ]
    );
    beside.refused.commands.push(" ".to_string());
    assert_eq!(
        faults(&beside, "claude"),
        ["flow/agents/refused.toml: each entry names a command"]
    );
}

/// Every one of the five words, in the order the flow's file writes them.
const WORDS: [Ability; 5] = [
    Ability::Shell,
    Ability::Files,
    Ability::TempFiles,
    Ability::Web,
    Ability::Subagents,
];

/// The words of the set that bit `mask` of `0..32` picks.
fn picked(mask: usize) -> Vec<Ability> {
    WORDS
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, w)| *w)
        .collect()
}

/// The tool names the program's tools reference lists as built-in tools that the engine may name in `--tools`.
const DOCUMENTED_TOOLS: [&str; 9] = [
    "Agent",
    "Bash",
    "Edit",
    "Glob",
    "Grep",
    "Read",
    "WebFetch",
    "WebSearch",
    "Write",
];

// Whatever set of words the flow holds, the round's tools, rules and directories are what the program can read and what
// the words say, and nothing more: no tool, rule or directory twice; no empty or flag-like item that the program's
// variadic flags would end at; no allow rule for a tool the round is not given, which would be dead; a tool, rule or
// directory only for a word that gives it; and a refusal that no word takes back. Reference: the program's
// command-line reference (`--tools` names the tools the session has and its flags take lists that end at the next flag)
// and permission rules (a tool name, or a name with a pattern between parentheses; `//path` is an absolute path). All
// 32 sets of words on a line that writes and one that only reads, in microseconds.
#[test]
fn every_set_of_words_gives_tools_rules_and_directories_the_program_can_read_and_nothing_a_word_does_not_give() {
    let refused_whatever = of(false).refused;
    for mask in 0..32 {
        let may = picked(mask);
        let has = |w: Ability| may.contains(&w);
        for reads_only in [false, true] {
            let p = Permissions::of(reads_only, &may, Path::new("/x/W/"), Path::new("/x/T/"), &commands());
            let at = format!("{may:?} reads_only {reads_only}: {p:?}");
            for (name, list) in [
                ("tools", &p.tools),
                ("allowed", &p.allowed),
                ("directories", &p.directories),
            ] {
                let mut seen = list.clone();
                seen.sort();
                seen.dedup();
                assert_eq!(seen.len(), list.len(), "{name} holds an item twice: {at}");
                assert!(
                    list.iter()
                        .all(|i| !i.is_empty() && !i.starts_with('-') && !i.contains(',')),
                    "{name} holds an item the flags would misread: {at}"
                );
            }
            for rule in p.allowed.iter().chain(&p.refused) {
                let (name, pattern) = match rule.split_once('(') {
                    Some((name, rest)) => (
                        name,
                        Some(rest.strip_suffix(')').unwrap_or_else(|| panic!("{rule}: {at}"))),
                    ),
                    None => (rule.as_str(), None),
                };
                assert!(name.chars().all(|c| c.is_ascii_alphabetic()), "{rule}");
                assert!(pattern.is_none_or(|p| !p.contains(['(', ')'])), "{rule}");
            }
            for rule in &p.allowed {
                let name = rule.split('(').next().unwrap();
                assert!(
                    p.tools.iter().any(|t| t == name),
                    "the allowed {rule} is for a tool the round lacks: {at}"
                );
            }
            assert_eq!(has(Ability::Shell), p.tools.contains(&"Bash".to_string()), "{at}");
            assert_eq!(has(Ability::Shell), p.allowed.contains(&"Bash".to_string()), "{at}");
            assert_eq!(
                has(Ability::Files),
                p.allowed.contains(&"Edit(//x/W/**)".to_string()),
                "{at}"
            );
            assert_eq!(has(Ability::Files), p.directories.contains(&"/x/W".to_string()), "{at}");
            assert_eq!(has(Ability::Web), p.tools.contains(&"WebFetch".to_string()), "{at}");
            assert_eq!(has(Ability::Web), p.tools.contains(&"WebSearch".to_string()), "{at}");
            assert_eq!(has(Ability::Subagents), p.tools.iter().any(|t| t == "Agent"), "{at}");
            assert_eq!(
                has(Ability::TempFiles),
                p.directories.contains(&"/x/T".to_string()),
                "{at}"
            );
            assert_eq!(
                p.directories.len(),
                usize::from(has(Ability::Files)) + usize::from(has(Ability::TempFiles)),
                "{at}"
            );
            assert_eq!(
                has(Ability::TempFiles),
                p.allowed.contains(&"Edit(//x/T/**)".to_string()),
                "{at}"
            );
            let reads = p.tools.contains(&"Read".to_string());
            assert_eq!(reads, has(Ability::Files) || has(Ability::TempFiles), "{at}");
            // A refusal is the same whatever the words, so that no word takes one back.
            let wanted = of(reads_only).refused;
            assert_eq!(p.refused, wanted, "{at}");
            assert!(refused_whatever.iter().all(|r| p.refused.contains(r)), "{at}");
        }
    }
}

// The tools the round is given are tool names the program documents, in its tools reference, as the exact strings that
// `--tools` and permission rules take: a name it does not list is not known to name a tool, and a tool the program does
// not give would take away what a word lets the agents do (the subagent tool, which the reference names `Agent`).
// Reference: the program's tools reference (code.claude.com/docs/en/tools-reference), its `--tools` row. Microseconds.
#[test]
fn every_tool_a_round_is_given_is_a_name_the_programs_tools_reference_lists() {
    let all = Permissions::of(false, &WORDS, Path::new("/x/W"), Path::new("/x/T"), &[]);
    for tool in &all.tools {
        assert!(
            DOCUMENTED_TOOLS.contains(&tool.as_str()),
            "{tool} is no documented tool name"
        );
    }
}

// What the agents may edit is the worktree by its absolute path, not by a path relative to the directory the program
// is in: the agents' shell keeps a `cd` from one command to the next, the program takes the directory it moved to as
// its current one, and a write outside it, such as one to `.github/workflows/` from `compiler/src`, is then asked
// about, which in a print-mode run is refused. So no rule that allows edits is relative, and the worktree is written
// as the temporary directory is: an absolute rule and a directory of its own. Reference: the program's permission
// rules (`./path` is relative to the current directory, `//path` is absolute) and what W208, W236 and W246 showed.
#[test]
fn what_files_lets_the_agents_edit_does_not_move_with_the_shell() {
    for mask in 0..32 {
        let may = picked(mask);
        if !may.contains(&Ability::Files) {
            continue;
        }
        let p = Permissions::of(false, &may, Path::new("/x/W/"), Path::new("/x/T"), &[]);
        let edits: Vec<&String> = p.allowed.iter().filter(|r| r.starts_with("Edit(")).collect();
        assert!(!edits.is_empty(), "{may:?}: no rule allows edits");
        for rule in &edits {
            let path = rule
                .strip_prefix("Edit(")
                .and_then(|r| r.strip_suffix(')'))
                .expect("a rule");
            assert!(path.starts_with("//"), "{rule} is not an absolute path: {may:?}");
        }
        assert!(edits.iter().any(|r| *r == "Edit(//x/W/**)"), "{may:?}: {edits:?}");
        assert!(p.directories.contains(&"/x/W".to_string()), "{may:?}: {p:?}");
        // Written as the temporary directory is, with the rule and the directory of the same path.
        if may.contains(&Ability::TempFiles) {
            assert!(edits.iter().any(|r| *r == "Edit(//x/T/**)") && p.directories.contains(&"/x/T".to_string()));
        }
    }
}
