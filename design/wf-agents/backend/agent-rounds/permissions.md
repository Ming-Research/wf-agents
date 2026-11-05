- Agents' tools and commands are wide: search, web search and fetching, formatting, and light read-only commands; forms
  that can execute, delete or write are refused. Unable to format or regenerate files, every item that changed an
  interface type stopped to wait for someone.

- The agents' command permissions refuse heavy work such as compiling and testing, which an agent might otherwise run
  locally for convenience.

- In each round an agent can read the data directory's tool output and logs, but cannot change them with its editing
  tools. The engine hands it those paths, which it otherwise could not open. Shell commands can still move or remove
  them, a known gap.
