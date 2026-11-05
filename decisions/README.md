# Decisions

This directory holds the decisions in force for wf-agents' design: how the design does something now, where research
left at least two options that each stand under the constitution and choosing wrong would cost much, so that the owner
chose. A fact that research can settle is no decision, and neither is a choice that is cheap to change.

One Markdown file for each decision, in English, saying:

- what is done now;
- why: what it is for and the reasons;
- each option not chosen, and why not;
- one line for each challenge considered and turned down: when, what change was proposed, and why it was not made.

A decision enters only through a design the owner reviewed, and merges with the work that made it. To depart from one,
a work either keeps to it or goes through a design that changes it. A decision replaced is rewritten in its file, the
commit saying what changed and why; one withdrawn is deleted, the commit saying why. Earlier versions live only in
git's history, never in this directory: `git log --follow` on the file gives them, with each commit's message.

While they are few, every agent that judges design is given them all, as it is given the constitution. This file is
no decision, and the engine gives it to no agent.
