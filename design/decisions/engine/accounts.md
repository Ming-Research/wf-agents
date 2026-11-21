# The accounts

The accounts the agents' program makes its calls under, which the owner adds on the page and switches between.

### decision an-account-is-a-name-and-a-token-in-the-keychain
An account is a name the owner gives and a one-year token from `claude setup-token`, kept in the macOS login keychain and in no file, setting or record.
- replaced: An account as a directory of the program's own, signed in by hand, was on main; the owner changed it, because a sign-in lasts about four weeks, so every account had to be signed in again by hand each month.

### decision the-account-in-use-is-the-engines-own-state
Which accounts there are and which is in use are records of the store only added to, the engine's own state, changed on the Settings page and in force from the next round.
- replaced: The account in use as the settings' `agents.config_dir`, written by the page and read as the engine starts, was on main; the owner changed it, because he would switch between his accounts on the Settings page at any time.

### decision no-default-credential
The agents' program runs under no credential but the owner's accounts: with none in use a round fails as a command line that cannot run, and nothing falls back to another sign-in.
- replaced: The sign-in of the program's own directory, run on where no account was added, was on main; the owner changed it, because he wanted no default.

### decision no-share-of-an-allowance-is-held-back
The agents use an account's allowance with no share of it held back for the owner, who takes another account when one reaches its limit.
- rejected: an agent proposed a limit keeping part of the weekly allowance for the owner; the owner rejected it, because he has several accounts and takes another when one runs out.

### decision usage-is-the-services-latest-word
What the engine knows of an account's usage is what the service last said under that account, each window's share used and its reset, kept in memory and read by nothing that decides.
- replaced: Knowing of an account's usage only that a round met its limit, and when that resets, was on main; the owner changed it, because choosing an account he needs to see how much of each is used and when it resets.
