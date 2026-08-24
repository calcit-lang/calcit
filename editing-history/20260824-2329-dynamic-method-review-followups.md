# Dynamic method analysis review follow-ups

- Documented the accepted `text` output alias and advertised `dynamic-methods` in parent CLI help.
- Preserved every imprecisely located generated warning while still de-duplicating warnings with exact Snapshot coordinates.
- Added a module-backed CLI fixture proving project-only and reachable dependency scopes end to end.
- Strengthened the policy smoke test to require individual finding rows.
- Updated the upgrade CI template, checklist, and troubleshooting matrix to use the focused dynamic-method policy.
