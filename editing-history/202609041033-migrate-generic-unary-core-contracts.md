# Migrate generic unary core contracts

- Replaced the open `Dynamic` input of `some?`, `struct-def?`, `thread-step?`,
  and `to-lispy-string` with an explicit unary generic `T` contract.
- Reduced bundled-core schema-Dynamic debt from 284 to 280 positions,
  unresolved debt from 190 to 186, and incomplete-type debt from 138 to 135.
- Regenerated the reviewed per-definition quality baseline and Dynamic
  classification, shrinking the public core migration queue from 45 to 41
  positions without increasing retained open boundaries.
- Verified the bundled core unit suite and both checked-in quality gates.
