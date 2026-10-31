Decision rules-in-flow: The rules agents follow are the flow's rules, written in `flow/flow.json` and independent of any project, each given to cells as a conditional input: the general rules to every agent's cell, the engineering standards to every engineer's cell and the advisor's, and the design tree writing, the review table and the design tree review to the cells that name them; a rule given to every cell of some lines names those lines, and a cell does not name it again, instead of rule files in the project's configuration, because how an engineer, a reviewer or an adversarial tester works is the same in every project and belongs to the flow those roles are cells of.

Rests on: C:independent

Rejected:
- Rule files in each project's configuration for how agents work: rejected because the same rules then have to be kept in every project, and the flow cannot be checked against them.

Decision architect-rule: The architect is given a rule of its own, on its line, after the general rules every graph shares: how it judges (by each project's constitution and the policy in force, the policy deciding between projects), what is its to decide and what the owner's, that it does not do the engineers' work and has what needs fixing fixed rather than fixing it, how it looks at works where they meet the whole and at design cards without ruling on them, when it pauses a work or holds new works and that it says why, what it never does, and how it writes its changes, its notebook and its record, instead of the architect working from its cells' questions alone, because the questions say what to answer at each step, while how the architect acts across them, and where its part ends and the owner's or an engineer's begins, holds at every step.

Rests on: C:information

Decision advisor-as-engineer: The advisor is an engineer that runs on a higher-level model, and is given what an engineer is given, the engineering standards included, instead of rules of its own, because the advisor does an engineer's work one level up.

Open: the premise behind the advisor doing an engineer's work one level up, which the decision on an item's level gives as the owner's decision; to ask the owner.

Decision field-writing-in-descriptions: How a field is written is said in the field's description, where the agent writing it reads it (the problem and its example in the understanding, the comparison in the approaches, the design and its cost in the design, the acceptance in the acceptance, the progress with two examples, what changed in the PR description), and how every field the owner reads is written, with how code examples are written, once in the forms every return shares, which those fields point to, together with the fact that only the return is read, instead of a rule of item writing given to engineers, because a separate rule repeats what the fields ask, and every piece of it is needed exactly where a field is written.

Rests on: C:information, F:agent-writes-by-description

Rejected:
- One rule of item writing given to every engineer's cell: rejected because it repeats the descriptions of the fields it governs and gives every cell the parts only other cells write.

Decision owner-language-native: A field in the owner's language is written in it directly, never as a translation, said in the forms every return shares beside how a field the owner reads is written, as the earlier board's rules said, instead of a text thought in English and put into the owner's language, because the owner reads such a field, and a translation reads as one.

Rests on: C:owner-page, F:owner-reads-item, F:owner-reads-work-group, F:translation-reads-as-one

Decision shared-forms-writing: The forms every return shares also say, as the earlier board's rules did, that a field in the owner's language is written in short sentences with subject, verb and object and no chains of possessives; that what a diagram shows more clearly than prose (a flow, states, a sequence, a structure, numbers compared) is drawn in a fence, `mermaid` for any kind mermaid draws or `dot` for Graphviz, and not said again in prose; that an item is named by its number and title; and that a code example explains itself inside the code, with comments only where the reader needs them, no numbered steps, and output explained in the sentence before it; and the engineer's research gives the like problems in other code or projects with how to handle each, and its design how the class of problem is kept from coming back; instead of leaving these out, because the owner reads these fields, and a design is understood far sooner from a diagram than from prose describing one.

Rests on: C:owner-page, F:owner-reads-item, F:owner-reads-work-group, F:diagram-understood-sooner

Decision agents-md-hygiene: The review table's column on repository hygiene finds a change to the project's AGENTS.md the owner did not agree to, as it finds anything added at the root, as the earlier board's rules held a project's AGENTS.md to what the owner ruled, instead of a change to it passing as any other file's, because a project's AGENTS.md is a rule its agents work by, which is the owner's to set.

Rests on: F:agents-md-rules

Open: the premise behind a project's AGENTS.md being the owner's to set, which the decision does not give; to ask the owner.

Decision no-todo-file: The engineering standards' lines on the repository say that a repository keeps no TODO file: open work is an item, which code and documents cite by its number, and a public repository by its number alone, as the earlier board's rules said, instead of a list of open work kept in the repository, because the item is where open work is ranked, started and done, and a list beside it goes stale; and the rest of an item is written for the owner, not for a public repository.

Rests on: C:design-on-merit, F:copies-drift, D:item-language

Decision item-writing-rule: How the architect writes an item is said in its rule of item writing (`item_writing`), given to its line and to the review of its batch: how the title, the problem, why and the acceptance are each written, how an item that takes in more is, and how large an item is; the fields of the change that makes an item say what each part is and the title's limit, instead of each part's rule in its field's description alone, as for the engineer's fields, because the review of a batch (`B2`), which checks that each change is written as items are, is given the rule and not the fields.

Rests on: C:information

Decision item-language: The change's description (D:item-writing-rule) says the owner reads the item's parts, its title written in English and the rest in the owner's language as every field the owner reads is, because the owner reads the item, as the items of the earlier board were written for the owner, while the engineer's own problem, shown over the item's, is in the owner's language already, and titles keep one language for now, which also keeps a title usable where the project must be English, as in a pull request or a branch.

Rests on: C:owner-page, F:owner-reads-item, F:repository-english, D:item-writing-rule, D:field-languages

Decision review-checklist: A review is given its checklist as the review table and the design tree review, and its own way of working in its tasks and its findings: it reviews only the change, reruns no test already green, adds no new style demand, reviews only the PR's own change where it merges main, marks each item passed, findings, cannot be verified or not applicable with no pass without evidence, and gives each finding where it is, the quoted text or the missing evidence, and the reason, quoting both sides of a contradiction; a defect in the PR description is a finding too, instead of the engineering standards as what a reviewer checks, because the standards say how agents work themselves, not what a review checks.

Rests on: C:information

Rejected:
- Giving a reviewer the engineering standards as a checklist: rejected because they say how an agent works, while what a review checks is the review table and the design tree review.

Decision project-rules: A project's own rule texts, which its configuration lists, are given to every agent's cell beside the flow's rules, and hold only what is particular to that project, instead of carrying the flow's rules too, because a rule a project needs (such as a goal its projects share) cannot be written into the flow, while the flow's rules hold in every project.

Rests on: C:independent, D:rules-in-flow

Decision no-read-yourself-input: An input that only says to read something yourself is not an input: what an agent reads to do its task is its own to find, and what a cell must not read, or must read first, is said in its task, instead of an input with nothing in it but that, because such an input told the agent what every agent does anyway.

Rests on: C:problem-first, F:agents-read-unprompted

Decision design-goal-input: The project's design goal is given to every agent's cell, as a conditional input, instead of to the cells that compare approaches only, because every judgment a cell makes is made toward it.

Rests on: C:information

Decision evidence-standards: The engineering standards keep the earlier board's lines on evidence, experiments and the repository whole: the smallest sample is timed and its spread read before the scale is chosen, never a run of hours first; the smallest sample and a rerun of the same comparison are no follow-up measurements; the noise floor is measured again when the machine, its settings or the toolchain change, a run noisier than it is rerun or reported unresolved, and pairs stop being added as soon as the interval decides; and a change searches for the old wording, numbers and names it made false and corrects each, instead of lines that kept only the first step of each, because each step left out is a run wasted or a false statement left behind.

Rests on: C:efficient, C:product-grade

Decision architecture-ownership: Architecture beyond an engineer's own item (how modules relate, their interfaces and contracts, the formats components exchange) is the architect's and the owner's: an engineer whose interface is not enough asks the architect, and the advisor hands such a question on, instead of the advisor deciding a change of an interface between projects itself, because an interface beyond one item serves the works on both sides of it, which one work's advisor does not see.

Rests on: C:information

Decision refused-actions: An action an agent's own permission check refuses is asked of the owner only when the agent needs it, and for one it does not need the agent says why in its return, as the earlier board's rules said, instead of every refusal asked of the owner, because a refused action the work can do without takes the owner's time for nothing.

Rests on: C:efficient

Decision review-table-standards: The review table names what the engineering standards ask of a change that a review can see in it: its column of repository hygiene checks that ported code keeps its license notice, that one place defines a thing, and that the PR description covers every file of the diff; and a column compares each test, check or expected result the diff changes without a column of its own with its version before, instead of the review given the engineering standards, because the standards say how an agent works, and an expected result changed but not listed among the test changes is a weakened test no other column reaches.

Rests on: C:design-on-merit, D:review-checklist

Decision no-design-tree: Every project should have a design tree; one with none yet names no design paths, so its works raise no design card, and the design tree review answers its four points on code against decisions not applicable there, as the earlier board's rules skipped every step about a tree, instead of a review that asks for nodes a project has no tree to hold, because such a finding could not be fixed and would only send the work round the fix.

Rests on: C:efficient

Decision work-split: A work is split when a part of it must merge on its own before the rest can go on, and when a question holds only part of it while the rest can merge on its own, the held part then split off with the question, as the earlier board's rules said, instead of its size alone deciding a split and a question holding the whole work, because the rest would otherwise wait for nothing.

Rests on: C:efficient

Decision ask-recommends: An ask says what its asker recommends, in what happened, beside what was tried, as the earlier board's rules asked of a question to the architect, instead of a field of its own, because the one who answers decides faster from a proposal, and the owner reads it in the same place in the work's group.

Rests on: C:efficient, F:proposal-decides-faster, F:owner-reads-work-group

Decision fix-searches-kind: The fix searches for others of a defect's kind before fixing it and fixes them with it, as the earlier board's rules said, instead of only the row it is given, because a defect found once is seldom alone, and each one left costs another round of review.

Rests on: C:efficient, F:defects-not-alone

Decision ruling-settles: A ruling on a finding or a disputed test settles it whoever gave it, the advisor or the architect or the owner answering in its place: the fix answers advisor_ruled for each, the review or adversarial testing withdraws it and does not insist, and the engine takes such a ruling once the work asked the advisor, instead of the advisor's own ruling only, because a question the advisor hands on is answered in its place, and the earlier board's reviewer took the architect's answer and the owner's ruling as settled too.

Rests on: D:hand-over-passes

Decision who-tells-owner: Only the engineer and the architect tell the owner (`tell_owner`); the lines that help them (the review, adversarial testing, the advisor, the review of the architect's batch and the search of observations) have no such field, which the forms every return shares keep to those two lines (`agentCommon.lines`). What a helper of a work wants known goes to the project's architect as a side report, and what the review of a batch or the search of observations wants known goes in its answer, which the architect reads; the architect takes a report in as an item or an observation, and tells the owner of it only as it tells the owner anything (below). The engineer tells the owner three things only: what only the owner can do (a permission, a credential, an account, money), a risk that cannot be undone, and a boundary crossed (a rule or a check gone around, the constitution or a norm breached, something written that must not be); everything else goes in its progress or a side report. The engine's own notices stay as they were. Instead of every agent telling the owner what it judged the owner should know, routine results only barred, because after that bar agents still sent the owner 27 messages in three hours, mostly technical detail and process notes, all but three from the engineer and the lines that help, while the architect weighs a report against the whole project before any of it reaches the owner.

Rests on: C:owner-page, F:helper-messages-flood

Rejected:
- Side reports for the review of the architect's batch and the search of observations: rejected because their answers go to the architect already, and a side report would queue for the architect a matter of the work it is in.

Decision architect-tells-owner: The architect tells the owner of its own accord (`tell_owner`) the same three things as the engineer, starting a work the owner paused counting among what only the owner can do, and never a guess or what it has not checked, so that it never has a guess of its own to correct; what it changed in the project (items made, cancelled or reordered, observations recorded, the plan, the policy) is not told, since the page shows it from the records, and everything else goes in its notebook or its progress. It answers the owner's own words in its reply to them, and its questions and its draft of a policy for approval go to the owner as before; a defect of the system it meets goes in its flow feedback, which reaches the architect of wf-agents. Instead of telling the owner what it judged the owner should know, because in a day of the sixth trial the architect sent the owner 10 messages of its own beside its 8 replies, most of them guesses marked unchecked, corrections of earlier guesses, or what it had changed, which buried what the owner had to act on.

Rests on: C:owner-page, F:architect-messages-bury, D:who-tells-owner
