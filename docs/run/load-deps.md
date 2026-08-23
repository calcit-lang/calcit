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
  :version |0.1.0
  :calcit-version |0.9.18
  :dependencies $ {}
    |calcit-lang/memof |0.0.11
    |calcit-lang/lilac |main
  :dev-dependencies $ {}
    |calcit-lang/calcit-test |0.1.0
```

Use `:dependencies` for modules required by consumers at runtime or compile time. Use
`:dev-dependencies` only for the current project's tests, examples, documentation checks, and
maintenance tasks. `caps` installs both groups for the root project, but recursive resolution reads
only each dependency's `:dependencies`; a dependency's own `:dev-dependencies` never leaks into the
consumer. Conflicting references for the same repository in both root groups are rejected.

Run `caps` to recursively resolve and install the graph. Sources are stored by resolved commit under
`~/.config/calcit/module-caches/`. The project gets a local module view in `.calcit/modules/`, with
`caps-state.cirru`, temporary files, and other generated state kept under `.calcit/`.
Package-style module paths resolve only through that project view. Explicit relative paths still resolve
from the snapshot directory, and absolute paths resolve directly.

SemVer tags are the recommended dependency refs. Branches are supported for development, but every
resolution warns and prints the current commit. Conflicting SemVer tags select the highest version actually
requested by the graph and emit a warning that lists the request sources.

Manage development dependencies explicitly:

```bash
caps add --dev calcit-lang/calcit-test@0.1.0
caps remove --dev calcit-lang/calcit-test
```

`caps outdated` and `caps upgrade --all` inspect both root groups and preserve each declaration's
group.

To load modules, use `:modules` configuration in `calcit.cirru` (legacy filename: `compact.cirru`):

```cirru.edn
{}
  :entries $ {}
    :default $ {}
      :modules $ [] |memof/calcit.cirru |lilac/
```

Paths defined in `:modules` are loaded only through `<project>/.calcit/modules/`. `caps` creates
this project view as links to the matching immutable revisions in the global store; it is therefore
an error to run a project before its dependencies have been installed with `caps`.

Modules that end with `/` are automatically suffixed with `calcit.cirru`, and still fall back to `compact.cirru` for compatibility.

### Dependency graph

```bash
caps tree
caps why calcit-lang/memof
caps status
caps verify
caps clean
```

`tree` displays selected recursive revisions, while `why` prints one shortest path from every root
dependency and all direct version requests. `status` checks project links; `verify` also checks immutable
store commits, local source modifications, and native realization receipts. `clean` is global: it retains
the newest materialized revision of each module under `module-caches/`, plus any revision still linked by
a registered project view, and removes the remaining older ones.

### Auditing dependency intent

Use the dependency manifest together with the graph, rather than treating installation as proof that an
application entry uses a module:

```bash
# Inspect the two root groups in deps.cirru, then resolve the graph and one module's path.
caps tree
caps why calcit-lang/memof

# Inspect the module list for each relevant executable entry.
calcit calcit.cirru config modules
calcit calcit.cirru config modules --entry test

# Validate the reachable paths for those entries.
calcit calcit.cirru --check-only
calcit calcit.cirru --entry test --check-only
```

`caps tree` and `caps why` explain resolver reachability: why a repository is installed and which
transitive dependency requests selected its revision. They do not inspect Calcit calls, macros, test
metadata, or Markdown snippets. They also currently merge root `:dependencies` and
`:dev-dependencies` in their display, so use `deps.cirru` as the authority for a root module's declared
group. An installed module is therefore not automatically a runtime dependency: it can be a development
module, a module configured only for another entry, or a module retained for a documentation check.

Named entries do not inherit the default entry's modules. Audit each entry that CI or a release supports.
For Markdown code, `calcit docs check-md` defaults to modules from the default entry; use an explicit
`--entry <snapshot>` and repeat `--dep <module-path>` for additional documentation-only modules. These
checks provide static evidence for selected paths, not a guarantee about dynamic loading or external
consumer usage.

The positional input may point to a standalone dependency file. Its parent directory becomes the project
root even when no `calcit.cirru` exists:

```bash
caps ./demo/deps.cirru
```

New projects may keep their package version in `deps.cirru` and manage it with:

```bash
caps version get
caps version set 1.2.3
caps version bump patch
```

The project version belongs in `deps.cirru :version`. If it is missing, `caps` invocations that read the file print a
migration warning; initialize it with `caps version set <version>`. `caps version get` and `caps version bump` fail when the
field is missing, even if a legacy `calcit.cirru :version` field exists. `calcit config version` / `calcit config set version`
are deprecated — use `caps version` instead.

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

Use `caps verify` for the stronger immutable-store and native-receipt checks. For JavaScript projects,
run `caps verify --toolchain` after `yarn install` to require that `:calcit-version`, `caps`, the
`package.json` `@calcit/procs` version anchor, and the installed runtime package agree. Shared
store revisions should not be edited directly; reinstall a damaged revision instead. `caps` overwrites
`~/.config/calcit/module-caches/AGENTS.md` on every invocation with the workflow for changing a dependency:
use its Git repository, commit the change, publish a new tag, then update `deps.cirru` and reinstall.

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
Usage: caps [<input>] [-v] [--pull-branch] [--ci] [--local-debug] [--strict] [<command>] [<args>]

Top-level command.

Positional Arguments:
  input             input file

Options:
  -v, --verbose     verbose mode
  --pull-branch     deprecated compatibility flag; branch refs resolve remotely
  --ci              CI mode loads shallow repo via HTTPS
  --local-debug     debug mode, clone to test-modules/
  --strict          reject branch and version-conflict warnings
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
  clean             remove old immutable revisions from the global module cache
  version           read or update the package version in deps.cirru
```

- `--pull-branch` is retained for CLI compatibility. Recursive resolution always checks
  the current remote commit for branch refs.
- "ci" does not support `git@` protocol, only `https://` protocol.
