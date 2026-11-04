# Checking the format before a `ci` call

Item #286. This note holds the facts the design of #286 rests on, taken on 2026-11-04 (the engine's records read at
about 21:30 UTC; the checks of the tree on origin/main at ece3499a). It changes no engine, CI or flow file. It serves
the review of #286's design and whoever measures the same thing again after #286 merges; it is deleted once that
measurement is read.

Each claim says how it is known: (measured) on the data or the run of the date above, (read) from the source,
(inferred) not tried.

## 1. The question

Can an engineer's agent check, before it calls `ci`, what CI's first step (`cargo fmt --all --check`) checks, with a
command its command line lets through? What would refute it: the engineer's command line refuses the command; the
command's verdict differs from CI's on a tree CI judged; or the calls it would save are too few to matter.

## 2. What was found

| Fact | How known |
|---|---|
| `cargo fmt --all --check` is refused on the engineer's command line: "Permission to use Bash with command cargo fmt --all --check has been denied." The command line holds `Bash(cargo fmt *)` both as allowed and as refused (the engine refuses it again, as a change of #266 meant). | measured, run in this round |
| `Bash(rustfmt *)` is among the command line's rules in all 140 recorded rounds of cells D1, I1 (Implement), R1, T1 and T6 of works 300 and later (24, 39, 39, 17 and 21). The engine's list of heavy commands holds no `rustfmt`, so the engine adds no refusal of it. | presence measured on the records (Q4); the heavy list read in `crates/config/src/lib.rs`. Q4 counts presence only, not whether a refusal follows it in a round; the one run in an Implement round's shell (`rustfmt --check` on `crates/engine/src/inputs.rs`) was not refused |
| `rustfmt --check crates/**/*.rs` in the worktree: 171 files (the number `git ls-files '*.rs'` gives, all under `crates/`), no output, exit 0, 2.3 s in all (1.97 s user), on origin/main at ece3499a, where the `ci` workflow succeeded. | measured |
| It reads the repository's `rustfmt.toml` (width 120, `edition = "2024"`). The `rustfmt` that rustup runs in the worktree reported `rustfmt 1.10.0-stable (58cf8f9c35 2026-10-13)`. That rustup runs the toolchain `rust-toolchain.toml` pins is rustup's documented behaviour, not checked here. | measured (config and version); inferred (toolchain selection) |
| It finds what CI found. Two heads CI turned red on format, run as `git show <head>:<file> \| rustfmt --check`: `crates/api/src/owner.rs` at f8754a2b gives a diff at line 119 that joins the `Err(format!(...))` on one line, the one CI's log shows at the same line; `crates/web/tests/flow_cost.rs` at 3465a967 gives a diff at line 51 that breaks `.args([...])` into one item per line, the one CI's log shows at the same line. | measured (CI's text from `tool_runs`, id 4700 and id 4363) |
| It finds a wrong format and follows modules: a scratch root with `mod child;` and a child with `vec![1,2,3]` gave exit 1 and the diff for the child; a file `mid.rs` with `mod leaf;` found `mid/leaf.rs`. The scratch files were removed and not committed. | measured |
| Every out-of-line `mod x;` of the workspace is in a target root (`lib.rs`, `main.rs`, `tests/e2e/main.rs`), so naming a changed file never resolves a child wrongly today. | read (a search over `crates/`) |
| `crates/**/*.rs` reaches all 171 files in zsh (this machine's shell). In a shell without recursive globs `**` means one level, and the pattern then matches only `crates/<crate>/build.rs`: a check that silently covers less. The design therefore tells the agent to name its files. | zsh measured; the other shell inferred |

## 3. What a missing check costs

From the engine's records of `ci` calls, 2026-10-26 to 2026-11-04 (measured, queries in section 4):

- 747 `ci` calls gave a result with checks; in 72 (9.6%) the lint job was red at its first step, `cargo fmt --all
  --check`; 69 of the 72 are from 2026-11-02 06:13 UTC on.
- In 23 of the 72 (3.1% of all calls; 17 works) no other check was red but `lint` and `check` (`check` is red whenever
  another job is: 212 red against 142 red `lint`, inferred). Each of those needed a second `ci` call for the format
  alone. The 23 calls took 9,671 s together; the median is 431 s, the shortest 12 s, the longest 530 s.
- In the other 49 a test or another check was red too, so a second call was needed anyway; a format check before the
  call would have saved those no call.

So a check before the call saves at most one call in 32, some 7 minutes of CI each, and an agent round to read the
result; the check itself takes seconds.

## 4. Queries and commands

Queries, on the engine's database with its `query` tool:

```sql
-- Q1: the calls, and those with rustfmt's diff text in the result.
SELECT count(*), sum(result LIKE '%Diff in %'), count(DISTINCT work), min(queued_at), max(queued_at)
FROM tool_runs WHERE tool = 'ci';

-- Q2: calls with a lint red at the format step, and those where nothing else was red.
WITH r AS (
  SELECT t.id, t.work, t.queued_at AS at, (t.ended_at - t.started_at) / 1000 AS secs,
    (SELECT count(*) FROM json_each(json_extract(t.result, '$.push.checks')) c
      WHERE json_extract(c.value, '$.result') = 'red'
        AND json_extract(c.value, '$.name') NOT IN ('lint', 'check')) AS other_reds,
    (SELECT count(*) FROM json_each(json_extract(t.result, '$.push.checks')) c
      WHERE json_extract(c.value, '$.result') = 'red'
        AND json_extract(c.value, '$.log_tail') LIKE 'Run cargo fmt --all --check%') AS fmt_reds
  FROM tool_runs t
  WHERE t.tool = 'ci' AND json_valid(t.result) AND json_extract(t.result, '$.push.checks') IS NOT NULL)
SELECT count(*) AS with_checks,
  sum(fmt_reds > 0) AS fmt_red,
  sum(fmt_reds > 0 AND other_reds = 0) AS fmt_only_red,
  count(DISTINCT CASE WHEN fmt_reds > 0 AND other_reds = 0 THEN work END) AS works_fmt_only,
  sum(CASE WHEN fmt_reds > 0 AND other_reds = 0 THEN secs END) AS secs_fmt_only
FROM r;

-- Q3: red checks by name.
SELECT json_extract(c.value, '$.name') AS name, count(*)
FROM tool_runs t, json_each(json_extract(t.result, '$.push.checks')) c
WHERE t.tool = 'ci' AND json_valid(t.result) AND json_extract(c.value, '$.result') = 'red'
GROUP BY name ORDER BY 2 DESC;

-- Q4: whether Bash(rustfmt *) is among the command line's rules, by cell. Presence only: the query does not say
-- whether a refusal follows it.
SELECT r.cell, count(*) AS rounds,
  sum(EXISTS (SELECT 1 FROM json_each(json_extract(r.input, '$.call.command'))
              WHERE value = 'Bash(rustfmt *)')) AS allow_rustfmt
FROM rounds r
WHERE r.cell IN ('D1', 'I1', 'R1', 'T1', 'T6') AND r.session IS NOT NULL AND r.work >= 300
GROUP BY r.cell;
```

Commands, in the worktree:

```sh
# The whole tree: no output and exit 0 when it is formatted (zsh).
rustfmt --check crates/**/*.rs
# A past head's file, as CI saw it: the diff must be CI's.
git show f8754a2b0aebd9874df9b4f778e5d4f504591502:crates/api/src/owner.rs | rustfmt --check
git show 3465a9670f7fd3f7ef5be5b7ad2028f6a8af1162:crates/web/tests/flow_cost.rs | rustfmt --check
```

## 5. After #286 merges

Run Q2 again over the `ci` calls made after the merge. If calls whose only red check is the format still come at a rate
near 3.1%, agents do not follow the sentence, and the check belongs in the `ci` tool (the engine runs a command the
project's configuration writes before it pushes). That command would run on the owner's machine outside the agent's
permission check, a kind of command the owner has not yet ruled on: the open question of #280's design (its round 7
answer says the owner decides it). The check waits for that ruling. If the calls are gone or rare, nothing more is
needed.

## 6. Ways of making the check certain that were set aside

None of these is built; each is judged against the sentence in AGENTS.md.

- A hook that runs `rustfmt` after every edit of a Rust file (the engine already passes a `--settings` hook of the
  command line, one that runs after a compaction, measured on the recorded command line) or a git pre-commit hook:
  both run a command outside the agent's permission check, a hook of the command line rests on a feature of one agent's
  command line, and a git hook needs a setting of the owner's clone that the repository cannot carry.
- A `format` tool the agent asks for: one more round of the agent for what a command does in 2 s.
- `cargo fmt` made runnable by listing it among the permissions' `locks` in the installation (inferred from
  `Permissions::locks_of` and `refused`, not tried): the list names commands that update lock files, and #283 deletes it.
