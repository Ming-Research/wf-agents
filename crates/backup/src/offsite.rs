//! The place off the machine the copies go to: an interface (principle `outside-behind-interfaces`), the real one
//! with the `gh` command line, and one for tests.

use std::path::Path;
use std::process::Command;

/// The release of the private repository whose assets the copies are.
pub const RELEASE: &str = "database-backups";

/// A place off the machine. A call that fails says why in words; none of them panics.
pub trait Offsite: Send + Sync {
    /// Whether the place is private. Asked before every upload: a repository can be made public after it was set.
    fn private(&self) -> Result<bool, String>;

    /// Puts the file there, as an asset named as the file is, over one of the same name.
    fn upload(&self, file: &Path) -> Result<(), String>;

    /// The names of the assets there.
    fn list(&self) -> Result<Vec<String>, String>;

    /// Takes the asset of this name away.
    fn remove(&self, name: &str) -> Result<(), String>;
}

/// A release of a repository on GitHub, reached with the `gh` command line and the token it is given.
#[derive(Clone, Debug)]
pub struct Gh {
    repository: String,
    token: String,
    command: String,
}

impl Gh {
    /// The place that is this repository (`owner/name`), reached with this token by this command line.
    pub fn new(repository: &str, token: &str, command: &str) -> Gh {
        Gh {
            repository: repository.to_string(),
            token: token.to_string(),
            command: command.to_string(),
        }
    }

    /// The arguments that ask whether the repository is private.
    pub fn private_args(&self) -> Vec<String> {
        strings(&["api", &format!("repos/{}", self.repository), "--jq", ".private"])
    }

    /// The arguments that ask whether the release is there.
    pub fn view_args(&self) -> Vec<String> {
        strings(&[
            "release",
            "view",
            RELEASE,
            "--repo",
            &self.repository,
            "--json",
            "tagName",
        ])
    }

    /// The arguments that make the release, which is not the latest and holds the copies as assets.
    pub fn create_args(&self) -> Vec<String> {
        strings(&[
            "release",
            "create",
            RELEASE,
            "--repo",
            &self.repository,
            "--title",
            "Database backups",
            "--notes",
            "Copies of the wf-agents database, compressed, the newest kept.",
            "--latest=false",
        ])
    }

    /// The arguments that upload the file over an asset of its name.
    pub fn upload_args(&self, file: &Path) -> Vec<String> {
        strings(&[
            "release",
            "upload",
            RELEASE,
            &file.to_string_lossy(),
            "--repo",
            &self.repository,
            "--clobber",
        ])
    }

    /// The arguments that list the names of the assets.
    pub fn list_args(&self) -> Vec<String> {
        strings(&[
            "release",
            "view",
            RELEASE,
            "--repo",
            &self.repository,
            "--json",
            "assets",
            "--jq",
            ".assets[].name",
        ])
    }

    /// The arguments that take the asset of this name away.
    pub fn remove_args(&self, name: &str) -> Vec<String> {
        strings(&[
            "release",
            "delete-asset",
            RELEASE,
            name,
            "--repo",
            &self.repository,
            "--yes",
        ])
    }

    /// Makes the release where it is not there. Where it cannot be seen and cannot be made, that is the error.
    fn ensure_release(&self) -> Result<(), String> {
        if self.run(&self.view_args()).is_err() {
            self.run(&self.create_args())?;
        }
        Ok(())
    }

    /// Runs `gh` with these arguments, with the environment it needs and the token, and gives what it printed.
    fn run(&self, args: &[String]) -> Result<String, String> {
        let mut command = Command::new(&self.command);
        command
            .args(args)
            .env_clear()
            .env("GH_TOKEN", &self.token)
            .env("GH_PROMPT_DISABLED", "1");
        for name in ["PATH", "HOME", "USER", "LANG", "TMPDIR"] {
            if let Ok(value) = std::env::var(name) {
                command.env(name, value);
            }
        }
        let out = command.output().map_err(|e| format!("{}: {e}", self.command))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!(
                "{} {}: {}",
                self.command,
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

impl Offsite for Gh {
    fn private(&self) -> Result<bool, String> {
        Ok(self.run(&self.private_args())?.trim() == "true")
    }

    fn upload(&self, file: &Path) -> Result<(), String> {
        self.ensure_release()?;
        self.run(&self.upload_args(file)).map(|_| ())
    }

    fn list(&self) -> Result<Vec<String>, String> {
        // The release is there before it is listed, so that a place with none is an empty one, and a listing that
        // fails for any reason is an error and never looks like an empty place.
        self.ensure_release()?;
        Ok(self.run(&self.list_args())?.lines().map(str::to_string).collect())
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        self.run(&self.remove_args(name)).map(|_| ())
    }
}
