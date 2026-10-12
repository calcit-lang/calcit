{}
  :issue |https://github.com/calcit-lang/calcit/issues/2002
  :decision |Collection writes inspect the receiver for legacy nominal-enum use, not stored values, keys or callback arguments.
  :reason |Triangles Sketch stores Option values in typed Store and Shape fields; the nullable-migration warning previously rejected those legitimate writes.
  :compatibility |No new diagnostics, fix rules or proofs. Ordinary field and collection contracts remain authoritative; structural Option receivers still warn.
  :verification |Definition-attached tests replay through the existing backend runner; existing Rust rejection tests cover invalid fields, wrong Option payloads and nominal receivers; replay the downstream full check and JS codegen.
