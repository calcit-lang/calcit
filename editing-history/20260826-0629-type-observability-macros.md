# Type observability macro contracts

- Migrated logging, timing, hygiene, documentation, and debug function-wrapper macros to strict phase-aware signatures.
- Preserve expression result types for `w-log`, `wo-log`, `with-cpu-time`, and `noted` with a generic `T` contract.
- Require callable expressions for `call-w-log` and `call-wo-log`, structural symbol/list inputs for function-definition and gensym helpers, and definition-shaped function expansions for `defn-w-log`/`defn-wo-log`.
- Declare `:platform-read` only for macros that inspect the Calcit running mode while expanding. Runtime logging and `cpu-time` emitted into quoted code do not count as compile-time capabilities.
- Benchmarked the full macro-contract migration against `3a7afb5e`: both release check-only medians remained about 0.15 seconds across 12 warm runs. Strict/cache-eligible expansion coverage rose from 4 to 2379 before this batch; this batch moves another 8 pure expansions to cache eligibility and classifies 5 platform-dependent expansions honestly.
- Verified earlier, structured failures for invalid callable/list/ref macro arguments. No expansion cache exists yet, so this work provides visible diagnostics and cache prerequisites rather than a current runtime speedup.
