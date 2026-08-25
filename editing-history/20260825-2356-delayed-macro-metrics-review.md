# Delayed macro metrics review

- Clarified that `cacheHits` and `cacheInvalidations` are reserved report fields until the expansion cache is implemented.
- Made the metrics report unit test operate on local state, avoiding interference with global opt-in metrics state when Rust tests run in parallel.
- Kept production collection behavior unchanged by extracting small state-local recording and serialization helpers.
