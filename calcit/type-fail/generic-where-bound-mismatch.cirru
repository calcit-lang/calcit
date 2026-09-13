
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-generic-where-bound
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'type-fail-generic-where-bound.main/main!
      :mode :native
      :reload-fn 'type-fail-generic-where-bound.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-generic-where-bound.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry
          :doc "|Entry for generic where-bound mismatch"
          :code $ quote $ defn main! () (require-mappable 1) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'require-mappable $ %{} 'CodeEntry
          :doc "|Requires the argument type to satisfy Mappable"
          :code $ quote $ defn require-mappable (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Mappable
      :ns $ %{} 'NsEntry
        :doc "|Namespace for generic where-bound mismatch"
        :code $ quote $ ns type-fail-generic-where-bound.main
