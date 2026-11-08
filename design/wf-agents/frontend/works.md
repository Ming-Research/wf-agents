- Architect and works share one Works tab, with no permanent side list of groups. A permanent side list takes too much
  screen, and charts need the screen. A tab of its own for the architect was not needed, since Works is split by project
  anyway.

- The works view is one table, a group for each project, those ended hidden by the state buttons as in the other lists
  and not by a fold or a table of their own; a work picked takes the whole window under the bar with its chart, and
  going back returns to the table as it was. The owner reads every project's works at once and the chart over the whole
  window. An area of its own that scrolls was rejected.

- The work page and the architect's page have one logic for what they share. A split screen once existed on the work
  page and not on the architect's.

- Project names and the buttons at their right stand outside the table as each project's title, and the name folds it.
  They are not part of the table, and there will be several projects.

- Where the window is narrower than the table, the table scrolls sideways and each project's head stays at the left, as
  wide as the window, so that its buttons stay in sight.

- A work's number links to its conversation, followed by a conversation icon; the name goes to the item and the rest of
  the row to the work page. A column for the conversation takes too much screen, the work column had no link, and this
  way it is plain where each leads.

- Every cell of the works table stays on one line, cut where its column ends and whole on hovering, and the columns are
  one across every group, their widths dragged and kept: the groups are read as one table, and a row that wraps makes it
  slow to scan. A state's reason is not prefixed with the state its badge shows, and a work's item is not followed by
  its project, which the group says: each would say it twice.

- A work on an older flow version, or moved to the one the engine runs, says so on its line, since the owner wants to
  tell a work an upgrade moved from one that started on the version it runs.

- At and Stage merge into one horizontal bar: a cell for each stage's letter, green when passed, the cells narrow, with
  no text beside them. The owner wants progress from this column, and wider cells only take screen.

- In the stage bar the stage where a cancelled work ends has no cell, being no step a work goes through, and the flow
  version is a column of its number alone, so that versions are compared by eye.

- The Last activity and PR columns go, and Cost stays. The owner has no use for those two.

- The screen saved goes to a time bar for each work, the same as Usage's where time went. The owner wants to see a
  session's state at a glance.

- Each row's time bar holds the work's span cut into equal cells, saying on hovering how long models, tools and the
  engine worked and how long nothing ran, since a list of every project's works cannot carry every round of every work.

- A work's run time is how long its agents and tools ran, waits left out: the owner judges from it whether a work ran
  too long, which its start time does not tell.

- The marks in the rounds column for sent back and not finished have a text on hover, and a click lists only those
  rounds. Without the text the owner cannot read them.

- A work's Start, Pause, Step, the choice of a new session and Start over are small icon buttons drawn alike, the page's
  own icons where the icon set has none, each saying on hovering what it does or why it cannot now: the five are one set
  (Article 8).

- The work page has no drop-down to other works, which is of no use.

- The work page shows the chart and the record of rounds with each round's input and output, and each round maps to its
  cell on the chart, jumping there when clicked. The owner diagnoses problems by these.

- How a work goes on is said once, on the line over its chart, as its row in the list says it. A yellow box over a
  work's rounds said again what that line says.

- A work's panel is tabs: what would need scrolling to the bottom is a click away. What the owner answers and chooses
  stays with the item page, not here, since the work page is for rounds and the chart.

- The work page cannot send messages; what the engineer and helpers such as advisors and reviewers say to each other is
  in Helpers, not in the group, split by who talks with whom and shown both ways with context. One side's words alone
  have no context and look odd.

- In Helpers each bubble's body is a paragraph, that round's account of progress, with the full JSON below in small
  type, folded by default. Full JSON by default is unfriendly to the owner.

- In a helper's thread a return sent back or a run that failed is said to no one: Helpers holds what the engineer and
  the helpers say to each other (frontend/works.md, "The work page cannot send messages"), and neither of them says it.

- The work page does not list recorded values. They are in every opened cell, and mean nothing to the owner.

- On the timeline each gap shows its own length on hover. One number for two gaps of clearly different lengths says
  nothing of either.

- A work's panel has a Usage tab drawn from its recorded rounds alone: its cost, time, rounds, tokens and cache share;
  where time went as a bar and a timeline; each model at the level its rounds recorded, never as the configuration reads
  when looked at; cost by kind of token and by cell; each session with why it started and its context round by round;
  the keep-warm pings and their cost; how long each tool ran. A stretch with no round is one block. The owner wants to
  see what a work spent, on which models and where.
