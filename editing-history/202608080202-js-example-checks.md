# JavaScript Example Checks

- Added `cr analyze check-examples --js` to compile the generated examples entry and execute it with Node.js.
- The generated entry declares the `:js-ffi` feature so legitimate JavaScript-only syntax, including `exists? js/process`, does not produce a preprocessing warning.
- Documented the JS mode and covered the generated runner and schema metadata with unit tests.
