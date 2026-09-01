# Strict fixture review / 严格模式测试夹具审查

- Low-level `&struct:nth` fixture calls should include the expected field Tag,
  even when the runtime primitive accepts the shorter form.
- The Tag keeps the runtime stale-layout assertion explicit. This fixture still
  intentionally exercises the low-level primitive on a runtime-extended Struct,
  so it has no nominal type evidence and is not a zero-debt strict-mode sample.
- `W_STRUCT_INDEX_RAW_ACCESS` follows the existing opt-in dynamic/FFI diagnostic
  switch. Ordinary compiler primitive fixtures keep running unchanged, while
  `--warn-dyn-method` and `--strict-types` surface the unsafe source boundary.
