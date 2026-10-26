# The wf-agents constitution

This constitution holds only the lasting values of wf-agents, and every decision in the project comes from them. The
owner sets it and changes it. How every agent behaves is not here but in the rules agents are given; what to do first
for a while is not here but in the owner's current policy.

## Article 1. Independent of any project; developed by itself

wf-agents makes agents do a project's work by a defined flow. It belongs to no project: the flow, the engine and the
data formats say nothing about any project, and everything about a project is in its configuration. Its own
development and maintenance also go through its own flow: it develops itself.

Violations: a branch, a name or a rule added to the flow or the code for one project; something that works only on
other projects and cannot run on wf-agents itself; wf-agents' own development going around its own flow.

## Article 2. Judgment comes from information

Whether the architect and the engineers judge right depends on whether the information they get is complete and
accurate. The data and the interfaces let each role (an agent, the architect, the owner's interface) get what it
should get, easily. When one cannot, that is a defect of the system, not of the agent.

Violations: information an agent needs to judge that it can only guess or dig out of logs; a cell's input missing what
its judgment needs; what should be seen drowned by other things.

## Article 3. Efficient

A second spent for nothing is waste; time that is needed is worth however long it takes. Everything that takes time
(a test, a check, an agent's round, a wait) must be able to say whether it is worth it: the side that does it proves
it, and the side that reviews it challenges it, rather than a budget cutting everything alike. Agents do not sit
idle waiting. Where performance can become a problem, use a fast language: Rust now, Whitefoot later.

Violations: adding a test, a check or a wait without being able to say why it is worth its time; writing something
that runs many times in a slow language because it is easier to write.

## Article 4. Start from the problem; record every anomaly

Every cell, field, rule and table must solve a problem that can be named; when none can be named, it is removed.
Every anomaly is recorded and summarized, so that it shows where the system needs to change.

Violations: adding something because it feels needed; an anomaly not recorded, or recorded and never summarized.

## Article 5. Product grade

Everything is built to the standard of the future product: names are final, never temporary; everything in the
repository can say whom it is for, where it is and when it will be removed; documents change together with the code.
Doing little is not doing it roughly: while the system is first being made to run, details are left out, and the part
that is built is still product grade.

Violations: temporary names, drafts or local paths entering the repository; code written to get something running
that will be thrown away whole, without saying when.

## Article 6. A design is judged only on itself

Whether a design is good depends only on whether it is right, elegant, maintainable and robust. How much code it
changes, how hard it is or how much it costs is no reason to choose it or not; it only decides what is done first.

Violations: giving up a better design because the change is too big or would take too long.

## When articles conflict

Article 1 governs all the others. Article 2 comes before Article 3: without the right information nothing works, and
being efficient means wasting nothing, not giving less than is needed. Article 4 decides whether a thing should exist
and Article 5 how a thing that exists is built; neither is ranked against the articles before them. Article 6 is the
way judgments are made.
