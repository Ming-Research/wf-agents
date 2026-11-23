-- From version 26 of the schema to version 27: what a round, a keep-warm ping and a fork took says how many of the
-- tokens it sent and did not read from the cache it wrote to it, the rest being sent fresh. The column that held the
-- tokens sent and not read from the cache is named for what it holds, `uncached_tokens`, its values as they were: the
-- program and `measured` call the tokens sent fresh `input_tokens`. A ping and a fork kept before say no tokens
-- written to the cache: they are NULL. Where a round has a try in `measured` whose tokens add up
-- to the round's, exactly as the program counts them, the tokens that try says it wrote to the cache are the round's;
-- a round whose last try's tokens do not add up to its own, or that has none kept, stays NULL. The tables are made
-- anew rather than altered, so that a database brought to version 27 holds the same tables as one made with it. The
-- version's one row is replaced last.
CREATE TABLE spent_new (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    uncached_tokens INTEGER NOT NULL,
    cache_written_tokens INTEGER,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    context_tokens INTEGER,
    compactions INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (work, run, round),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);
INSERT INTO spent_new (work, run, round, usd_micros, uncached_tokens, cache_written_tokens, cached_tokens,
                       output_tokens, context_tokens, compactions)
SELECT s.work, s.run, s.round, s.usd_micros, s.input_tokens,
       (SELECT m.cache_written_tokens FROM measured m
         WHERE m.work = s.work AND m.run = s.run AND m.round = s.round
           AND m.attempt = (SELECT max(x.attempt) FROM measured x
                             WHERE x.work = s.work AND x.run = s.run AND x.round = s.round)
           AND m.input_tokens + m.cache_written_tokens = s.input_tokens),
       s.cached_tokens, s.output_tokens, s.context_tokens, s.compactions
FROM spent s;
DROP TABLE spent;
ALTER TABLE spent_new RENAME TO spent;

CREATE TABLE pinged_new (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    line TEXT NOT NULL,
    session INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    uncached_tokens INTEGER NOT NULL,
    cache_written_tokens INTEGER,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    context_tokens INTEGER,
    compactions INTEGER NOT NULL,
    failed TEXT, -- Hidden from agents.
    PRIMARY KEY (work, at)
);
INSERT INTO pinged_new (work, at, line, session, usd_micros, uncached_tokens, cache_written_tokens, cached_tokens,
                        output_tokens, context_tokens, compactions, failed)
SELECT work, at, line, session, usd_micros, input_tokens, NULL, cached_tokens, output_tokens, context_tokens,
       compactions, failed
FROM pinged;
DROP TABLE pinged;
ALTER TABLE pinged_new RENAME TO pinged;

CREATE TABLE forked_new (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    uncached_tokens INTEGER NOT NULL,
    cache_written_tokens INTEGER,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    PRIMARY KEY (work, at)
);
INSERT INTO forked_new (work, at, run, round, usd_micros, uncached_tokens, cache_written_tokens, cached_tokens,
                        output_tokens)
SELECT work, at, run, round, usd_micros, input_tokens, NULL, cached_tokens, output_tokens
FROM forked;
DROP TABLE forked;
ALTER TABLE forked_new RENAME TO forked;
UPDATE schema_version SET version = 27;
