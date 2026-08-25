# Compile-time macro log capability

- Added the opt-in `:log` macro capability for compile-time `echo`, `println`, and `eprintln` calls.
- Registered procedures remain forbidden host FFI by default; the three platform console aliases are the narrow exception and still require an explicit capability declaration.
- Documented that quoted runtime logging stays pure while logging actually executed during expansion is observable and therefore cache-ineligible.
- Added positive, missing-capability, and unrelated-host-procedure tests for the policy plus schema round-trip coverage for `:log`.
- Migrated the final test helper macros: `inside-eval:`/`inside-js:` declare `:platform-read`, `add-11` declares `:log`, and the hygienic no-log helper remains pure.
- Macro metrics now report zero legacy bypasses: 2393 pure/cache-eligible expansions and 39 explicitly capability-dependent expansions out of 2432 total. Cache hits remain zero until the planned cache implementation lands.
