# The work's head on its branch

### decision the-works-head-leaves-out-merges-of-mains-code
The work's head is the head of its branch without the merges on top of it that bring only main's code, down to the first commit on main where the branch holds no change of its own; a version of `head` is that commit, and none is made while the branch holds no change of its own and has had none. Between two versions a round is given the command that shows what the work itself changed: their diff where both hold the same main, the diff from the clean merge of the earlier with the later's main where that main is later, and the whole change against main where the earlier does not merge with it so.
Why: Main taken in, by the engine or by an agent, is nothing the review has to see, and would run it on main's code at each catch-up. A version at the start would run the review on a branch with nothing in it.

### decision a-merge-of-mains-code-is-known-by-its-content
A merge brings only main's code where one of its parents is on main and its tree is the one `git merge-tree` makes of a clean merge of its two parents, whoever made it and whatever its words: an agent's clean merge of main is none of the work's, and one that resolved a conflict, or the engine's merge amended, is. The engine knows a merge as its own by its address, its words and the main it merged only to take up after a restart what a run or the check merged before it stopped, which says only what the round is told of it.
Why: Known by its address and its words, the engine's merge of main could be amended by an agent, which keeps both, to slip a change of its own in as main's, and the check passed it unseen. The address is kept where the engine takes up its own merge, as the file of its commits says it, though its words and the main they name tell it apart already: a judgment, since nothing the review or the check sees rests on it.

### decision each-head-is-kept-under-a-reference-of-the-engines
Each new version of a work's head is kept, as it is read, under a reference of the engine's own named after its branch, `refs/wf-agents/<branch>`, which nothing takes away.
Why: What a work changed is shown after its worktree is let go, and its branch, which a person may delete, may be gone with it; git collects a commit no reference holds. A branch was turned down as the keeper, since it shows among the project's branches; a tag shows among its releases.
