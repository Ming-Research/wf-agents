Decision harness-only: wf-agents is only the harness (the engine, the flow, the database schema, the flow compiler and the command line), and everything about a project (its configuration, the full text of the rules its agents are given, its data) lives in the owner's private data directory, instead of configuration kept in this repository or in each project's repository, because the harness must say nothing about any project, while the owner's projects share most of their settings and are best kept together, split by project, with shared parts.

Rests on: C:independent, F:projects-share-settings

Rejected:
- Configuration in each project's repository: rejected because the projects share most settings, so each would repeat them, and the repositories of projects that are not the owner's own could not hold private settings.
- Configuration in a folder of this repository: rejected because it would tie the harness to the owner's projects and make public what is private.

Decision flow-rules-in-flow: The rules that belong to the flow itself (how the design tree is written, the review tables, interjections) stay in the flow and travel with it, because they define how the flow works rather than how one project works.

Rests on: D:harness-only
