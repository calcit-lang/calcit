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
