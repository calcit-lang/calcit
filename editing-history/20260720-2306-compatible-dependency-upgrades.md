## Summary

Upgrade compatible Rust dependencies, including `cirru_edn`, without changing the existing `strum` line.

## Notes

- Bump `cirru_edn` to `0.7.8` in normal and build dependencies.
- Upgrade other compatible crates such as `argh`, `colored`, `ctrlc`, `regex`, `rpds`, `semver`, `serde`, `serde_json`, and `ureq`.
- Refresh `Cargo.lock` after version changes so the workspace resolves to the new compatible releases.
