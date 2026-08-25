# Nil and Unit runtime separation

- Split the static `Nil` and `Unit` annotations: `nil` remains absence and
  `&unit` is the only canonical no-result value.
- Aligned native effects, generated JavaScript, runtime identity, hashing,
  formatting, typed data-shape decoding, and serialization boundaries with
  that distinction; bumped the data-shape ABI to reject stale runtimes.
- Preserved `;nil` as a real Nil expression, normalized only missing safe JS
  property/call results to Nil, and kept raw `aget` capable of observing Unit.
- Migrated explicit effect tails in js-ffi and Respo, then exercised Recollect
  against all three local worktrees to protect dynamic framework behavior.

Validation: `cargo test`, `npm run compile`, `npm run check-js-runtime`, native
and generated-JS core suites, js-ffi Node/browser contracts, Respo tests/build,
and Recollect unit/JS/build checks.
