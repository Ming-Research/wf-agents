-- From version 23 of the schema to version 24: each fork of a round's session asked a question is kept with what it
-- took, as a keep-warm ping is. A work kept before has none. The version's one row is replaced last.
CREATE TABLE forked (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 24;
