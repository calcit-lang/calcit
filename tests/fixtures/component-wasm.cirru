
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
        'call-host-option-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-option-number (value) (host-option-number value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Option 'Number
            :return $ :: 'Option 'Number
        'call-host-ping $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-ping () (host-ping)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'call-host-result-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-result-number (value) (host-result-number value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Result 'Number 'String
            :return $ :: 'Result 'Number 'String
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
        'echo-option-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-option-number (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Option 'Number
            :return $ :: 'Option 'Number
          :tests $ []
            %{} 'TestEntry (:name |round-trips-some)
              :code $ quote $ assert= (%some 7)
                echo-option-number $ %some 7
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-none)
              :code $ quote $ assert= (%none)
                echo-option-number $ %none
              :tags $ #{} :wasm
        'echo-option-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-option-text (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Option 'String
            :return $ :: 'Option 'String
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-text)
            :code $ quote $ assert= (%some "|你好")
              echo-option-text $ %some "|你好"
            :tags $ #{} :wasm
        'echo-result-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-number (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Result 'Number 'String
            :return $ :: 'Result 'Number 'String
          :tests $ []
            %{} 'TestEntry (:name |round-trips-ok)
              :code $ quote $ assert= (%ok 7)
                echo-result-number $ %ok 7
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-err)
              :code $ quote $ assert= (%err |bad)
                echo-result-number $ %err |bad
              :tags $ #{} :wasm
        'echo-result-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-numbers (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Result (:: 'List 'Number) 'String
            :return $ :: 'Result (:: 'List 'Number) 'String
          :tests $ []
            %{} 'TestEntry (:name |round-trips-list)
              :code $ quote $ assert=
                %ok $ [] 1 2 3
                echo-result-numbers $ %ok $ [] 1 2 3
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-list-error)
              :code $ quote $ assert= (%err |bad)
                echo-result-numbers $ %err |bad
              :tags $ #{} :wasm
        'echo-result-unit $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-unit (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Result 'Unit 'String
            :return $ :: 'Result 'Unit 'String
          :tests $ []
            %{} 'TestEntry (:name |round-trips-unit)
              :code $ quote $ assert= (%ok &unit)
                echo-result-unit $ %ok &unit
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-unit-error)
              :code $ quote $ assert= (%err |bad)
                echo-result-unit $ %err |bad
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
        'host-option-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-option-number (value) |host |option-number
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Option 'Number
            :return $ :: 'Option 'Number
        'host-ping $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-ping () |host |ping
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'host-result-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-result-number (value) |host |result-number
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Result 'Number 'String
            :return $ :: 'Result 'Number 'String
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
        'ping $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export ping () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |returns-unit)
            :code $ quote $ assert= &unit (ping)
            :tags $ #{} :wasm
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm.main
