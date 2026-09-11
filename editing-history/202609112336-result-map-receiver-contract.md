# Bind Result map callbacks from the receiver

- Add a checked `result:map` contract that preserves `Result<T,E>`, `Fn(T)->U`, and `Result<U,E>` relations after receiver preprocessing.
- Prevent reachable `FsPath` core methods from emitting callback argument mismatch warnings in strict consumer checks.
- Cover both the contract shape and a strict `fs:path` end-to-end preprocessing regression.
