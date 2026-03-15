## Indentation Syntax in the MCP Server

When using the MCP (Model Context Protocol) server, each documentation or code file is exposed as a key (the filename) with its content as the value. This means you can programmatically fetch, update, or analyze any file as a single value, making it easy for tools and agents to process Calcit code and documentation. Indentation-based syntax is preserved in the file content, so structure and meaning are maintained when accessed through the MCP server.

# Indentation-based Syntax

Calcit was designed based on tools from [Cirru Project](http://cirru.org/), which means, it's suggested to be programming with [Calcit Editor](https://github.com/calcit-lang/editor/). It will emit a file `compact.cirru` containing data of the code. And the data is still written in [Cirru EDN](https://github.com/Cirru/cirru-edn#syntax), Clojure EDN but based on Cirru Syntax.

For Cirru Syntax, read <http://text.cirru.org/>, and you may find a live demo at <http://repo.cirru.org/parser.coffee/>. A normal snippet looks like: this

```cirru.no-check
defn fibo (x)
  if (< x 2) 1
    + (fibo $ - x 1) (fibo $ - x 2)
```

But also, you can write in files and bundle `compact.cirru` with a command line `bundle_calcit`.

To run `compact.cirru`, internally it's doing steps:

1. parse Cirru Syntax into vectors,
2. turn Cirru vectors into Cirru EDN, which is a piece of data,
3. build program data with quoted Calcit data(very similar to EDN, but got more data types),
4. interpret program data.

Since Cirru itself is very generic lispy syntax, it may represent various semantics, both for code and for data.

Inside `compact.cirru`, code is like quoted data inside `(quote ...)` blocks:

```cirru.no-run
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)

  :entries $ {}
    :prime $ {} (:init-fn |app.main/try-prime) (:reload-fn |app.main/try-prime)
      :modules $ []

  :files $ {}
    |app.main $ {}
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns app.main $ :require
      :defs $ {}
        |fibo $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn fibo (x)
              if (< x 2) (, 1)
                + (fibo $ - x 1) (fibo $ - x 2)
```

Notice that in Cirru `|s` prepresents a string `"s"`, it's always trying to use prefixed syntax. `"\"s"` also means `|s`, and double quote marks existed for providing context of "character escaping".

### MCP Tool

The tool `parse_cirru_to_json` can be used to parse Cirru syntax into JSON format, which is useful for understanding how Cirru syntax is structured.

You can generate Cirru from JSON using `format_json_to_cirru` vice versa.

### More about Cirru

A review of Cirru in Chinese:

<iframe width="720" height="405" frameborder="0" src="https://www.ixigua.com/iframe/7092697111279763975?autoplay=0" referrerpolicy="unsafe-url" allowfullscreen></iframe>
