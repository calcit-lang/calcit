# Keep the release path buildable on stable Rust

- Replaced three unstable `if let` match guards with equivalent nested stable
  control flow.
- Kept enum and function-schema resolution order unchanged.
- Changed the regular CI toolchain from nightly to stable so it matches the
  publish workflow and catches stable-channel build blockers before release.
