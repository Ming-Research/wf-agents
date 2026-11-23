# Two checkouts that share a build directory: which page does the program carry?

Item #93. This note holds the measurements that `crates/web/build.rs`, `crates/wf-agents/build.rs` and the `WF_CHECKOUT`
value of `.cargo/config.toml` rest on, taken on 2026-11-23 in the CI of the work (the runs are named below). It changes
no engine or flow file. It serves the review of #93 and whoever changes how the page or the flows are built into the
program; it is deleted when `WF_CHECKOUT` is removed from `.cargo/config.toml`.

Each claim says how it is known: (measured) in the CI run named, (read) from the source, (inferred) not tried.

## 1. The question

Built into one target directory from two checkouts of one layout, as the owner's worktrees may be, does a program carry
the page of the checkout it was built in? Which shape of the build script makes it so?

The test is `crates/web/tests/shared_target.rs`. It makes a small package of the workspace's layout in two checkouts,
`a` and `b`, with different pages, both written before the first build, and builds them into one target directory in the
order a, b, a. The criterion was written in the item's plan before the first shape was tried: a shape is rejected if a
program's embedded `index.html` is not the page of the checkout it was built in.

## 2. What was found

| Shape | Result | How known |
|---|---|---|
| The script this crate had before: the checkout's path from `env!("CARGO_MANIFEST_DIR")`, fixed when the script was compiled, and an absolute path given to `rerun-if-changed`. | The program built from `b` after `a` carries the page of `a`. 0.316 s in both runs. | measured, runs 45173314281 (at 7a2ad3ddb) and 45179776509 (at 43cec91ff) |
| The script as built now (the path from the run-time `CARGO_MANIFEST_DIR`, relative paths given to `rerun-if-changed`, `rerun-if-env-changed=WF_CHECKOUT`), with no `.cargo/config.toml` in the checkouts, so that the variable has no value. | The program built from `b` carries the page of `a`: the same result, so that relative paths alone do not help. 0.313 s. | measured, run 45179776509 |
| The same script with the repository's `.cargo/config.toml` in each checkout (`[env]`, `WF_CHECKOUT = { value = ".", relative = true }`), the test taking `WF_CHECKOUT` out of the environment of the builds it runs. | `a`, `b` and `a` each carry their own page. 0.435 s. | measured, run 45179776509 |
| The same, with the variable left in the environment of the builds the test runs. | The program built from `b` carries the page of `a`; the test failed in 0.309 s. The test's own process held `WF_CHECKOUT` with the value of the repository's checkout, which nextest gives each test from the config's `[env]` where the variable is not set, and cargo does not replace a variable that is set unless `force` is true. | failure measured, run 45173314281; nextest's rule read in `nextest_runner/cargo_config/env.rs` (nexte.st/rustdoc/src/nextest_runner/cargo_config/env.rs.html) |
| Why the first two fail. | A build script's `rerun-if-changed` paths count as changed only when a file is newer than the output of the script's last run (`LocalFingerprint::RerunIfChanged`, compared by modification time). Every file of `b` is older than the build of `a`, so `b` is taken as unchanged. A variable named by `rerun-if-env-changed` is read from the config's `[env]` first, then from the process, and its value is part of the fingerprint (`LocalFingerprint::from_env`), so a value that differs between checkouts makes cargo run the script again. | read, `compiler/fingerprint/mod.rs` of rust-lang/cargo's main branch on 2026-11-23, not of the 1.99.1 toolchain the repository pins |

Each check of the test was watched failing once, on a script made wrong in one way (runs 45180570315, at fdb59e3f3, and
45189713721, at d9a422691):

| Wrong script | Result | How known |
|---|---|---|
| A script that watches a file that is not there, so that cargo runs it on every build. | The second build says `Compiling`: the check of `a_build_with_nothing_changed_compiles_nothing_again` can fail, and the word it looks for is the one cargo uses. 0.598 s. | measured, run 45180570315 |
| The check of the stamp replaced by an empty list of sources. | A page whose source changed after the page was built is built in with no error, so `a_page_not_built_from_the_sources_at_hand_is_not_built_in` fails on it. 0.480 s. | measured, run 45180570315 |
| The watch of `client/dist` left out; the page's `index.html` written again with other text. | The program still carries the new text: rustc itself watches the files `include_bytes!` reads, so a changed file is never missed. The first form of `a_page_built_again_is_the_page_the_next_build_embeds` could not tell this script from the right one. 0.600 s. | measured, run 45180570315 |
| The same script, and the page built again as `npm run build` leaves it: an asset of one name taken away, one of another name put in, `index.html` written again. | The second build fails. The cause is inferred, since the message was not kept: the list of the page's files that the script wrote the first time names a file that is gone. The rewritten test, which makes this change and expects the right script to build and to carry the new files, is red on it. 0.272 s. | failure measured, run 45189713721; cause inferred |

## 3. What was not tried

- The two scripts that carry something into the program are the ones that watch the checkout's path. Cargo judges every
  crate of the workspace by file times, so the second checkout's sources, older than the first build, are taken for
  unchanged in the same way (inferred from the reading above). The owner's installation builds from one checkout, or from
  a build directory of its own, where this makes no difference.
- Other versions of cargo than the 1.99.1 of CI, and other systems than the runners'.
- Timing and spread: each test ran once in each run. The same test in two runs took 0.316 and 0.316 s (the old script),
  0.329 and 0.335 s (nothing changed), and 0.310 and 0.319 s (the stale sources), so the spread is a few milliseconds.
