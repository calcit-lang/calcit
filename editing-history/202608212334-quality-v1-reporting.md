# Honest v1 quality-baseline reporting

- Native quality baseline v1 intentionally lacks the later `unsafeCoerce`
  budget. It still enforces its original eight metrics.
- The report now labels this mode `native-baseline-v1` and normalizes the
  unenforced metric to the current count, preventing a positive delta from
  being mistaken for a rejected regression.
- This supports staged ecosystem upgrades: old released CLIs can keep their
  accepted baseline while a reviewed v2 migration starts enforcing explicit
  host-boundary inventory.
