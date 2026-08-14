# Deprecate cr config version in favor of caps version

- Version authority already lives in `deps.cirru :version`, managed by
  `caps version get/set/bump`; the snapshot `:version` is a migration mirror.
- `cr config version` / `cr config set version` now emit a deprecation notice
  pointing to `caps version`, while keeping the legacy write path working.
- CLI help text for `ConfigVersionCommand` and `ConfigSetCommand` marks the
  `version` key as deprecated.
- Docs updated: `docs/run/project-structure.md` documents `:version` as the
  snapshot mirror with `deps.cirru` authoritative; `docs/run/load-deps.md`
  mentions the deprecated commands.
