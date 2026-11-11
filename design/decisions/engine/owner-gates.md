### decision owner-paths-wait-for-the-owner
A work whose change touches a path the installation names for the owner (the flow's definition, a specification) merges only once the owner said yes to what it changes there, whatever anyone's confidence; the check before the merge finds the files touched and lists them, and no agent asks. Which paths is the installation's to say, and the flow names none.
Why: A rule kept only in the project's guide, by the engineer and the review, was turned down: when both miss it, the change merges, as a change to a view the flow reads once merged unasked. A list in the flow would make one project's paths part of a definition every project shares (Article 1).

### decision the-yes-is-to-the-files-as-they-are
The owner's yes holds for the files under the owner's paths as they were listed: the check writes the list again only when a file in it, or its content, changes, so a later commit that leaves them as they were keeps the yes, and one that changes them again is asked again; the list names each file with its content, not only its path.
Why: Asking again at every commit would have the owner approve the same change after each fix of something else; a yes kept by path alone would let a change made after it merge unseen.

### decision questions-hold-only-the-merge
A work that put questions to the owner goes on researching, planning and implementing on its recommended answers, and only its merge waits until the owner answered them or said the recommendations stand.
Why: The owner's answer takes hours, and most recommendations stand; waiting for it before the work went on would hold every work with a question for that long. An answer other than the recommendation sends the work back like any opinion, which costs only when the owner disagrees.

### decision a-confirmation-is-read-by-no-cell
The owner's yes is a choice only people write, which no cell reads or writes and only the merge's precondition reads, and it counts only when written after what the block it confirms holds last changed, whatever the block's writer says of the change; asking again needs no one to take the yes back.
Why: Having the check write a "not yet" into the owner's block, for the owner to turn into a yes, was turned down: a run that may write a block is made stale by a write of it, so the owner's yes given while the check waited for the CI would abandon the check; and a block written by both a cell and the owner would need a rule for whose write counts. Counting from the block's meaning was turned down too: the writer's own word decides it, so a question added with the word that the change does not matter would merge unseen, and the same questions said to matter would ask the owner again for nothing.
