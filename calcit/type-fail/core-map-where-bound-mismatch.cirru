
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-map-where-bound
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'type-fail-map-where-bound.main/main!
      :mode :native
      :reload-fn 'type-fail-map-where-bound.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-map-where-bound.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry
          :doc "|Entry for core map where-bound mismatch"
          :code $ quote $ defn main! () (map 1 inc)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
      :ns $ %{} 'NsEntry
        :doc "|Namespace for core map where-bound mismatch"
        :code $ quote $ ns type-fail-map-where-bound.main
