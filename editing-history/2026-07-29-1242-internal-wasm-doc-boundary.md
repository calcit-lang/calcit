# Internal WASM documentation boundary

- Removed `cr-wasm` from installation, quick-reference, feature, and public CLI documentation.
- Kept the WASM backend implementation and validation workflow unchanged.
- Moved the detailed backend notes under `scripts/` and reframed them as repository-internal validation documentation.
- Reduced `cr ir` coverage to a short compiler-debugging note instead of presenting it as a normal project workflow.

Validation:

- `bash scripts/check-docs-md.sh`
- `yarn check-all`
- `git diff --check`

