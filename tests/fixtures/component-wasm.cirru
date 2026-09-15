
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
        'Event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Event (:idle) (:named 'String) (:moved 'Number 'Number) (:profile 'component-wasm.main/Profile)
          :examples $ []
          :schema $ :: 'EnumDef
        'NumericScalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NumericScalars (:i8 'Int8) (:u8 'UInt8) (:i16 'Int16) (:u16 'UInt16) (:i32 'Int32) (:u32 'UInt32) (:i64 'Int64) (:u64 'UInt64) (:f32 'Float32) (:f64 'Float64)
          :examples $ []
          :schema $ :: 'StructDef
        'Profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Profile (:active 'Bool) (:name 'String)
            :scores $ :: 'List 'Number
            :stats 'component-wasm.main/ProfileStats
            :maybe-name $ :: 'Option 'String
            :outcome $ :: 'Result (:: 'List 'Number) 'String
          :examples $ []
          :schema $ :: 'StructDef
        'ProfileStats $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ProfileStats (:score 'Number)
          :examples $ []
          :schema $ :: 'StructDef
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
        'call-host-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-event (value) (host-event value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/Event)
            :args $ [] 'component-wasm.main/Event
        'call-host-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-numbers (value) (host-numbers value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Number
            :return $ :: 'List 'Number
        'call-host-numeric-scalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-numeric-scalars (value) (host-numeric-scalars value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/NumericScalars)
            :args $ [] 'component-wasm.main/NumericScalars
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
        'call-host-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-profile (value) (host-profile value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/Profile)
            :args $ [] 'component-wasm.main/Profile
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
        'echo-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-event (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/Event)
            :args $ [] 'component-wasm.main/Event
          :tests $ []
            %{} 'TestEntry (:name |round-trips-empty-case)
              :code $ quote $ let
                  value $ Event :idle
                assert= value $ echo-event value
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-single-payload)
              :code $ quote $ let
                  value $ Event :named |Ada
                assert= value $ echo-event value
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-multiple-payloads)
              :code $ quote $ let
                  value $ Event :moved 3 4
                assert= value $ echo-event value
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-struct-payload)
              :code $ quote $ let
                  value $ Event :profile $ sample-profile
                assert= value $ echo-event value
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
        'echo-numeric-scalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-numeric-scalars (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/NumericScalars)
            :args $ [] 'component-wasm.main/NumericScalars
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-numeric-refinements)
            :code $ quote $ let
                value $ NumericScalars :i8 -128 :u8 255 :i16 -32768 :u16 65535 :i32 -2147483648 :u32 4294967295 :i64 -9007199254740991 :u64 9007199254740991 :f32 1.5 :f64 1.25
              assert= value $ echo-numeric-scalars value
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
        'echo-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-profile (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/Profile)
            :args $ [] 'component-wasm.main/Profile
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-struct-record)
            :code $ quote $ let
                value $ sample-profile
              assert= value $ echo-profile value
            :tags $ #{} :wasm
        'echo-result-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-event (value) value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Result 'component-wasm.main/Event 'String
            :return $ :: 'Result 'component-wasm.main/Event 'String
          :tests $ [] $ %{} 'TestEntry (:name |nested-enum-inside-result)
            :code $ quote $ assert=
              %ok $ Event :named |Ada
              echo-result-event $ %ok $ Event :named |Ada
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
        'host-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-event (value) |host |event
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/Event)
            :args $ [] 'component-wasm.main/Event
        'host-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-numbers (value) |host |numbers
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'Number
            :return $ :: 'List 'Number
        'host-numeric-scalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-numeric-scalars (value) |host |numeric-scalars
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/NumericScalars)
            :args $ [] 'component-wasm.main/NumericScalars
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
        'host-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-profile (value) |host |profile
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/Profile)
            :args $ [] 'component-wasm.main/Profile
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
        'sample-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sample-profile ()
            Profile :name |Ada :active true :scores ([] 1 2 3) :stats (ProfileStats :score 7) :maybe-name (%some |Ada) :outcome $ %ok $ [] 4 5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'component-wasm.main/Profile)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm.main
