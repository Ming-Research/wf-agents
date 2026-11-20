# Which flow each recorded work ran on, and whether it can still be read

Research for item #21 (W21): reading every ended work by the version of the flow it ran on. Measured on 2026-11-18
against the owner's store (schema version 20) and main at `79f7270fc`.

## Questions

1. Can the version of the flow each existing work ran on be told from the records, round by round?
2. Can today's compiler read those versions?
3. What does a version weigh, kept whole?

A reading was to count as settled only where the records and git both say it; a version assigned from a commit's time
alone was to be rejected, since what the engine runs is the flow in its directory as it starts, not main.

## Method

Read-only queries on the store through the engine's `query` tool, and `git` on the repository. No program was built or
run: the installed engine's `wf-agents flow check` could not be run from the agent's sandbox, so whether an old version
compiles was settled by comparing the keys its files use with the shapes the compiler accepts
(`crates/flow/src/source.rs`, every shape `#[serde(deny_unknown_fields)]`).

Queries (all on the owner's store):

```sql
-- the cells each work ran, in order
SELECT work, group_concat(cell, ' ') FROM (SELECT work, run, cell FROM started ORDER BY work, run) GROUP BY work;
-- the blocks each work's runs started from
SELECT work, group_concat(block, ' ') FROM (SELECT DISTINCT work, block FROM basis ORDER BY work, block) GROUP BY work;
-- the engine's starts it kept a trace of
SELECT * FROM everything_paused ORDER BY n;
-- when each work began and ended (moments.time, milliseconds since 1970)
SELECT w.number, min(m.time), max(m.time) FROM works w JOIN moments m ON m.work = w.number GROUP BY w.number;
```

Git: `git grep -e '^title = ' <commits> -- flow/engineer` at the commits where the engineer flow's cells changed
(`git log --first-parent --format='%h %cI %s' HEAD -- flow/engineer/cells flow/engineer/flow.toml`).

## Findings

### The cells changed four times; each change gave an id to another cell

| From commit | Cells of the engineer flow |
|---|---|
| `80c14f113` (#115), files `flow/engineer/<id>.toml` | S1 R0 R1 D1 Deep research, **D2 Review the plan**, I1, T1 Review, T2 Adversarial testing, M1, M2 |
| `bee45991d` (#206), files `cells/<id>.toml` | adds R2 Is there a better way; **D2 Is there a better way**, **D3 Design review**, **D4 Design tree review** |
| `8bfaa6007` (#335) | D3 and D4 become **P1 Design review** and **P2 Design tree review** |
| `7956ec9c4` (#351) | **T2 Test review**, **T3 Design tree review of the change**, **T4 Adversarial testing** |

### Each recorded work ran within one of these, told by the names its records hold

| Works | Cells run | Names only that layout had | Ran on the cells of |
|---|---|---|---|
| W1–W3 | S1 R0 R1 D2 I1 T1 T2 M1 M2 | `answers_*`, `opinions_*`, `reply_*` | `80c14f113` to before `bee45991d` |
| W4–W10 | S1 R0 R1 R2 D4 I1 T1 T2 M1 M2 | `r1_spoken_to`, `i1_answered`, `d4_spoken_to` | `bee45991d` to before `8bfaa6007` |
| W11 | S1 R0 R1 R2 P1 P2 I1 T1 M1 M2 | `guide`, `i1_words` | `8bfaa6007` to before `388c91cb3` (#341, which took `guide` out) |
| W20 | S1 R0 R1 R2 P1 P2 I1 T1 T2 M1 M2 | `i1_words`, no `guide` | `388c91cb3` to before `7956ec9c4` |
| W21 | S1 R0 R1 D1 | `d1_questions` | `7956ec9c4` on |

W12 and W13 have no run; W14 to W19 were cancelled with no run. No work's records mix the names of two rows, so no
work crossed a change of the cells while it ran.

What today's flow makes of them: W2's and W3's D2 ("Review the plan") read as "Is there a better way"; W5's, W9's and
W10's D4 name no cell of today's flow, and the history, which draws a column for each cell of the flow
(`client/src/works/History.tsx`), has none for them; W9's and W20's T2 ("Adversarial testing") read as "Test review";
W11, which never had a test review, is drawn against today's T2 "Test review".

Each work that merged ended within seconds of its pull request's commit on main: W5 at 01:27:41 UTC on 2026-11-15 and
#244 at 01:27:37; W9 at 01:55:51 and #237 at 01:55:47; W11 at 04:03:53 on 2026-11-17 and #341 at 04:03:49; W20 at
03:16:20 on 2026-11-19 and #351 at 03:16:16. So W11 made #341, which took `guide` out, and W20 made #351, which
renumbered the T cells: each ran on the flow before its own change.

### Which version, round by round, cannot be told

- The engine reads the flow from its directory as it starts (`crates/wf-agents/src/running.rs`, `read`), and keeps no
  record of a start but the pause it writes where a work is going on (`crates/driver/src/lib.rs`, `Driver::open`): 12
  of the 28 rows of `everything_paused` are such starts. The start that installed #351's flow, after W20 ended at
  03:16 UTC and before W21 was added at 03:35, left no row.
- Main changed `flow/` 7, 37 and 6 times in the three periods between the commits that changed the cells; what the
  records hold tells those apart only where a datum's name changed.
- W11 is the one work whose version the records and git together nearly fix: it was added at 02:37:41 UTC on
  2026-11-17, 11 minutes after #335 (`8bfaa6007`) merged, its records hold #335's names, and main's next change of
  `flow/` was W11's own #341. That it ran on `8bfaa6007` still rests on the engine's directory following main, which
  nothing records.

So the layout of cells each work ran on is known; the exact version is recorded for none of them.

### Today's compiler reads none of the versions W1 to W11 ran on

- Before `8bfaa6007` a cell's file uses `triggered_by`, `context` and `[do]` (`bee45991d` on) or `inputs` (before it);
  today's cell shape (`CellFile`) has none of them and refuses a key it does not know.
- At `8bfaa6007`, `flow.toml`'s `[every_agent]` has `watches` and `[every_agent.do]`, and `data` names `guide`;
  today's `EveryAgentFile` has only `reads` and `kinds`, and `flow/data/guide.toml` was taken out by #341.
- From `4e30e635c` (the flow main had as W20 was added) to `79f7270fc`, `crates/flow/src` changed only by adding (a
  block's `of_cell`, and data made for a cell named in a condition), so W20's flow most likely compiles today; this is
  inferred, not run.

### A version weighs about 61 KB

The files the compiler reads for the engineer flow at `79f7270fc` (the flow's directory, `../data/`,
`../agents/rules/rules.toml` and `../agents/waits.toml`) sum to 60,728 bytes (`git ls-tree -r -l HEAD flow`).

## Conclusions

1. No version can be assigned to an existing work from the records; only the layout of its cells can, and that only
   with the engineer flow's own names, which no rule of the engine may hold (principle engine-knows-no-cell).
2. The versions W1 to W11 ran on do not compile today; W20's most likely does, but which of its candidates it ran on is
   not known.
3. Keeping a version whole costs about 61 KB a version installed.
