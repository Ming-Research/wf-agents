# The design tree

Read these rules as an engineer who knows the system and has common sense: judge by what each rule is for, and do
not pick at its words.

The code is the result: it shows how a thing is done. The design tree holds what an agent going by the code would get
wrong: what was wanted, and what holds because of it. That the code shows what an entry says to do is no ground for
leaving the entry out. Comments are not the code, and where a comment and an entry differ, the entry holds.

## What is in it

- `constitution.md`: what the project is for and its lasting values. The owner sets it, and it decides where two
  entries pull apart.
- `principles/`: the owner's rules for whatever is made later. A principle may come before the code, and no decision
  excuses a breach of one. An agent that finds one missing or wrong proposes the change to the owner.
- `decisions/`: what holds at one thing, written by the work that makes it true. A decision says only what the code
  does on the branch the works merge into: a work that makes one untrue rewrites or deletes it, and deletes one it
  finds not to belong here.

Each directory is a tree of concepts and does not follow the layout of the code. An entry is written once, in the
file of the lowest concept that holds everything it concerns; what is common to several is in the file of the concept
above them. Decisions in several files that rest on one want are one principle, to be proposed.

A design written as a whole, and what is yet to be built, is in `docs/`, outside the tree.

An agent reads the constitution whole, then both directories from the top down to each concept its work changes or
relies on, and the document in `docs/` of a design it works on.

## An entry

```
### decision <a-short-name>
So that <what is wanted>, <what holds>.
Not knowing this, an agent would <do what, what leads it there, and what would then go wrong>.
```

A principle is written the same, under `### principle <name>`. What an article or an entry above already says is
named, as `(Article 3)` or `(principle <its-name>)`, and not said again; what rests on judgment, with no case or
measurement behind it, says so.

An entry is these two lines and nothing more. CI refuses a file whose entries are not: two lines with these opening
words, at most 80 words each, no date, no path into the code, and every entry it names being in the tree.
