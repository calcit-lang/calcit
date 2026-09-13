
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-update-collection-contract
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'type-fail-update-collection-contract.main/main!
      :mode :native
      :reload-fn 'type-fail-update-collection-contract.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-update-collection-contract.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry
          :doc "|Reject mismatched List/Map update keys and callbacks"
          :code $ quote $ defn main! ()
            update ([] 1 2) :bad inc
            update (&{} :a 1) :a string-id
            update ([] 1 2) 0 $ fn (x) (str x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'string-id $ %{} 'CodeEntry
          :doc "|Deliberately incompatible updater"
          :code $ quote $ defn string-id (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
      :ns $ %{} 'NsEntry
        :doc "|Namespace for update collection contract mismatch"
        :code $ quote $ ns type-fail-update-collection-contract.main
