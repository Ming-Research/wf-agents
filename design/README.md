# The design tree

Read these rules as an engineer who knows the system and uses common sense: follow what each rule is for, and don't
nitpick its wording.

The code is the result of every choice made in the project so far. It is lossy: it shows what was chosen, but not what
was weighed against it, what was tried, or why one approach won. The design tree is a record, kept beside the code, of
how the project got to where it is. It holds what the code cannot show, so that later decisions, the code written for
them and the project's next steps build on what was learned before. Comments are not part of the code: where a comment
and an entry disagree, the entry is right.

## What is in it

- `constitution.md`: what the project is for and its lasting values. The owner writes it, and it decides when two
  entries conflict.
- `principles/`: the owner's rules for everything built from now on.
- `decisions/`: what is true of one part of the system, written by the work that makes it true. A decision's first line
  says only what the code on main does.

Each directory is a tree of concepts. It doesn't have to follow the layout of the code. Write each entry once, in the
file of the lowest concept that covers everything the entry is about. What several concepts share goes in the file of
the concept above them. If decisions in several files come from the same need, they are one principle: propose it to
the owner.

Each file in either directory has a title, one line saying what its concept covers, and its entries, each in the form
below. Put one blank line between entries, and write each line of an entry as one line of the file. A name is lowercase
words joined by hyphens, and no two entries in the tree have the same name. To refer to another entry, write
"principle <name>" or "decision <name>".

An agent reads the whole constitution, then reads both directories from the top down to each concept its work changes
or depends on. Given the entries that bear on its work, it reads those first, and reads the directories only for the
concepts they leave out.

## Principles

A principle is a rule for everything of one kind, including things not built yet. A rule about one existing thing is a
decision. Principles belong to the owner: an agent may propose a principle or a change to one, and the owner decides. A
principle can come before the code, and no decision can justify breaking one.

```
### principle <a-short-name>
<The rule, in one or two sentences, at most 300 characters.>
Why: <the reason for it, at most 300 characters>
```

## Decisions

```
### decision <a-short-name>
<What is true now, in one sentence of at most 200 characters.>
- rejected: <who> proposed <an approach>; <who> rejected it, because <the reason given>.
- replaced: <an approach> was on main; <who> changed it, because <the reason given>.
```

The first line says what is true now. Under it come one or more lines, each at most 300 characters, each about an
approach that was considered and not taken:

- `rejected`: an approach someone proposed that was turned down.
- `replaced`: an approach that was on main, in the code or in the tree, and was changed.

A decision records a choice the owner made. These choices are the owner's:

1. an approach that goes against what a decision says;
2. two or more approaches that would each work, where each gives up something the owner cares about, such as cost,
   time, or what the owner sees or does, each looked into far enough to say why the others lost;
3. a change that no decision covers to what the owner sees or does, where more than one approach would work and the page
   has no look already for the same meaning.

The agent decides the rest itself and says why in its plan: what the item already asks, what a principle or a decision
settles, the only approach that would work, the look the page already has for the same meaning, or how to make the
code correct.

The approach taken becomes the first line, of the decision that covers it or of a new one. Every approach not taken
becomes a line under it: `replaced` if it was on main, `rejected` if it was not. This holds whichever approach the
owner takes. Something done with no other approach considered has no decision. Keeping things as they are, or an option
that would not work, is not another approach.

For case 2: an approach that is simply better, with nothing given up, is no choice of the owner's and leaves no line.
But an approach an experiment showed to be better is always the owner's choice. An experiment measures every approach
with the same measure under the same conditions, and its numbers are kept in the repository. Looking at how something
looks or behaves is not an experiment. Fixing a fault is not a choice: a test guards the fix.

The tree change follows from the owner's choice. Update every entry the change makes untrue, and delete decisions about
things that no longer exist.

In a `rejected` line, the first `<who>` is whoever proposed the approach: the owner, an agent or a reviewer agent. The
`<who>` who rejected or changed it is the owner, who makes every choice. The reason is the one given at the time the
choice was made. If it rests on data, end it with the repository file that holds the data, as in "(data: `<path>`)". A
line says only what happened when the choice was made. It names no work, item or date: the file's history keeps those.
