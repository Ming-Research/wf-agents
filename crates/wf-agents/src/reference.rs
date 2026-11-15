//! `wf-agents help`: the whole reference of the command line in one call, made from clap's own rendering of each
//! command's long help, so that it is the same as `--help` gives and a command added later is in it.

use clap::Command;

/// The reference of the whole program: its description, then every command and every command under it, each under
/// a line with its full name, with its long help.
pub fn whole(program: &Command) -> String {
    let mut program = program.clone();
    program.build();
    let mut out = String::new();
    if let Some(about) = program.get_long_about().or(program.get_about()) {
        out.push_str(&format!("{about}\n"));
    }
    commands(&program, program.get_name(), &mut out);
    out
}

/// The long help of the command a path of names leads to from the program, as `<command> --help` gives it; none if
/// a name of the path is no command.
pub fn of(program: &Command, path: &[String]) -> Option<String> {
    let mut program = program.clone();
    program.build();
    let mut at = &mut program;
    for name in path {
        at = at.find_subcommand_mut(name)?;
    }
    Some(at.render_long_help().to_string())
}

/// Every command under `parent`, depth first, named by `prefix` and its own name.
fn commands(parent: &Command, prefix: &str, out: &mut String) {
    for command in parent.get_subcommands() {
        let name = format!("{prefix} {}", command.get_name());
        out.push_str(&format!("\n{name}\n\n{}\n", command.clone().render_long_help()));
        commands(command, &name, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The full name of every command under `parent`, nested ones too.
    fn names(parent: &Command, prefix: &str, into: &mut Vec<(String, Command)>) {
        for command in parent.get_subcommands() {
            let name = format!("{prefix} {}", command.get_name());
            into.push((name.clone(), command.clone()));
            names(command, &name, into);
        }
    }

    /// The reference holds every command clap knows, nested ones too, each under a line with its full name and
    /// with the long help `--help` gives it, so that a command added later is in it with nothing added by hand.
    #[test]
    fn the_reference_holds_every_command_with_its_long_help() {
        let mut program = crate::Cli::command();
        let said = whole(&program);
        program.build();
        let mut all = Vec::new();
        names(&program, program.get_name(), &mut all);
        assert!(all.len() > 10, "the walk found {} commands", all.len());
        assert!(all.iter().any(|(name, _)| name == "wf-agents flow check"));
        let lines: Vec<&str> = said.lines().collect();
        for (name, mut command) in all {
            assert_eq!(
                lines.iter().filter(|line| **line == name).count(),
                1,
                "`{name}` is not under a line of its own name"
            );
            let long = command.render_long_help().to_string();
            assert!(said.contains(&long), "`{name}` is not given its long help");
            let path: Vec<String> = name.split(' ').skip(1).map(String::from).collect();
            assert_eq!(of(&crate::Cli::command(), &path), Some(long));
        }
        assert!(said.starts_with(&crate::Cli::command().get_about().unwrap().to_string()));
    }
}
