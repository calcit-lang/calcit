# caps project module store and recursive dependency graph

## Summary

- Added a commit-addressed global module store under
  `~/.config/calcit/modules/.store/` and a per-project module view under
  `.calcit/modules/`.
- Made runtime module loading prefer the project view while retaining the
  legacy global module directory as a fallback.
- Added recursive `deps.cirru` resolution with deterministic highest-SemVer
  selection, branch warnings, request provenance, and basename collision
  rejection.
- Added `caps tree`, `caps why`, `caps verify`, strict warning handling, and
  `caps version get/set/bump`.
- Isolated native `build.sh` execution in keyed realizations so shared source
  revisions stay clean and incompatible build outputs are not reused.
- Kept generated project state, links, temporary files, and local ignore rules
  inside `.calcit/`.

## Compatibility findings

- Older module metadata may omit `:dependencies`; it is treated as an empty
  graph. A present but malformed `:dependencies` still fails the install.
- Existing projects frequently mix published tags with transitive `main`
  dependencies. Published SemVer tags take precedence over mutable refs and
  produce an explicit warning. Completely incomparable mutable refs still
  require a direct root decision.
- Dense graphs make exhaustive `why` path enumeration impractical. The command
  reports one shortest path per root dependency and all direct version
  requests instead.
- Review changed `caps reset` to rebuild project links instead of applying
  `git reset --hard` to shared state. Existing store hits now reject the wrong
  commit or local changes before they are linked.
- Remote discovery prefers non-interactive HTTPS, falls back to SSH outside CI,
  prints per-module progress, and resolves up to six repositories concurrently.
  Temporary clone names include the repository so equal refs such as `main`
  cannot collide.
- `cr config modules` now uses the same project-first candidate search as the
  runtime and continues from an invalid `calcit.cirru` to a valid legacy
  `compact.cirru` candidate.

## Real-project verification

- Fast-forwarded clean local checkouts of `lilac`, `calcit.std`, `calcit-http`,
  and `guidebook` before validation. `calcit.std` advanced to remote main;
  the other three were already current.
- Installed and checked standalone dependency files for `lilac`, `calcit.std`,
  `calcit-http`, and the ten-module transitive `guidebook` graph in temporary
  project roots sharing a temporary store.
- Confirmed project-view loading with a copied `memof` snapshot and
  `cr query ns lilac.core`.
- Built `calcit.std@0.2.15` as a native realization, reused the cached build on
  the second install, passed `caps verify`, then confirmed a changed ABI build
  key is rejected until a normal `caps` install switches the project link.
- Installed the release `cr` and `caps` binaries into `~/.cargo/bin`, then
  installed modules directly inside updated `lilac` and `guidebook` checkouts.
  `guidebook` resolved ten modules, reported all conflicts/branch refs, and
  passed `status`, `verify`, and `cr query ns respo.core` using project links.

## Repository verification

- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
