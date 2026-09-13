
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-dynamic-nominal-method-strict
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'type-fail-dynamic-nominal-method-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-dynamic-nominal-method-strict.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-dynamic-nominal-method-strict.main
    %{} 'FileEntry
      :defs $ {}
        'consume-dynamic $ %{} 'CodeEntry
          :doc "|Dynamic argument must be narrowed before nominal Option/Result method syntax."
          :code $ quote $ defn consume-dynamic (value) (value .unwrap-or 0)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
        'main! $ %{} 'CodeEntry
          :doc "|Entry that makes consume-dynamic reachable."
          :code $ quote $ defn main! ()
            consume-dynamic $ %some 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry
        :doc "|Strict Dynamic nominal-method fixture."
        :code $ quote $ ns type-fail-dynamic-nominal-method-strict.main
