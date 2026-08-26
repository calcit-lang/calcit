# Fix Result method chaining in file examples

The Markdown checker exposed a Cirru association pitfall in the new examples: `path .read-file .and-then` passes `.and-then` and its callback as extra arguments to `.read-file`. Parenthesize the first receiver call as `(path .read-file) .and-then` so the Result method receives the completed `Result` value. The checked example keeps its callback body direct to avoid unrelated nested callback inference obscuring the file-effect contract.

Validated with the repository-wide Markdown check.
