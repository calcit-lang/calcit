
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-schema-kind-mismatch) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-schema-kind-mismatch.main/main!) (:mode :native) (:reload-fn 'type-fail-schema-kind-mismatch.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-schema-kind-mismatch.main $ %{} :FileEntry
      :defs $ {}
        |bad-kind $ %{} :CodeEntry (:doc "|Expect preprocess error: schema :kind is :macro but code uses defn")
          :code $ quote
            defn bad-kind () 1
          :examples $ []
          :schema $ :: :macro
            {} $ :args ([])
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema kind mismatch")
          :code $ quote
            defn main! () $ do (; call to force preprocessing of bad-kind) (bad-kind) (do true)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc "|Namespace for schema kind mismatch")
        :code $ quote (ns type-fail-schema-kind-mismatch.main)
