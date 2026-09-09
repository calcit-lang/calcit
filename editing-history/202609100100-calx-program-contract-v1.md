# Define the first Calx program compilation unit

- Added the separate `calcit-calx-program/1` edition without changing the existing kernel `/1` and `/2` contracts.
- Derived one deterministic compilation unit from the explicitly selected Snapshot entry, preserving entry name, optional host target, and distinct init/reload roles.
- Kept duplicate lifecycle roles even when they reference one definition so later reachability can deduplicate code without losing lifecycle meaning.
- Added stable errors for missing entry selection and malformed qualified roots, with no fallback to another entry.
- Recorded that calx-vm 0.5.0 still requires `main`; named strict execution is tracked separately before multi-root program lowering.
