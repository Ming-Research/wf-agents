### decision normalise-shape-only
A return that is not in the form's shape but means the form is put into the shape before it is read; its content is never changed, and a field the form requires of an item is never made up.
Why: Reading slips strictly paused works for nothing the agent meant to say (principle converging-fixes). The raw return is kept whole beside the reading (principle the-past-as-it-ran).

### decision left-out-list-is-empty-list
A list the return leaves out or writes as null is an empty list, and so are the blocks it writes, so a return of `{}` does the cell's work and deals with nothing; a field of an item is a statement of its own and is never taken as empty.
Why: Leaving out a list says what an empty list says, and what such a return lacks is then sent back as any return's is, saying what. A field of an item taken as absent would be made up. This looks inconsistent and is deliberate (principle every-field-has-a-reader).

### decision closing-brackets-after-a-string-form
Where the whole form, or the one object that wraps it, comes as a string holding JSON, the object is taken when only whitespace and closing brackets follow it.
Why: Models write the form with a closing bracket too many, which makes the string unparseable though nothing is missing; any other text after it may be content, so it is not taken. Repairing text that does not parse at all is a different step, in the handling of failed rounds, and is not this one.

### decision a-return-given-up-on-is-read-from-the-stream
Where the command line gives up on a return, having refused every attempt at it, its last attempt, rebuilt from the pieces of it that the program's stream carries, is read as any return is.
Why: The command line checks a return against the form more strictly than the engine reads one (decision normalise-shape-only), so it gives up on returns the engine takes, such as a list written as a string, with the agent's work done. Its result holds no text of an attempt it refused, and an attempt cut off, which is no JSON, is in the stream only in pieces: that is why each message comes in pieces and those of the attempts are kept with the round.

### decision one-form-for-a-flow
Every round of every cell of a flow is given one form of return, made of the engine's fields and the tools the flow names; the blocks a cell writes go in `outputs` under their names, which the form does not list, and the engine checks each return against the cell it was entered for once it is read.
Why: With a form of its own for each cell, or one that listed each cell's blocks, going from one cell to the next within a session would change the bytes every call is given and lose the cache (principle cache-order). The program can then hold a return only to what every cell has; what one cell may write the engine checks.

### decision what-only-the-reading-sees-is-kept-with-the-return
What is wrong with a return that only its reading against the cell can see is found as it is read and kept with what came back: a return of both kinds, whatever each holds; a name in `dealt_with` that is none of the cell's triggers; a tool the cell may not ask for; a name in `meaning_unchanged` the return does not write; a block in `outputs` the cell does not write there or in a shape no block has, which is then said wrong for its shape alone and not as missing too; and a question that recommends none of its options, has the name of the question the engine itself asks, or is asked by a cell no answer runs. The run's check names these first, with everything else wrong with the return, in one send-back.
Why: The run's check reads the records alone, so that a run is decided again from them the same (principle state-from-records), and a name the cell does not have cannot be put into their terms, which are blocks. Sending such a return back from the reading, as a return that could not be read, was turned down: a return with something wrong of both kinds would be sent back twice (principle report-all-at-once).

### decision a-return-hands-in-what-its-cell-must-every-time
A block the cell must write is required of every return that asks for no call, one that deals with only part of what ran it included; only a return with a question of its agent's that waits for the owner's answer, and a round entered for the owner while its cell waits for the results of calls, may leave it out.
Why: Handed in as it stands, with its value unchanged, such a block runs no cell, so asking it of a partial return costs a line, and the words on returning tell the agent to hand it in always; a question that waits stops the work at the cell, and its answer may change the block. Asking it only of a return that dealt with everything that ran it was turned down for those words; a judgment, with no case behind it.

### decision what-the-engine-asks-is-beside-the-return
What the engine itself asks the owner as a round ends, such as whether a change of a protected path stays, comes with the round's end beside the agent's return and is recorded as a question of the round's cell as that end is taken, whether or not the return is: it holds the return to nothing and lets it off nothing.
Why: It is about the work's branch as the round left it, which a return sent back does not change; and it is no choice of the agent's to stop for the owner, so letting the return off its must-writes for it would take a return the agent did not finish.
