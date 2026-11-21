# The repository's guide

What AGENTS.md, at the root of the repository, tells the agents that work in it.

### decision agents-md-holds-what-nothing-else-tells
AGENTS.md holds only what an agent working in the repository must know before it acts and neither the code nor the design tree tells it.
- replaced: Lines on the Rust version, the toolchain, the lints and where new crates go was on main; the owner changed it, because a line stays only if an agent would act differently for reading it, and those can be seen in the manifests.
- replaced: A first line saying that the repository was built anew beside the engine that came before it was on main; the owner changed it, because AGENTS.md must hold nothing like where the project came from.
- replaced: A line on how the engine's agents name tests to its checks was on main; the owner changed it, because AGENTS.md is for the project's own information and must not be coupled to the engine.
- rejected: an agent proposed a line that every agent in the repository follows the rules the flow gives its agents; the owner rejected it, because the engineering rules already hold it, and the line would repeat them.
- rejected: an agent proposed writing that heavy work runs in CI as a decision of the engine in the tree; the owner rejected it, because it is how to work in this repository, which AGENTS.md says, not what the system is.

### decision agents-md-says-what-docs-holds
AGENTS.md, not the design tree, says that `docs/` holds designs read as a whole, built or still ahead of the code, and guides such as how wf-agents is tested.
- rejected: an agent proposed that the tree's README say the documents are in `docs/`, outside the tree; the owner rejected it, because the documents are outside the tree, so the tree says nothing about them.
- rejected: an agent proposed taking each paragraph of the documents through the bar into a decision, a principle or an item, or deleting it; the owner rejected it, because a design not yet built and a guide are not decisions, and stay as documents.
