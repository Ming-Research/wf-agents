//! Reading a flow's cases directory: `common.toml`, and each other `.toml` file, a cell's cases, in the order of their
//! names. Every file that cannot be read is said, each once.

use std::path::Path;

use crate::checked::Fault;
use crate::written::{CellFile, Common};

/// The file of what a flow's cases share.
pub const COMMON: &str = "common.toml";

/// A flow's cases as their files are written: what they share, and each cell's file, by its name.
#[derive(Clone, Debug, Default)]
pub struct Read {
    pub common: Common,
    pub cells: Vec<(String, CellFile)>,
}

/// Reads the cases directory of a flow; every file that cannot be read is a fault.
pub fn read(dir: &Path) -> Result<Read, Vec<Fault>> {
    let fault = |file: &str, what: String| Fault {
        file: file.to_string(),
        case: None,
        what,
    };
    let entries = std::fs::read_dir(dir).map_err(|e| vec![fault(&dir.display().to_string(), e.to_string())])?;
    let mut names: Vec<String> = (entries.filter_map(Result::ok))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".toml"))
        .collect();
    names.sort();
    let mut faults = Vec::new();
    let mut read = Read::default();
    if !names.iter().any(|n| n == COMMON) {
        faults.push(fault(COMMON, "there is no such file".to_string()));
    }
    for name in names {
        let text = match std::fs::read_to_string(dir.join(&name)) {
            Ok(text) => text,
            Err(e) => {
                faults.push(fault(&name, e.to_string()));
                continue;
            }
        };
        if name == COMMON {
            match toml::from_str::<Common>(&text) {
                Ok(common) => read.common = common,
                Err(e) => faults.push(fault(&name, e.message().to_string())),
            }
        } else {
            match toml::from_str::<CellFile>(&text) {
                Ok(file) => read.cells.push((name, file)),
                Err(e) => faults.push(fault(&name, e.message().to_string())),
            }
        }
    }
    match faults.is_empty() {
        true => Ok(read),
        false => Err(faults),
    }
}
