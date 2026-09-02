# Close strict Nil review gaps

## Context

Follow-up review on the strict `Nil`/`Unit` boundary found that legacy `;nil`
is represented after preprocessing as a zero-argument `hint-fn`. Generic type
inference did not classify that form as `Nil`, so `--strict-types` could miss
the invalid return. The same diagnostic also relied on stack frames or child
locations and could lose the enclosing definition call coordinate when stack
tracking was disabled.

## Changes

- Infer a zero-argument `hint-fn` expression as `Nil`; metadata-bearing
  `hint-fn` forms remain unaffected.
- Carry the source call location through `PreprocessContext` for function
  definitions and prefer it when constructing `E_NIL_FOR_UNIT`.
- Add a regression test covering the legacy expansion under strict mode with
  stack tracking disabled, including the exact definition call coordinate.

## Validation

- `cargo fmt`
- Targeted strict `Nil`/`Unit` preprocessing tests
- Full repository quality gates are recorded in the associated PR update.
