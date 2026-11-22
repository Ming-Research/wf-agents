# Does a changed system prompt reach a session that is resumed?

Question: when the agents' program (`claude -p`, version 2.1.327) resumes a session with another
`--append-system-prompt`, does the session use the new text? If it does, a line's system prompt could be renewed in
place and its history kept; if it does not, a changed prompt reaches a line only as a new session.

Result that rejects the proposal "renew in place", written before the run: the resumed call answers with the text of
the first call, or with no text.

Method, run once on 2026-11-21, model `haiku`, three calls in an empty directory, the same session id:

1. `--session-id <id> --append-system-prompt "The secret word is ALPHA."`, message "Say OK."
2. `--resume <id> --append-system-prompt "The secret word is BRAVO."`, message "What is the secret word given in your
   system prompt? One word, or NONE if there is none."
3. `--resume <id>` with no system prompt flag, the same message as call 2.

Result: call 1 answered `OK`; call 2 answered `ALPHA`; call 3 answered `ALPHA`. The history held only "Say OK." and its
answer, so the word came from the system prompt the session began with. A resumed session uses the system prompt it
began with and ignores the one given again.

Limits: one run, one model, one version of the program. It shows that the system prompt of a session cannot be
changed by resuming it. It says nothing of the form of return (`--json-schema`), which the engine already keeps with
the session and which, once changed, misses the whole prompt cache (principle one-form-of-return-for-a-session).
