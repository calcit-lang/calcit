
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |component-wasm-async-export
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'component-wasm-async-export.main/main!) (:mode :native) (:reload-fn 'component-wasm-async-export.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-wasm-async-export.main
    %{} 'FileEntry
      :defs $ {}
        'WideResult $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct WideResult (:v00 'Number) (:v01 'Number) (:v02 'Number) (:v03 'Number) (:v04 'Number) (:v05 'Number) (:v06 'Number) (:v07 'Number) (:v08 'Number) (:v09 'Number) (:v10 'Number) (:v11 'Number) (:v12 'Number) (:v13 'Number) (:v14 'Number) (:v15 'Number) (:v16 'Number)
          :examples $ []
          :schema $ quote StructDef
        'echo-result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result (value) value
          :examples $ []
          :schema $ quote $ Fn (Result String String)
            -> $ Result String String
            :async
          :tests $ [] $ %{} 'TestEntry (:name |keeps-typed-result-contract)
            :code $ quote $ assert= (Result :ok |ready) (Result :ok |ready)
            :tags $ #{} :wasm
        'load-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export load-text (text) text
          :examples $ []
          :schema $ quote $ Fn String (-> String) (:async)
          :tests $ [] $ %{} 'TestEntry (:name |keeps-typed-text-contract)
            :code $ quote $ assert= |ready |ready
            :tags $ #{} :wasm
        'load-wide $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export load-wide ()
            WideResult :v00 0 :v01 1 :v02 2 :v03 3 :v04 4 :v05 5 :v06 6 :v07 7 :v08 8 :v09 9 :v10 10 :v11 11 :v12 12 :v13 13 :v14 14 :v15 15 :v16 16
          :examples $ []
          :schema $ quote $ Fn (-> component-wasm-async-export.main/WideResult) (:async)
          :tests $ [] $ %{} 'TestEntry (:name |keeps-wide-struct-contract)
            :code $ quote $ assert= 17 17
            :tags $ #{} :wasm
        'load-wide-sync $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export load-wide-sync ()
            WideResult :v00 0 :v01 1 :v02 2 :v03 3 :v04 4 :v05 5 :v06 6 :v07 7 :v08 8 :v09 9 :v10 10 :v11 11 :v12 12 :v13 13 :v14 14 :v15 15 :v16 16
          :examples $ []
          :schema $ quote $ Fn (-> component-wasm-async-export.main/WideResult)
          :tests $ [] $ %{} 'TestEntry (:name |keeps-sync-wide-struct-contract)
            :code $ quote $ assert= 17 17
            :tags $ #{} :wasm
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm-async-export.main
