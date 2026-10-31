-- Takes a version 9 database to version 10: the engine's notice of a new ask of the owner names that ask, for the
-- owner's interface to link to.

ALTER TABLE engine_notices ADD COLUMN ask INTEGER REFERENCES owner_asks(id);
