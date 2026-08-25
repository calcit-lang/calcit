# Typed method native executable-op follow-up

- Updated performance roadmap issue #409 after #424/#425 merged and opened
  #426 for the remaining static-method/native-op metadata gap.
- Confirmed that typed method binding already existed; the missing link was
  rebuilding its resolved callable as an executable call with current
  `CalcitCallKind` classification.
- Added native typed remainder execution while retaining proc-identity checks,
  left-to-right single argument evaluation, and ordinary dynamic error
  fallback for stale static evidence. The fast path reuses the normal
  remainder implementation so its integer conversion and error behavior stay
  identical.
- Added native/generated-JS correctness fixtures and warm/sample benchmarks for
  typed method, intentionally Dynamic method, and direct proc forms.
- Against `main@159b4520`, the 500,000-iteration native medians changed from
  547.33 ms to 485.50 ms for typed `.rem` (~11.3% faster) and from 548.42 ms to
  500.36 ms for direct `&number:rem` (~8.8% faster). Dynamic dispatch remained
  intentionally unspecialized. Generated JS typed/direct forms remained
  equivalent and substantially faster than Dynamic dispatch.

Validation includes Rust formatting, clippy/tests, `yarn check-all`, the focused
benchmark, current upstream Recollect native/generated-JS runtime checks, and
Respo native tests plus generated-JS/Vite production build.
