# The work that merged a change checks, once it takes effect, what only real runs can show

## What is done now

An acceptance condition that no test, check of the CI or measurement on the branch can show, and that real runs show
once the change is in effect, is written `in_effect`. It says what carries the change (`when`: `installed`, the installed
program built from this project, or `merged`, main), the tool whose real run checks it (`query`, `gh` or `measure`), what
to run (`command`), what result means it holds (`holds`) and what evidence it waits for (`enough`). The check
`in_effect_known` refuses what a project cannot check: `installed` outside the project the engine is built from, `measure`
where the project lists no workflow, and any such condition in a work that delivers a report.

The work that merged it checks it. After it writes what it learned (cell `L2`) it does not end but waits at cell `W1`,
holding no slot and keeping no session warm, until every carrier holds its merge commit. That is one definition for every
project: main holds it as the work merges; the installed program holds it when the commit it was built from has the merge
commit among its ancestors in the project's checkout. A round in a new session (cell `W2`) then runs each condition's
command as written, putting in only the time the change took effect, and gives each a verdict. Evidence not there yet
sends the work back to wait as long as the round says (`not_yet`). Everything holding ends the work; a condition that
does not hold or cannot be judged goes to the project's architect as a report (`detail` `verification`), urgent when it is
doing harm now, and the work ends.

Two limits keep a merged work from waiting for good, since it can be neither cancelled nor started over, and it keeps its
worktree meanwhile. They are the project's settings, and at either the architect is told the conditions stand unverified
and the work ends:

- The wait for the change to take effect: 72 hours since the merge (`install_wait_hours`). Measured on 2026-11-04 over the
  39 merges of the flow on main that an installed build has taken in, from the merge to the first round of a build holding
  it: 17 minutes at least, 2.0 hours at the median, 23.4 hours at p90, 26.2 hours at most. Installing is by hand, and the
  longest two installed builds were ever apart is 62 hours, so the limit stays above all of these.
- The looks: 8 times back to wait (`verification_looks`), each of at most two days (`again_after_minutes`). Measured the
  same day over the 258 works of the engineer's graph that ran a round, from the first round's start to the last round's
  end, a work ran 34.6 hours at p90 and 6.2 days at most, the longest that evidence asking for a whole work after the change
  should need; 16 days holds that more than twice over.

Taken together, a work ends within 19 days of its merge at the worst, against the eight weeks of looks alone the design
first gave.

Two conditions show the definition holds. A change to the project's CI, which merging alone puts in effect (from #244
Switch to fast CI):

```json
{
  "by": "in_effect",
  "id": "A3",
  "condition": "At least 7 real runs of the tool ci after the change have a median time of at most 180 seconds.",
  "when": "merged",
  "tool": "query",
  "command": "SELECT ended_at - started_at AS ms FROM tool_runs WHERE tool = 'ci' AND started_at > :in_effect_at AND ended_at - started_at >= 10000 ORDER BY ms",
  "holds": "The median of ms is at most 180000.",
  "enough": "At least 7 rows."
}
```

And a tool that must first be installed and then configured, the case that shows whether in effect means what it should:
what #208 Read dependency code leaves to real runs, past the seven conditions no real call could reach, written as it
would be its eighth:

```json
{
  "by": "in_effect",
  "id": "A8",
  "condition": "Once the tool is installed and the project lists its lock files, a real call of dependency_sources gives fetched.",
  "when": "installed",
  "tool": "query",
  "command": "SELECT json_extract(result, '$.sources.verdict') AS verdict, json_extract(result, '$.sources.why') AS why FROM tool_runs WHERE tool = 'dependency_sources' AND started_at > :in_effect_at ORDER BY id",
  "holds": "At least one row's verdict is fetched.",
  "enough": "At least one row."
}
```

The change is in effect once the build is installed, whatever the configuration says. With the configuration not changed
yet, the tool refuses each call, so what a real run shows is `refused`, and the condition is not judged to hold: the
verdict must name a recorded run and copy what it showed from that run's result, so that a `holds` resting on `fetched` is
sent back, and a `fails` says that the configuration is what is missing, which the architect's report carries.

This reaches only a change that merges through the flow. A change pushed straight to main goes through no cell, so no work
waits for it and nothing here checks it: #244 and #269 were finished so, by the delegate, and their numbers stay
unmeasured for all of this. That part is #275's, which adds a review of what enters main outside the flow. A revert does
not change whose ancestor a commit is, so the engine cannot see one: whoever decides it says so to the work, and the round
writes `skipped`.

## Why

- Article 1: one definition of in effect for every project, the condition naming its carrier; the flow and the engine name
  no project, and `installed` is known by the configuration's project that the engine is built from. This work's own
  conditions go through the same cells.
- Article 2: the round is given the condition's command, what holds and what evidence to wait for, the time it took
  effect, the merge and its last verdicts; the architect gets each result with its run.
- Article 3: a wait holds no slot, writes no round and keeps no session warm. A change to the CI does not wait for an
  installation it has nothing to do with. Each look is one round at the item's level, and says what it waits for.
- Article 4: each cell and value answers a gap found on real work: no cell waits for a change to take effect, no form says
  how to check it, and the evidence is often not there the moment it does. Nothing that does not hold goes unrecorded.
- Article 7: waiting is `W1`'s rows, checking is `W2`, looking again is a row between them, each limit is a value on a row,
  and telling the architect is an action on a row.

## Options not chosen

- A work of its own created at the merge to check it (the other option of decision point P2 of #279). The item would run
  the engineer's whole graph, more than ten rounds for a few conditions, a graph made for changes and reports; and it would
  still need the same wait for evidence, only in another work. Its merit is that the merged work ends at once and the check
  can be ordered and cancelled on the board.
- The project's architect checks it. Its graph has no cell that waits, and each of its rounds forks from its base, which
  costs more than a round at the item's level.
- The condition carries SQL for when it can be judged, which the engine waits for by itself. That SQL has parameters, cannot
  be tried when it is written since its data does not exist yet, and once wrong waits forever; it needs the same limit and
  report; and gh and measure results need an agent to judge them anyway, so it would cover half of what one loop covers.
- A look that cannot judge at once goes straight to the architect. Most such conditions then go unchecked, which is the
  state this replaces.
- In effect always means installed, or main with its full CI finished, as an earlier design had it. A change to the CI has
  nothing to do with an installation, and whether a full CI has run is what `enough` says for a condition that needs it.

## How it was decided

The owner's choice at decision point P2 of the design of item #279, the same work waits and checks, was made by the
assistant on the owner's behalf, under the owner's authorization on the evening of 2026-11-04, and is to be confirmed by
the owner's own review of this change. A different choice made there replaces this file.

## Challenges considered and turned down

None yet.
