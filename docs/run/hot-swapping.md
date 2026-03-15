# Hot Swapping

Since there are two platforms for running Calcit, soutions for hot swapping are implemented differently.

### Rust runtime

Hot swapping is built inside Rust runtime. When you specity `:reload-fn` in `compact.cirru`:

```cirru
{}
  :configs $ {}
    :init-fn |app.main/main!
    :reload-fn |app.main/reload!
```

the interpreter learns that the function `reload!` is to be re-run after hot swapping.

It relies on change event on `.compact-inc.cirru` for detecting code changes. `.compact-inc.cirru` contains informations about which namespace / which definition has changed, and interpreter will patch into internal state of the program. Program caches of current namespace will be replaced, in case that dependants also need changes. Data inside atoms are retained. Calcit encourages usages of mostly pure functions with a few atoms, programs can be safely replaced in many cases.

But also notice that if you have effects like events listening, you have to dispose and re-attach listeners in `reload!`.

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
    "\"./calcit.build-errors" :default build-errors
    "\"bottom-tip" :default hud!

defn reload! () $ if (nil? build-errors)
  do (remove-watch *reel :changes) (clear-cache!)
    add-watch *reel :changes $ fn (reel prev) (render-app!)
    reset! *reel $ refresh-reel @*reel schema/store updater
    hud! "\"ok~" "\"Ok"
  hud! "\"error" build-errors
```

One tricky thing to hot swap is macros. But you don't need to worry about that in newer versions.

Vite is for browsers. When you want to HMR in Node.js , Webpack provides some mechanism for that, you can refer to the [boilerplate](https://github.com/minimal-xyz/minimal-webpack-esm-hmr). However I'm not using this since Calcit-js switched to `.mjs` files. Node.js can run `.mjs` files without a bundler, it's huge gain in debugging. Plus I want to try more in Calcit-rs when possible since packages from Rust also got good qualitiy, and it's better to have hot swapping in Calcit Rust runtime.
