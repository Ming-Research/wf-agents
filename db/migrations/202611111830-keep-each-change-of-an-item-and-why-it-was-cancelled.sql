-- From version 6 of the schema to version 7: an item may be given another title, come to depend on another item or no
-- longer depend on one, as well as be put at another level, each change kept in one table with its time, in the order
-- it was made; and a cancel says why where it was given a reason. Each level change kept before is kept as a change of
-- the same place and time, its table then taken away, so that a database brought to version 7 holds the same tables as
-- one made with it. A cancel kept before gave no reason, and is kept with none. The version's one row is replaced
-- last, so that code of any version reads the version it is now.
CREATE TABLE item_changes (
    item INTEGER NOT NULL REFERENCES items (number),
    n INTEGER NOT NULL,
    at INTEGER NOT NULL,
    change TEXT NOT NULL,
    PRIMARY KEY (item, n)
);
INSERT INTO item_changes (item, n, at, change) SELECT item, n, at, json_object('Level', level) FROM level_changes;
DROP TABLE level_changes;
ALTER TABLE cancelled ADD COLUMN why TEXT;
UPDATE schema_version SET version = 7;
