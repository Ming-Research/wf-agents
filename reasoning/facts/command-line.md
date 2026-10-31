Fact append-system-prompt: The command line appends the text it is given with `--append-system-prompt` to its own system prompt, at the end of the system block it caches, after the tools.

Fact cli-add-dir-permissions: Claude Code has no read-only working directory: `--add-dir` lets its tools read and edit there, and in the `acceptEdits` mode its own filesystem commands (mkdir, touch, rm, rmdir, mv, cp, sed) run there without asking.

Fact cli-counts-per-session: The agents' command line counts usage per model for the whole session.

Fact cli-edit-rule: A Claude Code Edit rule on a path refuses every tool that edits a file there and the writes Claude Code recognises (such as tee and redirections), but not its own filesystem commands, and a path in it that starts with two slashes is absolute.

Fact cli-if-then-refusal: The command line 2.1.312, tried with haiku, refuses an answer of cell D2 without `why` told apart by an `if` and a `then` with `/outcome: must have required property 'why', /outcome: must match "then" schema` and takes the answer with `why`.

Fact cli-image-size: The command line keeps an image at most 2,000 pixels on its long side: a PNG of 3,000 by 2,000 is kept as 2,000 by 1,333.

Fact cli-inherits-env: The command line inherits the engine's whole environment, and a variable there can change its prompt.

Fact cli-keeps-2048: The command line keeps in its session only the first 2,048 characters of a StructuredOutput input it could not parse.

Fact cli-reads-project-instructions: A command line session started in a project directory reads the project's instructions and settings by itself, such as the worktree's AGENTS.md, in every session whether the session needs them or not.

Fact cli-result-totals: The result the command line reports at the end of a run gives only the whole run's tokens and its last call's.

Fact cli-signs-in-in-terminal: The agents' command line signs in only in its own terminal.

Fact cli-snapshot: A command line shows a snapshot and needs exact command syntax for every answer.

Fact cli-switch: With an agent command line in print mode, the owner can switch accounts in a terminal and switch to another vendor's command line.

Fact cli-validator-where: The command line of version 2.1.312 refuses a return saying where and why in the words of its validator, and the model, told to put right what it refuses, puts it right at its next attempt.

Fact cli-working-dirs: Claude Code lets its tools reach only its working directories.

Fact extra-brace-string: A form written as an object with a closing brace too many does not parse as JSON and reaches the command line as a string of that text, which it refuses with `/outcome: must be object`.

Fact failed-run-report-quotes: The command line's report of a failed run quotes the first part of the return it could not take.

Fact global-failure-stops-all: A reached usage limit, an unreachable network, an expired login or a command line that cannot be found stops every work alike.

Fact hook-output-added: The command line adds a hook's output to the session where the agent is.

Fact hook-text-only: A hook of the command line adds text only to the session.

Fact prefix-change-rewrites: When what comes before a session's messages in the prompt changes (the model; the tools, among which the command line gives the structured-output schema; the command line's own version, whose system prompt and tools come first), the command line writes the whole session again by design.

Fact resume-same-directory: The command line resumes a session only in the directory it ran in.

Fact schema-top-level: The command line's structured-output schema takes no `anyOf` or `allOf` at its top level, and takes `allOf` inside a field.

Fact session-in-config-dir: The command line keeps a session in the configuration directory it ran in, so another account's directory does not hold it and `--resume` there fails.

Fact stream-json-images: The command line 2.1.312, tried with haiku, reads an image sent as base64 in a stream-json user message, and takes `--json-schema`, `--resume` (the session read from the cache, an image given in its first run still held) and its hooks as with plain text.

Fact transient-failures-pass: An overloaded service, a server error, a timeout and a crash of the command line pass.

Fact cli-schema-refusal: The command line refuses a return that does not fit its structured-output schema within the round, and the model hands it in again within the round.
