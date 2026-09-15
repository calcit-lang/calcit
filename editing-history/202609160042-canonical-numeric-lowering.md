# Canonical numeric lowering

- Component adapters preserve all ten numeric refinements as Canonical ABI `i32`, `i64`, `f32`, or `f64` values while keeping Calcit runtime values as `Number`/f64.
- Import and export paths reject narrow-integer overflow, invalid signedness, fractional integers, unsafe 64-bit integers, and inexact Float32 demotion instead of truncating, wrapping, or rounding.
- The recursive memory walker uses canonical scalar sizes and alignments inside List, Option, Result, Struct, and Enum values; variant flat joins now include f32.
- The real Component core-module fixture exposes one Struct containing every numeric refinement and exercises direct export, host import, return areas, boundary values, and traps from Node.js.
- This preview replaces the previously unsupported numeric boundary directly; no legacy numeric ABI adapter is retained.
- Verification: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `yarn compile`, `yarn check-agent-interface`, and `yarn check-all`.
