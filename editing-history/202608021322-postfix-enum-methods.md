# Receiver-first enum method calls

- Extended preprocess-time postfix method rewriting to recognize nominal enum
  receivers as well as records and traits.
- During core bootstrapping, infer the declared result type of `%some`, `%none`,
  `%ok`, and `%err` from their schemas, so their values keep enough type
  information for receiver-first calls such as `res-ok .unwrap-or 9`.
- Updated Option/Result trait tests and polymorphism documentation to use the
  receiver-first form, and verified the JavaScript backend emits normal method
  invocation code.
- Updated a formatting-advisory fixture assertion whose legacy inherent-impl
  warning was intentionally removed by the earlier symbol-type migration.
