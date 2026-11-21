# The work's head on its branch

Which commit on its branch is the work's head, and where its versions are kept.

### decision the-works-head-leaves-out-merges-of-mains-code
The work's head is its branch's head without the merges on top that bring in only main's code, each recognized by its content: one parent on main, and the tree of a clean merge.
- replaced: Recognizing the engine's own merge of main by its author's address and its message was on main; the owner changed it, because an agent could amend that merge, keeping both, to slip in a change of its own unreviewed.

### decision each-head-is-kept-under-a-reference-of-the-engines
Each new version of a work's head is kept, when it is read, under the engine's own reference named after its branch, `refs/wf-agents/<branch>`.
- rejected: the owner proposed showing a work's changes as GitHub holds them; the owner rejected it, because commits reach GitHub only to be checked, so local work would show up late.
- rejected: the owner proposed showing a work's changes from its worktree; the owner rejected it, because the worktree is gone once the work is done, and the changes of past works could not be seen.
