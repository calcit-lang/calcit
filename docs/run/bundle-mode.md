# Bundle Mode

Calcit programs are primarily designed to be written using the [calcit-editor](http://github.com/calcit-lang/editor), a structural editor.

You can also try short code snippets in eval mode:

```bash
cr eval "+ 1 2"
# => 3
```

If you prefer to write Calcit code without the calcit-editor, that's possible too. See the example in [minimal-calcit](https://github.com/calcit-lang/minimal-calcit).

With the `bundle_calcit` command, Calcit code can be written using indentation-based syntax. This means you don't need to match parentheses as in Clojure, but you must pay close attention to indentation.

First, bundle your files into a `compact.cirru` file. Then, use the `cr` command to run it. A `.compact-inc.cirru` file will also be generated to enable hot code swapping. Simply launch these two watchers in parallel.
