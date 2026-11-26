-- From version 37 of the schema to version 38: the store keeps the main dependency of each item some item depends on,
-- in the new table `mains`. A step of the store that runs after this SQL marks one for each such item: of the items
-- that depend on it, one not cancelled before one cancelled, then one of its own project before one of another, then
-- the one whose dependency was made first, then the one of the lowest number. The version's one row is replaced last.
-- The main dependency of an item, from this time on: the dependency on it of `item`, the one the owner's page draws
-- it whole under; or, with `item` null, none, as nothing depends on it any more. The latest row of an item says which,
-- and a row is kept each time that changes, by the store itself as each write leaves the items (module `mains` of the
-- crate `store`): an item some item depends on has exactly one main dependency, and an item nothing depends on none.
-- `at` is null for a row marked as the store first kept main dependencies.
CREATE TABLE mains (
    n INTEGER PRIMARY KEY,
    on_item INTEGER NOT NULL REFERENCES items (number),
    item INTEGER REFERENCES items (number),
    at INTEGER
);

UPDATE schema_version SET version = 38;
