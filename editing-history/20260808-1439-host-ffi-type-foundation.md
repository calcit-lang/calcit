# Host FFI Type Foundation

- Expanded the cross-backend host/FFI RFC with concise browser and DOM declaration patterns, plus explicit FFI conversion boundaries.
- Added `Host<T>` as a nominal FFI type annotation that reuses existing Calcit type references without making host values ordinary Calcit data.
- Added parsing, schema serialization, generic substitution, strict nominal matching, ordering, hashing, and regression coverage for `Host<T>`.
- Rejected host values from closed `data-shape` conversion and classified them as intentional FFI boundaries in coverage and mismatch diagnostics.
- Deferred `defhost-type` declaration storage and JS member inference until their registry and preprocessing model are designed.