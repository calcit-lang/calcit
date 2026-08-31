# Standalone caps cutover / 独立 caps 切换

- Remove the duplicated caps binary, resolver/store implementation, CLI contract test, `fs4` dependency, and release asset from Calcit core only after standalone `calcit-caps` 0.1.0 passed Calcium Workflow/Respo smoke.
- Keep Calcit's Snapshot/module-loading compatibility and user guidance, but link package-manager ownership and implementation details to `calcit-lang/caps`.
- Treat `:calcit-version` as the runtime/compiler pin. Caps has an independent release version and `setup-calcit` owns the verified CI default through `caps-version`.
- Measured cutover delta: Cargo packages/resolve nodes `150 → 149`, direct dependencies `31 → 30`, binary targets `4 → 3`, and Calcit release binaries in the manifest `3 → 2`.
- Removed four caps-owned source/contract files totaling 3,279 lines and 118,197 bytes, including 27 package-manager-only Rust tests. Their coverage lives in the standalone repository.
- The release `calcit` binary stayed effectively unchanged (`9,515,920 → 9,516,000` bytes, +80 bytes/build variance), confirming that the maintenance/package graph reduction does not alter runtime payload materially.
- Stable `calcit-caps 0.1.0` plus this branch's Calcit `0.13.72` passed latest-main `respo-calcit-workflow` install, five-module verification, toolchain guard, check-only, zero dynamic-method policy, JS codegen, and Node 24 Vite production build.
