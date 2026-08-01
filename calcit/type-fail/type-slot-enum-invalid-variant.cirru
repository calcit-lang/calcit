
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-type-slot-enum-invalid-variant) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-type-slot-enum-invalid-variant.main/main!) (:mode :native) (:reload-fn 'type-fail-type-slot-enum-invalid-variant.main/reload!)
      :modules $ []
      :type-slots $ {} (:dispatch-op |type-fail-type-slot-enum-invalid-variant.main/Action)
  :files $ {}
    |type-fail-type-slot-enum-invalid-variant.main $ %{} :FileEntry
      :defs $ {}
        |Action $ %{} :CodeEntry (:doc "|Enum used for type-slot binding")
          :code $ quote
            defenum Action (:add 'String) (:remove 'String) (:clear)
          :examples $ []
          :schema $ :: 'Dynamic
        |legacy-main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn legacy-main! () $ with-type-slot (:dispatch-op Action) 1 2
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc "|Entry testing enum auto-rewrite via type-slot with invalid variant")
          :code $ quote
            defn main! ()
              takes-action $ :: :nonexistent |hello
              , nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |takes-action $ %{} :CodeEntry (:doc "|Function expecting a type-slot-bound enum value")
          :code $ quote
            defn takes-action (x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] '*dispatch-op
      :ns $ %{} :NsEntry (:doc "|Namespace for type-slot enum invalid variant detection")
        :code $ quote (ns type-fail-type-slot-enum-invalid-variant.main)
