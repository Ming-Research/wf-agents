### decision normalise-shape-only
A return that is not in the form's shape but means the form is put into the shape before it is read; its content is never changed, and a field the form requires is never made up. `matters` left out fails the return; it is not taken as a hand-in that does not matter.
Why: Reading slips strictly paused works for nothing the agent meant to say (principle converging-fixes). A default of "did not matter" was once what a left-out `matters` meant, and it silently kept the cells that read the block from running again (principle nothing-passes-for-normal). The raw return is kept whole beside the reading (principle the-past-as-it-ran).

### decision left-out-list-is-empty-list
A list the return leaves out or writes as null is an empty list, so a return of `{}` hands in nothing; a field of an item is a statement of its own and is never taken as empty.
Why: Leaving out a list says what an empty list says, and the form lets a round hand in or ask for nothing. A field of an item taken as absent would be made up. This looks inconsistent and is deliberate (principle every-field-has-a-reader).

### decision closing-brackets-after-a-string-form
Where the whole form, or the one object that wraps it, comes as a string holding JSON, the object is taken when only whitespace and closing brackets follow it.
Why: Models write the form with a closing bracket too many, which makes the string unparseable though nothing is missing; any other text after it may be content, so it is not taken. Repairing text that does not parse at all is a different step, in the handling of failed rounds, and is not this one.

### decision one-form-for-every-return
Every round of every cell hands in in one form, which is given to the agents' program with what all rounds share.
Why: With a form of its own for each cell, going from one cell to the next within a session would lose the cache (principle cache-order).
