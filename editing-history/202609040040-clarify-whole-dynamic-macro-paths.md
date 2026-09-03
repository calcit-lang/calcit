# Clarify whole-Dynamic macro diagnostic paths

- Clarified that `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` also covers programmatically
  supplied macros that reach strict preprocessing without a structured root or
  embedded function contract.
- Distinguished that fallback from the normal Snapshot path: legacy runtime
  `Fn` and whole-Dynamic macro schemas fail earlier during Snapshot loading with
  their definition path.
- Aligned the CLI, static-analysis, upgrade, fixture, and original change notes.
