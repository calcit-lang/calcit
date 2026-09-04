# Align the generic FFI suggestion

- Remove Dynamic and callbacks from the generic unsupported-type suggestion now
  that each has a dedicated diagnostic and migration path.
- Keep the fallback guidance focused on the remaining non-portable types such as
  Map/Set, Ref, resources, and host objects.
