-- From version 26 of the schema to version 27: the pause of a work says the items it waits for to merge or be cancelled,
-- each in a row of its own. Every pause kept before waited for none, and is kept so; a pause the owner made now may say
-- why, which the table already kept. The version's one row is replaced last.
CREATE TABLE pause_waits (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    item INTEGER NOT NULL,
    PRIMARY KEY (work, at, item),
    FOREIGN KEY (work, at) REFERENCES pauses (work, at)
);
UPDATE schema_version SET version = 27;
