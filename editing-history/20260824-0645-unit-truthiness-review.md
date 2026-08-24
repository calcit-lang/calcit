# Align Unit truthiness across runtimes

- Treat `&unit` as false in native conditionals and `not`, matching JavaScript (`void 0`) and WebAssembly (`0`).
- Make `or` use the shared conditional semantics, then add a Unit regression test.
- Replace internal no-return `;nil` tails with `&unit`, while retaining `;nil` for legacy absence semantics.
- Cover `json-stringify &unit` as a type error.
