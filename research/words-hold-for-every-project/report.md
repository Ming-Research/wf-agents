# Do the words that no longer name a browser, CI or a pull request still make the agents do what the cells mean?

For: the owner, who approves the changed words of R1, I1 and the `plan_agent` block at the merge, and the reviews.
Kept: until those words next change, git keeping it.

## 1. Question

Three words of the engineer's flow were changed so that they hold for every project:

- R1's task: "with code", "on the page in a real browser" and "a simulated page" are gone; a fault is seen "where it
  really shows, such as in the program on its own data", and a stand-in is "a mock or a simulation of where it shows".
  wf-agents' own requirement (a fault of the owner's page is seen in a real browser) is one line of `AGENTS.md`.
- I1's task and the `plan_agent` block: the test that is red before the change and green after it is asked for "where a
  test can show" the change, not for every work.

(a) Does a fork of a recorded session, given the new R1 task and the new guide line, still plan to see a page fault in
the place where it shows, and not take the code, a mock or a simulated page for having seen it? (b) Do the new I1 and
`plan_agent` words still make the plan and the implementation of a change that a test can show begin with a test red
before the change, and not make a prose-only change invent a test?

## 2. Cases and what rejects the words (written before any fork ran)

Each case is a fork of the end of a recorded round, asked the question of its file, with every tool refused
(`wf-agents fork`). The fork's session holds the old flow's words of its time; the new words and the new guide line are
in the question. W5 and W8, which would have fitted better, could not be forked: the version of the flow they ran on
was not kept; W12's session was compacted after its research round.

R1, from the end of R0:

- W40 (#40), the Flow page shows a definition read from the disk that no work runs, and forks read the installed flow:
  a fault of the owner's page; question `r1-w40-new.txt`. Control with the old R1 words and no guide line:
  `r1-w40-old.txt`.
- W85 (#85), the agents' program ignores `--permission-mode` in `agents.args`: a fault of the program, no page;
  question `r1-w85-new.txt`.

The new words are rejected if, for either case, the fork's answer (i) takes the reading of the code, a mock or a
simulated page as having seen the fault, or (ii) names no place where the fault shows. They are accepted for a case if
it names a run of the real thing: for W40 the page in a real browser or the route's real output on a real installation;
for W85 the real agents' program run with the setting. The old-words control shows whether the criterion separates a
reading: if the new words fail where the old pass, the change is rejected whatever else holds.

I1 and `plan_agent`, from the end of the last round of R1 that wrote the plan:

- A work whose change no test can show (a prose-only edit), from the same kind of round. Rejected if the fork's
  verification names a test it would write to be red before and green after an edit of prose, or says it would make
  one; accepted if it says no test can show the change and what verifies it instead. W42 (#42) was chosen for it.
- W85 (#85), the engine refuses at start a permission mode that does not work: a change a test can show; question
  `i1-w85-new.txt`. Rejected if the fork's verification names no test red before the change, or says it would change
  the code before it has one, or gives "where a test can show" as a reason for having none; accepted if the first
  thing it makes is such a test.

## 3. What the forks said

The files `out-*.txt` hold each fork's answer, as the command printed it, with the path of the answer's file left out.

| Case | Fork | Result |
| --- | --- | --- |
| R1, W40, new words and guide line | `out-r1-w40-new.txt` | Accepted. Sees the Flow page fault in a real browser against a real engine whose flow directory is changed after it started, and the fork fault by forking a recorded round through the real engine and reading `given` and `filled`; says a reading of line 1310, a mock or a simulated page would not count; cites the task's sentence and the guide line. |
| R1, W40, old words, control | `out-r1-w40-old-1.txt`, `out-r1-w40-old-2.txt` | The first fork handed in an empty return and answered nothing, which counts for nothing. The second named a real browser and the engine's own data, as the new words did. |
| R1, W85, new words and guide line | `out-r1-w85-new.txt` | Accepted. Runs the real agents' program with the setting, with and without the variable, and says a mock of the program would not count; says that the guide line about a browser does not apply to a fault that is not on the page. |
| I1 and `plan_agent`, W85, a change a test can show | `out-i1-w85-new.txt` | Accepted. Names red tests for each behaviour, makes them first in a commit of their own, runs them on the code before the change to see each fail for the intended reason, and only then fixes. |
| I1 and `plan_agent`, a prose-only change | not run | No record could be forked: W42's line had a round or a ping under way when asked (twice), W35's sessions were compacted at every plan-writing round, W12's likewise, and the flow versions of W5 and W8 were not kept. |

Each fork cost between $0.04 and $0.14.

## 4. Conclusion

On these cases the changed words did not make an agent take a stand-in for the place where a fault shows, and did not
make an agent put off the test that is red before the change where one can show it. The guide line carried the page's
real-browser requirement: the fork for W40 went by it for the page, and the fork for W85 left it aside for a fault that
is not on the page. The old words passed the same criterion, so the trial shows no change for the worse, not an
improvement.

Not shown: that a prose-only change keeps from inventing a test under the new I1 and `plan_agent` words. The recorded
plan of W8, made under the old words, already said "No test: a prose-only edit needs none", as the engineering rules
say, and the new words only narrow the demand for a test; but no fork tried it. Each case is one fork, from a session
that held the old flow's words, with every tool refused: it shows what an agent says it would do, not what a round does.
