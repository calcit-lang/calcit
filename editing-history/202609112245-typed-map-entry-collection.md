# Add typed map-entry collection

- Add `map-list-kv` with the contract `Map<K,V>`, `Fn(K,V)->U` to `List<U>`.
- Preserve the legacy pair-taking `.map-list` and legacy `map-kv` behavior.
- Cover runtime behavior, strict positive and negative type checks, and document ordering explicitly.
