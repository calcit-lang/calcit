# Macro expansion metrics baseline

- Added opt-in `--macro-metrics` JSON-on-stderr instrumentation for expansion
  counts, exclusive evaluator/post-preprocess time, general evaluator fallback,
  and cache outcome reasons.
- Nested macro phases pause parent timers so aggregate costs are not inflated by
  recursive inclusive timing.
- Release-mode baselines on the Calcit test snapshot and latest Respo identify
  post-expansion processing and common structural macros as the next targets.
- This is the instrumentation checkpoint for issue #436; it deliberately does
  not add the typed Macro IR or pure expansion cache yet.
