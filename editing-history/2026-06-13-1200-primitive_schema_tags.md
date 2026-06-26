# Calcit Type schema primitive tags support expansion

## Timestamp: 202606131200

## Modification Summary:
- Expanded `PRIMITIVE_SCHEMA_TAGS` inside `/Users/chenyong/repo/calcit-lang/calcit/src/snapshot.rs` to include `"record"`, `"struct"`, `"enum"`, `"trait"`, `"impl"`.
- This ensures schema annotations utilizing these tags (e.g., `:trait` or `:enum` in code entries) pass verification rather than raising an unrecognized primitive tag validation error.
- Verified with the Calcit test suite which passed 223+85 integration and unit tests without error.
- Updated `respo/alerts` annotations using these tags to enforce more precise typing rules in the codebase.
