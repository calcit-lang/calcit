
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
        'call-host-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-numbers (value) (host-numbers value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Number
            :return $ :: 'List 'Number
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
        'echo-bools $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-bools (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Bool
            :return $ :: 'List 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert= ([] true false true)
              echo-bools $ [] true false true
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
        'echo-buffers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-buffers (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Buffer
            :return $ :: 'List 'Buffer
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert=
              [] (&buffer 0 255) (&buffer 17 128)
              echo-buffers $ [] (&buffer 0 255) (&buffer 17 128)
            :tags $ #{} :wasm
        'echo-number-lists $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-number-lists (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List (:: 'List 'Number)
            :return $ :: 'List $ :: 'List 'Number
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert=
              [] ([] 1 2) ([]) ([] 3 4 5)
              echo-number-lists $ [] ([] 1 2) ([]) ([] 3 4 5)
            :tags $ #{} :wasm
        'echo-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-numbers (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Number
            :return $ :: 'List 'Number
          :tests $ []
            %{} 'TestEntry (:name |round-trips-list)
              :code $ quote $ assert= ([] 1 2 2 7)
                echo-numbers $ [] 1 2 2 7
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-empty-list)
              :code $ quote $ assert= ([])
                echo-numbers $ []
              :tags $ #{} :wasm
        'echo-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-text (text) text
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |keeps-text)
            :code $ quote $ assert= |hello (echo-text |hello)
            :tags $ #{} :wasm
        'echo-texts $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-texts (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'String
            :return $ :: 'List 'String
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert= ([] |alpha || "|世界")
              echo-texts $ [] |alpha || "|世界"
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
        'host-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-numbers (value) |host |numbers
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Number
            :return $ :: 'List 'Number
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
