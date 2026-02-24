# Knowledge Notes

- Unified tuple enum reflection API by removing `&tuple:get-enum` and keeping `&tuple:enum` only.
- Renamed record struct reflection API from `&record:get-struct` to `&record:struct`.
- Updated type-annotation matching so proc warnings report `:record` (and other domain types) instead of falling back to `:tag`.
- Synced runtime layers (Rust + JS), core metadata entries, tests, and docs to prevent naming drift.

# Change Summary

- Rust proc registry and dispatcher updated for the new canonical API names.
- Record builtin error messages and hints updated to `&record:struct`.
- Core API metadata (`calcit-core.cirru`) updated to remove alias entries and keep canonical names.
- JS runtime exports updated accordingly.
- Tests updated to use canonical APIs.
