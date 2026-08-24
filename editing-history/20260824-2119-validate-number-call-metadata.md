# Validate specialized number-call metadata

- Guard the native NumberBinary fast path with the stored proc identity, so a
  manually constructed executable call with stale or mismatched metadata uses
  ordinary dispatch rather than a different arithmetic operation.
- Classify the small supported native-proc set before resolving argument types,
  keeping all other calls off the specialization analysis path.
- Rename the function-hint regression to describe its compile-time-only role.
