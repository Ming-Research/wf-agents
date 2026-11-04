# Where and how an agent may run git (item #250)

Serves: the engineer and the reviewer of #250, as the evidence behind the git paragraph the work adds to the agents'
`work:` text and behind the change to the `history_sources` input. Delete it when #250 merges: the text and the flow
are then the record.

## Question

Which git commands does the permission check let an agent run, where, and how do the three recorded refusals (matter
587: git on the project's checkout; observation 116: git pipes and `git grep`; observation 204: the checkout is outside
the session's directories) arise? Is the project's checkout, which `history_sources` gives the gatherer, a place an
agent needs at all?

## Method (exploratory: no rejection criterion was written before the probes)

Each command below was run once, as one Bash call, by the engineer line of work 302, on 2026-11-04, under the
installation's own `claude` command line and settings. The CLI version was not recorded (`claude --version` was itself
refused). `<wt>` is the work's worktree, which is the round's working directory; `<checkout>` is the project's
checkout, outside the session's directories. The recorded reports quote no command, so which form each refusal took
is inferred from this table.

## Results in the working directory (`<wt>`)

Pass: `git log --oneline -5 | head -2`; `git log --oneline | wc -l`; `git log --oneline -3 | grep -n fix`;
`... | sort`; `... | cut -c1-8`; `git show HEAD:README.md | sed -n 1,2p`; `git log -1 --format=%h && git log -1
--format=%s; git status --short | head -3`; `git log --oneline -2 2>&1 | tail -1`;
`git grep -n history_sources -- flow/flow.json`; `git grep -n history_sources origin/main -- flow/flow.json`;
`git grep -n -E 'a|b' origin/main -- README.md | head -2` (a `|` inside quotes is not a pipe);
`git log --format='%h %s' -2 --grep='checkout\|history'`; `git log -L :history_sources:<file> -s --oneline`;
`git blame -L 1,2 README.md`; `git diff origin/main...HEAD --stat`; `git for-each-ref refs/remotes/origin | head -2`;
`git worktree list | head -3`; `git -C <wt> grep -n history_sources -- flow/flow.json`; `cd <wt> && git log ...`;
`git log -S history_sources --oneline -2`; `git log --follow --oneline -2 -- README.md`; `git log --oneline -5 | tail -2`.

Refused:

| Command | Message |
|---|---|
| `git log --oneline -3 \| awk '{print $1}'` | This Bash command contains multiple operations. The following part requires approval: awk '{print $1}' |
| `git show $(git rev-parse HEAD):README.md \| head -1` | A nested command in this command can't be checked before it runs |
| ``git show `git rev-parse HEAD`:README.md \| head -1`` | the same message |

## Results on the checkout (`<checkout>`)

| Command | Result |
|---|---|
| `cd <checkout> && git log --oneline -1` | refused: This command changes directory before running a version-control command, which can pick up untrusted hooks or repository configuration from the target directory. Approve only if you trust it. |
| `git -C <checkout> grep -n history_sources -- flow/flow.json` | refused: git in '<checkout>/--' was blocked. For security, Claude Code may only access files with git from the allowed working directories for this session: ... |
| `git -C <checkout> grep -n history_sources HEAD` | refused, same message, for '<checkout>/HEAD' |
| `git -C <checkout> diff --stat HEAD~1 HEAD` | refused, same message, for '<checkout>/HEAD~1' |
| `git -C <checkout> log --oneline -1`; `log -2 -- README.md`; `show HEAD:README.md`; `show origin/main:README.md`; `ls-tree HEAD`; `ls-files \| head -2`; `branch --show-current`; `rev-parse origin/main main`; `log -S history_sources --oneline -1` | pass |

`ls <dir outside the session's directories>` and `Read` of a file there are refused too.

## What the checkout is, next to the worktree

- `git rev-parse --git-common-dir` in `<wt>` is `<checkout>/.git`: the worktree shares the checkout's objects and refs.
- `origin/main` is the same commit in both (`c2332346`); the checkout's own `main` branch stands at `0ac7ecc1`, 105
  commits behind it (`git rev-list --count 0ac7ecc1..origin/main`). `Project::main_ref` says why: nothing updates the
  checkout's `main`.

## Reading

- Every read form a gatherer needs (`log` with `-S`, `-L`, `--follow`, `show`, `grep` also at a revision, `blame`,
  `diff`, `ls-tree`) passes in the working directory, alone, chained with `&&` or `;`, or piped into `head`, `tail`,
  `sed`, `wc`, `grep`, `sort`, `cut`. The check judges each command of a chain or pipe by itself.
- What the checkout adds is only a stale `main` and refusals: a `cd` into it is refused, and `git -C` on it refuses
  `grep` and `diff`, which read their arguments as paths of that directory.
- So the checkout the input names is no place to run git; the worktree is the place, and holds all the checkout holds.
  That is also what #74 decided for the architect and what #204 decided for the engine's own reads.

## What would reject this

A round of the gatherer whose working directory is not a worktree of the project's repository, or whose `origin/<main>`
differs from the checkout's. The first is read from `steps::cell_request` (the worktree, or for an architect's cells its
graph's worktree); the second is the shared-refs result above.
