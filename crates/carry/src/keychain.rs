//! The login keychain of macOS, where the tokens of the accounts the agents' program may run under are kept, each a
//! generic password under a service of the installation's own and an account that is the account's name (decision
//! an-account-is-a-name-and-a-token-in-the-keychain). It is reached through `/usr/bin/security`, which an item made by
//! it trusts, and not through the Security framework from the engine, whose binary changes with every install. A token
//! is given to the program on its standard input, never in its arguments, and read from its standard output; what the
//! program says is passed on only with any token taken out of it. A token is kept nowhere else.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use service::{Token, Tokens};

/// The program that keeps the items of the login keychain.
pub const SECURITY: &str = "/usr/bin/security";

/// How long the program is given to answer: it answers at once, and a keychain that would ask the owner for a password,
/// as a login service can show no one, is said to be locked rather than waited on.
const WAIT: Duration = Duration::from_secs(30);

/// What the program ends with where the keychain has no such item.
const NOT_FOUND: i32 = 44;

/// The tokens of an installation's accounts, in the login keychain.
pub struct Keychain {
    /// The program, `/usr/bin/security` but in a test, which runs a stand-in.
    program: PathBuf,
    /// The service every item of the installation is kept under.
    service: String,
    /// Why no account can be added, as the settings say, where none can.
    refused: Option<String>,
    /// The command that makes a token, as the settings name the agents' program.
    making: String,
}

impl Keychain {
    /// The keychain of the installation of this id, through this program, an account refused where the settings say
    /// why, a token made by this command.
    pub fn new(program: &Path, installation: &uuid::Uuid, refused: Option<String>, making: String) -> Keychain {
        Keychain {
            program: program.to_path_buf(),
            service: format!("wf-agents.{installation}"),
            refused,
            making,
        }
    }

    /// The program run with these arguments, given this on its standard input, as it ended; or why it could not be
    /// run or did not end in time. Its output is read as it comes, so that a full pipe does not hold it.
    fn run(&self, args: &[&str], stdin: Option<&str>) -> Result<Output, String> {
        let mut child = Command::new(&self.program)
            .args(args)
            .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{} could not be run: {e}", self.program.display()))?;
        if let (Some(text), Some(mut input)) = (stdin, child.stdin.take()) {
            input
                .write_all(text.as_bytes())
                .map_err(|e| format!("{} could not be told: {e}", self.program.display()))?;
        }
        let readers = [child.stdout.take().map(reader), child.stderr.take().map(reader)];
        let status = waited(&mut child).ok_or_else(|| {
            format!(
                "{} did not answer within {} seconds: the login keychain may be locked",
                self.program.display(),
                WAIT.as_secs()
            )
        })?;
        let [stdout, stderr] = readers.map(|r| r.and_then(|r| r.join().ok()).unwrap_or_default());
        Ok(Output { status, stdout, stderr })
    }

    /// What the program said, its standard error, with this token taken out of it, in one line.
    fn said(output: &Output, token: Option<&Token>) -> String {
        let mut said = String::from_utf8_lossy(&output.stderr).trim().replace('\n', " ");
        if let Some(token) = token.filter(|t| !t.secret().is_empty()) {
            said = said.replace(token.secret(), "…");
        }
        if said.is_empty() {
            said = format!("{} ended with {}", SECURITY, output.status);
        }
        said
    }
}

/// Reads all of a pipe on a thread of its own.
fn reader(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut read = Vec::new();
        let _ = pipe.read_to_end(&mut read);
        read
    })
}

/// How the child ended, where it ended within [`WAIT`]; it is stopped where it did not.
fn waited(child: &mut Child) -> Option<std::process::ExitStatus> {
    let until = Instant::now() + WAIT;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Whether a text may stand as one word of a line the program reads on its standard input: some text with no space,
/// no quote, no backslash and no control character. A name is so by its rule, and every token the agents' program
/// makes is.
fn one_word(text: &str) -> bool {
    !text.is_empty()
        && (text.chars()).all(|c| !c.is_whitespace() && !c.is_control() && c != '"' && c != '\'' && c != '\\')
}

impl Tokens for Keychain {
    fn keep(&self, name: &str, token: &Token) -> Result<(), String> {
        if !one_word(name) {
            return Err(format!("{name:?} cannot name an item of the keychain"));
        }
        if !one_word(token.secret()) {
            return Err(
                "the token holds a space, a quote or a control character, which no token the program makes holds: \
                 copy it again whole"
                    .to_string(),
            );
        }
        // The token on the program's standard input, so that no list of processes shows it; an item kept under the name
        // before is changed to it.
        let line = format!(
            "add-generic-password -U -s \"{}\" -a \"{name}\" -w \"{}\"\n",
            self.service,
            token.secret()
        );
        let output = self.run(&["-i"], Some(&line))?;
        // The program reads a line of its standard input and goes on whatever came of it: what it kept is read back.
        match self.token(name) {
            Ok(kept) if kept == *token => Ok(()),
            _ => Err(format!(
                "the login keychain did not keep the token: {}",
                Keychain::said(&output, Some(token))
            )),
        }
    }

    fn token(&self, name: &str) -> Result<Token, String> {
        let output = self.run(&["find-generic-password", "-s", &self.service, "-a", name, "-w"], None)?;
        if !output.status.success() {
            return Err(Keychain::said(&output, None));
        }
        let text = String::from_utf8(output.stdout).map_err(|_| "the token kept is not text".to_string())?;
        let secret = text.strip_suffix('\n').unwrap_or(&text).to_string();
        if secret.is_empty() {
            return Err("the token kept is empty".to_string());
        }
        Ok(Token::new(secret))
    }

    fn let_go(&self, name: &str) -> Result<(), String> {
        let output = self.run(&["delete-generic-password", "-s", &self.service, "-a", name], None)?;
        match output.status.code() {
            Some(0) | Some(NOT_FOUND) => Ok(()),
            _ => Err(Keychain::said(&output, None)),
        }
    }

    fn closed(&self) -> Option<String> {
        self.refused.clone().or_else(|| {
            (!self.program.exists()).then(|| {
                format!(
                    "{} is not on this system, and no token can be kept in the login keychain",
                    self.program.display()
                )
            })
        })
    }

    fn making(&self) -> String {
        self.making.clone()
    }

    fn life_ms(&self) -> u64 {
        crate::account::LIFE_MS
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// A stand-in for the program, which keeps each item as a file named by its account in the directory, its
    /// arguments, a line a call, in `args`, and what it read on its standard input in `stdin`.
    fn stand_in(dir: &Path) -> PathBuf {
        let program = dir.join("security");
        let script = format!(
            r#"#!/bin/sh
dir='{dir}'
printf '%s\n' "$*" >> "$dir/args"
command=$1
if [ "$command" = -i ]; then
    line=$(cat)
    printf '%s\n' "$line" >> "$dir/stdin"
    eval "set -- $line"
    command=$1
fi
shift
account=''
secret=''
while [ $# -gt 0 ]; do
    case "$1" in
        -a) account=$2; shift ;;
        -w) [ $# -gt 1 ] && secret=$2 && shift ;;
    esac
    shift
done
case "$command" in
    add-generic-password) printf '%s' "$secret" > "$dir/item-$account" ;;
    find-generic-password)
        if [ -f "$dir/item-$account" ]; then cat "$dir/item-$account"; echo; else
            echo 'security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain.' >&2
            exit 44
        fi ;;
    delete-generic-password)
        if [ -f "$dir/item-$account" ]; then rm "$dir/item-$account"; else
            echo 'security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain.' >&2
            exit 44
        fi ;;
esac
"#,
            dir = dir.display()
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program
    }

    fn read(dir: &Path, file: &str) -> String {
        std::fs::read_to_string(dir.join(file)).unwrap_or_default()
    }

    // Each takes a few milliseconds: a shell script run a few times.
    #[test]
    fn a_token_is_kept_given_on_the_standard_input_read_back_and_let_go_of() {
        let dir = tempfile::tempdir().unwrap();
        let keychain = Keychain::new(
            &stand_in(dir.path()),
            &uuid::Uuid::nil(),
            None,
            "claude setup-token".into(),
        );
        let token = Token::new("sk-ant-oat01-secret_token-1".to_string());
        assert_eq!(keychain.closed(), None);
        keychain.keep("work", &token).expect("the token is kept");
        assert_eq!(keychain.token("work"), Ok(token.clone()));
        let args = read(dir.path(), "args");
        assert!(!args.contains("sk-ant"), "the token is in the arguments: {args}");
        let service = "wf-agents.00000000-0000-0000-0000-000000000000";
        assert!(
            args.contains(&format!("find-generic-password -s {service} -a work -w")),
            "{args}"
        );
        assert_eq!(
            read(dir.path(), "stdin"),
            format!("add-generic-password -U -s \"{service}\" -a \"work\" -w \"sk-ant-oat01-secret_token-1\"\n")
        );
        // Kept again, the item is changed.
        let again = Token::new("sk-ant-oat01-another".to_string());
        keychain.keep("work", &again).expect("the token is kept again");
        assert_eq!(keychain.token("work"), Ok(again));
        keychain.let_go("work").expect("it is let go of");
        let lacking = keychain.token("work").unwrap_err();
        assert!(lacking.contains("could not be found in the keychain"), "{lacking}");
        keychain.let_go("work").expect("none is nothing to let go of");
    }

    #[test]
    fn a_token_that_cannot_stand_as_one_word_is_refused_without_its_words_and_a_missing_program_closes_the_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let keychain = Keychain::new(
            &stand_in(dir.path()),
            &uuid::Uuid::nil(),
            None,
            "claude setup-token".into(),
        );
        for secret in ["sk-ant oat01", "sk-\"ant", "sk-ant\n"] {
            let refused = keychain.keep("work", &Token::new(secret.to_string())).unwrap_err();
            assert!(!refused.contains("sk-"), "{refused}");
        }
        assert_eq!(read(dir.path(), "args"), "", "nothing was run");
        let missing = Keychain::new(&dir.path().join("missing"), &uuid::Uuid::nil(), None, String::new());
        assert!(
            missing
                .closed()
                .is_some_and(|why| why.contains("is not on this system"))
        );
        let refused = Keychain::new(
            &stand_in(dir.path()),
            &uuid::Uuid::nil(),
            Some("agents.env".into()),
            String::new(),
        );
        assert_eq!(refused.closed().as_deref(), Some("agents.env"));
    }
}
