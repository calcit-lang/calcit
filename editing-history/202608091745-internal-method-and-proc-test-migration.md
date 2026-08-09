# Internal method and proc test migration

- Added 19 definition-attached `:unit :core` tests beside list and map method
  implementations, including `&list:find-last`, `&list:find-last-index`,
  `&list:map`, `&list:map-pair`, sorting, direct list primitives, map filters,
  mapping, association, removal, emptiness, values, and pair conversion.
- Attached tests directly to Rust-backed core procs such as `&list:first`,
  `&list:nth`, `&list:sort`, `&map:vals`, and `&map:to-list`. These tests verify
  the proc contract in the core snapshot; backend-specific execution remains in
  the JS/WASM fixtures.
- Removed the duplicated pure method assertion blocks from `calcit/test-list.cirru`
  and `calcit/test-map.cirru`. Kept list shorthand method coverage and the
  remaining method/type/backend integration fixtures.

Validation:

- `yarn try-core-tests` (186 passed)
- `yarn check-all` (core, native, JavaScript, IR, WASM, and agent interface)
- `cr calcit/test-list.cirru`, `cr calcit/test-map.cirru`, and
  `cr calcit/test-string.cirru`
- `git diff --check`
