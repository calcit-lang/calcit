# Document parser backend boundary

- Clarified that Result-returning parser methods are currently native/JavaScript APIs.
- Distinguished a green whole-project WASM regression from support for parser definitions: WASM deliberately skips the unsupported parser/`try` definitions while validating all supported exports.
