{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-unsafe-coerce-scoped-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for a lexically scoped unsafe host assertion.") (:init-fn 'type-fail-unsafe-coerce-scoped-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-unsafe-coerce-scoped-strict.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-unsafe-coerce-scoped-strict.main $ %{} 'FileEntry
      :defs $ {}
        'coerce-host $ %{} 'CodeEntry (:doc "|The same assertion is allowed only in this marked adapter body.")
          :code $ quote
            defn coerce-host (value) (unsafe-coerce value 'String)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Dynamic
              :features $ #{} :js-ffi
              :return 'String
        'main! $ %{} 'CodeEntry (:doc "|Typed callers do not inherit or need the adapter capability.")
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
      :ns $ %{} 'NsEntry (:doc "|Strict scoped unsafe-coerce fixture.")
        :code $ quote (ns type-fail-unsafe-coerce-scoped-strict.main)
