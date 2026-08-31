# Calx benchmark session adapter

- Added the revision-pinned `calcit::codegen::calx::benchmark_session` boundary for the standalone benchmark harness.
- Require an explicit complete source/schema corpus, reject non-function schemas and non-`defn` source before installation, serialize isolated setup, reject namespace replacement, preprocess once, and keep the immutable program snapshot plus cached Calcit callable private.
- Expose only measured Calx compilation/cache preparation, prepared Calcit and strict Calx execution, boundary conversion, stage timings, and stable program counts.
- Migrated the transitional in-core runner away from `PROGRAM_CODE_DATA`, `ProgramFileData`, `ensure_def_id`, `run_fn`, raw `CalxVM`, and other private setup hooks.
- Added adapter lifecycle, complete-schema, Calcit/Calx differential, and artifact-cache tests; preserved all three existing benchmark report schemas in release-mode smoke runs.
- Updated the bilingual extraction contract, repository boundary, AGENTS guidance, and machine-readable bootstrap state for the confirmed standalone repository.
