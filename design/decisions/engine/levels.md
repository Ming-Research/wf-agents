# The levels

What an item's level chooses for the rounds of its agents, the reasoning effort each cell runs at, and which lines run above their item's level.

### decision a-level-is-a-difficulty
An item has one level, which says how hard its work is and chooses only the model its agents run on, never their effort; a level's description talks only about difficulty.
- replaced: Five levels, each a model and an effort together, was on main; the owner changed it, because a step should think as hard as it needs to, and a gate think little whatever the item, which one effort for all of a level's steps could not express.
- rejected: an agent proposed telling the levels apart by whether a work needs new decisions; the owner rejected it, because a level is how hard the architect understands the work to be, which has nothing to do with whether it needs the decision tree.
- replaced: A level's description that raised an item when its work met new decisions or facts to find was on main; the owner changed it, because items were rated too high, and this project's work needs no third level.
- replaced: One model for every round, set by the installation's single setting, was on main; the owner changed it, because small items then ran on Opus, while the old system had most work at level 2.

### decision effort-is-the-flows
Every agent cell names its reasoning effort in the flow's definition, none of them the lowest, and neither the page nor the installation's settings has a setting for it.
- rejected: an agent proposed keeping each cell's effort in the installation's settings, changed on the page as the owner had asked; the owner rejected it, because if one needs changing, changing the flow is then no trouble.
- replaced: Searching and judging cells on the cheapest model at the lowest effort was on main; the owner changed it, because the cheapest model at the lowest effort judges too poorly.

### decision the-research-and-the-code-reviews-think-high
The initial research and the reviews of the change run at the effort high; the implementation, the deep research and the plan's reviews run at xhigh.
- replaced: Running the initial research and the reviews of the change at xhigh was on main; the owner changed it, because high might take a third less time on a research that took 7 to 13 minutes, though that was not measured.

### decision the-deep-research-and-the-plan-review-run-above
The deep research and the plan's reviews run a level above their item's by `levels_above`, the code's reviews at its own; a round stops at `agents.highest_level`, an item at `items.highest_level`.
- replaced: Items at any level, the top one included, was on main; the owner changed it, because the top level's model is not much stronger than the one below it and costs far more.
- rejected: an agent proposed running the review a level higher when the design tree changes; the owner rejected it, because the item's own level is enough until a trial of both shows that one level more helps.
- replaced: Running the deep research at its item's level was on main; the owner changed it, because a weaker agent may fix the surface and stop, hiding a larger problem that no later step finds, and with few works going that deep the higher model costs little.
- replaced: One setting, `items.highest_level`, holding both the level an item may be at and the level every round runs at was on main; the owner changed it, because the two highests mean different things: one setting could not keep items at level 3 and let a line one level up run above them.
- replaced: Running the plan review at its item's level was on main; the owner changed it, because engineers mostly do what a review says, so a review weaker than the plan it reviews turns good work bad; he held to this again once it ran on every work.
