
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ nominal-contains-results
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'nominal-contains-results $ %{} 'CodeEntry
          :doc "|Public contains? on nominal Enum receivers in project code, checked by the default proof audit."
          :code $ quote $ defn nominal-contains-results ()
            let
                value $ Option :some 1
              []
                contains? (Option :some 1) 0
                contains? value 1.5
                contains? value 2
                contains? value -1
                contains? (Option :none) 0
                contains? (Result :ok 1) 0
                contains? (Result :ok 1) 1
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'List 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |option-and-result-index-range)
            :code $ quote $ assert= ([] true true false false true true true) (nominal-contains-results)
            :tags $ #{} :public-contains-nominal :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
