
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-schema-kind-mismatch
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'type-fail-schema-kind-mismatch.main/main!
      :mode :native
      :reload-fn 'type-fail-schema-kind-mismatch.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-schema-kind-mismatch.main
    %{} 'FileEntry
      :defs $ {}
        'bad-kind $ %{} 'CodeEntry
          :doc "|Formatter-normalized function schema fixture."
          :code $ quote $ defn bad-kind () 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'main! $ %{} 'CodeEntry
          :doc "|Entry for type-fail schema kind mismatch"
          :code $ quote $ defn main! ()
            do (; call to force preprocessing of bad-kind) (bad-kind) (do true)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
      :ns $ %{} 'NsEntry
        :doc "|Namespace for schema kind mismatch"
        :code $ quote $ ns type-fail-schema-kind-mismatch.main
