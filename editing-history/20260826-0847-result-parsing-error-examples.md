# Cross-backend parser failure examples

- Added a malformed-input example to each Result-returning String parser method.
- Native and JavaScript example runners now exercise both `:ok` and caught `:err` paths for Cirru, Cirru-list, Cirru-EDN, and JSON parsing.
- The JavaScript Cirru-EDN parser still emits its existing parser-library diagnostic while returning the caught Result; this PR preserves compatibility and verifies the Result semantics without claiming that host parser output is fully silent.
