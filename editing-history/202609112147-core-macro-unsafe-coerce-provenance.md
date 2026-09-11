# Preserve core macro provenance for `unsafe-coerce`

- Strict definition tests wrap test bodies in generated project definitions, so the wrapper namespace cannot identify the source of syntax emitted by nested macros.
- `Calcit::Syntax` retains the namespace where that syntax was authored. A trusted `calcit.core` macro expansion can therefore be distinguished from a project macro or direct project source without granting the generated wrapper `:js-ffi`.
- The regression test exercises `calcit.test/is=` through the real definition-test runner. Existing unscoped and lexically scoped fixtures continue to protect the project-source boundary.
