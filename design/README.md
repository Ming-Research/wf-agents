# The design tree

Read these rules as an engineer who knows the system and has common sense: judge by what each rule is for, and do
not pick at its words.

The code is what every choice made in the project so far has come to, as it stands now. It is lossy: it keeps what
was chosen, and loses what was weighed against it, what was tried, and why one way won. The design tree is the record
kept beside the code of how the project came to be as it is. It holds what the code does not, so that each later
decision, the code built for it and the project's next step are made with what was learned before. Comments are not
the code, and where a comment and an entry differ, the entry holds.

## What is in it

- `constitution.md`: what the project is for and its lasting values. The owner sets it, and it decides where two
  entries pull apart.
- `principles/`: the owner's rules for whatever is made later.
- `decisions/`: what holds at one thing, written by the work that makes it true. A decision's first line says only
  what the code does on main.

Each directory is a tree of concepts, which need not follow the layout of the code. An entry is written once, in the
file of the lowest concept that holds everything it concerns; what is common to several is in the file of the concept
above them. Decisions in several files that rest on one want are one principle, to be proposed.

A file of either directory holds its title, one line saying what its concept covers, and its entries, each in its form
below, with one blank line between two entries and each line of an entry one line of the file. A name is lowercase
words joined by hyphens, and no two entries of the tree share one. An entry names another as "principle <name>" or
"decision <name>".

An agent reads the constitution whole, then both directories from the top down to each concept its work changes or
relies on.

## A principle

A principle is a rule for every thing of a kind, those not yet made included; a rule for one thing that is there is
a decision. Principles are the owner's: an agent may propose one, or a change to one, and the owner decides. A
principle may come before the code, and no decision excuses a breach of one.

```
### principle <a-short-name>
<The rule, in one or two sentences, at most 300 characters.>
Why: <its reason, at most 300 characters>
```

## A decision

```
### decision <a-short-name>
<What holds now, in one sentence of at most 200 characters.>
- rejected: <who> proposed <a way>; <who> rejected it, because <the reason given>.
- replaced: <a way> stood on main; <who> changed it, because <the reason given>.
```

The first line says what holds now. Under it, one line or more, each at most 300 characters, tells a way weighed
against it and not taken:

- `rejected`: a way put forward and turned down.
- `replaced`: a way that stood on main, in the code or in the tree, and was changed.

A decision comes only from a choice put to the owner, and a choice is put wherever one of these is settled:

1. a way that goes against what a decision says;
2. two or more ways that would each do, each researched in earnest or tried in an experiment, far enough to say why
   each one not taken was not;
3. a change of an important mechanism of the code that no decision holds.

The way taken is what holds now: the first line of the decision that holds it, or of a new one. Each way not taken
becomes a line under it, `replaced` where it stood on main and `rejected` where it did not, whichever way the owner
took. What was done with no other way weighed has no decision.

In the second, a way plainly better, with nothing given up for it, is taken without a choice and leaves no line; but
a way an experiment showed better is always put. An experiment measures each way by the same measure, under the same
conditions, and keeps the numbers in the repository; a look at how a way shows or behaves is none. A fault mended is
no choice: a test keeps it.

A choice is put with the change of the tree it brings, in substance, and the tree takes the change as the answer
settles it, rewriting with it each entry the change makes untrue and deleting each decision whose thing is gone.

In a `rejected` line, the first `<who>` is the one who put the way forward: the owner, an agent or a reviewer agent. The
`<who>` who rejected or changed a way is the one whose word settled it: the owner who answered. The reason is the one
given at the time, with the choice or in the answer; where it rests on data, it ends with the file of the repository
that keeps the data, as in "(data: `<path>`)". A line tells only what happened, as the choice was put and answered; it
names no work, item or date, which the history of the file keeps.
