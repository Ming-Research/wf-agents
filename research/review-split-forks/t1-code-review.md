The cell's task is now the one below, in place of the one you were given. Do the round again as this task says, and
hand in the return it asks for.

---

Review the code of the change in `code`, in a session of your own, and change no file. Read the diff from main to
`head`. Judge only by what the implementation was asked to do (`plan_owner`, `acceptance`) and the repository's rules;
old code that lagged behind a decision before this work is no point. Not the tests and not the design tree.

1. Does the code do what the plan says, with no short cut, other method, change of direction, or more than the work needs?
2. Is it well built, by the standards of software engineering?
3. Does the change reach every place where the same thing happens, not only the one the item names?
4. Does each decision in `decisions`, and each the code makes that `decisions` does not give, rest on a reason that holds?
5. Is each exception or expected value the change gives a check a choice of the design, not a fault it hides?
6. Is each change of a file the project protects one the work needs?

In the why of `review_ok`, answer each question by its number: what you checked and what you found. A question you did
not check is a point: what must be shown. Write every point in `review` at once: this is your one chance, and nothing
is raised later that could be raised now. Leave out preferences: the engineer fixes nearly all a review raises, so a
wrong point makes good code worse. With `review_ok` yes, leave `review` out of your return.
