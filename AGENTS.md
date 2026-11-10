# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given.

- This branch, `next`, holds wf-agents built anew from nothing, beside the engine on `main`, which it replaces once it
  does what that one does. Nothing here depends on the code of `main`: what is taken from it is read, judged and
  brought over, the commit saying from where. Until then, "main" in the rules of the design tree means this branch.
- [design/README.md](design/README.md) says how the design tree is read and changed, and what to read before designing
  or changing code. [design/constitution.md](design/constitution.md) decides between designs.
- Everything in the repository is written in English.
- Keep keys, credentials, personal paths and machine-local settings out of the repository, its commits and its PR
  descriptions.
