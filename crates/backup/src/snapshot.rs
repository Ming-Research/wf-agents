//! A copy of the database, made whole in a file that is not yet the copy.

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
/// writers of the loop go on, compresses it, and makes sure it is on the disk, in a file named as the copy is with
/// `.partial` after it. Gives the copy's name. The file is made the copy, whole under its name, by the step that asks
/// for it, through the ground, so that a stop between that and the record of it can be made: until then the only
/// files this makes end in `.partial`.
pub(crate) fn take(database: &Path, dir: &Path, time: Time) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = name_at(time);
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
        // Never a copy of the file: the store keeps a write-ahead log while the engine writes, and a copied file can
        // lack what is still in the log, or be torn. `VACUUM INTO` reads one snapshot.
        conn.execute("VACUUM INTO ?1", [plain.to_string_lossy()])
            .map_err(|e| format!("copying {}: {e}", database.display()))?;
    }
    compress(&plain, &zipped)?;
    std::fs::remove_file(&plain).map_err(|e| format!("{}: {e}", plain.display()))?;
    Ok(name)
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
