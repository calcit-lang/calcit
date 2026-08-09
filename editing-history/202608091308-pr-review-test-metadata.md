# PR review: definition-attached test metadata

- Test names now reject surrounding whitespace consistently while loading compact snapshots, loading detailed snapshots, and adding tests through the CLI. The common validator also owns duplicate detection.
- `cr test --format json --name <missing>` writes its empty report envelope before returning its non-zero selection error, preserving stdout's JSON protocol.
- `cr query tests` now treats special core builtins as definitions with no attached tests, consistent with the other query commands.
- Corrected the affected-test documentation: static-analysis failures are selected and reported as failures; they are not executed.
