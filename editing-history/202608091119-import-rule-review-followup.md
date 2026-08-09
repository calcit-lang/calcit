# Import rule regression review follow-up

- Changed the malformed-import persistence assertions to compare raw file
  bytes instead of UTF-8 strings, matching the byte-for-byte guarantee and
  keeping the tests independent of snapshot text encoding.

Validation:

- `cargo fmt`
- `cargo clippy --bin cr -- -D warnings`
- `cargo test --bin cr malformed_rule`
- `git diff --check`
