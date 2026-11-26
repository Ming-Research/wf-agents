# Installing a build of the engine

How to get the program `wf-agents` that an installation runs: for whoever installs a new build, which today is the
assistant. It covers getting the program and putting it where the installation runs it, and, at its end, the gate a
machine of self-hosted runners needs where measurements run on it. What belongs to one machine, such as stopping the
engine, keeping a copy of the store and starting the engine again, is that installation's own and is not said here.

The program is made by the workflow `build.yml` on each push to `main`, for macOS on arm64, with the owner's page and
the flows built into it, and kept 30 days as the artifact `wf-agents-macos-arm64-<commit>`. Nothing is compiled on the
machine that installs. The program says the commit it was built from in `wf-agents --version`.

## Steps

1. Take the commit to install, which is main's head: `sha=$(git rev-parse origin/main)`, after `git fetch origin main`
   in the checkout.
2. Find the run of `build.yml` for it:
   `gh run list --workflow build.yml --commit "$sha" --json databaseId,status,conclusion`.
   Install only where exactly one run is there, `completed` and `success`. Where there is none, it is still going
   (`queued` or `in_progress`), or it is red, do not install an earlier build: say which of the three it is, and wait
   for the run or have the cause of the red mended.
3. Download its artifact into a new, empty directory:
   `gh run download <databaseId> -n "wf-agents-macos-arm64-$sha" -D <the directory>`, and unpack it there with
   `tar -xf wf-agents-macos-arm64.tar`.
4. Check the program names the commit: `<the directory>/wf-agents --version` ends with `($sha)`.
5. Remove the old program, then copy the new one in its place: `rm <the installed program>`,
   `cp <the directory>/wf-agents <the installed program>`. Copying over the old file makes macOS kill the program at its
   start (exit code 137), because it keeps a record of the old file's signature.
6. Start the engine as the installation does, and check that it answers.

The old program is not kept: to run an earlier one, install the build of its commit, while its artifact is still kept.

## When a build is not there

A push to `main` that merges while the previous run is still going cancels that run, so the head's run is the one that
is never cancelled. The run of a commit that is not main's head may therefore be missing or cancelled, which is no
fault. A red run of the head is one: the commit does not build on macOS, and the work that mends it is an item.

## The gate on a machine that measurements run on

A measurement that a project declares on a self-hosted runner (`[ci.measure.<name>]` of its `wf-agents.toml`) times on a
machine that other jobs run on too. The gate, `runner/measure-gate.sh`, keeps them apart: every runner of the machine
runs it as its job-started hook, so that a job that starts while a measurement times waits in the hook; and the step
of a measurement's workflow that times sources it and calls `wait_for_a_quiet_machine`, which takes the gate and waits
for the jobs already running to end. The engine runs one measurement at a time on the machine, of whatever project, so
measurements do not wait for each other at the gate. A measurement's workflow fails, naming this guide, where the gate
is not installed.

It is installed once on the machine, by someone who may restart its runners:

1. As the user the runners run as, copy the gate from a checkout of main:
   `mkdir -p ~/.wf-agents-runner && cp runner/measure-gate.sh ~/.wf-agents-runner/measure-gate.sh`. The runners of the
   machine share that home, and the gate keeps its lock and its marks of the jobs that wait in that directory.
2. In each runner's directory, add to its `.env` file the line
   `ACTIONS_RUNNER_HOOK_JOB_STARTED=<that home>/.wf-agents-runner/measure-gate.sh`, the path written out whole.
3. Restart each runner's service in its directory: `sudo ./svc.sh stop`, then `sudo ./svc.sh start`.
4. Check that the next job of each runner shows the hook in its log, as a step `A job started hook` under `Set up job`.

When the gate changes on main, copy it again (step 1); a runner reads it at each job, so nothing is restarted. To take
it away, remove the line from each `.env`, restart the runners, and delete the directory.

A job that meets a measurement waits in the hook as long as the measurement's step that times takes, and that wait
counts in the job's own `timeout-minutes`: each workflow that runs on the machine gives its jobs room for it. The engine
does not count that wait in a run's `too_long` while it runs a measurement on the machine.
