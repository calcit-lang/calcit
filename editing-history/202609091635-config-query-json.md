# Add structured JSON config queries

- Added schema-versioned single-envelope JSON output to `config show`, `config modules`, and `config type-slots` while preserving their human output.
- Included complete selected-entry metadata, deterministic all-entry ordering, resolved module status, Snapshot revisions, and structured non-zero diagnostics.
- Covered all four entry targets, empty and populated maps/lists, missing and malformed configurations, CLI help, embedded Agent guidance, and automated inventory of the real `calcit-core` library.
