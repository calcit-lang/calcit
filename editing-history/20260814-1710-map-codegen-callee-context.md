# Empty-map codegen needs callee context

- In JS codegen a bare `CalcitProc::NativeMap` must produce an empty map by
  invoking the runtime constructor.
- The same proc at the head of a list is the constructor callee and must not be
  pre-invoked. Otherwise generated code becomes `map()(...entries)`.
- Keep separate regression assertions for the bare value and an entry-bearing
  map literal, and run the emitted-JS Node suite because unit output alone
  cannot catch a callee/value context mix-up.
