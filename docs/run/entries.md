---
title: "Entries"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "entry points"
  - "init-fn"
  - "reload-fn"
  - "run modes"
  - "entry type slots"
id: core/run/entries
related:
  - core/run/upgrade
  - core/run/library-quality
---
# Entries

Every executable configuration lives under `:entries`. Calcit selects the entry named `default` when `--entry` is omitted. Each entry declares its runtime with `:mode :native` or `:mode :js`, so the normal invocation does not need a separate `js` argument.

You may still override the functions explicitly for diagnostics:

```bash
calcit calcit.cirru --init-fn='app.main/main!' --reload-fn='app.main/reload!'
```

Select another entry with:

```bash
calcit calcit.cirru --entry server
```

Here's an example, first lines of a `calcit.cirru` file may look like:

```cirru.no-check
{} (:package |app)
  :version |0.0.1
  :entries $ {}
    :default $ {} (:mode :js) (:init-fn 'app.client/main!) (:reload-fn 'app.client/reload!)
      :description "|Interactive browser client"
      :modules $ [] |respo.calcit/ |lilac/ |recollect/ |memof/ |respo-ui.calcit/ |ws-edn.calcit/ |cumulo-util.calcit/ |respo-message.calcit/ |cumulo-reel.calcit/
      :type-slots $ {} (:dispatch-op |app.schema/ClientOp)
    :server $ {} (:mode :native) (:init-fn 'app.server/main!) (:reload-fn 'app.server/reload!)
      :description "|HTTP and WebSocket server"
      :modules $ [] |lilac/ |recollect/ |memof/ |ws-edn.calcit/ |cumulo-util.calcit/ |cumulo-reel.calcit/ |calcit-wss/ |calcit.std/
      :type-slots $ {} (:dispatch-op |app.schema/ServerOp)
  :files $ {}
```

Bare `calcit calcit.cirru` selects `entries.default` and emits JavaScript because its mode is `:js`. `calcit calcit.cirru --entry server` selects the native server entry. An explicit `js` subcommand remains available as a compatibility/debug override, but project scripts should normally rely on the selected entry's mode.

Use `:description` for a concise, human- and agent-readable explanation of the entry's purpose. It has no runtime effect and may be omitted in existing snapshots. Update it with `calcit config set description "Interactive browser client"` (or add `--entry server` for a named entry).

`:init-fn` and `:reload-fn` are Calcit definition symbols, written as `'app.main/main!` rather than strings. Existing string-valued entries remain compatible on read and are converted on the next canonical snapshot write.

A named entry is a complete configuration, not a partial override. In particular, it does not inherit the default entry's modules or type slots. Bind a slot for each entry that needs it:

```bash
calcit config set mode js
calcit config set-type-slot :dispatch-op app.schema/ClientOp
calcit config set-type-slot --entry server :dispatch-op app.schema/ServerOp
calcit config type-slots
calcit config type-slots --entry server
```

The type-slot environment is selected before preprocessing starts, so the binding applies to the whole reachable call graph. Entry functions do not need a `with-type-slot` wrapper.

Legacy snapshots containing top-level `:configs` still load. Calcit maps that object to `entries.default` with `:mode :native`; the next canonical snapshot write emits only the unified format.
