{} (:about "|Strict preprocessing fixture for a reachable whole-Dynamic function schema.") (:package |type-fail-whole-dynamic-schema-strict) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-whole-dynamic-schema-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-whole-dynamic-schema-strict.main/reload!)
      :modules $ []
  :files $ {}
    |type-fail-whole-dynamic-schema-strict.main $ %{} 'FileEntry
      :defs $ {}
        |open-identity $ %{} 'CodeEntry (:doc "|Reachable function whose root Dynamic schema must become a structured Fn contract.")
          :code $ quote
            defn open-identity (value) value
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc "|Entry that makes open-identity reachable.")
          :code $ quote
            defn main! ()
              open-identity 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
        |reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict whole-Dynamic schema fixture.")
        :code $ quote (ns type-fail-whole-dynamic-schema-strict.main)
