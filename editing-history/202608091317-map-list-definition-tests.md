# Migrate map and list API guards to definition metadata

- Reviewed 37 `calcit/test-*.cirru` fixtures (about 1,250 explicit assertions). Keep JS/WASM, macro expansion, type/preprocess, and multi-definition fixtures as target or integration coverage.
- Moved 20 map API contracts and 14 list API contracts to `calcit.core` definition-attached `:unit :core` tests. The core suite now has 111 runnable unit tests, up from 77.
- `test-map.cirru` remains in the JavaScript integration entry and `test-list.cirru` remains in the WASM suite. They are now cross-target regression coverage rather than the only native guardrail for the migrated APIs.
