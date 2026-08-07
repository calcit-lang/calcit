
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-schema-call-arg-type) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-schema-call-arg-type.main/main!) (:mode :native) (:reload-fn 'type-fail-schema-call-arg-type.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-schema-call-arg-type.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Entry for type-fail schema call-site arg type mismatch")
          :code $ quote
            defn main! (option) (option .unwrap-or 0)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] (:: 'Option 'String)
        |plus1 $ %{} :CodeEntry (:doc "|Schema expects :number, call-site passes :string")
          :code $ quote
            defn plus1 (x) (&+ x 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc "|Namespace for schema call-site mismatch")
        :code $ quote (ns type-fail-schema-call-arg-type.main)
