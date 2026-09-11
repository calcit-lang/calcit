# Add a compact Agent mutation contract

- Add `calcit docs agents --contract`, extracting a bounded safety-critical section from the authoritative embedded Agent guide.
- Print contract version, embedded guide source, stable MD5 content digest, byte length, and line count before the contract body.
- Keep the contract structurally embedded in the full guide so the two views cannot drift, and test its mandatory rules and size budget.
- Update Snapshot guidance and workflow docs to require the compact contract at repository boundaries while reserving `--full` for orientation, digest changes, and task-specific detail.
- Extend the Agent interface smoke test to verify the real CLI contract output and output-size bound.
