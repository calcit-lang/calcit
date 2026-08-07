# Static Method Generic Validation

- Postfix method rewrites now preprocess their receiver and arguments before static method validation and code generation.
- Static implementation methods use their declared schemas, binding generic variables from the receiver before validating other arguments.
- Added regressions for `Option<String>.unwrap-or Number` and an inline anonymous callback passed to `Option.map`.
