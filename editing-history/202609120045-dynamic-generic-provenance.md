# Add bounded Dynamic provenance

- Attach one deterministic provenance entry when `Map<K,Dynamic>` flows through typed `get` into `E_ERASED_GENERIC_RELATION`.
- Keep direct Dynamic arguments concise and avoid speculative or unbounded traces.
- Expose operation, definition, path, receiver/output types, flow, and migration guidance in structured diagnostics.
- Keep `CalcitErr` below clippy's large-error threshold while carrying optional provenance.
- Serialize the existing global strict-mode regression with the shared preprocess test guard.
