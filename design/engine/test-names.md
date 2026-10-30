Decision: A test name given to the ci or ci_before tool means the test whose name in its test binary equals it or ends in `::` and it, or, written as cargo-nextest prints a test (the binary, a space, the name), that name in that binary; one piece of code decides which tests of a log a name holds, instead of every test whose name contains it, because a name that starts or ends another test's name would read that test's result as the first's, and because the names agents give are the function's own name, which the full name with its module path would not match.

Rejected:

- Substring matching, as the name filters of cargo-nextest do: rejected because naming one test would give the result of every test whose name contains it, and the result of one could stand for the other.
- The full name only, with its module path: rejected because it asks agents for a path they do not write and the logs of a test binary do not show with the function's own name alone.
