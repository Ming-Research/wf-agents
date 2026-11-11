//! A copy of the database, made whole and complete under its name.

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use work::Time;

use crate::name_at;

/// The ending of a file not yet complete.
pub(crate) const PARTIAL: &str = ".partial";

/// How long the copy waits for a lock the database's writers hold.
const WAIT: Duration = Duration::from_secs(30);

/// Copies the database at `database` as one snapshot, on a connection of its own that only reads it, so that the
/// writers of the loop go on, compresses it, and gives it its name by one rename once it is whole and on the disk:
/// until then the only files it makes end in `.partial`. Gives the copy's name. `halt` is asked after each step that
/// leaves something on the disk, and where it answers true the copy is left as it is, as a stopped process would.
pub(crate) fn take(
    database: &Path,
    dir: &Path,
    time: Time,
    halt: &mut dyn FnMut() -> bool,
) -> Result<Option<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = name_at(time);
    let to = dir.join(&name);
    let plain = dir.join(format!("{name}.raw{PARTIAL}"));
    let zipped = dir.join(format!("{name}{PARTIAL}"));
    for left in [&plain, &zipped] {
        if left.exists() {
            std::fs::remove_file(left).map_err(|e| format!("{}: {e}", left.display()))?;
        }
    }
    {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(database, flags).map_err(|e| format!("{}: {e}", database.display()))?;
        conn.busy_timeout(WAIT).map_err(|e| e.to_string())?;
        conn.execute("VACUUM INTO ?1", [plain.to_string_lossy()])
            .map_err(|e| format!("copying {}: {e}", database.display()))?;
    }
    if halt() {
        return Ok(None);
    }
    compress(&plain, &zipped)?;
    std::fs::remove_file(&plain).map_err(|e| format!("{}: {e}", plain.display()))?;
    if halt() {
        return Ok(None);
    }
    std::fs::rename(&zipped, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    if halt() {
        return Ok(None);
    }
    Ok(Some(name))
}

/// Compresses `from` with gzip into `to`, and makes sure it is on the disk.
fn compress(from: &Path, to: &Path) -> Result<(), String> {
    let failed = |e: std::io::Error| format!("{}: {e}", to.display());
    let mut input = BufReader::new(File::open(from).map_err(|e| format!("{}: {e}", from.display()))?);
    let out = File::create(to).map_err(failed)?;
    let mut gz = flate2::write::GzEncoder::new(BufWriter::new(out), flate2::Compression::default());
    std::io::copy(&mut input, &mut gz).map_err(failed)?;
    let mut writer = gz.finish().map_err(failed)?;
    writer.flush().map_err(failed)?;
    let file = writer.into_inner().map_err(|e| failed(e.into_error()))?;
    file.sync_all().map_err(failed)
}
