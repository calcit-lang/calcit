# Reject retired compact.cirru snapshots

- Reject `compact.cirru` before deserialization instead of silently running it
  as a default or module fallback.
- Point users to the canonical copy/rename, format, and check-only sequence.
- Record 0.13.48 as the final compatibility release for the old filename.
- Remove the obsolete formatter warning because formatting a retired filename
  is no longer an accepted operation.
