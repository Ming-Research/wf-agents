# Returns

What the agent of a cell hands back at the end of a round, in what form, and how the engine reads it.

### decision a-return-is-tools-or-outcome
A return is one of two kinds, never both: `tools` asks the engine for tools and holds nothing else, and `outcome` does the cell's work; the engine sends back a return that mixes them.
- rejected: an agent proposed one return that holds both what the cell dealt with and the tools it asks for; the owner rejected it, because a cell that asks for tools hands in nothing else, as in the old form: a trial cell that did both named all that ran as dealt with, and nothing ran it again.
- replaced: The rebuild's `asks`, which was either a tool with an input string or a free question to the owner, was on main; the owner changed it, because it was settled that tools need no handling of their own, and a question is to be designed as data.
- rejected: an agent proposed keeping the name `calls` for the kind that asks for tools, so that it could also hold pauses and rulings; the owner rejected it, because pauses and rulings already have their own mechanisms, and the kind holds only tools.

### decision a-return-says-what-it-dealt-with
A return that does the cell's work names in `dealt_with` which of the things that made the round run it dealt with; only those are consumed, and anything it leaves out runs the cell again.
- replaced: Counting every trigger a round was told of as dealt with once the round ended was on main; the owner changed it, because a cell stopped during its run could not be told apart from one with nothing to change, and then one rule covers stopping, waiting on a call and answering first alike.

### decision one-form-for-a-line
All rounds of a line are given one form of return: the engine's fields, the flow's tools, and each block the line's cells write, as a field of `outputs` under the block's name, in its own shape.
- replaced: One form for the whole flow, which listed no block and let each name in `outputs` hold a text or a value with its why, was on main; the owner changed it, because a block with its own shape, such as the questions, then needed its own field, which every cell's form and words carried.
- rejected: an agent proposed one form with a branch for each cell's own shape; the owner rejected it, because it put about 88K tokens into the fixed beginning of every round of the architect's line, and the service takes at most 16 branches, while that line had 27.

### decision every-block-is-written-in-outputs
Every block a cell writes, including its questions and its words to the owner, is a field of `outputs` under its name; the engine sends back a return with a block not among the cell's outputs.
- replaced: Writing the questions and the answers to the owner in their own fields of the return, `questions` and `reply`, was on main; the owner changed it, because no rule is to hold only with exceptions.
- rejected: an agent proposed keeping those two fields beside a form made for each line, with the questions checked the way `outputs` are; the owner rejected it, because that kept the exception.
- rejected: an agent proposed a tool for answering the owner during the round; the owner rejected it, because an answer is handed in like any other data, and whatever the cell has not dealt with runs the cell again.

### decision a-choice-is-handed-in-with-its-why
A block that is a choice is handed in as its value, which is one of its allowed values, together with one line of why in the owner's language, and the two are kept as one version.
- rejected: an agent proposed handing in a choice as its value alone, with its reason written in an optional `summary` of the round; the owner rejected it, because `summary` was there only to give a cell that writes no block a place to report, and no cell's task was to depend on it.

### decision a-return-holds-no-summary
A return that does the cell's work holds what it dealt with, what it writes, its observations and why it changes protected files, and no summary of the round.
- rejected: an agent proposed keeping an optional `summary`, since three cells put the reason for a yes there and the page showed it; the owner rejected it, because it was added only so that a cell writing no block had somewhere to say what it did.

### decision the-form-and-its-words-are-made-from-the-flows-files
The engine makes the form of return and the words about returning from the flow's files, which define each field, tool and block once, with its shape and its words together.
- replaced: Defining what a cell may return in three places kept alike by hand (the cell's file, the words of its system prompt and the form) was on main; the owner changed it, because a thing is defined in one place.
- replaced: A form written by hand, with `hands_in` and `asks`, was on main; the owner changed it, because a form made from the definitions cannot drift from them.

### decision what-the-owner-reads-is-written-in-his-language
Agents are told to write what the owner reads in `owner.language` where they are told what to write: after a cell's list of blocks when one is his, and in the system prompt for `protected_changes`.
- replaced: A language rule in the agents' rules (English in the repository, the owner's language for him) was on main; the owner changed it, because the language matters only for what agents hand in, so the rule goes with what they hand in.
- replaced: The system prompt naming each field the owner reads, including his answers and questions, was on main; the owner changed it, because once those were blocks nothing asked for them in his language, and one sentence covers every block he reads.
- rejected: an agent proposed that the engine check the language of what the owner reads and send back what is not written in it; the owner rejected it, because it does not matter if a small model does not keep to it.

### decision a-group-is-dealt-with-by-its-name
A return names a group of blocks the cell watches in `dealt_with` by the group's name; the name counts if any block of the group made the round run, and each block that did is consumed.
- rejected: an agent proposed that the owner change the cell's file, or the flow, so that the cell watches no group; the owner rejected it, because the engine was to match the flow's definition, and a flow without groups would lose what groups are for.
- rejected: an agent proposed letting the cell go on past its limit from the page; the owner rejected it, because the fault was in the engine and was to be fixed there, not worked around.

### decision a-return-hands-in-what-its-cell-must-every-time
A return that asks for no call must hand in the blocks its cell must write, unless it has a question its cell waits for, comes from a round entered while the cell waits, or only answers the owner.
- replaced: Requiring those blocks of every return that asks for no call, including one with a question its cell waits for, was on main; the owner changed it, because a cell stopped by a question then had to force a value, and the work stops at the cell until the answer comes.
- rejected: the owner proposed deleting the rule, so that a cell testing the change would not give its verdict before any result; the owner rejected it, because the rule asks for the blocks only in a return that makes no call, so a review that asks for the tests gives its verdict after the results.

### decision normalise-shape-only
If a return is not in the form's shape but means it, like a list written as a string or the form wrapped in one more object, the engine puts it into shape before reading it; its content never changes.
- replaced: Sending back a return that wrote a field at another level of the form was on main; the owner changed it, because returns put their short fields inside `outcome`, and the engine can move them out without changing them.

### decision the-form-takes-outcome-as-an-object-alone
The form takes `outcome` only as an object; a model sometimes writes a long return as a string, which the agents' program refuses, and the engine then reads the last attempt from the stream.
- rejected: an agent proposed letting the form take `outcome` as a string, or changing the words agents are given; the owner rejected it, because no words asked for a string, nearly every refusal was whole JSON with one bracket too many, which the engine reads, and strings taken could grow commoner.
