# Accounts

The accounts the agents run under, each a directory of their program's settings and sign-in, listed by the
installation's settings in the owner's order.

### decision the-account-in-use-is-kept-in-the-store
The account in use is the latest the store says was taken, by the owner or by the engine, that the settings still list,
else the first they list; the switch that lets the engine move on is kept in the store as well, off until set.
Why: Both change while the engine runs, the account with no restart, and the settings are the owner's own writing,
read as the engine starts; kept in the file, a take would wait for a restart or have the engine write the owner's file.
A key of the settings for the switch, as `sessions.warm_while_paused` is, was turned down: it would act only once the
engine started again.

### decision the-engine-moves-on-only-at-the-weeks-limit
With the switch on, the engine moves to another account only where the account in use is at a usage limit that resets
more than five hours after it was met; a limit of a few hours, or one whose reset is not known, is waited out.
Why: Moving on spends another account's usage and has every line read its context again, worth it only for a wait of
days; a limit of a few hours resets within five hours of being met, so a later reset is the week's, and a week's met
within its last five hours ends as soon as a few hours' would. Reading the stream's own name of the limit was turned
down: a limit said only in words has none.

### decision moving-on-checks-the-next-account-first
At the week's limit of the account in use, the engine takes no account blind: it checks the next listed after it, in
the owner's order, by the check it takes of the outside, under that account, and takes the first found well, passing
over each the records or a check found at a limit or signed out; with none left, the account in use waits for its
reset.
Why: A round given to an account that cannot serve writes the whole of a new session's message for nothing, at every
line, and a row of such rounds would pause works as if the check had missed a cause (decision
a-row-of-outside-failures-pauses-its-work); the check costs one short request on the cheapest model, at a limit that
comes about once a week.
