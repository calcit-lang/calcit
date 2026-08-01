
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |sync-calcit) (:version |0.0.1)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :modules $ [] |bisection-key/
      :type-slots $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Synchronize a compact snapshot into a detailed calcit.cirru snapshot.")
          :code $ quote
            defn main! () $ let
                Leaf $ defstruct Leaf (:at 'Number) (:by 'String) (:text 'String)
                Expr $ defstruct Expr (:at 'Number) (:by 'String) (:data 'Map)
                CodeEntry $ defstruct CodeEntry (:code 'Dynamic) (:doc 'String) (:examples 'List)
                NsEntry $ defstruct NsEntry (:code 'Dynamic) (:doc 'String)
                FileEntry $ defstruct FileEntry (:defs 'Map) (:ns 'Dynamic)
                code->detail $ fn (node now recur-fn)
                  let
                      data $ &cirru-quote:to-list node
                    if (string? data)
                      %{} Leaf (:at now) (:by |sync) (:text data)
                      %{} Expr (:at now) (:by |sync)
                        :data $ foldl
                          range $ count data
                          {}
                          fn (acc idx)
                            bisection-key.util/assoc-append acc $ recur-fn (&cirru-nth node idx) now recur-fn
                detail->data $ fn (node recur-fn)
                  let
                      text $ get node :text
                    if (some? text) text $ let
                        data $ get node :data
                      map
                        range $ count data
                        fn (idx)
                          recur-fn (bisection-key.util/val-nth data idx) recur-fn
                examples->detail $ fn (examples now)
                  map
                    if (some? examples) examples $ []
                    fn (example) (code->detail example now code->detail)
                sync-entry $ fn (old incoming now)
                  let
                      next-code $ get incoming :code
                      next-doc $ get incoming :doc
                      next-examples $ examples->detail (get incoming :examples) now
                    if (some? old)
                      let
                          old-code $ get old :code
                          code-changed? $ not= (detail->data old-code detail->data) (&cirru-quote:to-list next-code)
                          base $ assoc old :doc next-doc
                          base-with-examples $ if (contains? old :examples) (assoc base :examples next-examples) base
                        assoc base-with-examples :code $ if code-changed? (code->detail next-code now code->detail) old-code
                      %{} CodeEntry
                        :code $ code->detail next-code now code->detail
                        :doc next-doc
                        :examples next-examples
                sync-ns $ fn (old incoming now)
                  let
                      next-code $ get incoming :code
                      next-doc $ get incoming :doc
                    if (some? old)
                      let
                          old-code $ get old :code
                          code-changed? $ not= (detail->data old-code detail->data) (&cirru-quote:to-list next-code)
                        assoc (assoc old :doc next-doc) :code $ if code-changed? (code->detail next-code now code->detail) old-code
                      %{} NsEntry
                        :code $ code->detail next-code now code->detail
                        :doc next-doc
                sync-file $ fn (old incoming now)
                  let
                      old-defs $ if (some? old) (get old :defs) ({})
                      next-defs $ foldl (get incoming :defs) ({})
                        fn (acc pair)
                          let[] (name entry) pair $ assoc acc name
                            sync-entry (get old-defs name) entry now
                      next-ns $ sync-ns
                        if (some? old) (get old :ns) nil
                        get incoming :ns
                        , now
                    if (some? old)
                      assoc (assoc old :defs next-defs) :ns next-ns
                      %{} FileEntry (:defs next-defs) (:ns next-ns)
                compact-path $ get-env |SYNC_COMPACT |compact.cirru
                calcit-path $ get-env |SYNC_CALCIT |calcit.cirru
                compact $ parse-cirru-edn (read-file compact-path)
                detailed $ parse-cirru-edn (read-file calcit-path)
                now $ unix-time-ms
                compact-files $ get compact :files
                detailed-files $ get detailed :files
                synced-files $ foldl compact-files ({})
                  fn (acc pair)
                    let[] (name file) pair $ if (ends-with? name |.$meta) acc
                      assoc acc name $ sync-file (get detailed-files name) file now
                all-files $ foldl detailed-files synced-files
                  fn (acc pair)
                    let[] (name file) pair $ if
                      or (ends-with? name |.$meta) (contains? compact-files name)
                      , acc
                        assoc acc name $ assoc file :defs ({})
                result $ assoc
                  assoc
                    assoc
                      assoc detailed :package $ get compact :package
                      , :version $ get compact :version
                    , :entries $ get compact :entries
                  , :files all-files
              write-file calcit-path $ format-cirru-edn result
              println $ str-spaced |synced compact-path |to calcit-path
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns app.main $ :require
            bisection-key.util :refer $ assoc-append val-nth
