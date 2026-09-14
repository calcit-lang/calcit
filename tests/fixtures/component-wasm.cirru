
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
        'bool-not $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export bool-not (flag) (not flag)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |negates-true)
              :code $ quote $ assert= false (bool-not true)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |negates-false)
              :code $ quote $ assert= true (bool-not false)
              :tags $ #{} :wasm
        'call-host-add-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-add-one (value) (host-add-one value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
        'call-host-bool-not $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-bool-not (flag) (host-bool-not flag)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Bool
        'call-host-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-buffer (value) (host-buffer value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Buffer)
            :args $ [] 'Buffer
        'call-host-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-echo (text) (host-echo text)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'choose-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export choose-buffer (flag yes no) (if flag yes no)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Buffer)
            :args $ [] 'Bool 'Buffer 'Buffer
          :tests $ [] $ %{} 'TestEntry (:name |chooses-binary-branch)
            :code $ quote $ assert= (&buffer 2 0)
              choose-buffer false (&buffer 1 255) (&buffer 2 0)
            :tags $ #{} :wasm
        'choose-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export choose-number (flag yes no) (if flag yes no)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool 'Number 'Number
          :tests $ []
            %{} 'TestEntry (:name |chooses-yes)
              :code $ quote $ assert= 3 (choose-number true 3 4)
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |chooses-no)
              :code $ quote $ assert= 4 (choose-number false 3 4)
              :tags $ #{} :wasm
        'echo-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-buffer (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Buffer)
            :args $ [] 'Buffer
          :tests $ [] $ %{} 'TestEntry (:name |keeps-binary-bytes)
            :code $ quote $ assert= (&buffer 0 255 17)
              echo-buffer $ &buffer 0 255 17
            :tags $ #{} :wasm
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
        'host-bool-not $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-bool-not (flag) |host |bool-not
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Bool
        'host-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-buffer (value) |host |buffer
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Buffer)
            :args $ [] 'Buffer
        'host-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-echo (text) |host |echo
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'is-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export is-buffer (value) (buffer? value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Buffer
          :tests $ [] $ %{} 'TestEntry (:name |keeps-buffer-tag)
            :code $ quote $ assert= true
              is-buffer $ &buffer 0 255 17
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
        :code $ quote $ ns component-wasm.main
