# Dynamic method analysis

- Added `analyze dynamic-methods` to inventory unresolved method dispatch without mixing in unrelated preprocessing warnings.
- Added stable codes for prefix and postfix dynamic dispatch findings.
- Added project-only default scope, opt-in dependency scope, deterministic de-duplication, JSON and summary output.
- Added `--max` as an incremental CI performance policy for existing typed projects.
- Verified the initial project-only inventory against current Respo and Recollect main branches.
- Respo reports 8 project findings and passes a reviewed `--max 8`; Recollect reports 0 project findings and 4 reachable dependency findings.
- Respo native tests pass 25/25 and Recollect native tests pass 9/9 with the installed branch build.
