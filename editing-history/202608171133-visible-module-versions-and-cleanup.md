# Visible module versions and cache cleanup

- Store immutable module revisions under `~/.config/calcit/modules/versions/` instead of a hidden `.store/` directory, retaining the owner, repository, and resolved commit in the path.
- Write `metadata.txt` beside each cached revision so the requested reference remains inspectable and global cleanup can select the highest SemVer release without a network request.
- Add `caps clean` as an explicit global cleanup command. It preserves one newest revision per module (SemVer takes precedence; materialization time breaks non-SemVer ties) and removes older revision directories.
- Rewrite `~/.config/calcit/modules/AGENTS.md` on each `caps` invocation. The guide treats `versions/` as immutable cache data and directs dependency changes through a repository commit, new tag, dependency update, and reinstall.
