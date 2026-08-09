# CI core-test enforcement and follow-up migration

- Added definition-attached core test execution to both pull-request CI and release publish workflows.
- Migrated `sin` and `cos` behavior checks into `calcit-core.cirru` and removed the redundant pure numeric assertions from the legacy math fixture while retaining method-dispatch and cross-target coverage.
- Kept legacy fixtures that validate macros, methods, type/preprocess behavior, typed EDN, JavaScript, and WASM targets.
