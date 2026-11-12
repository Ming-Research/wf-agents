# The design tree

Read these rules as an engineer who knows the system and has common sense: judge by what each rule is for, and do
not pick at its words.

## What the tree is

The code is the result: it shows how a thing is done. The design tree holds why, where the code cannot show it. An
agent taking the work up in another session reads the code for how, and the tree for why.

The tree is `design/`. It holds four kinds of thing:

- the constitution, `constitution.md`;
- the principles, in `principles/`, kept by what they govern: the whole system, or one area of it;
- the decisions, in `decisions/`, one file for each ability or component of the system, named for it, under the area
  it belongs to;
- the documents, in `docs/`, one file for each design that holds only as a whole.

## How it is used

An agent reads the constitution whole, and whole the principles of the whole system and of each area its work is in;
then the document of each design it works on, and the decisions of each ability or component its work changes or
relies on.

Where the right solution needs a decision changed, the work changes it. The constitution, a principle or a document
is the owner's: an agent that finds one missing or wrong, or whose right solution breaks one, says so, and the owner
decides.

## What needs an entry

An entry is written only where all of this holds.

- It gives a reason, and the code does not show that reason. What keeps an entry out is that the code shows its
  reason, never that the code shows what it says to do; and the code shows a reason only where a comment says it.
- The reason is not one place's. A reason that concerns one file of the code is a comment in that file, where whoever
  would undo it must pass, and is not written here as well.
- Going against it would pass unnoticed. What the system itself refuses, so that it would not build, start or pass
  its checks, and what any agent would do unasked, need no entry.
- It is not said already. What the constitution, these rules or a document says is not written again; an entry that
  adds to one of them names what it rests on.

## The four kinds

### The constitution

The constitution holds what the project is for and its lasting values. Every principle and every decision comes from
it, and it decides where two of them pull apart. An entry may rest on one of its articles by naming it. Only the owner
sets it and changes it.

### A principle

A principle is a rule that whatever is made later must follow. It speaks of every thing of a kind, those not yet made
included, or of every time a kind of work is done; the kind may be wide or narrow. A rule for everything in one part
of the system, what is added to that part later included, speaks of a kind and is a principle. What speaks of one
thing that is there, and binds nothing made later, is a decision of that thing.

Principles are the owner's and may come before the code. Only the owner adds or changes one. No decision excuses a
breach of a principle.

### A decision

A decision is about one thing, an ability or a component: what holds there and why, where the code does not show the
why. It enters only if an agent that has read the code, its comments and the principles would not know the reason,
and would so change the code wrongly; which is one of three things: a way that was considered and turned down, which
leaves no trace in the code; something done on purpose that looks like a fault or an omission, which a later agent
would mend; a relation between two places that neither of them shows.

A decision that concerns two components is written in the file of the one whose code would be changed wrongly, and
named in the other's by a line that points to it.

### A document

A document is a design that holds only as a whole: its parts cannot be told one at a time, and an agent given any one
of them alone would change the system wrongly. It is complete and consistent with itself. It is the owner's and may
come before the code, as a principle may: only the owner adds or changes one.

A sentence of a document that binds only work on that design stays in the document; it is made a principle, in one
sentence that names the document, only where it must bind work that never opens it. A principle or a decision that
follows from a document names it and does not repeat it.

## Writing an entry

A principle and a decision are each a few lines, in one form, a principle under `### principle <name>`:

```
### decision <a-short-name>
<What holds, in one or two sentences.>
Why: <the reason, in one or two sentences; where the point is a way turned down, which way and why it lost.>
```

- An entry holds one rule: one that is about one thing and also binds every thing of a kind is two entries.
- No identifier from the code (a function, a type, a file). The system's own terms, a field or a value of a format an
  agent meets, a setting's key, and the names of principles and decisions may be used.
- No word that dates the entry. What stands behind it (when it was settled, what was tried) is in the pull request of
  the work; the case or the number that is the reason may be a clause of the Why.
- A reason that rests on judgment, with no measurement or case behind it, says so in the Why.

## Changing the tree

A decision is added by the work that makes it true: its plan says it, its change writes it, and its review holds it to
these rules. A change that makes a decision untrue rewrites or deletes it in the same work, and an entry found not to
need writing is deleted by the work that finds it. The files of decisions hold only what the code on the branch the
works merge into does. A design for one component that the code does not yet follow waits in `pending-design.md`, and
so does what the code does not yet do of a document.

A principle and a decision, or two decisions of one file, that cannot both hold are a fault of the tree. A file of
decisions that grows long is itself a sign: what no longer needs an entry is deleted from it, and decisions in more
than one file that rest on one reason are put to the owner as one principle.
