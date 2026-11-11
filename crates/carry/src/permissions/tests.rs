use super::*;

/// The permissions of a line for the installation's arguments, with nothing added by its settings.
fn of(reads_only: bool, args: &[String]) -> Permissions {
    Permissions::of(reads_only, args, &PermissionsAdded::default())
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

// Every line is refused the heavy commands and pushing. A line that writes may update a lock file with the package
// manager, and no heavy rule refuses that: the heavy commands the lock commands fall under are left out there, every
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
    for gone in ["Bash(cargo *)", "Bash(npm install *)"] {
        assert!(!writes.refused.iter().any(|r| r == gone), "{gone}");
    }
    for kept in [
        "Bash(rustc *)",
        "Bash(npm run *)",
        "Bash(npm test *)",
        "Bash(npm ci *)",
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
    assert_eq!(writes.refused.len(), HEAVY.len() - 2 + LOCKS_EXCEPT.len());

    let reads = of(true, &[]);
    assert!(reads.allowed.is_empty() && reads.locks.is_empty());
    for heavy in HEAVY {
        assert!(reads.refused.contains(&format!("Bash({heavy} *)")), "{heavy}");
    }
    assert!(!reads.refused.iter().any(|r| r.contains("-no-")));
}

// A heavy command left out for a lock command stays refused in every other use the installation's arguments allow:
// each such rule is refused again, as `cargo fmt` that the installation allows, but not a lock command or a use under
// one; and where the arguments allow the whole heavy command, or every command, it is refused whole.
#[test]
fn what_the_installation_allows_under_a_heavy_command_stays_refused() {
    let installed = args(&[
        "--permission-mode",
        "acceptEdits",
        "--allowedTools",
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
    assert_eq!(refused.len(), HEAVY.len() - 2 + LOCKS_EXCEPT.len() + 1);
    // A value after `=`, an exact command, and several rules in one word split at commas and spaces.
    let exact = args(&["--allowed-tools=Bash(cargo fmt --check) Bash(npm install foo:*)"]);
    let refused = of(false, &exact).refused;
    assert!(refused.iter().any(|r| r == "Bash(cargo fmt --check)"), "{refused:?}");
    assert!(refused.iter().any(|r| r == "Bash(npm install foo:*)"), "{refused:?}");
    // The whole of a heavy command allowed, every command allowed, or the permission checks skipped or left to the
    // command line's own judgment: every heavy command the arguments allow whole stays refused whole, and the lock
    // commands under it are neither allowed nor told; the others are.
    let npm = "npm install --package-lock-only --ignore-scripts".to_string();
    let cases: [(&[&str], Vec<String>); 7] = [
        (&["--allowedTools", "Bash(cargo:*)"], vec![npm.clone()]),
        (&["--allowedTools", "Bash"], vec![]),
        (&["--allowedTools", "Bash(* --version)"], vec![]),
        (&["--dangerously-skip-permissions"], vec![]),
        (&["--permission-mode", "bypassPermissions"], vec![]),
        (&["--permission-mode", "auto"], vec![]),
        (&["--permission-mode=auto"], vec![]),
    ];
    for (given, kept) in cases {
        let permissions = of(false, &args(given));
        assert!(permissions.refused.iter().any(|r| r == "Bash(cargo *)"), "{given:?}");
        assert_eq!(permissions.locks, kept, "{given:?}");
        let allowed: Vec<String> = kept.iter().map(|k| format!("Bash({k} *)")).collect();
        assert_eq!(permissions.allowed, allowed, "{given:?}");
        let npm_whole = permissions.refused.iter().any(|r| r == "Bash(npm install *)");
        assert_eq!(npm_whole, kept.is_empty(), "{given:?}");
        assert!(permissions.refused.iter().any(|r| r == "Bash(git push *)"), "{given:?}");
    }
    // The permission mode the sample's installation runs in leaves every lock command allowed.
    assert_eq!(of(false, &installed).locks, LOCKS);
    // A line that only reads keeps every heavy command whole whatever the arguments allow, and refuses no rule of
    // theirs.
    let review = of(true, &installed).refused;
    assert!(review.iter().any(|r| r == "Bash(cargo *)") && !review.iter().any(|r| r == "Bash(cargo fmt *)"));
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
    let reads = of(true, &[]).told();
    assert_eq!(reads, "You only read it, and change no file.");

    let writes = of(false, &[]).told();
    assert_eq!(
        writes,
        "Make your changes there and commit them there. Update a lock file with `cargo update`, `cargo add` or \
         `npm install --package-lock-only --ignore-scripts`, your arguments after it."
    );
    let open = of(false, &args(&["--allowedTools", "Bash(cargo:*)"])).told();
    assert_eq!(
        open,
        "Make your changes there and commit them there. Update a lock file with \
         `npm install --package-lock-only --ignore-scripts`, your arguments after it."
    );
    let none = of(false, &args(&["--dangerously-skip-permissions"])).told();
    assert_eq!(none, "Make your changes there and commit them there.");
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
    let within = of(false, &args(&["--allowedTools", "Bash(npm ins* --global)"]));
    assert!(
        within.refused.iter().any(|r| r == "Bash(npm install *)"),
        "{:?}",
        within.refused
    );
    assert_eq!(within.locks, ["cargo update", "cargo add"]);
}

// The installation adds to the engine's lists: a heavy command of its toolchain is refused, its lock command is
// allowed though a heavy command covers it, and the uses of it it names are refused; nothing of the engine's goes.
#[test]
fn the_installation_adds_heavy_and_lock_commands_of_its_own() {
    let added = PermissionsAdded {
        heavy: vec!["uv".to_string()],
        locks: vec!["uv lock".to_string(), "pnpm install --lockfile-only".to_string()],
        locks_except: vec!["pnpm install *--global*".to_string()],
    };
    let writes = Permissions::of(false, &[], &added);
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
    // The engine's heavy commands and `uv`, less the four the lock commands fall under; the exceptions, and one more.
    assert_eq!(writes.refused.len(), HEAVY.len() + 1 - 4 + LOCKS_EXCEPT.len() + 1);
    let reads = Permissions::of(true, &[], &added);
    for rule in ["Bash(uv *)", "Bash(pnpm *)", "Bash(cargo *)"] {
        assert!(reads.refused.iter().any(|r| r == rule), "{rule}");
    }
    // A heavy command narrower than a lock command still refuses what it names.
    let narrower = PermissionsAdded {
        heavy: vec!["cargo update --precise".to_string()],
        ..PermissionsAdded::default()
    };
    let refused = Permissions::of(false, &[], &narrower).refused;
    assert!(refused.iter().any(|r| r == "Bash(cargo update --precise *)"));
}
