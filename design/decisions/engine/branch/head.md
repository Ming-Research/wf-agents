# The work's head on its branch

Which commit of its branch the work's head is, and where its versions are kept.

### decision the-works-head-leaves-out-merges-of-mains-code
The work's head is its branch's head without the merges on top that bring only main's code, each known by its content: one parent on main, and a clean merge's tree.
- replaced: Knowing the engine's own merge of main by its author's address and its words stood on main; the owner changed it, because an agent could amend that merge, keeping both, to slip a change of its own in unreviewed.

### decision each-head-is-kept-under-a-reference-of-the-engines
Each new version of a work's head is kept, as it is read, under a reference of the engine's own named after its branch, `refs/wf-agents/<branch>`.
- rejected: the owner proposed showing a work's changes as GitHub holds them; the owner rejected it, because commits reach GitHub only to be checked, so local work would show late.
- rejected: the owner proposed showing a work's changes from its worktree; the owner rejected it, because the worktree is gone once the work is done, and the changes of past works could not be seen.
