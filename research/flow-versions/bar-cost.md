# What drawing a work's time bar takes, by how many waits it has, measured on 2026-11-21

Research for item #21 (W21). The bar of a work that has not ended is drawn anew for each request of the works list and
of the work's page, so its cost is paid per request. This note serves the review of that cost, and is kept until the way
the bar decides what a work waited for changes again; then it is replaced by the numbers of that change.

## Question

Main drew a work's bar in 96 cells and decided what the work waited for at the middle of each cell less than half of
which ran, one decision for each place of the records among them. W21 moved the stretches of a work's time into the
engine and kept them exact, deciding each stretch between two places where something changed. Does drawing the bar of
a work going on now cost more per request than main, at the size of the largest works in the owner's store?

The size, by the store on 2026-11-21: W21 had 496 moments and 83 rounds, about 180 to 204 places where it waited; W2
238 moments and 180 waits; W20 402 moments and 151 waits; every other work fewer than 110 waits. Only a work going on
on the installed version decides its bar at a request: a work that ended draws what it kept as it ended, and one kept
before the versions draws its records alone.

## How it is measured

A test of `crates/web/src/works.rs`, kept for one run and then reverted, builds a work on the small test flow whose
writer's round fails `n` times, the owner letting it on a second after each failure, so that it has `n` waits and about
`2n` moments. For `n` = 20, 100 and 200 it times nine draws of each bar on the same work and reports the median, the
lowest and the highest:

- head: `bar` as the branch has it;
- main: main's `bar` and `waited` at 91994061f, copied into the test, given the runs as the page has them already.

It runs in CI, which builds the project's own crates unoptimized (decision wf-agents-tests-build-dependencies-optimized),
so the times overstate what the installed engine takes; both bars are built alike, so their ratio holds. The small flow
has two cells and the engineer flow fifteen, so one decision costs more in the owner's store; again alike for both.

## The first run: the exact stretches at each request (exploratory)

Run 44980854442, on b5aa00175, timed the branch's first bar, which drew a work going on from its exact stretches, one
decision for each wait, against main's 96 cells approximated in the engine's test. No criterion was written before it,
so it tests no prediction.

| waits | moments | head median (min, max) | main's decisions | main median (min, max) |
|---|---|---|---|---|
| 20 | 41 | 0.86 ms (0.82, 3.05) | 20 | 0.81 ms (0.80, 0.82) |
| 100 | 201 | 11.7 ms (11.5, 11.8) | 54 | 6.3 ms (6.2, 12.1) |
| 200 | 401 | 101.6 ms (43.1, 150.7) | 44 | 10.0 ms (9.9, 10.1) |

Each decision reads the records before its place, so the exact stretches grow with the square of the waits: at W21's
size the branch took ten times main's time for each request. So the bar of a work going on was changed to decide only
at the middles of its cells, as main did, through the same decision the kept stretches are made by
(`driver::time::Waits::at`); the stretches are worked out whole only once, as a work ends.

## The second run: deciding at the middles of the cells

Written before this run. The branch's bar decides at most once for each cell less than half of which ran, as main's did.
The proposal is rejected if, at 100 or at 200 waits, the median of head's draws is more than 1.5 times the median of
main's.

Results: to be filled in from the run.
