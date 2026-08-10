## Release PR workflow

- `main` is protected by a pull-request requirement, including release version
  changes; release commits must be created on a dedicated release branch.
- A release tag is created only after the release PR is green, merged, and
  fetched back onto local `main`.
- Publication verification uses non-interactive `gh pr checks` and `gh run
  list` polling, then confirms matching crates.io and npm versions.
