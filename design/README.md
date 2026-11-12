# The design tree

Read these rules as an engineer who knows the system and has common sense: judge by what each rule is for, and do
not pick at its words.

1. The design tree is `design/`: the constitution, `principles/`, and `decisions/`, which holds one file of decisions
   for each ability or component of the system, named for it, under the area it belongs to: `engine`, `flow` or
   `frontend`; and `docs/`, which holds the designs that hold only as a whole (rule 8). The tree holds what an agent taking the work up in another session needs and the code cannot tell it:
   the principles of rule 2 and the decisions of rule 3. What the code shows is not written here: what a thing does,
   its names, its numbers, the order of its steps, a property a test holds. A comment is part of what the code shows:
   a reason that concerns one place in the code and no other is a comment at that place, where whoever would undo it
   must pass, and is not written here as well.

2. A principle is a rule: a general standard that guides what is done after it. It may come from a choice made in a
   design, or belong to no design at all, as a way of working does. It is needed where, without it, something done
   elsewhere later could go against it.

   A principle enters only if it passes two tests.

   - It looks forward. A decision explains one thing that is there; a principle binds what is not there yet. Take away
     the thing an entry speaks of: a decision goes with it, and a principle still has its use.
   - It is general. It governs the whole system or a whole area of it (the engine, the flow, the page), or every time
     a kind of work is done; never one mechanism within an area. This is judged by what the entry is about, not by
     how far its rule reaches: a rule about one mechanism is about that mechanism, however many things it binds. A
     rule for what one mechanism is to become is a decision of that component where rule 3 admits it, and otherwise is
     not written here.

   Whoever proposes a principle names two places that share no code where it holds or would be broken without it, or,
   for a way of working, two occasions; the principle itself names none. A sentence of a document (rule 8) that binds
   only work on that design stays in the document: it is made a principle, in one sentence that names the document,
   only where it must bind work that never opens it.

   Principles are kept by what they govern, the whole system or one area of it. They are the owner's and may come
   before the code. Only the owner adds or changes one, or the constitution: an agent that finds one missing or wrong,
   or whose right solution breaks one, says so and the owner decides. No decision excuses a breach of a principle.

3. A decision is about one ability or component. It enters only if an agent that has read the code and the principles
   would still change the code wrongly, which is one of three things: a way that was considered and turned down, which
   leaves no trace in the code; something done on purpose that looks like a fault or an omission, which a later agent
   would mend; a relation between two places that neither of them shows. A decision that concerns two of them is
   written in the file of the one whose code would be changed wrongly, and named in the other's by a line that points
   to it.

4. A decision is added by the work that makes it true: its plan says it, its change writes it, and its review holds it
   to rule 3. A change that makes a decision untrue rewrites or deletes it in the same work, and an entry found to
   fail rule 3 is deleted by the work that finds it. The files of decisions hold only what the code on the branch the
   works merge into does. A direction for the whole system is a principle; a design for one component that the code
   does not yet follow waits in `pending-design.md`.

5. A principle and a decision are each a few lines, in one form, a principle under `### principle <name>`:

   ```
   ### decision <a-short-name>
   <What holds, in one or two sentences.>
   Why: <the reason, in one or two sentences; where the point is a way turned down, which way and why it lost.>
   ```

   No identifier from the code (a function, a type, a file, a cell's id): the system's own terms, a field or a value of
   a format an agent meets, a setting's key, and the names of principles and decisions may be used, and a Why may rest
   on a principle or an article of the constitution by naming it. No word that dates the entry ("now", "no longer", "the old engine"). A reason that rests on judgment, with no
   measurement or case behind it, says so in the Why. What stands behind an entry (when it was settled, what was tried) is not written in
   the tree: the case or the number that is the reason may be a clause of the Why, and the rest is in the pull request
   of the work.

6. An agent reads the constitution whole, and whole the principles of the whole system and of each area its work is
   in; then the document of each design it works on, and the decisions of each ability or component its work changes
   or relies on. Where the right solution needs a decision changed, the work changes it; a principle or a document
   goes to the owner (rules 2 and 8).

7. A principle and a decision, or two decisions of one file, that cannot both hold are a fault of the tree. A file of decisions that grows long is itself a sign:
   what the code now shows is deleted from it, and decisions in more than one file that rest on one reason are put to
   the owner as one principle.

8. A design that holds only as a whole is a document in `docs/`, one file for each design, named for it. It enters
   only if its parts cannot be told one at a time: an agent given any one of them alone would change the system
   wrongly. A document is complete and consistent with itself. It is the owner's and may come before the code, as a
   principle may: only the owner adds or changes one. A principle or a decision that follows from a document names it
   and does not repeat it. What the code does not yet do of it waits in `pending-design.md`.
