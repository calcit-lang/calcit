# Pattern-matching macro contracts

- Migrated `tag-match`, `list-match`, `struct-match`, `case`, and their internal expansion helpers to strict, compile-time-pure macro contracts.
- Kept branch bodies as `SyntaxList`; retained `list-match`'s heterogeneous public argument sequence as an intentional `Syntax` rest boundary while typing its internal helper exactly.
- Corrected the pre-existing `tag-match` example expectation from `got:hello` to `hello:got`, matching `str x |:got`.
- Added exact Snapshot assertions for required, optional, rest, capability, and expansion contracts across both `calcit.core` and `calcit.internal`.
- Increased strict macro coverage in `calcit/test.cirru` from 1805 to 1949 of 2432 expansions.
- Verified all repository gates plus Respo `be8141e`, Recollect `6c235d0`, and js-ffi `25869b6`; js-ffi dependencies were reinstalled from its 0.13.44 lockfile before Node/browser contract tests.
