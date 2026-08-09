# Core API test migration and fixture cleanup

- Extended definition-attached tests in `src/cirru/calcit-core.cirru` across collection helpers, string/text formatting, Cirru/EDN parsing, map diffing, numeric helpers, and update operations.
- Removed the duplicated pure API test definitions and their calls from `calcit/test-list.cirru`, `test-map.cirru`, `test-math.cirru`, `test-set.cirru`, and `test-string.cirru`.
- Kept method-dispatch, macro expansion, type/preprocess, JS/WASM, and internal destructuring cases in the fixture snapshots because they exercise target-specific or integration behavior not represented by a standalone core definition test.
- Verified native project execution, JS code generation/runtime execution, and the definition-attached core suite after cleanup.
