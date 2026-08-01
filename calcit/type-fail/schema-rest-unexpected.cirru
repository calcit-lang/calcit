
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-schema-rest-unexpected) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-schema-rest-unexpected.main/main!) (:mode :native) (:reload-fn 'type-fail-schema-rest-unexpected.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-schema-rest-unexpected.main $ %{} :FileEntry
      :defs $ {}
        |bad-rest $ %{} :CodeEntry (:doc "|Expect preprocess error: schema has :rest but code has no & param")
          :code $ quote
            defn bad-rest (x) (do x)
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Number)
              :args $ [] 'Number
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema unexpected rest")
          :code $ quote
            defn main! () $ do (; calling to force preprocessing of bad-rest) (bad-rest 1) (println |unreachable)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} :NsEntry (:doc "|Namespace for schema unexpected rest")
        :code $ quote (ns type-fail-schema-rest-unexpected.main)
