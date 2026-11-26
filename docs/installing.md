# Installing a build of the engine

How to get the program `wf-agents` that an installation runs: for whoever installs a new build, which today is the
assistant. It covers getting the program and putting it where the installation runs it. What belongs to one machine,
such as stopping the engine, keeping a copy of the store and starting the engine again, is that installation's own and
is not said here.

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
