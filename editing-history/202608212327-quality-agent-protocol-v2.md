# Quality agent protocol v2 assertion

- The Agent CLI smoke check now expects `analyze.quality` protocol v2 and
  asserts that the newly versioned `unsafeCoerce` metric is present.
- This keeps the development-process check aligned with the public JSON
  contract, preventing a future accidental downgrade or silent omission of the
  new boundary budget.
