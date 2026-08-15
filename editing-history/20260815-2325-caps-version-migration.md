# Caps project-version migration

- `caps` now warns whenever the selected `deps.cirru` has no `:version`, with the direct `caps ... version set` migration command.
- `caps version get` and `caps version bump` no longer read the legacy `calcit.cirru`/`compact.cirru` `:version`; they error until `deps.cirru :version` is initialized.
- Documentation now presents `deps.cirru :version` as the only project-version source and removes `:version` from the recommended Snapshot example.
- Added a regression test proving a legacy Snapshot version cannot satisfy the `caps version` commands.
