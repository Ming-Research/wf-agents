-- From version 14 of the schema to version 15: what the calls of each try of an agent's round came to is kept, the
-- tokens written to the prompt cache apart from those sent fresh and those read from it, and so is each keep-warm ping
-- of a line's session. A try kept before says nothing of its calls, and is no part of the history a later round is
-- judged by; no ping was sent before. The version's one row is replaced last.
CREATE TABLE measured (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    attempt INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cache_read_tokens INTEGER NOT NULL,
    cache_written_tokens INTEGER NOT NULL,
    thinking_tokens INTEGER,
    turns INTEGER NOT NULL,
    calls INTEGER NOT NULL,
    first_read INTEGER,
    first_context INTEGER,
    added INTEGER NOT NULL,
    given INTEGER NOT NULL,
    before TEXT NOT NULL,
    received INTEGER NOT NULL,
    crossed INTEGER NOT NULL,
    priced INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
CREATE TABLE pinged (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    line TEXT NOT NULL,
    session INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    context_tokens INTEGER,
    compactions INTEGER NOT NULL,
    failed TEXT, -- Hidden from agents.
    PRIMARY KEY (work, at)
);
UPDATE schema_version SET version = 15;
