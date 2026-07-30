
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-generic-where-bound) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-generic-where-bound.main/main!) (:mode :native) (:reload-fn 'type-fail-generic-where-bound.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-generic-where-bound.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Entry for generic where-bound mismatch")
          :code $ quote
            defn main! () (require-mappable 1) nil
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
        |require-mappable $ %{} :CodeEntry (:doc "|Requires the argument type to satisfy Mappable")
          :code $ quote
            defn require-mappable (x) x
          :examples $ []
          :schema $ :: :fn
            {} (:return 'T)
              :args $ [] 'T
              :generics $ [] 'T
              :where $ {} ('T :Mappable)
      :ns $ %{} :NsEntry (:doc "|Namespace for generic where-bound mismatch")
        :code $ quote (ns type-fail-generic-where-bound.main)
