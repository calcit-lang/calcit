# Typed literal collection paths

## Context

Issue #424 completes the last executable optimization in roadmap #409. Public
`get-in` and `assoc-in` are recursive Calcit functions, so literal paths still
paid for path-list traversal and dynamic dispatch even when type inference had
already proved every hop.

The public APIs deliberately reject Struct traversal. Static evidence must not
turn either API into an alternate Struct field accessor; required fields remain
`(:field value)` and direct `assoc` operations.

## Changes

- Expand a non-empty literal `get-in` path only when the base, every collection
  hop, and the final payload are statically non-Dynamic and no hop is a Struct.
- Preserve single evaluation, nil short-circuiting, `%some` / `%none`, missing
  keys, and an explicit Struct guard at every generated lookup hop.
- Expand `assoc-in` only for fully typed Map-only paths in this first phase.
  Generated code uses direct map contains/get/assoc primitives while retaining
  missing-map construction, nil normalization, evaluation order, and Struct
  rejection.
- Keep empty paths, Dynamic or unknown hops, Struct boundaries, and mixed
  `assoc-in` containers on the original recursive functions.
- Add a checked-in native/generated-JS comparison benchmark and update RFC P6
  to the reconciled Struct-safe contract.

## Measurements

The 100,000-operation release benchmark produced the same outputs for typed and
dynamic variants. The checked-in script measured native at 3.46 s versus 6.70 s
for read (~1.9x faster), and 2.31 s versus 6.32 s for write (~2.7x faster).
Generated JS medians were 39.3 ms versus 74.2 ms for read (~47% faster), and
19.9 ms versus 867.0 ms for write (~43.6x faster). Run `yarn
bench-literal-paths` to reproduce the focused comparison.

## Validation

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn check-all`
- `yarn bench-literal-paths`
- current `calcit-lang/recollect` main: native tests 9/9 and generated test-entry
  JS runtime passed
- current `Respo/respo.calcit` main: native tests 25/25, JS generation passed,
  and the browser demo rendered without runtime errors when Vite was pointed at
  this workspace's matching 0.13.43 JS runtime

The published downstream JS runtimes lagged the compiler during validation
(Recollect 0.13.19, Respo 0.13.40). Respo's default package therefore lacks the
new struct `nthAt` method until the matching runtime release is installed; no
downstream files were changed. Recollect's default demo build also retains four
pre-existing Respo dependency type warnings, while its own test entry passes.
