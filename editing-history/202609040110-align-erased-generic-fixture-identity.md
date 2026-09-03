# Align erased-generic fixture identity

- Renamed the strict fixture package, namespace, and entry references to match
  `erased-generic-relation-strict.cirru` so diagnostic stacks and preprocessing
  cache keys cannot collide with the whole-Dynamic fixture.
- Ran `calcit edit format` to validate canonical Snapshot structure and reran
  the focused CLI regression test.
- Kept the original `202609040101` editing-history timestamp: repository
  timestamps use the configured Asia/Shanghai local time, and the associated
  commit was created at `2026-09-04T01:03:08+08:00`.
