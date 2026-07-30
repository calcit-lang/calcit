
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-map-where-bound) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:init-fn |type-fail-map-where-bound.main/main!) (:mode :native) (:reload-fn |type-fail-map-where-bound.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-map-where-bound.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Entry for core map where-bound mismatch")
          :code $ quote
            defn main! () $ map 1 inc
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
      :ns $ %{} :NsEntry (:doc "|Namespace for core map where-bound mismatch")
        :code $ quote (ns type-fail-map-where-bound.main)
