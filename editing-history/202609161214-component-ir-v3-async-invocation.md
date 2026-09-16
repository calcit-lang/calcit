# Component IR v3 async invocation

- Component Interface IR v3 adds a required `sync`/`async` invocation field derived from the existing function schema marker instead of introducing a second task type.
- The Snapshot schema writer now preserves EDN boolean and nil literals, so `calcit edit schema` can round-trip `:async true` instead of silently deleting it.
- The synchronous core Component adapter fails closed on explicit async boundaries until the WASI 0.3 native-async adapter is implemented.
- The Calcit fixture and CLI conformance test prove that default Cirru EDN and explicit JSON preserve the same async contract.
