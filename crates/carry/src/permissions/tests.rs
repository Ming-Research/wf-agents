use super::*;

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

/// The lock commands of a project on Cargo and npm, as its installation's settings name them: `cargo update` and
/// `cargo add` resolve the dependencies and write Cargo.lock without compiling anything; `npm install
/// --package-lock-only` writes package-lock.json and leaves node_modules alone, and with `--ignore-scripts` runs none of
/// the package's own scripts, which it runs when it is given no package.
const LOCKS: [&str; 3] = [
    "cargo update",
    "cargo add",
    "npm install --package-lock-only --ignore-scripts",
];

/// The uses of those lock commands the settings refuse all the same: npm's options that would undo
/// `--package-lock-only` or `--ignore-scripts` or install elsewhere, as a later npm option overrides an earlier one.
/// They are a negated option, which npm reads in any case (`--no-ignore-scripts`, `--NO-ignore-scripts`); any option
/// given a value after `=`, which turns a flag off with `false`, `null`, `0` or nothing and sets `--location=global` or
/// `--prefix=<dir>`; a flag turned off by a separate `false`; and `-g`, `--global`, `--location`, `--prefix` and `-C`,
/// written out or shortened, and the short ones with two dashes, which npm takes as well. A short option in a cluster
/// of them, as the `g` of `-Dg`, cannot be named in the command line's patterns without refusing the lock command
/// itself, and is a risk left.
const LOCKS_EXCEPT: [&str; 12] = [
    "npm install *-no-*",
    "npm install *-NO-*",
    "npm install *-No-*",
    "npm install *-nO-*",
    "npm install *=*",
    "npm install * false*",
    "npm install * -g*",
    "npm install *--g*",
    "npm install *--loc*",
    "npm install *--pref*",
    "npm install * -C*",
    "npm install *--C*",
];

/// What the settings of an installation whose project is on Cargo and npm add: those lock commands and their
/// exceptions.
fn cargo_and_npm() -> PermissionsAdded {
    let list = |l: &[&str]| l.iter().map(|c| c.to_string()).collect();
    PermissionsAdded {
        locks: list(&LOCKS),
        locks_except: list(&LOCKS_EXCEPT),
    }
}

/// The permissions of a line for the installation's arguments, with the lock commands of a project on Cargo and npm.
fn of(reads_only: bool, args: &[String]) -> Permissions {
    Permissions::of(reads_only, args, &commands(), &cargo_and_npm())
}

fn args(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

// A line that only reads is refused every tool that edits a file and every git command that changes the repository,
// and may still read with git; a line that writes is refused neither.
#[test]
fn a_line_that_only_reads_edits_nothing_and_changes_no_repository() {
    let refused = of(true, &[]).refused;
    for t in EDIT_TOOLS {
        assert!(refused.iter().any(|r| r == t), "{t}");
    }
    for g in GIT_CHANGES {
        assert!(refused.contains(&format!("Bash(git {g} *)")), "git {g}");
    }
    for g in ["show", "diff", "log", "fetch", "grep"] {
        assert!(!refused.contains(&format!("Bash(git {g} *)")), "git {g}");
    }
    let writes = of(false, &[]).refused;
    assert!(!writes.iter().any(|r| r == "Edit" || r == "Bash(git commit *)"));
}

// Every line is refused the heavy commands and pushing. A line that writes may update a lock file with the lock
// commands the settings name, and no heavy rule refuses that: the heavy commands the lock commands fall under are left out there, every
// other stays, and the npm options that would undo a lock command are refused. A line that only reads is allowed none
// of them, and refused every heavy command whole.
#[test]
fn only_a_line_that_writes_may_update_a_lock_file() {
    let writes = of(false, &[]);
    let locks = [
        "Bash(cargo update *)",
        "Bash(cargo add *)",
        "Bash(npm install --package-lock-only --ignore-scripts *)",
    ];
    assert_eq!(writes.allowed, locks);
    assert_eq!(writes.locks, LOCKS);
    for gone in ["Bash(cargo *)", "Bash(npm *)"] {
        assert!(!writes.refused.iter().any(|r| r == gone), "{gone}");
    }
    for kept in [
        "Bash(rustc *)",
        "Bash(npx *)",
        "Bash(vitest *)",
        "Bash(pytest *)",
        "Bash(git push *)",
        "Bash(npm install *-no-*)",
        "Bash(npm install *=*)",
        "Bash(npm install * -g*)",
        "Bash(npm install *--pref*)",
    ] {
        assert!(writes.refused.iter().any(|r| r == kept), "{kept}");
    }
    assert_eq!(writes.refused.len(), REFUSED.len() - 2 + LOCKS_EXCEPT.len());

    let reads = of(true, &[]);
    assert!(reads.allowed.is_empty() && reads.locks.is_empty());
    for command in REFUSED {
        assert!(reads.refused.contains(&format!("Bash({command} *)")), "{command}");
    }
    assert!(!reads.refused.iter().any(|r| r.contains("-no-")));
}

// The engine knows no lock command of its own: where the settings name none, a line that writes may update no lock
// file, and is refused every heavy command of its project whole.
#[test]
fn a_lock_command_is_the_settings_alone() {
    let writes = Permissions::of(false, &[], &commands(), &PermissionsAdded::default());
    assert!(writes.allowed.is_empty() && writes.locks.is_empty());
    assert_eq!(writes.refused.len(), REFUSED.len());
    for command in REFUSED {
        assert!(writes.refused.contains(&format!("Bash({command} *)")), "{command}");
    }
    assert_eq!(
        writes.told(&words(), &[], None),
        "Make your changes there and commit them there."
    );
}

// A heavy command left out for a lock command stays refused in every other use the installation's arguments allow:
// each such rule is refused again, as `cargo fmt` that the installation allows, but not a lock command or a use under
// one; and where the arguments allow the whole heavy command, or every command, it is refused whole.
#[test]
fn what_the_installation_allows_under_a_heavy_command_stays_refused() {
    let installed = args(&[
        "--allowedTools",
        "Edit(./**)",
        "Bash(git *)",
        "Bash(rustfmt *)",
        "Bash(cargo fmt *)",
        "Bash(cargo update -p:*),Read",
        "--disallowedTools",
        "Bash(npm install *)",
    ]);
    assert_eq!(
        allows(&installed),
        [
            "Edit(./**)",
            "Bash(git *)",
            "Bash(rustfmt *)",
            "Bash(cargo fmt *)",
            "Bash(cargo update -p:*)",
            "Read"
        ]
    );
    let refused = of(false, &installed).refused;
    assert!(refused.iter().any(|r| r == "Bash(cargo fmt *)"), "{refused:?}");
    assert!(
        !(refused.iter()).any(|r| r == "Bash(cargo *)" || r.starts_with("Bash(cargo update") || r == "Bash(git *)"),
        "{refused:?}"
    );
    assert_eq!(refused.len(), REFUSED.len() - 2 + LOCKS_EXCEPT.len() + 1);
    // A value after `=`, an exact command, and several rules in one word split at commas and spaces.
    let exact = args(&["--allowed-tools=Bash(cargo fmt --check) Bash(npm install foo:*)"]);
    let refused = of(false, &exact).refused;
    assert!(refused.iter().any(|r| r == "Bash(cargo fmt --check)"), "{refused:?}");
    assert!(refused.iter().any(|r| r == "Bash(npm install foo:*)"), "{refused:?}");
    // The whole of a heavy command allowed, or every command allowed: every heavy command the arguments allow whole
    // stays refused whole, and the lock commands under it are neither allowed nor told; the others are.
    let npm = "npm install --package-lock-only --ignore-scripts".to_string();
    let cases: [(&[&str], Vec<String>); 3] = [
        (&["--allowedTools", "Bash(cargo:*)"], vec![npm.clone()]),
        (&["--allowedTools", "Bash"], vec![]),
        (&["--allowedTools", "Bash(* --version)"], vec![]),
    ];
    for (given, kept) in cases {
        let permissions = of(false, &args(given));
        assert!(permissions.refused.iter().any(|r| r == "Bash(cargo *)"), "{given:?}");
        assert_eq!(permissions.locks, kept, "{given:?}");
        let allowed: Vec<String> = kept.iter().map(|k| format!("Bash({k} *)")).collect();
        assert_eq!(permissions.allowed, allowed, "{given:?}");
        let npm_whole = permissions.refused.iter().any(|r| r == "Bash(npm *)");
        assert_eq!(npm_whole, kept.is_empty(), "{given:?}");
        assert!(permissions.refused.iter().any(|r| r == "Bash(git push *)"), "{given:?}");
    }
    // The rules the sample's installation gives leave every lock command allowed.
    assert_eq!(of(false, &installed).locks, LOCKS);
    // A line that only reads keeps every heavy command whole whatever the arguments allow, and refuses no rule of
    // theirs.
    let review = of(true, &installed).refused;
    assert!(review.iter().any(|r| r == "Bash(cargo *)") && !review.iter().any(|r| r == "Bash(cargo fmt *)"));
}

// The program starts in default permission mode whatever it is given, so a permission mode or a flag that skips the
// checks allows no command: the lock commands stay allowed and the heavy commands refused, as with no such argument.
#[test]
fn a_permission_mode_allows_no_command() {
    for given in [
        &["--dangerously-skip-permissions"][..],
        &[
            "--allow-dangerously-skip-permissions",
            "--permission-mode",
            "bypassPermissions",
        ],
        &["--permission-mode", "bypassPermissions"],
        &["--permission-mode", "auto"],
        &["--permission-mode=auto"],
    ] {
        let given = args(given);
        assert_eq!(allows(&given), Vec::<String>::new(), "{given:?}");
        let permissions = of(false, &given);
        assert_eq!(permissions.locks, LOCKS, "{given:?}");
        assert_eq!(permissions, of(false, &[]), "{given:?}");
        assert!(permissions.refused.iter().any(|r| r == "Bash(cargo *)"), "{given:?}");
    }
}

/// Whether a command matches a pattern of the command line's tool rules, in which `*` stands for any text.
fn matches(pattern: &str, command: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == command,
        Some((head, rest)) => {
            (command.strip_prefix(head)).is_some_and(|tail| (0..=tail.len()).any(|i| matches(rest, &tail[i..])))
        }
    }
}

// The npm lock command, written as an agent writes it, is allowed and refused by no exception; each later option that
// would undo `--package-lock-only` or `--ignore-scripts`, or install elsewhere, is refused by one.
#[test]
fn the_npm_options_that_would_undo_the_lock_command_are_refused_and_it_is_not() {
    let refused = |command: &str| LOCKS_EXCEPT.iter().any(|e| matches(e, command));
    let lock = "npm install --package-lock-only --ignore-scripts";
    assert!(!refused(lock));
    let allowed = of(false, &[]).allowed;
    for given in ["happy-dom", "-D --save-exact happy-dom@21.0.4", "-w client vitest"] {
        let command = format!("{lock} {given}");
        assert!(
            allowed.iter().any(|a| matches(&a[5..a.len() - 1], &command)),
            "{command}"
        );
        assert!(!refused(&command), "{command}");
    }
    for given in [
        "--no-package-lock-only",
        "--no-ignore-scripts",
        "--NO-ignore-scripts",
        "--No-package-lock-only",
        "-nO-ignore-scripts",
        "--ignore-scripts=false",
        "--ignore-scripts=0",
        "--package-lock-only=",
        "--ignore-scripts false",
        "-g",
        "--g",
        "--globa",
        "--global",
        "--location global",
        "--location=global",
        "--prefix /elsewhere",
        "--prefix=/elsewhere",
        "-C /elsewhere",
        "--C /elsewhere",
    ] {
        let command = format!("{lock} {given} happy-dom");
        assert!(refused(&command), "{command}");
    }
}

// The permissions reach the command line as two lists of rules, the allowed first; a line with nothing allowed gives
// no empty list.
#[test]
fn the_permissions_are_given_as_the_command_line_s_lists_of_rules() {
    let writes = of(false, &[]);
    let given = writes.arguments();
    let refused_at = given.iter().position(|a| a == "--disallowedTools").expect("refused");
    assert_eq!(given[0], "--allowedTools");
    assert_eq!(given[1..refused_at], writes.allowed[..]);
    assert_eq!(given[refused_at + 1..], writes.refused[..]);
    let reads = of(true, &[]).arguments();
    assert_eq!(reads[0], "--disallowedTools");
    assert!(!reads.iter().any(|a| a == "--allowedTools"));
}

// An agent is told what its line may do in the worktree that no rule says by itself: that it changes no file, or that
// it commits there and with which commands it updates a lock file; nothing of a lock command its line is not allowed.
#[test]
fn an_agent_is_told_what_its_line_may_do() {
    let reads = of(true, &[]).told(&words(), &[], None);
    assert_eq!(reads, "You only read it, and change no file.");

    let writes = of(false, &[]).told(&words(), &[], None);
    assert_eq!(
        writes,
        "Make your changes there and commit them there. To update a lock file, run `cargo update`, `cargo add` or \
         `npm install --package-lock-only --ignore-scripts`, with your arguments after it."
    );
    let open = of(false, &args(&["--allowedTools", "Bash(cargo:*)"])).told(&words(), &[], None);
    assert_eq!(
        open,
        "Make your changes there and commit them there. To update a lock file, run \
         `npm install --package-lock-only --ignore-scripts`, with your arguments after it."
    );
    let none = of(false, &args(&["--allowedTools", "Bash"])).told(&words(), &[], None);
    assert_eq!(none, "Make your changes there and commit them there.");
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
    let writes = of(false, &args(&["--allowedTools", "Bash"])).told(&words(), &protected, None);
    assert_eq!(
        writes,
        "Make your changes there and commit them there. The project protects `AGENTS.md`, `flow/` and \
         `.github/workflows/`: change what is there only where the work needs it."
    );
    let one = of(false, &args(&["--allowedTools", "Bash"])).told(&words(), &protected[..1], None);
    assert!(
        one.ends_with(" The project protects `AGENTS.md`: change what is there only where the work needs it."),
        "{one}"
    );
    assert_eq!(
        of(true, &[]).told(&words(), &protected, None),
        "You only read it, and change no file. The project protects `AGENTS.md`, `flow/` and `.github/workflows/`: the \
         work may change what is there only where it needs to."
    );
}

// A line that changes only the project's tests is told their paths in place of the lock commands, a lock file being
// none of them, and the paths the project protects after them; one that only reads is told it changes no file, and
// the paths the project protects after that.
#[test]
fn a_line_that_changes_only_the_tests_is_told_their_paths_and_no_lock_command() {
    let tests = args(&[":(glob)crates/**/tests/**", ":(glob)client/**/*.test.ts"]);
    let protected = args(&["AGENTS.md"]);
    let writes = of(false, &[]);
    assert!(
        !writes.locks.is_empty(),
        "the line may run the lock commands as any line that writes"
    );
    assert_eq!(
        writes.told(&words(), &protected, Some(&tests[..])),
        "Make your changes there and commit them there. Change only the project's tests, the files that \
         `:(glob)crates/**/tests/**` or `:(glob)client/**/*.test.ts` match. If a test must go into another file, say \
         so in what you hand in. The project protects `AGENTS.md`: change \
         what is there only where the work needs it."
    );
    assert_eq!(
        of(true, &[]).told(&words(), &protected, Some(&tests[..])),
        "You only read it, and change no file. The project protects `AGENTS.md`: the work may change what is there \
         only where it needs to."
    );
}

// An allow of the installation's that matches a heavy command's text without a word's end counts as reaching it: one
// that may match all of it refuses it whole, and one that matches only some of its uses is refused again.
#[test]
fn an_allow_without_a_word_s_end_reaches_the_heavy_commands_it_can_match() {
    let whole = of(false, &args(&["--allowedTools", "Bash(carg*)"]));
    assert!(
        whole.refused.iter().any(|r| r == "Bash(cargo *)"),
        "{:?}",
        whole.refused
    );
    assert!(!whole.locks.iter().any(|l| l.starts_with("cargo")), "{:?}", whole.locks);
    let some = of(false, &args(&["--allowedTools", "Bash(cargo f*)", "Bash(cargo up*)"]));
    for rule in ["Bash(cargo f*)", "Bash(cargo up*)"] {
        assert!(some.refused.iter().any(|r| r == rule), "{rule}: {:?}", some.refused);
    }
    assert_eq!(some.locks, LOCKS);
    // One that matches only uses of a lock command is left to allow them.
    let lock = of(false, &args(&["--allowedTools", "Bash(cargo update -p*)"]));
    assert!(
        !lock.refused.iter().any(|r| r.starts_with("Bash(cargo update")),
        "{:?}",
        lock.refused
    );
    // A `*` within a rule makes what comes before it a beginning of any text.
    let within = of(false, &args(&["--allowedTools", "Bash(carg* --release)"]));
    assert!(
        within.refused.iter().any(|r| r == "Bash(cargo *)"),
        "{:?}",
        within.refused
    );
    assert_eq!(within.locks, ["npm install --package-lock-only --ignore-scripts"]);
}

/// Whether the command line refuses `command` to a round with these permissions, run with the installation's
/// arguments `given`: a refused rule matches it, or no rule allows it, which in the permission modes the installations
/// run in is refused as well.
fn refuses(permissions: &Permissions, given: &[String], command: &str) -> bool {
    let rule = |r: &String| {
        r.strip_prefix("Bash(")
            .and_then(|r| r.strip_suffix(')'))
            .map(str::to_string)
    };
    let pattern = |r: &String| rule(r).is_some_and(|p| matches(&p, command));
    let installed = allows(given);
    permissions.refused.iter().any(pattern) || !(permissions.allowed.iter().chain(&installed)).any(pattern)
}

// npm is heavy whole, as Cargo is: every use of it but its lock command is refused on every line, whatever names it
// goes by and though the installation allows all of npm; the lock command stays allowed on a line that writes.
#[test]
fn every_use_of_npm_but_its_lock_command_is_refused() {
    let uses = [
        "npm i left-pad",
        "npm add left-pad",
        "npm ic",
        "npm t",
        "npm x vitest",
        "npm rum build",
        "npm run-script build",
        "npm rebuild",
        "npm start",
        "npm init vite",
        "npm update",
        "npm install",
    ];
    let lock = "npm install --package-lock-only --ignore-scripts left-pad";
    for (reads_only, given) in [
        (false, args(&[])),
        (false, args(&["--allowedTools", "Bash(npm *)"])),
        (false, args(&["--allowedTools", "Bash(npm i*)"])),
        (true, args(&[])),
        (true, args(&["--allowedTools", "Bash(npm *)"])),
    ] {
        let permissions = of(reads_only, &given);
        for command in uses {
            assert!(
                refuses(&permissions, &given, command),
                "{command}: {reads_only} {given:?}"
            );
        }
        let open = !reads_only && given.is_empty();
        assert_eq!(!refuses(&permissions, &given, lock), open, "{reads_only} {given:?}");
    }
}

// A heavy command of any toolchain the project's declaration names is refused, and a lock command the installation
// names under it is allowed, with the uses of it it names refused; nothing else of the project's goes.
#[test]
fn a_heavy_command_of_any_toolchain_is_refused_and_a_lock_command_under_it_allowed() {
    let added = PermissionsAdded {
        locks: vec!["uv lock".to_string(), "pnpm install --lockfile-only".to_string()],
        locks_except: vec!["pnpm install *--global*".to_string()],
    };
    let with_uv = [commands(), vec!["uv".to_string()]].concat();
    let writes = Permissions::of(false, &[], &with_uv, &added);
    assert!(
        writes.allowed.iter().any(|a| a == "Bash(uv lock *)"),
        "{:?}",
        writes.allowed
    );
    assert!(
        writes
            .allowed
            .iter()
            .any(|a| a == "Bash(pnpm install --lockfile-only *)")
    );
    for gone in ["Bash(uv *)", "Bash(pnpm *)"] {
        assert!(!writes.refused.iter().any(|r| r == gone), "{gone}");
    }
    assert!(writes.refused.iter().any(|r| r == "Bash(pnpm install *--global*)"));
    // The refused commands and `uv`, less the two the lock commands fall under, and the one exception.
    assert_eq!(writes.refused.len(), REFUSED.len() + 1 - 2 + 1);
    let reads = Permissions::of(true, &[], &with_uv, &added);
    for rule in ["Bash(uv *)", "Bash(pnpm *)", "Bash(cargo *)"] {
        assert!(reads.refused.iter().any(|r| r == rule), "{rule}");
    }
    // A heavy command narrower than a lock command still refuses what it names.
    let narrower = [commands(), vec!["cargo update --precise".to_string()]].concat();
    let refused = Permissions::of(false, &[], &narrower, &cargo_and_npm()).refused;
    assert!(refused.iter().any(|r| r == "Bash(cargo update --precise *)"));
}

// What every agent is refused, beside the flows, and a project's heavy commands, in its declaration, are each held to
// the shape of a command, and the lock commands the installation names to them: a lock command that falls under what
// every agent is refused, as pushing, is said against that file, and one that a heavy command falls under whole against
// the declaration; neither is said against the other. The repository's own files and the lock commands of a project on
// Cargo and npm hold none of these.
#[test]
fn no_lock_command_undoes_what_the_files_beside_the_flows_or_the_declaration_refuse() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let mut beside = flow::agents::read(&root).expect("what is written beside the flows is read");
    assert_eq!(faults(&beside, &cargo_and_npm()), Vec::<String>::new());
    assert_eq!(heavy_faults(&beside.project, &cargo_and_npm()), Vec::<String>::new());
    let refused = refused_commands(&beside);
    assert!(refused.iter().any(|c| c == "git push") && refused.iter().any(|c| c == "cargo"));
    let locks = |lock: &str| PermissionsAdded {
        locks: vec![lock.to_string()],
        ..PermissionsAdded::default()
    };
    for (lock, says) in [
        (
            "git push",
            "agents.permissions.locks: \"git push\" falls under \"git push\", which every agent is refused \
             (flow/agents/refused.toml)",
        ),
        (
            "git push origin",
            "agents.permissions.locks: \"git push origin\" falls under \"git push\", which every agent is refused \
             (flow/agents/refused.toml)",
        ),
    ] {
        assert_eq!(faults(&beside, &locks(lock)), [says], "{lock}");
        assert_eq!(
            heavy_faults(&beside.project, &locks(lock)),
            Vec::<String>::new(),
            "{lock}"
        );
    }
    for lock in ["cargo", "npm"] {
        let says = format!(
            "agents.permissions.locks: {lock:?} would allow all of the heavy command {lock:?} \
             (flow/project.toml [heavy])"
        );
        assert_eq!(heavy_faults(&beside.project, &locks(lock)), [says], "{lock}");
        assert_eq!(faults(&beside, &locks(lock)), Vec::<String>::new(), "{lock}");
    }
    beside.project.heavy.commands.push("uv*".to_string());
    beside.refused.commands.push(" ".to_string());
    assert_eq!(
        heavy_faults(&beside.project, &PermissionsAdded::default()),
        ["flow/project.toml [heavy]: \"uv*\" holds a `*` or a parenthesis"]
    );
    assert_eq!(
        faults(&beside, &PermissionsAdded::default()),
        ["flow/agents/refused.toml: each entry names a command"]
    );
}
