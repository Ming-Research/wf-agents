# The accounts

The accounts the agents' program makes its calls under, which the owner adds on the page and switches between.

### decision an-account-is-a-name-and-a-token-in-the-keychain
An account is a name the owner gives and a one-year token from `claude setup-token`, kept in the macOS login keychain and not in any file, setting or record.
- replaced: An account as the program's own directory, signed in by hand, was on main; the owner changed it, because a sign-in lasts about four weeks, so every account had to be signed in again by hand each month.

### decision the-account-in-use-is-the-engines-own-state
Which accounts exist and which one is in use are records in the store, the engine's own state: the owner changes them on the Settings page, the engine at a usage limit where he lets it.
- replaced: The account in use kept as the settings' `agents.config_dir`, written by the page and read when the engine starts, was on main; the owner changed it, because he wanted to switch between his accounts on the Settings page at any time.

### decision no-default-credential
The agents' program runs under no credential except the owner's accounts: with none in use, a round fails as a command line that cannot run, and nothing falls back to another sign-in.
- replaced: Running on the sign-in of the program's own directory when no account was added was on main; the owner changed it, because he wanted no default.

### decision no-share-of-an-allowance-is-held-back
The agents use an account's allowance without holding any share of it back for the owner, who switches to another account when one reaches its limit, or lets the engine switch.
- rejected: an agent proposed a limit keeping part of the weekly allowance for the owner; the owner rejected it, because he has several accounts and takes another when one runs out.

### decision usage-is-the-services-latest-word
What the engine knows of an account's usage is what the service last reported for that account: each window's share used and when it resets, kept in memory and read by nothing that decides.
- replaced: Knowing of an account's usage only that a round hit its limit, and when that resets, was on main; the owner changed it, because to choose an account he needs to see how much of each is used and when it resets.

### decision the-owner-lets-the-engine-move-by-a-switch
While the owner's switch is on, the engine at a usage limit checks each other account like the outside, one unchecked at once, and moves to the first found well; the switch acts at once, off at first.
- rejected: an agent proposed no switch, the engine asking the owner at each usage limit; the owner rejected it, because nobody is there to answer at night, so the work stands still, which is what moving is for, and an account is the installation's, with no work to ask in.

### decision a-fork-runs-under-its-sessions-account-unless-it-is-at-its-limit
A fork runs under the account its session last ran under, whose cache holds the session, or, saying why, under the account in use where that one was removed or is at its usage limit.
- replaced: Running a fork under its session's account wherever that account was still added, at its usage limit too, was on main; the owner changed it, because a fork of a session made under an account at its limit could not run until the limit reset, days later.
