# Canonical Cirru Snapshot Formatting

- Ran `cr edit format` across every repository Snapshot identified by top-level `:files` metadata.
- Canonicalized 57 Snapshot files, including runnable tests, debug/script fixtures, and type-fail fixtures.
- The serializer normalized legacy Snapshot record tags to quoted symbols while preserving program semantics.
- Verified formatting idempotence with a second formatter pass and ran `yarn check-all` successfully.
