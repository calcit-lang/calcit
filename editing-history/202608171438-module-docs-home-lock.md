# 2026-08-17 14:38 — Serialize documentation HOME access

- Shared the documentation test HOME mutex between `TestHome` and tests that read the guidebook through the process environment.
- Guarded `collect_docs_for_query_uses_guidebook_without_module` so it cannot race with tests that temporarily replace `HOME`.
