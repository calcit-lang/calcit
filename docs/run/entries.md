# Entries

By default Calcit reads `:init-fn` and `:reload-fn` inside `compact.cirru` configs. You may also specify functions,

```bash
cr compact.cirru --init-fn='app.main/main!' --reload-fn='app.main/reload!'
```

and even configure `:entries` in `compact.cirru`:

```bash
cr compact.cirru --entry server
```

Here's an example, first lines of a `compact.cirru` file may look like:

```cirru
{} (:package |app)
  :configs $ {} (:init-fn |app.client/main!) (:reload-fn |app.client/reload!) (:version |0.0.1)
    :modules $ [] |respo.calcit/ |lilac/ |recollect/ |memof/ |respo-ui.calcit/ |ws-edn.calcit/ |cumulo-util.calcit/ |respo-message.calcit/ |cumulo-reel.calcit/
  :entries $ {}
    :server $ {} (:init-fn |app.server/main!) (:port 6001) (:reload-fn |app.server/reload!) (:storage-key |calcit.cirru)
      :modules $ [] |lilac/ |recollect/ |memof/ |ws-edn.calcit/ |cumulo-util.calcit/ |cumulo-reel.calcit/ |calcit-wss/ |calcit.std/
  :files $ {}
```

There is base configs attached with `:configs`, with `:init-fn` `:reload-fn` defined, which is the inital entry of the program.

Then there is `:entries` with `:server` entry defined, which is another entry of the program. It has its own `:init-fn` `:reload-fn` and `:modules` options. And to invoke it, you may use `--entry server` option.
