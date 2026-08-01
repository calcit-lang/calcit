# Editing history

This directory keeps short, task-specific notes that help with nearby follow-up
work. It is not a second changelog: exact prior notes remain available from Git
history after they leave this directory.

## Retention

- Keep individual notes for the current development window (normally the most
  recent 45 days).
- Periodically replace older clusters with a concise topic summary in
  [ARCHIVE.md](ARCHIVE.md), then remove their individual files.
- Each code commit still adds one timestamped note, but it should capture only
  durable context: the reason for the change, compatibility constraints, and
  the relevant verification command.

To recover a removed note, search the repository history, for example:

```bash
git log --all -- editing-history/
```
