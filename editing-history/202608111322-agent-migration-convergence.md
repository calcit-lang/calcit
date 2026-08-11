# Improve agent migration convergence

Migrating Respo and its dependent modules to stricter Calcit types exposed a
few CLI and compiler failures that made automated repairs unnecessarily slow or
unsafe:

- `cr edit imports` documented Cirru input but only accepted a single flat rule;
  multi-rule input effectively required JSON.
- Cirru transaction output expanded quoted empty lists into a shape that did not
  round-trip, blocking safe insertion of zero-argument function definitions.
- definition revision hashes rejected leaf-shaped examples/tests, so semantic
  queries failed on otherwise valid definition-attached tests.
- large but finite UI/Markdown dependency graphs overflowed the 16 MiB codegen
  worker stack before diagnostics could identify the actual type errors.
- several tree command help strings still advertised removed comma-separated
  paths even though the parser requires `@2.1.0` coordinates.
- concurrent mutation processes can overwrite each other's snapshot changes,
  which was easy to trigger when an agent updated multiple entries in parallel.

Changes:

- Accept a quoted Cirru `[]` whose child expressions are import rules in
  `cr edit imports`, while preserving JSON compatibility and rejecting
  malformed flat children.
- Emit inline Cirru for transaction quoted code so empty lists round-trip.
- Give leaf examples/tests stable revision encodings without changing existing
  list revision hashes.
- Raise the dedicated codegen worker stack to 64 MiB, which is sufficient for
  the current Respo UI/Markdown graph and still keeps recursion isolated from
  the process main thread.
- Align all path option help with the dot-separated coordinate parser.
- Document that writes to one snapshot must be serialized, or grouped in a
  revision-guarded transaction; independent snapshots/worktrees may still run
  in parallel.

Real-project validation used local Respo, Router, Markdown, UI, Reel, and Alerts
snapshots after removing the obsolete Lilac module dependency. UI, Router, Reel,
Alerts, and all three Markdown entries completed JS codegen. Calcit built-in
tests passed 35/35 in Respo and 6/6 in Router; no external `calcit-test` runner
was used.
