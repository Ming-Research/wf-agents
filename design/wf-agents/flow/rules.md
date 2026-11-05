- Rules that depend on no project are in the flow's definition, and a project's own rules are in the data directory. The
  first do not change by project, and roles such as engineer and reviewer are in the flow anyway.

- Rules do not say what the flow, tools or permissions already decide: who obeys whom, how often to check GitHub,
  stopping to ask before acting. Roles are set by the flow, agents do not poll GitHub since the engine runs CI and hands
  over its results, and what an agent can do is limited by permissions.
