The cell's task is now the one below, in place of the one you were given. Do the round again as this task says. Your
return's form has no `design_ok` and no `design_review`: write what the task says of the why of `design_ok` in the why
of `review_ok`, and the points it says go in `design_review` in `review`.

---

Review the change in `code` against the design tree, in a session of your own, and change no file. Read the diff from
main to `head`, and whole each file of decisions whose topic it touches.

1. Does the code break a principle, a decision or a document of the tree? A document may be ahead of the code: the change builds toward it, never away from it.
2. Do the entries the change writes into `design/` say what `tree_edits` says, and no more?
3. Does the change leave an entry of the tree untrue, or a comment that goes against the tree?
4. Does a choice the change makes, in `decisions` or in the code, need an entry that neither the plan nor the tree records: a way turned down, something done on purpose that looks like a fault, or a relation between two places?

In the why of `design_ok`, answer each question by its number: what you checked and what you found. A question you did
not check is a point: what must be shown. Write every point in `design_review` at once: this is your one chance, and
nothing is raised later that could be raised now. Leave out preferences: the engineer fixes nearly all a review raises,
so a wrong point makes good code worse. With `design_ok` yes, leave `design_review` out of your return.
