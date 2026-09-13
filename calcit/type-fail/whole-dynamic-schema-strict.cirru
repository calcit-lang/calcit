
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-whole-dynamic-schema-strict
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'type-fail-whole-dynamic-schema-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-whole-dynamic-schema-strict.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-whole-dynamic-schema-strict.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|Entry that makes open-identity reachable.")
          :code $ quote $ defn main! () (open-identity 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'open-identity $ %{} 'CodeEntry
          :doc "|Reachable function whose root Dynamic schema must become a structured Fn contract."
          :code $ quote $ defn open-identity (value) value
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict whole-Dynamic schema fixture.")
        :code $ quote $ ns type-fail-whole-dynamic-schema-strict.main
