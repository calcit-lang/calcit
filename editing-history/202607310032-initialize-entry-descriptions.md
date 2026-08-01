# Initialize entry descriptions in repository snapshots

- Installed the current `cr` 0.12.54 binary globally from this checkout.
- Used `cr <snapshot> config set description ''` to canonicalize every complete `calcit/**/*.cirru` snapshot.
- The migration updates 58 snapshots and 60 entry configurations, including named entries, with an explicit empty `:description` field.
- Empty values preserve existing runtime behavior while providing a stable place for future semantic descriptions.
