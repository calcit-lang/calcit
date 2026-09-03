{} (:about "|Strict preprocessing fixture for nominal method dispatch on a Dynamic receiver.") (:package |type-fail-dynamic-nominal-method-strict) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-dynamic-nominal-method-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-dynamic-nominal-method-strict.main/reload!)
      :modules $ []
  :files $ {}
    |type-fail-dynamic-nominal-method-strict.main $ %{} 'FileEntry
      :defs $ {}
        |consume-dynamic $ %{} 'CodeEntry (:doc "|Dynamic argument must be narrowed before nominal Option/Result method syntax.")
          :code $ quote
            defn consume-dynamic (value)
              value .unwrap-or 0
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Dynamic
        |main! $ %{} 'CodeEntry (:doc "|Entry that makes consume-dynamic reachable.")
          :code $ quote
            defn main! ()
              consume-dynamic $ %some 1
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
      :ns $ %{} 'NsEntry (:doc "|Strict Dynamic nominal-method fixture.")
        :code $ quote (ns type-fail-dynamic-nominal-method-strict.main)
