# Calcit command migration

- Make `calcit` the only compiled primary CLI binary; do not keep a second Rust `cr` artifact.
- Keep old GitHub Actions workflows compatible in `setup-calcit` by creating a lightweight `cr` link beside the installed `calcit` binary.
- Move repository scripts, CI, user-facing diagnostics, and documentation to canonical `calcit` command examples.
- Refresh the agent-interface weak-type schema assertion from v2 to v3 so the renamed CLI continues to exercise the current machine protocol.
