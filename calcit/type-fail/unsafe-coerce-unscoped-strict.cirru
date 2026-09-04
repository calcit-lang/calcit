{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-unsafe-coerce-unscoped-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for an unscoped unsafe host assertion.") (:init-fn 'type-fail-unsafe-coerce-unscoped-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-unsafe-coerce-unscoped-strict.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-unsafe-coerce-unscoped-strict.main $ %{} 'FileEntry
      :defs $ {}
        'coerce-host $ %{} 'CodeEntry (:doc "|An unsafe assertion without lexical FFI capability must fail.")
          :code $ quote
            defn coerce-host (value) (unsafe-coerce value 'String)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'Dynamic
        'main! $ %{} 'CodeEntry (:doc "|Entry that makes coerce-host reachable.")
          :code $ quote (defn main! () (coerce-host 1))
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote (defn reload! () &unit)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict unscoped unsafe-coerce fixture.")
        :code $ quote (ns type-fail-unsafe-coerce-unscoped-strict.main)
