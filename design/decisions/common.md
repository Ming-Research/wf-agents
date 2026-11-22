# Common

What is the same in the engine, on the owner's page and in the command line.

### decision a-version-is-numbered-within-its-datum
A version is named, shown and accepted by its number among its datum's versions, counted from one in the order they were written; the position of the entry that wrote it stays inside the engine.
- replaced: Naming and showing a version by the position of the entry that wrote it, which counts every event of the work, was on main; the owner changed it, because "version 42" of a datum written once, or "from 27 to 49", reads as if the datum changed many times and most changes were missed.

### decision the-part-that-carries-out-the-flow-is-the-engine
The part of the program that carries out the flow is called the engine, and nothing is called the board: each place names the records or the owner's page it means.
- rejected: an agent proposed calling that part wf-agents itself, or giving it a name of its own such as musher; the owner rejected it, because wf-agents does not say which part is meant, and the policy already called it the engine.
- rejected: an agent proposed a new name for what was the board; the owner rejected it, because every reader would first have to learn which thing a new name means.

### decision interface-words
The command line's output and the page interface's replies keep their words in their own code, and write a work's number as `Wn` and an item's as `#n` by the one writer the engine uses.
- rejected: an agent proposed moving the page interface's error words into the flow's files, the command line's output staying in its code; the owner rejected it, because those words typeset records, not the engine's behaviour, and the page's words already stand in one catalogue.
- rejected: an agent proposed moving the interface's and the command line's words into the flow's files, the engine handing the command line its words; the owner rejected it, because the command line would need a way to ask the engine for words, and most of its hundreds of keys would be used once.
