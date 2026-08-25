# Macro query and capability scope follow-up

- Preserve strict Macro contracts in `query schema`, `query context`, and JSON
  output instead of degrading them through ordinary type serialization.
- Serialize an explicit empty `:capabilities` set for every strict/pure Macro;
  legacy signatures retain their compatibility representation.
- Limit a strict macro's capability context to evaluation of its own body.
  Post-preprocessing an emitted expansion may invoke a separate nested macro,
  whose effects must not be charged to the outer emitter.
- Added query regression coverage and validated the boundary with the migrated
  core `fn` macro emitting bodies that contain platform-sensitive legacy macros.
