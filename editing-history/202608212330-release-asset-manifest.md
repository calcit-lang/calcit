# Versioned release asset manifest

- The release workflow now publishes `calcit-release-manifest.json` alongside
  `calcit`, `caps`, and `cr-wasm`.
- Manifest schema v1 records release version, filename, byte size, and SHA-256
  for each asset. Setup tooling can verify a downloaded binary before adding it
  to `PATH`, without trusting a successful HTTP response alone.
- The generator uses Node standard-library APIs and has a focused fixture test
  so the release file remains deterministic and machine-readable.
