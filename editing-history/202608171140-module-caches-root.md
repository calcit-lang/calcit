# Module caches root

- Move the global immutable cache from the nested `modules/versions/` proposal to the clearer sibling path `~/.config/calcit/module-caches/`.
- Keep `~/.config/calcit/modules/` reserved for the legacy module root and per-project link vocabulary; `module-caches/AGENTS.md` is the authoritative generated guidance for cached revisions.
- Derive the cache root as a sibling of `CALCIT_MODULES_DIR`, so custom test and CI roots preserve the same `modules/` plus `module-caches/` layout.
