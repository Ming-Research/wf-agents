Decision: A test name given to the ci or ci_before tool means the test whose name in its test binary equals it or ends in `::` and it, or, written as cargo-nextest prints a test (the binary, a space, the name), that name in that binary; one piece of code decides which tests of a log a name holds, instead of every test whose name contains it, because a name that starts or ends another test's name would read that test's result as the first's, and because the names agents give are the function's own name, which the full name with its module path would not match.

Rejected:

- Substring matching, as the name filters of cargo-nextest do: rejected because naming one test would give the result of every test whose name contains it, and the result of one could stand for the other.
- The full name only, with its module path: rejected because the names agents give are the function's own name, without a module path, and a rule that takes the full name only would answer not run to all of them.
- A name that ends in `::` and the test's name also naming it, so that a path from the crate such as `engine::tools::tests::x` reads the result of `tools::tests::x` in the binary `engine`: rejected because `mod_a::a_row` would then read the result of a top-level `a_row`; no name given so far uses it.
