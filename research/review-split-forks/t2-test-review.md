The cell's task is now the one below, in place of the one you were given. Do the round again as this task says. Your
return's form has no `test_ok` and no `test_review`: write what the task says of the why of `test_ok` in the why of
`review_ok`, and the points it says go in `test_review` in `review`.

---

Review the tests and the evidence of the change in `code`, in a session of your own, and change no file. Read the diff
from main to `head`. Judge only by what the implementation was asked to do (`plan_agent`, `acceptance`) and the
repository's rules. Not the code and not the tree: the other two reviews have them.

1. Does `evidence` show each condition of `acceptance` on the change?
2. Was each new or extended test red on the code before the change, where its kind needs it, as a run shows?
3. Of each test the change removes, skips, narrows or weakens: is what it checked still checked, or is a technical reason given?
4. Is each new test worth its time: what it catches that no other test does, and why it cannot be smaller?
5. Where cost per operation or request can change, is what it can make slower measured?
6. Does the change touch a kind that `needs_testing` names?

In the why of `test_ok`, answer each question by its number: what you checked and what you found; in the why of
`needs_testing`, the kind the change touches, or that it touches none. A question you did not check is a point: what
must be shown. Write every point in `test_review` at once: this is your one chance, and nothing is raised later that
could be raised now. Leave out preferences: the engineer fixes nearly all a review raises, so a wrong point makes good
code worse. With `test_ok` yes, leave `test_review` out of your return. Having said yes to `needs_testing`, keep it while
the part of the change that made you say so is still there.
