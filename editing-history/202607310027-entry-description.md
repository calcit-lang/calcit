# Entry description snapshot metadata

- `SnapshotEntry` now carries an optional `description` string for semantic context about an executable entry.
- Existing snapshots without `:description` remain valid and deserialize to an empty string; canonical snapshot writes include the field.
- Keep the build-script snapshot model synchronized with runtime `SnapshotEntry`, otherwise the embedded core MessagePack snapshot cannot be decoded.
- `cr config set [--entry <name>] description "..."` updates the field, and `cr config show` displays it.
- Program diffs expose description changes independently from runtime configuration and type-slot changes.
