# Returns

What the agent of a cell hands back at the end of a round, in what form, and how the engine reads it.

### decision a-return-is-tools-or-outcome
A return is one of two kinds, never both: `tools`, which asks the engine for tools and holds nothing else, or `outcome`, which does the cell's work; one that mixes them is sent back.
- rejected: an agent proposed one return holding both what the cell dealt with and the tools it asks for; the owner rejected it, because a cell that asks for tools hands in nothing else, as the old form held: a trial cell that did both named all it ran dealt with, and nothing brought it back.
- replaced: The rebuild's `asks`, a tool with an input string or a free question to the owner, was on main; the owner changed it, because tools were settled as needing no handling of their own, and a question is to be thought out as data.
- rejected: an agent proposed keeping the name `calls` for the kind that asks for tools, so that it could hold pauses and rulings too; the owner rejected it, because those have their own ways already, and the kind holds tools alone.

### decision a-return-says-what-it-dealt-with
A return that does the cell's work names in `dealt_with` what of what ran the round it dealt with; only that is consumed, and what it leaves out runs the cell again.
- replaced: Every trigger a round was told of counting as dealt with once it ended was on main; the owner changed it, because a cell stopped in its run could not be told from one with nothing to change, and one rule then serves stopping, waiting on a call and answering first alike.

### decision one-form-for-a-line
Every round of a line is given one form of return: the engine's fields, each block the line's cells write as a field of `outputs` under its name in its own shape, and the flow's tools.
- replaced: One form for the whole flow, listing no block, each name in `outputs` holding a text or a value with its why, was on main; the owner changed it, because a block with a shape of its own, as the questions, then needed a field of its own, which every cell's form and words carried.
- rejected: an agent proposed one form with a branch for each cell's own shape; the owner rejected it, because it put about 88K tokens into the fixed beginning of every round of the architect's line, and the service takes 16 branches where that line had 27.

### decision every-block-is-written-in-outputs
Every block a cell writes, its questions and its words to the owner among them, is a field of `outputs` under its name, and one that is not among the cell's outputs sends the return back.
- replaced: The questions and the answers to the owner written through fields of the return of their own, `questions` and `reply`, was on main; the owner changed it, because no rule is to hold only with exceptions.
- rejected: an agent proposed keeping those two fields beside a form made for each line, the questions checked as `outputs` are; the owner rejected it, because that kept the exception.
- rejected: an agent proposed a tool for answering the owner within the round; the owner rejected it, because an answer is handed in like any other data, and what the cell has not dealt with brings it back.

### decision a-choice-is-handed-in-with-its-why
A block that is a choice is handed in as its value, one of its values, and a line of why in the owner's language, kept together as one version.
- rejected: an agent proposed a choice handed in as its value alone, its reason written in an optional `summary` of the round; the owner rejected it, because `summary` was there only to give a cell that writes no block somewhere to report, and no cell's task was to rest on it.

### decision a-return-holds-no-summary
A return that does the cell's work holds what it dealt with, what it writes, its observations and why it changes protected files, and no summary of the round.
- rejected: an agent proposed keeping an optional `summary`, since three cells put the reason of a yes there and the page showed it; the owner rejected it, because it was added only so that a cell writing no block had somewhere to say what it did.

### decision the-form-and-its-words-are-made-from-the-flows-files
The form of return and the words on returning are made from the flow's files, in which each field, tool and block is defined once, its shape and its words together.
- replaced: What a cell may return defined in three places kept alike by hand, the cell's file, the words of its system prompt and the form, was on main; the owner changed it, because a thing is defined in one place.
- replaced: A form written by hand, with `hands_in` and `asks`, was on main; the owner changed it, because a form made from the definitions cannot drift from them.

### decision what-the-owner-reads-is-written-in-his-language
Agents are told to write in `owner.language` what the owner reads where they are told what they write: after a cell's list of blocks where one is his, and in the system prompt for `protected_changes`.
- replaced: A rule of language in the agents' rules, English in the repository and the owner's language for him, was on main; the owner changed it, because the language bears only on what agents hand in, so it goes with what they hand in.
- replaced: The system prompt naming each field the owner reads, his answers and questions among them, was on main; the owner changed it, because once those were blocks nothing asked for them in his language, and one sentence covers every block he reads.
- rejected: an agent proposed that the engine check the language of what the owner reads and send back what is not in it; the owner rejected it, because a small model not keeping to it does not matter.

### decision a-group-is-dealt-with-by-its-name
A return names a group of blocks the cell watches by the group's name in `dealt_with`; it counts where any block of the group ran the round, and each that did is consumed.
- rejected: an agent proposed that the owner change the cell's file, or the flow, so that the cell watches no group; the owner rejected it, because the engine was to agree with the flow's definition, and a flow without groups would lose what a group is for.
- rejected: an agent proposed letting the cell go on past its limit on the page; the owner rejected it, because the fault was the engine's, to be fixed in it rather than gone around.

### decision a-return-hands-in-what-its-cell-must-every-time
A block the cell must write is required of every return that asks for no call, but one with a question its cell waits for, one of a round entered while it waits, and one that only answers the owner.
- replaced: Every return that asks for no call handing in those blocks, a question waited for among them, was on main; the owner changed it, because a cell stopped by a question then forced a value, and the work stops at the cell until the answer comes.
- rejected: the owner proposed deleting the rule, so that a cell testing the change would not give its verdict before any result; the owner rejected it, because the rule asks the blocks only of a return that makes no call, so a review that asks for the tests gives its verdict after their results.

### decision normalise-shape-only
A return that is not in the form's shape but means it, as a list written as a string or the form wrapped in one more object, is put into the shape before it is read; its content is never changed.
- replaced: A return sent back for a field written at another level of the form was on main; the owner changed it, because returns put their short fields inside `outcome`, and the engine can move them out without changing them.

### decision the-form-takes-outcome-as-an-object-alone
The form takes `outcome` only as an object, though a model at times writes a long return as a string, which the agents' program refuses; the engine then reads the last attempt from the stream.
- rejected: an agent proposed letting the form take `outcome` as a string, or changing the words agents are given; the owner rejected it, because no words asked for a string, nearly every refusal was whole JSON with one bracket too many, which the engine reads, and strings taken could grow commoner.
