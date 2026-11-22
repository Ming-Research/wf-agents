# The permission mode under `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`

This note holds the facts behind decision permissions-are-rules-not-a-mode, taken on 2026-11-22 on `claude` 2.1.327
(measured: `probe.sh` here, run on this machine). It serves whoever asks why `agents.args` and the program's settings
file may name no permission mode, and is deleted when the probe gives `acceptEdits` for it under the variable.

## The question

Does the program start in the permission mode it is given when the engine's variable `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`
is set? What would refute the decision: a mode that the program starts in under the variable.

## What was found

The mode is the `permissionMode` field of the first line of the program's stream (`--output-format stream-json`).

| Given | Variable set | Variable empty |
|---|---|---|
| `--permission-mode acceptEdits` | default | acceptEdits |
| `--permission-mode bypassPermissions` | default | bypassPermissions |
| `--permission-mode auto` | default | auto |
| `--permission-mode plan` | default | plan |
| `--permission-mode dontAsk` | default | dontAsk |
| `--permission-mode default` | default | default |
| `--dangerously-skip-permissions` | default | bypassPermissions |
| `--allow-dangerously-skip-permissions` alone | default | default |
| `--allow-dangerously-skip-permissions --permission-mode bypassPermissions` | default, and the first line is the program's warning that the mode was forced to default because the variable is set, asking for `allowedTools` to be declared explicitly | bypassPermissions |
| `permissions.defaultMode` of `acceptEdits` in the settings file of the program's directory | default | acceptEdits |

`--allowedTools` rules are rules and no mode: the engine gives them under the variable and they hold.

## How to run it again

`probe.sh <arguments>` prints the two columns for one set of arguments. The settings-file row is run with
`CLAUDE_CONFIG_DIR` set to a directory holding a `settings.json` with that key.
