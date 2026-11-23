//! The tokens a round, a keep-warm ping and a fork wrote to the cache are kept apart from those they sent fresh. A file
//! made before they were kept apart is brought to this version without a value of it changed: the column of the tokens
//! sent and not read from the cache is renamed, and a round whose last try in `measured` has tokens that add up to its
//! own has the tokens that try wrote to the cache; no other row says how many it wrote. How a work's rounds, pings and forks
//! are kept and read back is held by the random histories and the tests of the forks.

use store::SCHEMA_VERSION;

use super::opening::{SCHEMA_1, columns};
use super::*;

/// A try of a round as `measured` of version 26 holds it: the tokens sent fresh and those written to the cache, the
/// others being nothing.
fn tried(file: &Connection, at: u64, run: u64, round: u64, attempt: u64, fresh: u64, written: u64) {
    file.execute(
        "INSERT INTO measured (work, at, run, round, attempt, input_tokens, cache_read_tokens, cache_written_tokens, \
         thinking_tokens, turns, calls, first_read, first_context, added, given, before, received, crossed, priced) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, 0, ?6, NULL, 1, 1, NULL, NULL, 0, 0, '{}', 1, 0, 0)",
        rusqlite::params![at, run, round, attempt, fresh, written],
    )
    .expect("a try is kept");
}

/// A round of a file of version 26: the run, the round and the tokens it sent and did not read from the cache.
fn took(file: &Connection, run: u64, round: u64, uncached: u64) {
    file.execute(
        "INSERT INTO spent (work, run, round, usd_micros, input_tokens, cached_tokens, output_tokens, context_tokens, \
         compactions) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![run, round, 7 * run + round, uncached, 11 * uncached, 3, 100 + round, 2],
    )
    .expect("a round is kept");
}

/// The rounds of the file by their run and round, each with its cost, the tokens it sent and did not read from the cache
/// (read from the column of this name), those it read from it, those that came back, its context (-1 where it has
/// none) and its compactions.
fn rounds(path: &Path, uncached: &str) -> Vec<(i64, i64, Vec<i64>)> {
    let file = Connection::open(path).expect("the database opens");
    let mut query = file
        .prepare(&format!(
            "SELECT run, round, usd_micros, {uncached}, cached_tokens, output_tokens, COALESCE(context_tokens, -1), \
             compactions FROM spent ORDER BY run, round"
        ))
        .expect("the rounds are read");
    let rows = query
        .query_map([], |row| {
            let values = (2..8).map(|i| row.get(i)).collect::<Result<Vec<i64>, _>>()?;
            Ok((row.get(0)?, row.get(1)?, values))
        })
        .expect("the rounds are read");
    rows.collect::<Result<_, _>>().expect("the rounds are read")
}

/// The tokens each round of the file wrote to the cache, by its run and round, where it says.
fn written(path: &Path) -> Vec<(i64, i64, Option<i64>)> {
    let file = Connection::open(path).expect("the database opens");
    let mut query = file
        .prepare("SELECT run, round, cache_written_tokens FROM spent ORDER BY run, round")
        .expect("the rounds are read");
    let rows = query
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("the rounds are read");
    rows.collect::<Result<_, _>>().expect("the rounds are read")
}

#[test]
fn a_file_made_before_the_tokens_written_to_the_cache_were_kept_apart_is_brought_here_with_every_value_as_it_was() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.pragma_update(None, "foreign_keys", "OFF")
            .expect("the foreign keys are let be");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        store::migrate(&file, 1, 26).expect("the migrations run");
        // The work and its two runs, which every round, ping and fork below belongs to.
        file.execute("INSERT INTO items (number, title, added) VALUES (1, 'kept', 1)", [])
            .expect("an item is kept");
        file.execute(
            "INSERT INTO works (number, item, flow, added) VALUES (1, 1, 'engineer', 1)",
            [],
        )
        .expect("a work is kept");
        for (at, run) in [(2, 1), (3, 2)] {
            file.execute(
                "INSERT INTO started (work, at, run, cell, because) VALUES (1, ?1, ?2, 'plan', '[]')",
                [at, run],
            )
            .expect("a run is kept");
        }
        // Run 1 round 1: its one try's tokens add up to its own. Round 2: they do not. Round 3: it has no try kept.
        // Run 2 round 1: its first try's tokens add up to its own and its last try's do not. Run 2 round 2: its first
        // try's tokens do not add up to its own and its last try's do.
        took(&file, 1, 1, 1_060);
        tried(&file, 10, 1, 1, 1, 60, 1_000);
        took(&file, 1, 2, 500);
        tried(&file, 20, 1, 2, 1, 10, 20);
        took(&file, 1, 3, 800);
        took(&file, 2, 1, 400);
        tried(&file, 30, 2, 1, 1, 400, 0);
        tried(&file, 31, 2, 1, 2, 100, 200);
        took(&file, 2, 2, 400);
        tried(&file, 32, 2, 2, 1, 50, 50);
        tried(&file, 33, 2, 2, 2, 100, 300);
        file.execute(
            "INSERT INTO pinged (work, at, line, session, usd_micros, input_tokens, cached_tokens, output_tokens, \
             context_tokens, compactions, failed) VALUES (1, 40, 'main', 0, 5, 70, 80, 9, NULL, 0, NULL)",
            [],
        )
        .expect("a ping is kept");
        file.execute(
            "INSERT INTO forked (work, at, run, round, usd_micros, input_tokens, cached_tokens, output_tokens) \
             VALUES (1, 50, 1, 1, 6, 120, 18, 30)",
            [],
        )
        .expect("a fork is kept");
    }
    let before = rounds(&db.path, "input_tokens");
    assert_eq!(before.len(), 5);
    db.open();
    let file = Connection::open(&db.path).expect("the database opens");
    let version: u32 = (file.query_row("SELECT version FROM schema_version", [], |row| row.get(0))).expect("a version");
    assert_eq!(version, SCHEMA_VERSION);
    // Every value is as it was.
    assert_eq!(rounds(&db.path, "uncached_tokens"), before);
    // The tokens written to the cache are those of a last try that adds up to the round's own, and no others.
    assert_eq!(
        written(&db.path),
        [
            (1, 1, Some(1_000)),
            (1, 2, None),
            (1, 3, None),
            (2, 1, None),
            (2, 2, Some(300))
        ]
    );
    // A ping and a fork kept before say nothing of them, and keep every other value.
    let ping: (i64, Option<i64>, i64) = file
        .query_row(
            "SELECT uncached_tokens, cache_written_tokens, cached_tokens FROM pinged",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the ping is read");
    assert_eq!(ping, (70, None, 80));
    let fork: (i64, Option<i64>, i64, i64) = file
        .query_row(
            "SELECT uncached_tokens, cache_written_tokens, cached_tokens, output_tokens FROM forked",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("the fork is read");
    assert_eq!(fork, (120, None, 18, 30));
    // The migrated file holds the same tables as a new one.
    let new = Db::new();
    new.open();
    assert_eq!(
        columns(&db.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
}

/// A round of a file of version 26 of the work, with its tokens sent and not read from the cache, and one try of it in
/// `measured` whose tokens sent fresh and written to the cache are these.
fn round_of(file: &Connection, work: u64, run: u64, round: u64, uncached: u64, tries: &[(u64, u64, u64)]) {
    file.execute(
        "INSERT INTO spent (work, run, round, usd_micros, input_tokens, cached_tokens, output_tokens, context_tokens, \
         compactions) VALUES (?1, ?2, ?3, 1, ?4, 0, 0, NULL, 0)",
        rusqlite::params![work, run, round, uncached],
    )
    .expect("a round is kept");
    for (attempt, fresh, written) in tries {
        file.execute(
            "INSERT INTO measured (work, at, run, round, attempt, input_tokens, cache_read_tokens, \
             cache_written_tokens, thinking_tokens, turns, calls, first_read, first_context, added, given, before, \
             received, crossed, priced) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, NULL, 1, 1, NULL, NULL, 0, 0, '{}', 1, 0, 0)",
            rusqlite::params![work, 100 + attempt, run, round, attempt, fresh, written],
        )
        .expect("a try is kept");
    }
}

// The tries of a round are those of its own work, run and round; the last of them is the one with the greatest attempt
// number, whatever numbers come before it; a round that sent and wrote nothing is one that wrote none (0), and not one
// that is not known.
#[test]
fn a_round_has_the_tokens_written_to_the_cache_of_its_own_tries_only() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.pragma_update(None, "foreign_keys", "OFF")
            .expect("the foreign keys are let be");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        store::migrate(&file, 1, 26).expect("the migrations run");
        for work in [1, 2] {
            file.execute(
                "INSERT INTO items (number, title, added) VALUES (?1, 'kept', 1)",
                [work],
            )
            .expect("an item is kept");
            file.execute(
                "INSERT INTO works (number, item, flow, added) VALUES (?1, ?1, 'engineer', 1)",
                [work],
            )
            .expect("a work is kept");
            file.execute(
                "INSERT INTO started (work, at, run, cell, because) VALUES (?1, 2, 1, 'plan', '[]')",
                [work],
            )
            .expect("a run is kept");
        }
        // The same run and round of two works, the tries of each adding up to its own round's tokens.
        round_of(&file, 1, 1, 1, 500, &[(1, 100, 400)]);
        round_of(&file, 2, 1, 1, 500, &[(1, 200, 300)]);
        // A round whose tries are numbered 1 and 3, the last of them adding up and the first not.
        round_of(&file, 1, 1, 2, 900, &[(1, 5, 5), (3, 400, 500)]);
        // A round that sent and wrote nothing.
        round_of(&file, 1, 1, 3, 0, &[(1, 0, 0)]);
        // Two rounds of one run: the tries of the one are not the tries of the other.
        round_of(&file, 2, 1, 2, 0, &[]);
        round_of(&file, 2, 1, 3, 800, &[(1, 300, 500)]);
    }
    db.open();
    let file = Connection::open(&db.path).expect("the database opens");
    let mut query = file
        .prepare("SELECT work, round, cache_written_tokens FROM spent ORDER BY work, run, round")
        .expect("the rounds are read");
    let told: Vec<(i64, i64, Option<i64>)> = query
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("the rounds are read")
        .collect::<Result<_, _>>()
        .expect("the rounds are read");
    assert_eq!(
        told,
        [
            (1, 1, Some(400)),
            (1, 2, Some(500)),
            (1, 3, Some(0)),
            (2, 1, Some(300)),
            (2, 2, None),
            (2, 3, Some(500))
        ]
    );
}
