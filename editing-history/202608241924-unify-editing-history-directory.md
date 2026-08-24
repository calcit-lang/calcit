# Unify editing history storage

- Moved the remaining timestamped history notes from the repository root and legacy `history/` directory into `editing-history/`.
- Renamed `Agents.md` to the conventional `AGENTS.md` filename.
- Documented `editing-history/` as the single required destination for future per-commit notes.

Keeping one directory prevents validation tools and contributors from repeatedly migrating the same files and makes the historical index predictable.
