# Caps review follow-up

- Corrected the migration hints in the missing-version warning and `caps version get/bump` errors to match the CLI's trailing input-file syntax: `caps version set <version> <deps-file>`.
- Added regression assertions that both errors contain the copy/paste-safe command for a non-default dependency path.
