# WASM dynamic string tag

- Schema type migration removed legacy type tags from program data, which can shift WASM's program-local tag indices.
- The Node.js WASM test host must not assume that the `string` type tag always has index 34.
- Export the generated module's actual string tag as `__string_tag` and use that value when the host validates or allocates strings.
- Keep the test host compatible with older generated modules by recovering the tag from their static string pool when the export is absent.
- Regression command: `cargo build --bin cr-wasm && CR_WASM_BIN=./target/debug/cr-wasm bash scripts/test-wasm.sh`.
