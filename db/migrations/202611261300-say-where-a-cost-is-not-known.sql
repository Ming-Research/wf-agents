-- From version 35 of the schema to version 36: what a round, a keep-warm ping and a fork took says whether some of its
-- cost is not known, `unpriced`: calls of it that the agents' program reported no cost of, and that no earlier report
-- of their model prices, the engine keeping no table of prices. What was kept before was priced, from a table of prices
-- where the program reported nothing: it is 0. The tables are made anew rather than altered, so that a database
-- brought to version 36 holds the same tables as one made with it. The version's one row is replaced last.
CREATE TABLE spent_new (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    unpriced INTEGER NOT NULL DEFAULT 0,
    uncached_tokens INTEGER NOT NULL,
    cache_written_tokens INTEGER,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    context_tokens INTEGER,
    compactions INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (work, run, round),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
INSERT INTO spent_new (work, run, round, usd_micros, unpriced, uncached_tokens, cache_written_tokens, cached_tokens,
                       output_tokens, context_tokens, compactions)
SELECT work, run, round, usd_micros, 0, uncached_tokens, cache_written_tokens, cached_tokens, output_tokens,
       context_tokens, compactions
FROM spent;
DROP TABLE spent;
ALTER TABLE spent_new RENAME TO spent;

CREATE TABLE pinged_new (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    line TEXT NOT NULL,
    session INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    unpriced INTEGER NOT NULL DEFAULT 0,
    uncached_tokens INTEGER NOT NULL,
    cache_written_tokens INTEGER,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    context_tokens INTEGER,
    compactions INTEGER NOT NULL,
    failed TEXT, -- Hidden from agents.
    PRIMARY KEY (work, at)
);
INSERT INTO pinged_new (work, at, line, session, usd_micros, unpriced, uncached_tokens, cache_written_tokens,
                        cached_tokens, output_tokens, context_tokens, compactions, failed)
SELECT work, at, line, session, usd_micros, 0, uncached_tokens, cache_written_tokens, cached_tokens, output_tokens,
       context_tokens, compactions, failed
FROM pinged;
DROP TABLE pinged;
ALTER TABLE pinged_new RENAME TO pinged;

CREATE TABLE forked_new (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    unpriced INTEGER NOT NULL DEFAULT 0,
    uncached_tokens INTEGER NOT NULL,
    cache_written_tokens INTEGER,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    PRIMARY KEY (work, at)
);
INSERT INTO forked_new (work, at, run, round, usd_micros, unpriced, uncached_tokens, cache_written_tokens,
                        cached_tokens, output_tokens)
SELECT work, at, run, round, usd_micros, 0, uncached_tokens, cache_written_tokens, cached_tokens, output_tokens
FROM forked;
DROP TABLE forked;
ALTER TABLE forked_new RENAME TO forked;
UPDATE schema_version SET version = 36;
