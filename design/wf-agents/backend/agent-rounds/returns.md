- A return that cannot be parsed is repaired in order: the engine first closes missing brackets and moves things into
  place; failing that, the cheapest model repairs it once, and the content is checked unchanged; failing that, the agent
  hands it in again in its session; only then is the work paused and the flow's keeper told. The round is not rerun. A
  cheap model costs little and lengthens no session. A cut-off return lacks a large part, and forcing it into valid JSON
  would lose content, so a repair must not change content.

- A return sent back gets a few words on what is wrong and to rewrite it; the task, basis and form are not sent again,
  and the work is not redone. It only handed in the wrong thing, the work is done, and more words waste tokens.

- A work that changes one of the engine's checks of returns is judged, until it is installed, by the check as it was,
  and its own returns can be sent back by the very fault it mends. So it names each check it changes, with why the
  installed check would send a right return back; what only those checks find is set aside and not sent back, while the
  installed checks still run on every return; each finding set aside is judged by a review, the declaration is checked
  against the diff, and none reaches the merge unjudged. Only the project the engine is built from can declare. Such a
  work once went around the check by asking the advisor, at seven more rounds against two. Splitting the change to the
  check into an item merged and installed first costs an item, a whole graph and an installation each time, and cannot
  split a work whose change is the check itself. Running the checks in CI cannot work, since a check reads the live
  database, the worktree and the configuration. Installing new builds promptly was no answer either: no installation,
  however prompt, holds a change that is only on the work's branch.

- The checks that judge a work's declaration of the checks it changes, and that hold the reviews to answering each
  finding set aside, can never be declared themselves, and what they find is never set aside: they are what makes a
  declaration no exemption, the one judging it and the other holding the reviews to answer each finding.

- A finding set aside from the re-check's own return goes to the fix and the re-check again, since no review comes after
  the re-check to judge it. The second time the same check is set aside from it, the fix takes it to the advisor, which
  judges whether the re-check can write a return the installed check takes, and where it cannot, the architect has the
  work wait for the installation or changes the item: the re-check is given the installed prompt, so such a finding
  shows the installed check disagreeing with that prompt, which the engineer cannot mend.

- The agent's command line runs with partial output on, and each failure to parse a return keeps that return's full text
  in the round's record. A log of the first 2,048 characters does not show the real cause; with the full text the next
  error shows where it broke, and automatic repair and testing a repair need it.

- Streaming of tool inputs as they are generated is off in the agent's command line, so that the service checks each
  argument's JSON before passing it back. Bad JSON may be stopped or corrected at the source, and as nobody watches it
  written out, waiting for it affects nothing.
