# Route discovered issues to their owning repository

## Summary

- Add an Agent-guide rule requiring reproducible Calcit language, runtime, compiler, and CLI issues to be filed with their maintenance repository.
- Route module and library defects from their resolved local path to the library's own GitHub repository, rather than reporting them to the consuming project or Calcit core by default.
- Document explicit cross-repository issue creation with `gh issue create --repo OWNER/REPO`, target verification with `gh repo view`, evidence requirements, and safe handling of sensitive reproduction data.

## Validation

- Confirmed `gh issue create --help` exposes `--repo [HOST/]OWNER/REPO` for selecting another repository.
- Confirmed the current GitHub CLI authentication can query the non-current `calcit-lang/setup-cr` repository.
- `cargo build --bin cr`
- `cargo test --bin cr agents_docs_default_to_the_version_matched_embedded_guide`
- `./target/debug/cr docs check-md docs/CalcitAgent.md --entry calcit/test.cirru --failures-only`
- `./target/debug/cr docs graph check`
- `git diff --check`
