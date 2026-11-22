# How long taking a work through a flow on a script takes in CI, measured on 2026-11-22

Item #49. Whom it serves: the plan of #49 and its reviews, which judge whether one test of the suite can play every
script kept beside the flows inside the 60 s bound of decision wf-agents-ci-bounds-each-test-and-job. It is kept until
#49's own test that plays every script has measured times in CI, and is then replaced by those numbers.

Each claim says how it is known: (measured) in CI on the day, (read) from source, (inferred) from measurements, not
tried.

## 1. The question, the comparison and the result that rejects the proposal

Written before the measuring.

- Question: does taking a work of the engineer's flow through every cell to its merge, on the engine's loop with
  scripted carriers, take few enough seconds in CI that one test can play every script beside the flows, the judge
  included, inside the 60 s bound?
- Proposal: one test plays every file of `flow/scripts/` in turn and judges each work.
- What is measured: the CI time of three tests that exist on main, none built for #49, as a stand-in for a play, which
  is not built yet:
  - (a) `delivering::a_deliverable_made_of_an_engineers_item_is_delivered_once_it_merges_and_lets_on_what_depends_on_it`
    (`crates/wf-agents/tests/engine/delivering.rs`): the `wf-agents run --script` command on the engineer's and the
    deliver flows as the repository defines them, two engineer works and one deliver work taken to their merge, on the
    real clock, each round taking 20 ms (read).
  - (b) `going::a_work_goes_through_its_cells_round_the_objection_to_the_merge`
    (`crates/service/tests/works/going.rs`): the service's loop in the test's process, on the trial flow, with the
    runtime's time paused, one work round one objection to the merge (read).
  - (c) `going::a_work_whose_script_says_nothing_comes_to_its_merge`: the same loop and flow, one work straight to the
    merge (read).
- Result that rejects the proposal: (a) takes more than 15 s. A play of the engineer's script runs about twice the runs
  of one straight engineer work (its reviews object, the testing finds a fault, the check fails once) and adds the
  judge's reading; a quarter of the bound leaves room for that and for more scripts. Above 15 s, the plan must make the
  play cheaper or give each script a test of its own.

## 2. The numbers

Filled in from the run of `run_tests` that measured them.
