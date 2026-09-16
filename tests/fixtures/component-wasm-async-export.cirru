{}
  :about |Async Component export integration fixture.
  :package |component-wasm-async-export
  :entries $ {} $ :default
    {} (:description |Async Component export integration fixture.) (:init-fn 'component-wasm-async-export.main/main!) (:mode :native) (:reload-fn 'component-wasm-async-export.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-wasm-async-export.main
    %{} 'FileEntry
      :defs $ {}
        'WideResult $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct WideResult (:v00 'Number) (:v01 'Number) (:v02 'Number) (:v03 'Number) (:v04 'Number) (:v05 'Number) (:v06 'Number) (:v07 'Number) (:v08 'Number) (:v09 'Number) (:v10 'Number) (:v11 'Number) (:v12 'Number) (:v13 'Number) (:v14 'Number) (:v15 'Number) (:v16 'Number)
          :examples $ []
          :schema $ :: 'StructDef
        'echo-result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] $ :: 'Result 'String 'String
            :return $ :: 'Result 'String 'String
          :tests $ [] $ %{} 'TestEntry (:name |keeps-typed-result-contract)
            :code $ quote $ assert= (%ok |ready) (%ok |ready)
            :tags $ #{} :wasm
        'load-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export load-text (text) text
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'String)
            :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |keeps-typed-text-contract)
            :code $ quote $ assert= |ready |ready
            :tags $ #{} :wasm
        'load-wide $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export load-wide ()
            WideResult :v00 0 :v01 1 :v02 2 :v03 3 :v04 4 :v05 5 :v06 6 :v07 7 :v08 8 :v09 9 :v10 10 :v11 11 :v12 12 :v13 13 :v14 14 :v15 15 :v16 16
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'component-wasm-async-export.main/WideResult)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |keeps-wide-struct-contract)
            :code $ quote $ assert= 17 17
            :tags $ #{} :wasm
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm-async-export.main
