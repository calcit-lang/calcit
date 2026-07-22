
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |bundle-calcit)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.1)
    :modules $ []
  :entries $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Bundle indentation-based Calcit source files into a runnable snapshot.") (:schema :dynamic)
          :code $ quote
            defn main! () $ let
                CodeEntry $ defstruct CodeEntry (:doc :string) (:code :dynamic) (:examples :list)
                FileEntry $ defstruct FileEntry (:ns CodeEntry) (:defs :map)
                make-code-entry $ fn (form)
                  %{} CodeEntry (:doc |) (:code form)
                    :examples $ []
                parse-source $ fn (path)
                  let
                      parsed $ parse-cirru (read-file path)
                      parsed-data $ &cirru-quote:to-list parsed
                      forms $ map
                        range $ count parsed-data
                        fn (idx) (&cirru-nth parsed idx)
                      ns-form $ first forms
                      ns-data $ &cirru-quote:to-list ns-form
                      ns-op $ nth ns-data 0
                      ns-name $ nth ns-data 1
                      defs $ foldl (rest forms) ({})
                        fn (acc form)
                          let
                              form-data $ &cirru-quote:to-list form
                              op $ nth form-data 0
                              def-name $ nth form-data 1
                            assert (str-spaced "|invalid definition operator" op |in path) (starts-with? op |def)
                            assoc acc def-name $ make-code-entry form
                    assert (str-spaced "|first form must be ns in" path) (= ns-op |ns)
                    [] ns-name $ %{} FileEntry
                      :ns $ make-code-entry ns-form
                      :defs defs
                source-dir $ get-env |BUNDLE_SRC |src
                config-path $ get-env |BUNDLE_CONFIG |deps.cirru
                output-path $ get-env |BUNDLE_OUT |calcit.cirru
                package-data $ parse-cirru-edn (read-file config-path)
                source-paths $ filter (read-dir source-dir true)
                  fn (path) (ends-with? path |.cirru)
                files $ foldl source-paths ({})
                  fn (acc path)
                    let
                        pair $ parse-source path
                        ns-name $ nth pair 0
                        file-entry $ nth pair 1
                      println $ str-spaced |bundling path |as ns-name
                      assoc acc ns-name file-entry
                snapshot $ {}
                  :package $ get package-data :package
                  :configs package-data
                  :files files
              write-file output-path $ format-cirru-edn snapshot
              println $ str-spaced |wrote output-path |with (count files) |namespaces
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () nil
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns app.main)
