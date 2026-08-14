# Bare empty-map JS codegen

- A bare Cirru `{}` survives preprocessing as `CalcitProc::NativeMap`, rather
  than as an already-created map value.
- JS code generation must invoke that runtime constructor when it is used as a
  value. Emitting the escaped proc name alone produced `$clt._$M_`, an alias
  that the runtime does not export.
- Keep a focused codegen test for the emitted `$clt._$n__$M_()` call. This
  protects empty map values in typed struct constructors and ordinary calls
  such as `merge {} other`.
