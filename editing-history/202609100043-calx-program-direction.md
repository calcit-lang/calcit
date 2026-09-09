# Define the Calx program-backend direction

- Reframed Calx from a manually selected numeric-kernel experiment into a staged backend for the statically analyzable Calcit language core.
- Kept parsing, type analysis, macro expansion, lifecycle, and platform FFI in Calcit/the embedding host; external capabilities cross only exact typed import boundaries.
- Added a checked ownership and rollout inventory for every type and syntax variant, plus a review gate for all 234 current built-in proc variants.
- Preserved the kernel ABI as a compatibility foundation and explicitly excluded generic Nil, Dynamic, runtime eval, untyped FFI, and silent mixed-mode fallback.
- Added separate English and Chinese program-contract documentation and retained the existing executable kernel documentation as a narrower compatibility reference.
