
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |component-wasm
  :entries $ {} $ :default
    {} (:description "|Canonical ABI adapter integration fixture.") (:init-fn 'component-wasm.main/main!) (:mode :native) (:reload-fn 'component-wasm.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'component-wasm.main $ %{} 'FileEntry
      :defs $ {}
        'Event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Event (:idle) (:named 'String) (:moved 'Number 'Number) (:profile 'component-wasm.main/Profile)
          :examples $ []
          :schema $ quote EnumDef
        'NumericScalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NumericScalars (:i8 'Int8) (:u8 'UInt8) (:i16 'Int16) (:u16 'UInt16) (:i32 'Int32) (:u32 'UInt32) (:i64 'Int64) (:u64 'UInt64) (:f32 'Float32) (:f64 'Float64)
          :examples $ []
          :schema $ quote StructDef
        'Profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Profile (:active 'Bool) (:name 'String)
            :scores $ quote $ List Number
            :stats 'component-wasm.main/ProfileStats
            :maybe-name $ quote $ Option String
            :outcome $ quote $ Result (List Number) String
          :examples $ []
          :schema $ quote StructDef
        'ProfileStats $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ProfileStats (:score 'Number)
          :examples $ []
          :schema $ quote StructDef
        'add-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export add-one (value) (&+ value 1)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |adds-one)
            :code $ quote $ assert= 42 (add-one 41)
            :tags $ #{} :wasm
        'bool-not $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export bool-not (flag) (not flag)
          :examples $ []
          :schema $ quote $ Fn Bool (-> Bool)
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
          :schema $ quote $ Fn Number (-> Number)
        'call-host-bool-not $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-bool-not (flag) (host-bool-not flag)
          :examples $ []
          :schema $ quote $ Fn Bool (-> Bool)
        'call-host-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-buffer (value) (host-buffer value)
          :examples $ []
          :schema $ quote $ Fn Buffer (-> Buffer)
        'call-host-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-echo (text) (host-echo text)
          :examples $ []
          :schema $ quote $ Fn String (-> String)
        'call-host-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-event (value) (host-event value)
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/Event (-> component-wasm.main/Event)
        'call-host-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-numbers (value) (host-numbers value)
          :examples $ []
          :schema $ quote $ Fn (List Number)
            -> $ List Number
        'call-host-numeric-scalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-numeric-scalars (value) (host-numeric-scalars value)
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/NumericScalars (-> component-wasm.main/NumericScalars)
        'call-host-option-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-option-number (value) (host-option-number value)
          :examples $ []
          :schema $ quote $ Fn (Option Number)
            -> $ Option Number
        'call-host-ping $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-ping () (host-ping)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'call-host-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-profile (value) (host-profile value)
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/Profile (-> component-wasm.main/Profile)
        'call-host-result-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-host-result-number (value) (host-result-number value)
          :examples $ []
          :schema $ quote $ Fn (Result Number String)
            -> $ Result Number String
        'choose-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export choose-buffer (flag yes no) (if flag yes no)
          :examples $ []
          :schema $ quote $ Fn Bool Buffer Buffer (-> Buffer)
          :tests $ [] $ %{} 'TestEntry (:name |chooses-binary-branch)
            :code $ quote $ assert= (&buffer 2 0)
              choose-buffer false (&buffer 1 255) (&buffer 2 0)
            :tags $ #{} :wasm
        'choose-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export choose-number (flag yes no) (if flag yes no)
          :examples $ []
          :schema $ quote $ Fn Bool Number Number (-> Number)
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
          :schema $ quote $ Fn (List Bool)
            -> $ List Bool
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert= ([] true false true)
              echo-bools $ [] true false true
            :tags $ #{} :wasm
        'echo-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-buffer (value) value
          :examples $ []
          :schema $ quote $ Fn Buffer (-> Buffer)
          :tests $ [] $ %{} 'TestEntry (:name |keeps-binary-bytes)
            :code $ quote $ assert= (&buffer 0 255 17)
              echo-buffer $ &buffer 0 255 17
            :tags $ #{} :wasm
        'echo-buffers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-buffers (value) value
          :examples $ []
          :schema $ quote $ Fn (List Buffer)
            -> $ List Buffer
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert=
              [] (&buffer 0 255) (&buffer 17 128)
              echo-buffers $ [] (&buffer 0 255) (&buffer 17 128)
            :tags $ #{} :wasm
        'echo-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-event (value) value
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/Event (-> component-wasm.main/Event)
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
          :schema $ quote $ Fn
            List $ List Number
            -> $ List $ List Number
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert=
              [] ([] 1 2) ([]) ([] 3 4 5)
              echo-number-lists $ [] ([] 1 2) ([]) ([] 3 4 5)
            :tags $ #{} :wasm
        'echo-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-numbers (value) value
          :examples $ []
          :schema $ quote $ Fn (List Number)
            -> $ List Number
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
          :schema $ quote $ Fn component-wasm.main/NumericScalars (-> component-wasm.main/NumericScalars)
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-numeric-refinements)
            :code $ quote $ let
                value $ NumericScalars :i8 -128 :u8 255 :i16 -32768 :u16 65535 :i32 -2147483648 :u32 4294967295 :i64 -9007199254740991 :u64 9007199254740991 :f32 1.5 :f64 1.25
              assert= value $ echo-numeric-scalars value
        'echo-option-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-option-number (value) value
          :examples $ []
          :schema $ quote $ Fn (Option Number)
            -> $ Option Number
          :tests $ []
            %{} 'TestEntry (:name |round-trips-some)
              :code $ quote $ assert= (Option :some 7)
                echo-option-number $ Option :some 7
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-none)
              :code $ quote $ assert= (Option :none)
                echo-option-number $ Option :none
              :tags $ #{} :wasm
        'echo-option-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-option-text (value) value
          :examples $ []
          :schema $ quote $ Fn (Option String)
            -> $ Option String
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-text)
            :code $ quote $ assert= (Option :some "|你好")
              echo-option-text $ Option :some "|你好"
            :tags $ #{} :wasm
        'echo-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-profile (value) value
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/Profile (-> component-wasm.main/Profile)
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-struct-record)
            :code $ quote $ let
                value $ sample-profile
              assert= value $ echo-profile value
            :tags $ #{} :wasm
        'echo-result-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-event (value) value
          :examples $ []
          :schema $ quote $ Fn (Result component-wasm.main/Event String)
            -> $ Result component-wasm.main/Event String
          :tests $ [] $ %{} 'TestEntry (:name |nested-enum-inside-result)
            :code $ quote $ assert=
              Result :ok $ Event :named |Ada
              echo-result-event $ Result :ok $ Event :named |Ada
            :tags $ #{} :wasm
        'echo-result-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-number (value) value
          :examples $ []
          :schema $ quote $ Fn (Result Number String)
            -> $ Result Number String
          :tests $ []
            %{} 'TestEntry (:name |round-trips-ok)
              :code $ quote $ assert= (Result :ok 7)
                echo-result-number $ Result :ok 7
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-err)
              :code $ quote $ assert= (Result :err |bad)
                echo-result-number $ Result :err |bad
              :tags $ #{} :wasm
        'echo-result-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-numbers (value) value
          :examples $ []
          :schema $ quote $ Fn
            Result (List Number) String
            -> $ Result (List Number) String
          :tests $ []
            %{} 'TestEntry (:name |round-trips-list)
              :code $ quote $ assert=
                Result :ok $ [] 1 2 3
                echo-result-numbers $ Result :ok $ [] 1 2 3
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-list-error)
              :code $ quote $ assert= (Result :err |bad)
                echo-result-numbers $ Result :err |bad
              :tags $ #{} :wasm
        'echo-result-unit $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-result-unit (value) value
          :examples $ []
          :schema $ quote $ Fn (Result Unit String)
            -> $ Result Unit String
          :tests $ []
            %{} 'TestEntry (:name |round-trips-unit)
              :code $ quote $ assert= (Result :ok &unit)
                echo-result-unit $ Result :ok &unit
              :tags $ #{} :wasm
            %{} 'TestEntry (:name |round-trips-unit-error)
              :code $ quote $ assert= (Result :err |bad)
                echo-result-unit $ Result :err |bad
              :tags $ #{} :wasm
        'echo-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-text (text) text
          :examples $ []
          :schema $ quote $ Fn String (-> String)
          :tests $ [] $ %{} 'TestEntry (:name |keeps-text)
            :code $ quote $ assert= |hello (echo-text |hello)
            :tags $ #{} :wasm
        'echo-texts $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-texts (value) value
          :examples $ []
          :schema $ quote $ Fn (List String)
            -> $ List String
          :tests $ [] $ %{} 'TestEntry (:name |round-trips-list)
            :code $ quote $ assert= ([] |alpha || "|世界")
              echo-texts $ [] |alpha || "|世界"
            :tags $ #{} :wasm
        'host-add-one $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-add-one (value) |host |add-one
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'host-bool-not $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-bool-not (flag) |host |bool-not
          :examples $ []
          :schema $ quote $ Fn Bool (-> Bool)
        'host-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-buffer (value) |host |buffer
          :examples $ []
          :schema $ quote $ Fn Buffer (-> Buffer)
        'host-echo $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-echo (text) |host |echo
          :examples $ []
          :schema $ quote $ Fn String (-> String)
        'host-event $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-event (value) |host |event
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/Event (-> component-wasm.main/Event)
        'host-numbers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-numbers (value) |host |numbers
          :examples $ []
          :schema $ quote $ Fn (List Number)
            -> $ List Number
        'host-numeric-scalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-numeric-scalars (value) |host |numeric-scalars
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/NumericScalars (-> component-wasm.main/NumericScalars)
        'host-option-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-option-number (value) |host |option-number
          :examples $ []
          :schema $ quote $ Fn (Option Number)
            -> $ Option Number
        'host-ping $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-ping () |host |ping
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'host-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-profile (value) |host |profile
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/Profile (-> component-wasm.main/Profile)
        'host-result-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-import host-result-number (value) |host |result-number
          :examples $ []
          :schema $ quote $ Fn (Result Number String)
            -> $ Result Number String
        'is-buffer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export is-buffer (value) (buffer? value)
          :examples $ []
          :schema $ quote $ Fn Buffer (-> Bool)
          :tests $ [] $ %{} 'TestEntry (:name |keeps-buffer-tag)
            :code $ quote $ assert= true
              is-buffer $ &buffer 0 255 17
            :tags $ #{} :wasm
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'ping $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export ping () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
          :tests $ [] $ %{} 'TestEntry (:name |returns-unit)
            :code $ quote $ assert= &unit (ping)
            :tags $ #{} :wasm
        'profile-stats-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export profile-stats-identity (value)
            let
                score $ :score value
                same $ ProfileStats :score score
                different $ other/ProfileStats :score score
              and (= value same)
                = (&hash value) (&hash same)
                not $ &= value different
          :examples $ []
          :schema $ quote $ Fn component-wasm.main/ProfileStats (-> Bool)
          :tests $ [] $ %{} 'TestEntry (:name |lifted-definition)
            :code $ quote $ assert= true
              profile-stats-identity $ ProfileStats :score 7.5
            :tags $ #{} :struct-identity :unit
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'sample-profile $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sample-profile ()
            Profile :name |Ada :active true :scores ([] 1 2 3) :stats (ProfileStats :score 7) :maybe-name (Option :some |Ada) :outcome $ Result :ok $ [] 4 5
          :examples $ []
          :schema $ quote $ Fn (-> component-wasm.main/Profile)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm.main
          :require $ component-wasm.other :as other
    'component-wasm.other $ %{} 'FileEntry
      :defs $ {} $ 'ProfileStats
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ProfileStats (:score 'Number)
          :examples $ []
          :schema $ quote StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-wasm.other
