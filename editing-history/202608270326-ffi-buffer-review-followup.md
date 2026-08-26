# FFI buffer protocol review follow-up

- Corrected the FFI documentation heading hierarchy.
- Made nonzero-status payloads obey the same strict UTF-8 contract as successful EDN responses.
- Added a focused regression test that rejects malformed error bytes rather than replacing them lossily.
- Kept the original 03:16 history timestamp because it records the actual Asia/Shanghai commit time.
