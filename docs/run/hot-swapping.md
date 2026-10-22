---
title: "Hot Swapping"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "hot reload"
  - "hot swapping"
  - "compact-inc"
  - "incremental compile"
entry_for:
  - "calcit edit inc"
---
# Hot Swapping

Since there are two platforms for running Calcit, soutions for hot swapping are implemented differently.

### Rust runtime

Hot swapping is built inside Rust runtime. When you specify `:reload-fn` in `calcit.cirru`:

```cirru
{}
  :entries $ {}
    :default $ {}
      :mode :native
      :init-fn 'app.main/main!
      :reload-fn 'app.main/reload!
```

the interpreter learns that the function `reload!` is to be re-run after hot swapping.

It relies on change event on `.compact-inc.cirru` for detecting code changes. `.compact-inc.cirru` contains informations about which namespace / which definition has changed, and interpreter will patch into internal state of the program. Program caches of current namespace will be replaced, in case that dependants also need changes. Data inside atoms are retained. Calcit encourages usages of mostly pure functions with a few atoms, programs can be safely replaced in many cases.

But also notice that if you have effects like events listening, you have to dispose and re-attach listeners in `reload!`.

#### `.compact-inc.cirru` 中的定义元数据（#2178）

`:changed` 下每个命名空间的 `:added-defs` 与 `:changed-defs` 是「定义名 → 条目」的 map，条目接受两种形式：

```cirru.no-check
{}
  :changed $ {}
    |app.main $ {}
      :changed-defs $ {}
        ; 完整 CodeEntry：与 Snapshot 中的定义条目格式相同
        |add $ %{} 'CodeEntry (:doc |Add two numbers.)
          :code $ quote $ defn add (a b) (&+ a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number) (:args $ [] 'Number 'Number)
        ; 旧格式：只有代码
        |legacy $ quote $ defn legacy () 1
```

- **完整 `CodeEntry`**：使用完整 Snapshot 的同一个加载器解析（含 schema 归一化、`:ffi` 与 JS FFI 校验），应用后代码与 `:schema`、`:doc`、`:examples`、`:ffi`、`:tags` 全部替换为该条目的值，结果与冷启动加载同一条目一致。代码不变、只改元数据的定义也通过 `:changed-defs` 表达；其编译缓存及依赖它的定义都会失效重建。`:tests` 会被解析校验，但与冷启动相同，运行时不保存它。任何条目不合法时，该命名空间不做部分应用。
- **旧格式（裸 quote 代码）**：继续接受，只替换代码。`:changed-defs` 保留内存中已加载的元数据；`:added-defs` 新定义以空元数据开始（`Dynamic` schema，没有 doc/examples/ffi/tags）。watcher 会打印 code-only 条目数量提醒。需要让元数据生效时，发送完整 `CodeEntry`，或重启 watcher。
- watcher 只读取增量文件，不写回完整 Snapshot，两种形式都不会覆盖或丢弃 Snapshot 中保存的元数据。

`calcit edit inc` 写出的 `--added` / `--changed` 条目始终是 Snapshot 中保存的完整 `CodeEntry`。

### JavaScript runtime

While Calcit-js is compiled to JavaScript beforing running, we need tools from JavaScript side for hot swapping, or HMR(hot module replacement). The tool I use most frequestly is [Vite](https://vitejs.dev/), with extra entry file of code:

```js
import { main_$x_ } from "./js-out/app.main.mjs";

main_$x_();

if (import.meta.hot) {
  import.meta.hot.accept("./js-out/app.main.mjs", (main) => {
    main.reload_$x_();
  });
}
```

There's also a `js-out/calcit.build-errors.mjs` file for hot swapping when compilation errors are detected. With this file, you can hook up you own HUD error alert with some extra code, `hud!` is the function for showing the alert:

```cirru.no-check
ns app.main
  :require
    "|./calcit.build-errors" :default build-errors
    "|bottom-tip" :default hud!

defn reload! () $ if (nil? build-errors)
  do (remove-watch! *reel :changes) (clear-cache!)
    add-watch! *reel :changes $ fn (reel prev) (render-app!)
    reset! *reel $ refresh-reel @*reel schema/store updater
    hud! "|ok~" "|Ok"
  hud! "|error" build-errors
```

One tricky thing to hot swap is macros. But you don't need to worry about that in newer versions.

Vite is for browsers. When you want to HMR in Node.js , Webpack provides some mechanism for that, you can refer to the [boilerplate](https://github.com/minimal-xyz/minimal-webpack-esm-hmr). However I'm not using this since Calcit-js switched to `.mjs` files. Node.js can run `.mjs` files without a bundler, it's huge gain in debugging. Plus I want to try more in Calcit-rs when possible since packages from Rust also got good qualitiy, and it's better to have hot swapping in Calcit Rust runtime.
