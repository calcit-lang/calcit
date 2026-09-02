# strict map-kv nil sentinel diagnostic

- Added strict-mode diagnostic `E_NIL_CALLBACK_SENTINEL` for inline `map-kv` callbacks with structurally visible nil return paths.
- Kept compatibility mode behavior unchanged and preserved nil values nested inside returned key/value pairs.
- Guided migrations to `filter-map-kv` with explicit `MapEntryDecision :keep/:drop` results.
- Added focused tests and updated the strict nil, hashmap, and CLI diagnostic documentation.
- Validated the ecosystem migration in `editor`, plus downstream `gen-code` with the local Respo FFI fix.
