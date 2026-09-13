
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-schema-call-arg-type
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'type-fail-schema-call-arg-type.main/main!) (:mode :native) (:reload-fn 'type-fail-schema-call-arg-type.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-schema-call-arg-type.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry
          :doc "|Entry for type-fail schema call-site arg type mismatch"
          :code $ quote $ defn main! ()
            let
                text |hello
              assert-type text 'String
              ; should generate warning $ treated as error in --check-only
              plus1 text
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'plus1 $ %{} 'CodeEntry
          :doc "|Schema expects :number, call-site passes :string"
          :code $ quote $ defn plus1 (x) (&+ x 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
      :ns $ %{} 'NsEntry (:doc "|Namespace for schema call-site mismatch")
        :code $ quote $ ns type-fail-schema-call-arg-type.main
