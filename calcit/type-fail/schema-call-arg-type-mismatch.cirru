
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
            defn main! () $ let
                text |hello
              assert-type text :string
              ; should generate warning $ treated as error in --check-only
              plus1 text
              , nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |plus1 $ %{} :CodeEntry (:doc "|Schema expects :number, call-site passes :string")
          :code $ quote
            defn plus1 (x) (&+ x 1)
          :examples $ []
          :schema $ :: :fn
            {} (:return :number)
              :args $ [] :number
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc "|Namespace for schema call-site mismatch")
        :code $ quote (ns type-fail-schema-call-arg-type.main)
