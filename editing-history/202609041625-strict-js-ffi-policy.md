# Strict JS FFI feature-policy default

- `--strict-types` now gives a selected entry with no explicit `js-ffi` feature policy an in-memory `Error` default. It does not rewrite the project Snapshot.
- Explicit `allow`, `warn`, and `error` values remain intact, so existing projects can make their migration choice visible instead of depending on the compatibility default.
- `calcit config show` now displays feature policies deterministically, and `calcit config set feature-policy.<name> allow|warn|error` validates and persists a selected entry policy.
- Focused tests cover compatibility behavior, strict defaulting, explicit-policy preservation, deterministic display, persistence, and rejection without partial writes.
