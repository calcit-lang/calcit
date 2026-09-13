
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-schema-rest-missing
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'type-fail-schema-rest-missing.main/main!) (:mode :native) (:reload-fn 'type-fail-schema-rest-missing.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-schema-rest-missing.main
    %{} 'FileEntry
      :defs $ {}
        'bad-rest $ %{} 'CodeEntry
          :doc "|Expect preprocess error: code has & rest but schema is missing :rest"
          :code $ quote $ defn bad-rest (& xs) (do xs)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'List)
            :args $ []
        'main! $ %{} 'CodeEntry (:doc "|Entry for type-fail schema rest mismatch")
          :code $ quote $ defn main! ()
            do (; calling to force preprocessing of bad-rest) (bad-rest 1 2 3) (println |unreachable)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc "|Namespace for schema rest mismatch")
        :code $ quote $ ns type-fail-schema-rest-missing.main
