{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-raw-primitive-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for a hand-written raw collection primitive.") (:init-fn 'type-fail-raw-primitive-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-raw-primitive-strict.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-raw-primitive-strict.main $ %{} 'FileEntry
      :defs $ {}
        'read-raw $ %{} 'CodeEntry (:doc "|Typed collection code must use an Option-returning public lookup.")
          :code $ quote
            defn read-raw (value) (&get-raw value :x)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] (:: 'Map 'Tag 'Number)
        'main! $ %{} 'CodeEntry (:doc "|Entry that makes read-raw reachable.")
          :code $ quote
            defn main! ()
              read-raw $ {} (:x 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote (defn reload! () &unit)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict raw-primitive fixture.")
        :code $ quote (ns type-fail-raw-primitive-strict.main)
