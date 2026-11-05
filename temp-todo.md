# Temporary to-do: what follows the design tree

The design tree landed in #96 (`design/`, walked from `design/wf-agents.md`). What follows from it is below. None of it has
been handed to the architect yet. A change to the flow, or to a cell's prompt, input or output, waits for the owner's
approval when it comes, with its size in characters.

## 1. Retire `decisions/`

Three of its four decisions are now in the tree, so they are written in two places: a work that changes the engine's checks,
commands the engine runs for an agent, and checking after a change takes effect. The fourth, that wf-agents lists no lock file
for `lock_update`, stays with #284.

`decisions/` is still read by a cell input, the tool `decision_history` and some checks (`crates/config/src/decisions.rs`,
`crates/engine/src/tools.rs`, `crates/engine/src/checks.rs`, `crates/engine/src/open_questions.rs`, `flow/flow.json`). Retiring
it is a flow change, and the architect brings it to the owner with its sizes.

## 2. Decide what becomes of `client/ui-spec.md`

It is 272K characters, mostly written by agents, and three tests read it by path (`ui-check.test.ts`, `roles.test.ts`,
`diagram.test.tsx`). It differs from the tree, for example on two graphs side by side and on the Anomalies part. `AGENTS.md`
says the tree holds where they differ, which is only for the time being and goes once this is decided.

## 3. Bring the code up to the tree

Where the tree was ahead of the code at b8dc9f9c. Look again before acting: the code may have moved.

### To ask the owner about first

- The cell that handles an interjection (A4) answers one of five named ways, against one yes/no question for each fork: split
  it, or name it an exception.
- A cell's definition has no fields for its problem, how it solves it, why by this tool and at what cost. Where they go is open.
- Test cells run T2, T3, T1, T6, T4, and numbers have gaps, against numbers following the order of running. Renumbering is a flow
  change.
- The paths that wait for the owner cover only `flow/flow.json`. Whether they should also cover the views the flow reads and the
  compiler is open: #313 changed such a view and merged without being asked.
- Tool or check failures reach the architect, outside its list of duties. Where they should go is open.
- The item page's summaries of problem and design need new outputs from two cells.

### Flow and rules

- The design's "why" field demands what it was compared with, against comparing only where another direction really exists.
- No review prompt mentions the effect on performance.
- First research does not ask how to prevent a recurrence, and the prompts do not tell a wide change from a local one, while one
  rule demands reading first for everything.
- No prompt names changing the design on one's own; that a design only partly done is no problem is not said, and a review may
  report it. Nothing covers migrations or work in steps.
- The architect's second look has no way to be disputed.
- Several ways back with the owner's answer have no note.
- The decision form has a letter-for-letter quote of the owner, as has a branch of V2, against Article 9.
- The field for flow feedback and the configuration's comment still say the architect reads it, and the architect's rules ask it
  to write it.
- The rules still say to run heavy work through CI, how to act on a refusal and not to poll, which permissions and the flow
  decide.
- The rules on writing items run to about 1.3K words; they do not ask for a plain first paragraph and say no account of how
  things stand; and no rule says that what another need will run into is not recorded.
- The plan field's description still says the owner's instructions in force.
- The action that queues a policy check keeps its old wording, and the one on an urgent report promises it is given at the top of
  the next round, which nothing does.
- The note on the cell where the architect asks the owner says a new fork of the base, against joining the running fork.
- The compiler demands line colours unique across the whole flow, against only within a graph.

### Engine

- The base is rebuilt only at 40K characters of what was added, against under 10K for a forked round.
- Small fixes take a median of 26 minutes, against minutes.
- The question for approval links the pull request's files and not the flow page, nothing writes the flow snapshot now, and a flow
  changed after approval is not asked again.
- The numbers the architect is given say no test count and no CI time, and nothing watches compile or test time adding up.
- Similar items only match paths changed by running works, though the input's description promises closeness of title or problem,
  and nothing looks before an item is created.
- The full text of a return is kept only when the command line gave it up; attempts the agent corrected in the same round keep only
  why.
- The table of flow versions is not hidden from agents' queries.
- The comments on `architect_matters` and flow versions in `db/schema.sql`, and the session chooser's header, describe what no longer
  holds.

### Page

- The flow page draws both graphs side by side, with their frames and names, against one graph at a time.
- The Works list: the project's name is in the table with no folding, there is a Conversation column, At and Stage are text, Last
  activity and PR remain, and there is no time bar.
- The item page has problem, design and acceptance open, against summaries first and the rest folded.
- The System page keeps the Anomalies part, and no curve has a watch.
- The range of time has 1d, 7d, 30d and all, default 7d, only on Usage, against one day, six hours and one hour for what has no
  window of rows.
- The work page keeps a decision cards tab, and `README.md` still speaks of cards (#203).
- wf-agents' own conversation has grey engine lines and a fault panel, and pauses and faults have a pinned line but no bubble.
- A note's earlier versions are full texts, with no difference shown.
- A round panel's In does not start from the task, and a cell panel may wrap itself in tabs.
- A framed text still gets a name and share notes from the page.
- The legend does not explain thick borders and bold titles.
- The engine's texts write the word work and not Wn, and `README.md` and one line of the page still say board.
- The cost view still says stopped by the architect, and the settings page keeps a line with its name and commit.
- Logging in still takes a command in a terminal, and some settings only a configuration file.

### May be deliberate: to the architect unchanged

- A circle pointing at another cell takes that cell's colour.
- The item, observation and settings pages have a maximum width.
- A matter handled with nothing changed leaves the conversation.

## Delete this file

This file is temporary. When everything above is done, it must be deleted too.
