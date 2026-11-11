-- From version 2 of the schema to version 3: the copies of the database that were taken, which of them were uploaded
-- off the machine, and what failed in keeping them. They belong to no work, and a database brought to version 3 holds
-- the same tables as one made with it. The version's one row is replaced last.
CREATE TABLE backups (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    taken INTEGER NOT NULL
);
CREATE TABLE backup_uploads (
    name TEXT PRIMARY KEY REFERENCES backups (name),
    at INTEGER NOT NULL
);
CREATE TABLE backup_failures (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL,
    name TEXT NOT NULL,
    step TEXT NOT NULL,
    why TEXT NOT NULL
);
CREATE TABLE backup_steps (
    mended INTEGER PRIMARY KEY REFERENCES backup_failures (seq),
    at INTEGER NOT NULL
);
UPDATE schema_version SET version = 3;
