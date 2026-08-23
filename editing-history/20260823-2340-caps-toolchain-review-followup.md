# Follow up: exact JS runtime toolchain guard

- The CI guard now treats a `package.json` without `@calcit/procs` as an error. The no-`package.json` case remains the explicit native-project escape hatch.
- Both `caps upgrade --all` and `caps verify --toolchain` use exact `@calcit/procs` versions. A SemVer caret could otherwise resolve a newer patch and make the upgrade command violate its own gate.
- `caps upgrade --all <path/to/deps.cirru>` now runs Yarn in the directory containing that dependency file, rather than the caller's working directory.
- Toolchain helper tests cover absent, exact, range, and installed-version cases, plus deriving the project root from a non-default `deps.cirru` path.
