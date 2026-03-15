# Structural Editor

> **Deprecated:** As Calcit shifts toward LLM-generated code workflows, command-line operations and type annotations have become more important. The structural editor approach is no longer recommended. Agent interfaces are preferred over direct user interaction.

As demonstrated in [Cirru Project](http://cirru.org/), it's for higher goals of auto-layout code editor. [Calcit Editor](https://github.com/calcit-lang/editor) was incubated in Cirru.

Structural editing makes Calcit a lot different from existing languages, even unique among Lisps.

Calcit Editor uses a `calcit.cirru` as snapshot file, which contains much informations. And it is compiled into `compact.cirru` for evaluating.
Example of a `compact.cirru` file is more readable:

```cirru.no-run
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
    :modules $ []
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (+ 1 2)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! ()
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
        ns app.main $ :require
```

![Calcit Editor](https://cos-sh.tiye.me/cos-up/07eb872cbbe9826474bb1343d8757c39/image.png)

Also [Hovenia Editor](https://github.com/Cirru/hovenia-editor) is another experiment rendering S-Expressions into Canvas.

![Hovernia Editor](https://cos-sh.tiye.me/cos-up/bb9cf76c519fa03d52cb64856761afc6/image.png)
