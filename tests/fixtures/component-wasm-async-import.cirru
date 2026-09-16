{}
  :about |Async Component import integration fixture.
  :package |component-wasm-async-import
  :entries $ {} $ :default
    {} (:description |Async Component import integration fixture.) (:init-fn 'component-wasm-async-import.main/main!) (:mode :native) (:reload-fn 'component-wasm-async-import.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-wasm-async-import.main
    %{} 'FileEntry
      :defs $ {}
        'call-host-flag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-flag (flag)
            host-flag flag
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'Bool)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |keeps-async-bool-contract)
            :code $ quote $ assert= true true
            :tags $ #{} :wasm
        'call-host-load $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-load (text)
            host-load text
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'String
            :return $ :: 'Result 'String 'String
          :tests $ [] $ %{} 'TestEntry (:name |keeps-async-result-contract)
            :code $ quote $ assert= (%ok |ready) (%ok |ready)
            :tags $ #{} :wasm
        'host-flag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-flag (flag) |host |flag
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true) (:return 'Bool)
            :args $ [] 'Bool
        'host-load $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-load (text) |host |load
          :examples $ []
          :schema $ :: 'Fn $ {} (:async true)
            :args $ [] 'String
            :return $ :: 'Result 'String 'String
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
        :code $ quote $ ns component-wasm-async-import.main
