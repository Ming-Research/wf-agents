//! The wf-agents database. `db/schema.sql` is its only definition; this crate carries it into the binary.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;

/// The database's definition, as `db/schema.sql` writes it.
pub const SCHEMA: &str = include_str!("../../../db/schema.sql");

/// Every table and view a schema defines, with its columns, from the schema loaded into a database in memory.
pub fn relations(schema: &str) -> rusqlite::Result<BTreeMap<String, BTreeSet<String>>> {
    let db = Connection::open_in_memory()?;
    db.execute_batch(schema)?;
    let mut names =
        db.prepare("SELECT name FROM sqlite_schema WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'")?;
    let names: Vec<String> = names.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    let mut out = BTreeMap::new();
    for n in names {
        let mut cols = db.prepare("SELECT name FROM pragma_table_info(?1)")?;
        let cols: BTreeSet<String> = cols.query_map([&n], |r| r.get(0))?.collect::<Result<_, _>>()?;
        out.insert(n, cols);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every view loads and can be read, and each lists its columns.
    #[test]
    fn the_schema_loads_and_every_view_reads() {
        let rels = relations(SCHEMA).unwrap();
        assert!(rels["items"].contains("problem"));
        assert!(rels["work_state"].contains("work"));
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        for name in rels.keys() {
            db.prepare(&format!("SELECT * FROM {name} LIMIT 0")).unwrap();
        }
    }
}
