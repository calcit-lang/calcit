# WASM FFI declarations

- Added `defwasm-export` for stable host-facing WASM functions. A marked function that cannot be compiled now fails code generation instead of exporting a placeholder.
- Added `defwasm-import name (args) |module |field`. The emitter collects these declarations before function indices are assigned and emits the corresponding WASM imports.
- The initial ABI uses `f64`: Number is direct, while String is its tagged Calcit heap pointer carried as `f64`. Host code may create returned strings through `__str_new` or the documented layout.
- Import declarations must remain parseable after schema preprocessing, so module and field are discovered from the first two literal strings following the argument list rather than fixed preprocessed positions.
- Regression coverage checks module exports/imports and executes Number plus String host round trips. Both pull-request and release workflows run the check.
