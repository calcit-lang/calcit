# Address Result file-effect review

- Grouped both receiver expressions in the documented Result chain so Cirru does not pass later method tokens as extra arguments.
- Made `.read-dir` preserve the default non-recursive call without reintroducing a legacy nil parameter: its trailing recursion flag is `Option<Bool>`, omission supplies `%none`, and explicit recursion uses `%some true` or `%some false`.
- Added omitted and explicit Option cases to native and generated-JavaScript tests.
- Repeated the native/JavaScript support and WASM host-effect boundary at each public documentation entry.

Validated with targeted examples/tests, native and generated JavaScript runs, and the full Markdown checker.
