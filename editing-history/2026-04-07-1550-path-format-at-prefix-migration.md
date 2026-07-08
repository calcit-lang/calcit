## Path format migration: `@` prefix for display paths

### What changed

All display-level Cirru tree paths now use `@` prefix notation when rendered to user.

### Display format change
- `1.2.3.4` → `@1.2.3.4` (bare path)
- `[1.2.3.4]` → `[@1.2.3.4]` (bracketed path)
- `NodeLocation::Display` now outputs `ns/def [@1.2.3]` instead of `ns/def [1.2.3]`

### Backward compatibility
- `parse_path()` strips leading `@` if present, so CLI input still accepts both `@1.2.3` and `1.2.3`
- Error messages and help text updated to show `@` format

### Files modified

**Core functions:**
- `src/bin/cli_handlers/common.rs`: `format_path()` returns `@1.2.3`; `parse_path()` strips `@` prefix; test assertions updated
- `src/calcit.rs`: `NodeLocation::Display` outputs `[@1.2.3]`; test assertion updated
- `src/bin/cli_handlers/cirru_validator.rs`: local `format_path()` updated
- `src/type_coverage.rs`: `format_cirru_path()` updated
- `src/bin/cli_handlers/chunk_display.rs`: `compare_coords()` strips `@`; test `fragment()` helper uses `@`
- `src/runner/preprocess/mod.rs`: `[&inspect-type]` debug output uses `@`

**Tree/edit handlers:**
- `src/bin/cli_handlers/tree.rs`: inline path formatting in swap handler
- `src/bin/cli_handlers/edit.rs`: error message example
- `src/cli_args.rs`: help text examples (3 locations)

**Documentation:**
- `docs/CalcitAgent.md`: 19 path examples updated
- `docs/run/agent-advanced.md`: 23 path examples updated
- `docs/run/edit-tree.md`: 6 path examples updated
- `docs/Respo-Agent.md` (respo project): 26 path examples updated

### Why
- `@` prefix helps LLMs distinguish path coordinates from other dotted numeric patterns (like version numbers or floats)
- `@` is safe in shell (no escaping needed)
- All display output now consistent with `@` prefix
