-- From version 33 of the schema to version 34: the engine keeps each time a call was held because the host could not
-- carry it out for now, with why and the wait of the engine's own it is held until, in a row of the new table `held`.
-- A call kept before was never held, and has no row. The version's one row is replaced last.
CREATE TABLE held (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    call INTEGER NOT NULL,
    wait INTEGER NOT NULL,
    obstacle TEXT NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, call) REFERENCES called (work, call),
    FOREIGN KEY (work, wait) REFERENCES called (work, call)
);
UPDATE schema_version SET version = 34;
