# Conservative pure macro expansion cache

- Added a watch-mode raw expansion cache for strict macros with no declared
  compile-time capabilities. Once-mode builds bypass it to avoid cold-build
  overhead.
- Keyed entries by stable macro call site, macro runtime identity, phase-aware
  signature, and exact raw syntax including source locations. Macro/helper
  reloads recreate the macro identity; signature and input changes have separate
  invalidation reasons in `--macro-metrics`.
- Preserved hygiene by recording the per-definition gensym position and only
  reusing generated-symbol expansions when the sequence still matches. Cache
  hits advance the counter by the recorded delta.
- Cached only successfully validated raw expansions. Emitted code is still
  preprocessed and type-checked on every hit, so this slice cannot reuse stale
  type/import results and does not claim the higher post-preprocess speedup yet.
- Kept legacy and capability-bearing macros on the general evaluator with
  explicit bypass reasons.
