# Testing wf-agents

How wf-agents itself is tested: its own CI workflows, the machines they run on and how they stay within their time.

### decision wf-agents-ci-runs-on-the-owners-machine
The jobs of wf-agents' own workflows run on the owner's machine, on the runners labelled `14900k`, except one that builds nothing.
- replaced: The fast CI on GitHub's hosted runners was on main; the owner changed it, because a runner on his build machine changes nothing in the engine, keeps one definition of how tests run, and stays separate from his laptop.
- replaced: The full CI on GitHub's runners was on main; the owner changed it, because his machine runs it faster, as the old system came to do.
- rejected: an agent proposed checking several works together in one run of the full CI; the owner rejected it, because he chose to move the full CI to his machine instead, which is faster.
- rejected: an agent proposed not using the machine's runners until the owner narrowed them to this repository in GitHub's settings; the owner rejected it, because a pull request from outside runs only with his approval.

### decision wf-agents-ci-runs-only-when-the-engine-asks
wf-agents' full workflow runs only when the engine asks for it by hand, on the commit to merge; no pull request and no push to main starts it.
- replaced: The full workflow also ran on every pull request into main and on every push to main, was on main; the owner changed it, because the engine reads only the runs it asked for, and most of CI's time went to runs nothing read (data: `research/ci-unread-runs.md`).

### decision a-change-of-only-the-tree-runs-only-the-trees-check
Where a commit differs from main only under `design/`, the full workflow runs only the design tree's check, and its other jobs are skipped and count as passed.
- replaced: Every job of the full workflow running on a change of only the tree was on main; the owner changed it, because such a change can break nothing but the form of the tree, and the page, the lint and the Rust tests cost minutes for nothing.

### decision wf-agents-ci-bounds-each-test-and-job
wf-agents' workflows stop and fail a test still running after 60 seconds, except the walks of random histories, and a job past its limit, which is 15 minutes for the Rust tests.
- replaced: A 30-minute limit on a job, and no limit on a single test, was on main; the owner changed it, because the flow's gates limited only single tests, and nothing limited the whole run.

### decision wf-agents-tests-build-dependencies-optimized
wf-agents' CI builds its tests with the dependencies fully optimized and its own crates unoptimized, except the flow crate, which is built at level 1.
- replaced: Test builds unoptimized throughout, with SQLite built for debugging, was on main; the owner changed it, because slow CI was the first thing to fix, or the iterations would never end.
- replaced: Building the workspace's own crates at level 1 was on main; the owner changed it, because the build, not the tests, was the larger part of each run.

### decision wf-agents-ci-splits-its-tests-by-count
wf-agents' CI runs its Rust tests in three jobs, each running a third of every binary's tests by nextest's count, not by any list kept by hand.
- replaced: One machine running the groups of tests one after another was on main; the owner changed it, because CI took about 23 minutes, and slow CI makes the iterations endless.
- rejected: an agent proposed four jobs, each first running its share of a hand-kept list of the long tests; the owner rejected it, because it was about ten seconds faster on two runs, and the list goes stale as the tests change.
