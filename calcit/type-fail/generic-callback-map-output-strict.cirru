
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-generic-callback-map-output-strict
  :entries $ {} $ :default
    {} (:description "|泛型回调输出类型的严格负例")
      :init-fn 'type-fail-generic-callback-map-output-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-generic-callback-map-output-strict.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-generic-callback-map-output-strict.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|验证泛型回调结果仍然约束映射值类型")
          :code $ quote $ defn main! ()
            let
                input $ {} $ |a 1
                selected $ filter-map-kv input $ fn (id value) (%:: MapEntryDecision :keep id value)
              &map:assoc selected |b |wrong
              , |done
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc "|泛型回调输出类型的严格负例")
        :code $ quote $ ns type-fail-generic-callback-map-output-strict.main
