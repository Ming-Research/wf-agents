//! The sessions of each line of each work. A session's id is worked out, never stored: a name-based UUID (version 5,
//! from a SHA-1 hash) of the line within the work and of which of the line's sessions it is, in the namespace of this
//! installation, whose own id is made at random once and kept in the data directory. Which of its sessions a round of
//! a line runs in is counted from the work's records (`work::sessions`). The records and that file are then all that
//! is needed to find a session again, and two installations never share one.

use std::path::Path;

use store::WorkNumber;
use uuid::Uuid;

/// The file in the data directory that holds the installation's id.
pub const INSTALLATION: &str = "installation";

/// The installation's id, read from its file in the data directory, or made and written there where there is none.
pub fn installation(data: &Path) -> Result<Uuid, String> {
    let path = data.join(INSTALLATION);
    match std::fs::read_to_string(&path) {
        Ok(text) => Uuid::parse_str(text.trim()).map_err(|e| format!("{}: not an id: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let id = Uuid::new_v4();
            std::fs::write(&path, format!("{id}\n")).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(id)
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The id of the session of this number of the line in the work, the line's first being none.
pub fn session(installation: Uuid, work: WorkNumber, line: &str, number: u32) -> Uuid {
    let name = match number {
        0 => format!("work/{}/line/{line}", work.0),
        n => format!("work/{}/line/{line}/session/{n}", work.0),
    };
    Uuid::new_v5(&installation, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_id_is_the_same_whenever_it_is_worked_out() {
        let installation = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let id = session(installation, WorkNumber(7), "engineer", 0);
        assert_eq!(id, session(installation, WorkNumber(7), "engineer", 0));
        assert_eq!(id.get_version_num(), 5);
        assert_ne!(id, session(installation, WorkNumber(7), "reviewer", 0));
        assert_ne!(id, session(installation, WorkNumber(8), "engineer", 0));
        let other = Uuid::parse_str("0d9e4b61-3f2a-4c8d-b7e5-91a6c3f0d2b8").unwrap();
        assert_ne!(id, session(other, WorkNumber(7), "engineer", 0));
        let next = session(installation, WorkNumber(7), "engineer", 1);
        assert_ne!(id, next, "a new session of the line has an id of its own");
        assert_ne!(next, session(installation, WorkNumber(7), "engineer", 2));
    }

    #[test]
    fn the_installation_is_made_once_and_kept() {
        let data = tempfile::tempdir().unwrap();
        let first = installation(data.path()).unwrap();
        assert_eq!(installation(data.path()).unwrap(), first);
    }
}
