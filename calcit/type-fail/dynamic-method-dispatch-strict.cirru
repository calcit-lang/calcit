
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-dynamic-method-dispatch-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for an unspecialized Dynamic method receiver.") (:init-fn 'type-fail-dynamic-method-dispatch-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-dynamic-method-dispatch-strict.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-dynamic-method-dispatch-strict.main $ %{} 'FileEntry
      :defs $ {}
        'run-open $ %{} 'CodeEntry (:doc "|Dynamic input must be narrowed before ordinary method syntax.")
          :code $ quote
            defn run-open (value)
              value .custom
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        'main! $ %{} 'CodeEntry (:doc "|Entry that makes run-open reachable.")
          :code $ quote
            defn main! () $ run-open 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict dynamic-method-dispatch fixture.")
        :code $ quote (ns type-fail-dynamic-method-dispatch-strict.main)
