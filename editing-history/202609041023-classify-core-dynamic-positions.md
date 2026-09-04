# Classify bundled-core Dynamic positions

- Added a checked-in, position-level classification for all 284 current
  bundled-core schema-Dynamic occurrences across 199 definitions.
- Assigned every position to an owning subsystem with an explicit
  `migrate` or `retain-reviewed` decision and rationale. The resulting queue
  contains 46 caller-visible migration positions; 238 reviewed compiler,
  macro, runtime, and open-data positions remain baseline-locked.
- Added a deterministic generator and a `check-all` drift gate so source or
  classification changes require explicit review.
- Verified the gate rejects a deliberately stale inventory count, then
  regenerated and rechecked the canonical document.
