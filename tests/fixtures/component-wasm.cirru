
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |component-wasm
  :entries $ {} $ :default
    {} (:description "|Canonical ABI adapter integration fixture.") (:init-fn 'component-wasm.main/main!) (:mode :native) (:reload-fn 'component-wasm.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-wasm.main
    %{} 'FileEntry
      :defs $ {}
        'add-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export add-one (value) (&+ value 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |adds-one)
            :code $ quote $ assert= 42 (add-one 41)
            :tags $ #{} :wasm
        'call-host-add-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-add-one (value) (host-add-one value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'call-host-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-echo (text) (host-echo text)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'echo-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-text (text) text
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |keeps-text)
            :code $ quote $ assert= |hello (echo-text |hello)
            :tags $ #{} :wasm
        'host-add-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-add-one (value) |host |add-one
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'host-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-echo (text) |host |echo
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
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
        :code $ quote $ ns component-wasm.main
