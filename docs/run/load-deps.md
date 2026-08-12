---
title: "Load Dependencies"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "load dependencies"
  - "deps.cirru"
  - "caps"
---
# Load Dependencies

`caps` command is used for downloading dependencies declared in `deps.cirru`. The name "caps" stands for "Calcit Dependencies".

`deps.cirru` declares dependencies, which correspond to repositories on GitHub. Specify a branch or a tag:

```cirru
{}
  :calcit-version |0.9.18
  :dependencies $ {}
    |calcit-lang/memof |0.0.11
    |calcit-lang/lilac |main
```

Run `caps` to recursively resolve and install the graph. Sources are stored by resolved commit under
`~/.config/calcit/modules/.store/`. The project gets a local module view in `.calcit/modules/`, with
`caps-state.cirru`, temporary files, and other generated state kept under `.calcit/`.

SemVer tags are the recommended dependency refs. Branches are supported for development, but every
resolution warns and prints the current commit. Conflicting SemVer tags select the highest version actually
requested by the graph and emit a warning that lists the request sources.

To load modules, use `:modules` configuration in `calcit.cirru` (legacy filename: `compact.cirru`):

```cirru.edn
{}
  :entries $ {}
    :default $ {}
      :modules $ [] |memof/calcit.cirru |lilac/
```

Paths defined in `:modules` first use `<project>/.calcit/modules/` and then the legacy
`~/.config/calcit/modules/` fallback. Existing snapshot `:modules` values do not need to change.

Modules that ends with `/`s are automatically suffixed `calcit.cirru`, and still fall back to `compact.cirru` for compatibility.

### Dependency graph

```bash
caps tree
caps why calcit-lang/memof
caps status
caps verify
```

`tree` displays selected recursive revisions, while `why` prints one shortest path from every root
dependency and all direct version requests. `status` checks project links; `verify` also checks immutable
store commits, local source modifications, and native realization receipts.

The positional input may point to a standalone dependency file. Its parent directory becomes the project
root even when no `calcit.cirru` exists:

```bash
caps /tmp/demo/deps.cirru
```

New projects may keep their package version in `deps.cirru` and manage it with:

```bash
caps version get
caps version set 1.2.3
caps version bump patch
```

During migration, the snapshot `:version` field remains supported by `cr`; `caps version` only manages the
dependency metadata file and does not rewrite the snapshot automatically.

### Outdated

To check outdated modules, run:

```bash
caps outdated
```

To update `deps.cirru` directly without confirmation:

```bash
caps upgrade --all
```

### Module status and local changes

Check that every project link points to the revision selected from `deps.cirru` with:

```bash
caps status
```

Use `caps verify` for the stronger immutable-store and native-receipt checks. Shared
store revisions should not be edited directly; reinstall a damaged revision instead.

`caps reset` rebuilds the current project's `.calcit/modules/` links from the resolved immutable
store entries:

```bash
caps reset
```

It never runs `git reset` inside shared store revisions. A damaged or edited store entry is
reported as an error and should be moved aside before reinstalling.

### CLI Options

```
caps --help
Usage: caps [<input>] [-v] [--pull-branch] [--ci] [--local-debug] [<command>] [<args>]

Top-level command.

Positional Arguments:
  input             input file

Options:
  -v, --verbose     verbose mode
  --pull-branch     deprecated compatibility flag; branch refs resolve remotely
  --ci              CI mode loads shallow repo via HTTPS
  --local-debug     debug mode, clone to test-modules/
  --help, help      display usage information

Commands:
  outdated          show outdated versions
  upgrade           upgrade dependencies
  download          download named packages with org/repo@branch
  add               add dependencies to deps.cirru then install
  remove            remove dependencies from deps.cirru then install
  tree              show the resolved recursive dependency graph
  why               explain why a module is present in the resolved graph
  status            check installed module versions and local modifications
  verify            verify store contents, project links, and native receipts
  reset             rebuild project links from immutable store entries
  version           read or update the package version in deps.cirru
```

- `--pull-branch` is retained for CLI compatibility. Recursive resolution always checks
  the current remote commit for branch refs.
- "ci" does not support `git@` protocol, only `https://` protocol.
