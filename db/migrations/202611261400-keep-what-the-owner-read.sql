-- From version 36 of the schema to version 37: the engine keeps what the owner read, in a row of the new table
-- `owner_read` for each version read, at the place of the read in the work's order. A step of the store that runs after
-- this SQL gives each work kept before its one place of reads (see the table). The version's one row is replaced last.
-- The owner read these versions of the work's blocks, as of this place in the work's order: a place holds the versions
-- read together, a row for each, a version named by its block and the place of the entry that wrote it, as in
-- `contents`. The versions that have no row are the ones the owner has not read. A work kept before the engine kept
-- what the owner read has one such place, the first after its latest entry, which holds every version it had then
-- except each version a run wrote to a cell's words block: those are the cells' replies, and they stay unread.
CREATE TABLE owner_read (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    block TEXT NOT NULL,
    version INTEGER NOT NULL,
    PRIMARY KEY (work, at, block, version)
);

UPDATE schema_version SET version = 37;
