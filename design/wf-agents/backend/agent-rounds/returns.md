- A return that cannot be parsed is repaired in order: the engine first closes missing brackets and moves things into
  place; failing that, the cheapest model repairs it once, and the content is checked unchanged; failing that, the agent
  hands it in again in its session; only then is the work paused and the owner told. The round is not rerun. A cheap
  model costs little and lengthens no session. A cut-off return lacks a large part, and forcing it into valid JSON would
  lose content, so a repair must not change content.

- A return sent back gets a few words on what is wrong and to rewrite it; the task, basis and form are not sent again,
  and the work is not redone. It only handed in the wrong thing, the work is done, and more words waste tokens.

- The agent's command line runs with partial output on, and each failure to parse a return keeps that return's full text
  in the round's record. A log of the first 2,048 characters does not show the real cause; with the full text the next
  error shows where it broke, and automatic repair and testing a repair need it.

- Streaming of tool inputs as they are generated is off in the agent's command line, so that the service checks each
  argument's JSON before passing it back. Bad JSON may be stopped or corrected at the source, and as nobody watches it
  written out, waiting for it affects nothing.
