
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-schema-required-arity
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'type-fail-schema-required-arity.main/main!) (:mode :native) (:reload-fn 'type-fail-schema-required-arity.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-schema-required-arity.main
    %{} 'FileEntry
      :defs $ {}
        'bad-arity $ %{} 'CodeEntry
          :doc "|Expect preprocess error: schema has 2 required args but code has 1"
          :code $ quote $ defn bad-arity (x) (do x)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number 'Number
        'main! $ %{} 'CodeEntry (:doc "|Entry for type-fail schema arity mismatch")
          :code $ quote $ defn main! ()
            do (; calling to force preprocessing of bad-arity) (bad-arity 1) (println |unreachable)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc "|Namespace for schema arity mismatch")
        :code $ quote $ ns type-fail-schema-required-arity.main
