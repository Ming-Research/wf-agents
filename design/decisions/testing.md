# Testing wf-agents

How wf-agents itself is tested: its own CI workflows, the machines they run on and how they keep their time.

### decision wf-agents-ci-runs-on-the-owners-machine
The jobs of wf-agents' own workflows run on the owner's machine, on the runners labelled `14900k`, but for one that builds nothing.
- replaced: The fast CI on GitHub's hosted runners stood on main; the owner changed it, because a runner on his build machine changes nothing in the engine, keeps one definition of how tests run, and stays apart from his laptop.
- replaced: The full CI on GitHub's runners stood on main; the owner changed it, because his machine runs it faster, as the old system came to do.
- rejected: an agent proposed checking several works together in one run of the full CI; the owner rejected it, because he chose to move the full CI to his machine instead, which is faster.
- rejected: an agent proposed not using the machine's runners until the owner narrowed them to this repository in GitHub's settings; the owner rejected it, because a pull request from outside runs only with his approval.

### decision wf-agents-ci-bounds-each-test-and-job
wf-agents' workflows stop and fail a test still running after 60 seconds, but for the walks of random histories, and a job past its limit, 15 minutes for the Rust tests.
- replaced: A job's limit of 30 minutes, and none on one test, stood on main; the owner changed it, because the flow's gates bounded single tests only, and nothing bounded the whole run.

### decision wf-agents-tests-build-dependencies-optimized
wf-agents' CI builds its tests with the dependencies fully optimized and its own crates unoptimized, but for the flow crate, at level 1.
- replaced: Test builds unoptimized throughout, SQLite built for debugging, stood on main; the owner changed it, because slow CI was the first thing to mend, or the iterations would never end.
- replaced: The workspace's own crates built at level 1 stood on main; the owner changed it, because the build, not the tests, was the larger part of each run.

### decision wf-agents-ci-splits-its-tests-by-count
wf-agents' CI runs its Rust tests in three jobs, each a third of every binary's tests by nextest's count, by no list kept by hand.
- replaced: One machine running the groups of tests one after another stood on main; the owner changed it, because CI took about 23 minutes, and slow CI makes the iterations endless.
- rejected: an agent proposed four jobs, each first running its share of a hand-kept list of the long tests; the owner rejected it, because it was about ten seconds faster on two runs, and the list goes stale as the tests change.
