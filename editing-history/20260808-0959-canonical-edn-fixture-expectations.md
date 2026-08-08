# Canonical EDN Fixture Expectations

修复 PR CI 的 Cirru integration suite：Snapshot canonicalization 后，部分旧 golden
expectation 仍把 enum/struct discriminator 或 nominal name 写为 tags。

- `format-cirru-edn` 对 enum discriminator、typed enum name/variant 和 struct nominal name
  输出 symbol quote；相应 fixture 改为 canonical 格式。
- `&get-def-schema` 返回 `:Fn` tag discriminator；definition metadata fixture 同步断言该值。

验证：`cargo test --bin cr cr_cirru_suite_tests::cirru_test_suite`。