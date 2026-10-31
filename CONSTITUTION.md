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

## Article 7. What the flow can do, the flow does

How work goes on is drawn in the flow, where the owner and every agent can read it, and a work that went wrong can be
traced cell by cell: where it stops, who handles what, when something is handed on or taken higher. The engine carries
the flow out, keeps the data and checks it; it does nothing of its own that a cell or a row could do, and as little as
possible happens by itself.

Violations: a hand-over, an escalation, a retry or a wait that the engine does out of the flow's sight; behaviour that
can be found only by reading the engine's code.

## Article 8. The owner's page holds what the owner uses

Everything on the owner's page is there for the owner to know something, decide something or do something. Each element
answers five questions: why it is there; why it is shown this way; what the owner does with it where it stands; whether
a better way to show it exists; whether its space matches its weight. What the owner needs often and at once takes the
room and the first look, what is needed now and then is folded or a click away, and what is never needed is not there.
A page and each of its parts is designed from what it is for: what it must hold, what it leaves out and how it is laid
out; leaving out is half the work, and putting on what the owner needs and does not yet have is the other half. Each
thing has one place, named for what produced it, and is shown there once. Like things look alike: the same kind of
content (a message, a text an agent wrote, a verdict, a list of items, a link to a round, a form to answer) is shown the
same way wherever it appears, from one set of parts, so that the owner learns each look once. On a chart of a flow, a
cell's shape and colour say who does its work: a box is an agent's cell, in its session's colour; a rounded purple cell
is the engine's, a tool it runs included; a rounded orange cell is the owner's. The page is built to its UI spec
(`design/harness/owner-interface/ui-spec.md`), which a check in CI holds: a change that needs what the spec lacks
changes the spec in the same change, saying why.

Violations: a column, a field or a log put on the page because the data exists (the engine's bookkeeping, raw values,
internal names, JSON); the same thing shown in two places; text written for agents or the engine shown as if written for
the owner; a table squeezed until it cannot be read; marks, motion or colour that tell the owner nothing; a page whose
parts follow how the system stores things rather than what the owner does with them; the same kind of content in a style
of its own in each place it appears; a cell no agent works drawn as a box, or an agent's cell drawn otherwise; a change
to the page that departs from the UI spec without changing it.

## When articles conflict

Article 1 governs all the others. Article 2 comes before Article 3: without the right information nothing works, and
being efficient means wasting nothing, not giving less than is needed. Article 4 decides whether a thing should exist
and Article 5 how a thing that exists is built; neither is ranked against the articles before them. Article 6 is the
way judgments are made. Article 7 decides where a behaviour is written: in the flow, unless the flow cannot say it.
Article 8 is Article 2 for the owner: what the owner's page shows, and how.
