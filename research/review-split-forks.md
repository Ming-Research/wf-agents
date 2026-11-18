# The words of the split reviews and of the better-way cell, tried in forks of W11

Whom it serves: the owner and the review of W20, which split the code review into three cells (T1 code, T2 tests and
evidence, T3 the design tree) and gave the better-way cells (R2, D2) words that hand in `better_found` or nothing.
Principle change-cells-on-what-their-forks-bear-out holds that changed words take effect once a fork from the same
moment reads them as the cell means and does what the cell means (design document `testing`, 27). This file records
those forks; it is kept while the words stand as tried here, and is deleted or rewritten when they change again.

## The question

Does each new cell's task, given in place of the old one to a fork of a real round, get the return it means?

- T1, T2, T3: a fork of W11 run 26, the one code review of W11, whose words asked eleven things in one paragraph. It
  judged neither whether new tests were red before the change, nor the tests the change deleted (1,137 lines in the
  test paths of the merged commit 388c91cb3, by `git diff --shortstat 388c91cb3^ 388c91cb3` over the test paths), nor
  four of the five decisions the implementation made itself, nor time and speed.
- R2: a fork of W11 run 4, R2's first round in W11.

## What would answer it either way

A fork whose return answers each numbered question by its number, saying what it checked and what it found, or makes it
a point, bears the words out. A return that passes any of the four things run 26 missed with a bare yes, or leaves a
question without an answer, does not, and the words change until a fork from the same moment does. For R2: a return
that hands in nothing, or a better way of a few sentences that leaves out what the plan says, bears the words out; one
that rewrites the plan does not.

## How each is run

From the root of this repository, with the engine running:

    wf-agents fork 11 26 --question research/review-split-forks/t1-code-review.md
    wf-agents fork 11 26 --question research/review-split-forks/t2-test-review.md
    wf-agents fork 11 26 --question research/review-split-forks/t3-design-tree-review.md
    wf-agents fork 11 4 --question research/review-split-forks/r2-better-way.md

Each costs about $0.65 (decision a-fork-begins-at-the-end-of-its-round). A fork answers in the form of its round's
return, which has no block of the new cells, so each question says where the new block's words go.

## What the forks answered

Not run yet: the command needs the owner's approval in the implementation's session, which the work asks for.
