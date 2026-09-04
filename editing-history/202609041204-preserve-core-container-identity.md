# Preserve core container identity contracts

- Replace the open receiver and return positions of `filter`, `slice`, and
  `update` with an explicit shared container type variable `C`.
- Give `filter` a structured callback contract, and make `update` retain its
  key type and `T -> T` updater relationship instead of accepting open values.
- Add static contract tests for List, Set, Map, and String results while
  retaining the existing runtime coverage.
- Regenerate the reviewed quality baseline and Dynamic classification. The
  schema-Dynamic inventory falls from 280 to 273, unresolved positions from
  186 to 179, incomplete definitions from 135 to 132, and the public-core
  migration queue from 41 to 34.
- Verify cargo fmt, clippy, Rust tests, agent interface, 239 bundled-core unit
  tests, classification drift, native/JavaScript/IR/WASM, and performance gates.
