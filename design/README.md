# The design tree

Read these rules as an engineer who knows the system and has common sense: judge by what each rule is for, and do
not pick at its words.

## What the tree is

The code is the result: it shows how a thing is done. The design tree holds why, where the code cannot show it. The
code shows a reason only where a comment says it, and that the code shows what an entry says to do is no ground for
leaving the entry out. The tree holds no more than is needed: what the system itself refuses, so that it would not
build, start or pass its checks, and what any agent would do unasked, is not written.

The tree is `design/`. It holds four kinds of thing:

- the constitution, `constitution.md`;
- the principles, in `principles/`, kept by what they govern: the whole system, or one area of it;
- the decisions, in `decisions/`, one file for each ability or component of the system, named for it, under the area
  it belongs to;
- the documents, in `docs/`, one file for each design that holds only as a whole.

## How it is used

An agent taking the work up in another session reads the code for how, and the tree for why. It reads the
constitution whole, and whole the principles of the whole system and of each area its work is in; then the document
of each design it works on, and the decisions of each ability or component its work changes or relies on.

Where the right solution needs a decision changed, the work changes it. An agent that finds the constitution, a
principle or a document missing or wrong, or whose right solution breaks one, says so and proposes the change.

## The four kinds

### The constitution

The constitution holds what the project is for and its lasting values. Every principle and every decision comes from
it, and it decides where two of them pull apart. An entry may rest on one of its articles by naming it. The owner
sets it and changes it.

### A principle

A principle is a rule that whatever is made later must follow, with its reason. It speaks of every thing of a kind,
those not yet made included, or of every time a kind of work is done; the kind may be wide or narrow. A rule for
everything in one part of the system, what is added to that part later included, speaks of a kind and is a principle.

It is not a principle where it speaks of one thing that is there and binds nothing made later: that is a decision.
Nor where the constitution or a document already says it: one that adds to an article or to a document says only
what is new, and names what it rests on.

Principles are the owner's and may come before the code. No decision excuses a breach of a principle.

### A decision

A decision is about one thing, an ability or a component: what holds there and why, where the code does not show the
why. It enters only if an agent that has read the code, its comments and the principles would not know the reason,
and would so change the code wrongly; which is one of three things: a way that was considered and turned down, which
leaves no trace in the code; something done on purpose that looks like a fault or an omission, which a later agent
would mend; a relation between two places that neither of them shows.

It is not a decision where its reason concerns one file of the code: that is a comment in that file, where whoever
would undo it must pass, and it is not written here as well. Nor where a principle already says it. A decision is
never ahead of the code: it says what the code does, and what is yet to be built is a document's to say.

### A document

A document is a design that holds only as a whole: its parts cannot be told one at a time, and an agent given any one
of them alone would change the system wrongly. It is complete and consistent with itself.

A document may be ahead of the code: a design is there to guide what is built after it. Where the two differ, the
document says where the system is going, and an agent builds toward it and does not change the document to match the
code.

A principle or a decision that holds only within what a document covers belongs to the document, and is not written
as an entry of its own. One that reaches beyond it is an entry, which names the document and does not repeat it. A
document goes against no principle outside it. Where it is ahead of the code, a decision may still say what the code
does, until the work that builds that part of the document rewrites or deletes it: that is what is left to build, and
no fault.

## Writing an entry

A principle and a decision are each a few lines, in one form, a principle under `### principle <name>`:

```
### decision <a-short-name>
<What holds, in one or two sentences.>
Why: <the reason, in one or two sentences; where the point is a way turned down, which way and why it lost.>
```

- An entry holds one rule: one that is about one thing and also binds every thing of a kind is two entries.
- A decision that concerns two components is written once, in the tree's file of decisions of the component whose
  code would be changed wrongly. The other component's file of decisions names it in one line that points to it.
- No identifier from the code (a function, a type, a file). The system's own terms, a field or a value of a format an
  agent meets, a setting's key, and the names of principles and decisions may be used.
- No word that dates the entry. What stands behind it (when it was settled, what was tried) is in the pull request of
  the work; the case or the number that is the reason may be a clause of the Why.
- A reason that rests on judgment, with no measurement or case behind it, says so in the Why.

## Changing the tree

A decision is added by the work that makes it true: its plan says it, its change writes it, and its review holds it to
these rules. A change that makes a decision untrue rewrites or deletes it in the same work, and an entry found not to
belong here is deleted by the work that finds it. The files of decisions hold only what the code on the branch the
works merge into does.

A principle and a decision, two decisions of one file, or a document and an entry outside it, that cannot both hold
are a fault of the tree. A file of
decisions that grows long is itself a sign: what no longer belongs here is deleted from it, and decisions in more
than one file that rest on one reason are put to the owner as one principle.
