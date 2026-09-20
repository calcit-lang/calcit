
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'EdnEnvelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct EdnEnvelope (:request-id 'String)
            :note $ :: 'Option 'String
            :outcome 'EdnOutcome
          :examples $ []
          :schema $ :: 'StructDef
        'EdnJob $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct EdnJob (:name 'String) (:count 'Int32) (:ready 'Bool)
          :examples $ []
          :schema $ :: 'StructDef
        'EdnOutcome $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum EdnOutcome
            :ready $ :: 'List 'Int32
            :failed 'String
            :idle
          :examples $ []
          :schema $ :: 'EnumDef
        'clock-fixed-main! $ %{} 'CodeEntry (:doc "|用确定性的 WASI 假宿主验证时钟编号与纳秒到毫秒的换算。")
          :code $ quote $ defn clock-fixed-main! ()
            if
              and
                = 1234 $ unix-time-ms
                = 5678 $ cpu-time
              println "|WASI-fixed-clocks: ok"
              quit! 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :time :wasi
        'clock-main! $ %{} 'CodeEntry (:doc "|输出系统时钟与单调时钟，用于验证 WASI capability wiring。")
          :code $ quote $ defn clock-main! ()
            if (clocks-valid?) (println "|WASI-clocks: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :time :wasi
        'clocks-valid? $ %{} 'CodeEntry (:doc "|验证系统时钟为正数，且单调时钟在同一进程内不会倒退。")
          :code $ quote $ defn clocks-valid? ()
            let
                wall $ unix-time-ms
                before $ cpu-time
                after $ cpu-time
              and (number? wall) (> wall 0) (number? before) (number? after) (>= after before)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tags $ #{} :core :time :unit :wasi :wasm
          :tests $ [] $ %{} 'TestEntry (:name |valid-readings)
            :code $ quote $ assert= true (clocks-valid?)
            :tags $ #{} :core :time :unit :wasi :wasm
        'edn-file-roundtrip-main! $ %{} 'CodeEntry (:doc "|从预开放目录读取有类型 Cirru EDN，更新计数并写回规范化数据。")
          :code $ quote $ defn edn-file-roundtrip-main! ()
            let
                input $ .read-text $ fs:path |workspace/input.cirru
              match input
                (:err _) (quit! 1)
                (:ok content)
                  let
                      parsed $ edn-parse-string-int-map content
                    match parsed
                      (:err _) (quit! 1)
                      (:ok data)
                        let
                            transformed $ edn-transform-count-map data
                          match transformed
                            (:err _) (quit! 1)
                            (:ok updated)
                              let
                                  output $ .write-text (fs:path |workspace/output.cirru) (format-cirru-edn updated)
                                match output
                                  (:ok _) (println |WASI-typed-EDN-file:-ok)
                                  (:err _) (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :edn :file :wasi
        'edn-format-escaped-over-limit-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-format-escaped-over-limit-main! ()
            let
                x0 "|\n"
                x1 $ &str:concat x0 x0
                x2 $ &str:concat x1 x1
                x3 $ &str:concat x2 x2
                x4 $ &str:concat x3 x3
                x5 $ &str:concat x4 x4
                x6 $ &str:concat x5 x5
                x7 $ &str:concat x6 x6
                x8 $ &str:concat x7 x7
                x9 $ &str:concat x8 x8
                x10 $ &str:concat x9 x9
                x11 $ &str:concat x10 x10
                x12 $ &str:concat x11 x11
                x13 $ &str:concat x12 x12
                x14 $ &str:concat x13 x13
                x15 $ &str:concat x14 x14
              println $ format-cirru-edn x15
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-format-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-format-main! ()
            println $ format-cirru-edn $ [] :ready :paused
            println $ format-cirru-edn $ [] |hello "|hello world"
            println $ format-cirru-edn $ [] ([] |a |b) ([] "|c d")
            println $ format-cirru-edn true
            println $ format-cirru-edn nil
            println $ format-cirru-edn $ assert-type 42 'Int32
            println $ format-cirru-edn 1.25
            println $ format-cirru-edn -0.5
            println $ format-cirru-edn $ {} (:b |two) (:a |one)
            println $ format-cirru-edn $ {} ("|b key" |two) (|a |one)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-format-over-limit-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-format-over-limit-main! ()
            println $ format-cirru-edn $ repeat :item 22000
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-format-samples $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-format-samples ()
            []
              format-cirru-edn $ [] :ready :paused
              format-cirru-edn $ [] |hello "|hello world"
              format-cirru-edn $ [] ([] |a |b) ([] "|c d")
              format-cirru-edn true
              format-cirru-edn nil
              format-cirru-edn $ assert-type 42 'Int32
              format-cirru-edn 1.25
              format-cirru-edn -0.5
              format-cirru-edn $ {} (:b |two) (:a |one)
              format-cirru-edn $ {} ("|b key" |two) (|a |one)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'List 'String
          :tests $ [] $ %{} 'TestEntry (:name |formats-closed-scalars-and-lists)
            :code $ quote $ assert=
              []
                str (char-from-code 10) "|[] :ready :paused" $ char-from-code 10
                str (char-from-code 10) "|[] |hello \"|hello world\"" $ char-from-code 10
                str (char-from-code 10) "|[] ([] |a |b) ([] \"|c d\")" $ char-from-code 10
                str (char-from-code 10) "|do true" $ char-from-code 10
                str (char-from-code 10) "|do nil" $ char-from-code 10
                str (char-from-code 10) "|do 42" $ char-from-code 10
                str (char-from-code 10) "|do 1.25" $ char-from-code 10
                str (char-from-code 10) "|do -0.5" $ char-from-code 10
                str (char-from-code 10) "|{} (:a |one) (:b |two)" $ char-from-code 10
                str (char-from-code 10) "|{} (|a |one) (\"|b key\" |two)" $ char-from-code 10
              edn-format-samples
        'edn-format-source-over-limit-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-format-source-over-limit-main! ()
            let
                x0 |12345678
                x1 $ &str:concat x0 x0
                x2 $ &str:concat x1 x1
                x3 $ &str:concat x2 x2
                x4 $ &str:concat x3 x3
                x5 $ &str:concat x4 x4
                x6 $ &str:concat x5 x5
                x7 $ &str:concat x6 x6
                x8 $ &str:concat x7 x7
                x9 $ &str:concat x8 x8
                x10 $ &str:concat x9 x9
                x11 $ &str:concat x10 x10
                x12 $ &str:concat x11 x11
                x13 $ &str:concat x12 x12
                x14 $ &str:concat x13 x13
              println $ format-cirru-edn x14
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-nested-roundtrip-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-nested-roundtrip-main! ()
            let
                ints-result $ edn-parse-nested-int-lists "|[] ([] 1 2) ([] -3)"
                strings-result $ edn-parse-nested-string-lists "|[] ([] \"|(\" \"|)\")"
              match ints-result
                (:err _) (quit! 1)
                (:ok ints)
                  match strings-result
                    (:err _) (quit! 1)
                    (:ok strings)
                      if
                        and
                          = (format-cirru-edn ints)
                            str (char-from-code 10) "|[] ([] 1 2) ([] -3)" $ char-from-code 10
                          = (format-cirru-edn strings)
                            str (char-from-code 10) "|[] ([] \"|(\" \"|)\")" $ char-from-code 10
                        println |WASI-recursive-typed-EDN:-ok
                        quit! 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-nominal-roundtrip-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-nominal-roundtrip-main! ()
            let
                outcome $ EdnOutcome :ready $ assert-type ([] 1 -2) (:: 'List 'Int32)
                envelope $ EdnEnvelope :request-id |r1 :note (%some "|hello world") :outcome outcome
                typed-result $ assert-type (%ok outcome) (:: 'Result 'app.main/EdnOutcome 'String)
              match
                edn-parse-outcome $ format-cirru-edn outcome
                (:err _) (quit! 1)
                (:ok decoded-outcome)
                  match
                    edn-parse-envelope $ format-cirru-edn envelope
                    (:err _) (quit! 1)
                    (:ok decoded-envelope)
                      match
                        edn-parse-result-outcome $ format-cirru-edn typed-result
                        (:err _) (quit! 1)
                        (:ok decoded-result)
                          if
                            and
                              = (format-cirru-edn outcome) (format-cirru-edn decoded-outcome)
                              = (format-cirru-edn envelope) (format-cirru-edn decoded-envelope)
                              = (format-cirru-edn typed-result) (format-cirru-edn decoded-result)
                              result:err? $ edn-parse-outcome "|%:: 'EdnOutcome 'unknown"
                              result:err? $ edn-parse-outcome "|%:: 'EdnOutcome 'ready |wrong"
                            println |WASI-nominal-typed-EDN:-ok
                            quit! 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-parse-envelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-envelope (text) (try-parse-cirru-edn-as text 'EdnEnvelope)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result 'app.main/EdnEnvelope 'String
        'edn-parse-int-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-int-list (text)
            try-parse-cirru-edn-as text $ :: 'List 'Int32
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result (:: 'List 'Int32) 'String
          :tests $ [] $ %{} 'TestEntry (:name |parses-bounded-int-lists)
            :code $ quote $ do
              assert=
                %ok $ [] 1 -2 3
                edn-parse-int-list "|[] 1 -2 3"
              assert=
                %ok $ []
                edn-parse-int-list |[]
              assert= true $ result:err? $ edn-parse-int-list "|[] 1 2147483648"
              assert= true $ result:err? $ edn-parse-int-list "|[] 1 |bad"
            :tags $ #{} :core :unit
        'edn-parse-job $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-job (text) (try-parse-cirru-edn-as text 'EdnJob)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result 'app.main/EdnJob 'String
          :tests $ []
            %{} 'TestEntry (:name |round-trips-scalar-struct)
              :code $ quote $ do
                assert=
                  %ok $ EdnJob :name |Ada :count (assert-type 3 'Int32) :ready true
                  edn-parse-job "|%{} 'EdnJob (:ready true) (:count 3) (:name |Ada)"
                let
                    parsed $ edn-parse-job "|%{} 'EdnJob (:name |Ada) (:count 3) (:ready true)"
                  match parsed
                    (:err message) (raise message)
                    (:ok value)
                      assert=
                        str (char-from-code 10) "|%{} 'EdnJob (:count 3) (:name |Ada) (:ready true)" $ char-from-code 10
                        format-cirru-edn value
              :tags $ #{} :core :edn :unit :wasi :wasm
            %{} 'TestEntry (:name |rejects-invalid-struct-shapes)
              :code $ quote $ do
                assert= true $ result:err? $ edn-parse-job "|%{} 'Other (:count 3) (:name |Ada) (:ready true)"
                assert= true $ result:err? $ edn-parse-job "|%{} 'EdnJob (:count 3) (:name |Ada)"
                assert= true $ result:err? $ edn-parse-job "|%{} 'EdnJob (:count 3) (:name |Ada) (:ready true) (:extra |x)"
                assert= true $ result:err? $ edn-parse-job "|%{} 'EdnJob (:count 3) (:count 4) (:name |Ada) (:ready true)"
                assert= true $ result:err? $ edn-parse-job "|%{} 'EdnJob (:count 2147483648) (:name |Ada) (:ready true)"
                assert= true $ result:err? $ edn-parse-job "|%{}'EdnJob (:count 3) (:name |Ada) (:ready true)"
              :tags $ #{} :core :edn :unit :wasi :wasm
        'edn-parse-large-fraction $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-large-fraction (text) (try-parse-cirru-edn-as text 'Int64)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result 'Int64 'String
          :tests $ [] $ %{} 'TestEntry (:name |rejects-rounded-fraction)
            :code $ quote $ assert= true
              result:err? $ edn-parse-large-fraction |9007199254740990.5
            :tags $ #{} :core :unit
        'edn-parse-list-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-list-main! ()
            let
                ints-result $ edn-parse-int-list "|[] 1 -2 3"
                words-result $ edn-parse-string-list "|[] |hello \"|two words\" |tail"
                ints $ result:unwrap-or ints-result $ []
                words $ result:unwrap-or words-result $ []
              assert= true $ result:ok? ints-result
              assert= 3 $ count ints
              assert= 1 $ &list:nth ints 0
              assert= -2 $ &list:nth ints 1
              assert= 3 $ &list:nth ints 2
              assert= true $ result:ok? words-result
              assert= 3 $ count words
              assert= |hello $ &list:nth words 0
              assert= "|two words" $ &list:nth words 1
              assert= |tail $ &list:nth words 2
              assert= true $ result:err? $ edn-parse-int-list "|[] 1 |bad"
              println |WASI-typed-EDN-lists:-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-parse-list-over-limit-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-list-over-limit-main! ()
            let
                x0 "| 1"
                x1 $ &str:concat x0 x0
                x2 $ &str:concat x1 x1
                x3 $ &str:concat x2 x2
                x4 $ &str:concat x3 x3
                x5 $ &str:concat x4 x4
                x6 $ &str:concat x5 x5
                x7 $ &str:concat x6 x6
                x8 $ &str:concat x7 x7
                x9 $ &str:concat x8 x8
                x10 $ &str:concat x9 x9
                x11 $ &str:concat x10 x10
                x12 $ &str:concat x11 x11
                input $ &str:concat (&str:concat |[] x12) x0
              assert= true $ result:err? $ edn-parse-int-list input
              println |WASI-typed-EDN-list-limit:-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-parse-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-main! ()
            let
                results $ edn-parse-scalars
              assert= true $ result:ok? $ &list:nth results 0
              assert= true $ result:ok? $ &list:nth results 1
              assert= true $ result:ok? $ &list:nth results 2
              assert= true $ result:ok? $ &list:nth results 3
              assert= true $ result:ok? $ &list:nth results 4
              assert= 42 $ result:unwrap-or (&list:nth results 0) 0
              assert= |hello $ result:unwrap-or (&list:nth results 1) |fallback
              assert= :ready $ result:unwrap-or (&list:nth results 2) :paused
              assert= true $ result:unwrap-or (&list:nth results 3) false
              assert= true $ result:err? $ &list:nth results 5
              assert= true $ result:err? $ &list:nth results 6
              assert= true $ result:err? $ &list:nth results 7
              assert= true $ result:err? $ &list:nth results 8
              assert= true $ result:err? $ &list:nth results 9
              assert= "|hello world" $ result:unwrap-or (&list:nth results 10) |fallback
              assert=
                str |line (char-from-code 10) |next
                result:unwrap-or (&list:nth results 11) |fallback
              assert=
                str |a (char-from-code 9) (char-from-code 34) (char-from-code 92) |z
                result:unwrap-or (&list:nth results 12) |fallback
              assert= true $ result:err? $ &list:nth results 13
              println |WASI-typed-EDN-scalars:-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-parse-map-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-map-main! ()
            let
                tags-result $ edn-parse-tag-string-map "|{} (:a |one) (:b \"|two words\")"
                strings-result $ edn-parse-string-int-map "|{} (\"|two words\" 2) (|tail -3)"
                tags $ result:unwrap-or tags-result $ {}
                strings $ result:unwrap-or strings-result $ {}
                without-two $ &map:dissoc strings "|two words"
              assert= true $ result:ok? tags-result
              assert= 2 $ count tags
              assert= |one $ &map:get tags :a
              assert= "|two words" $ &map:get tags :b
              assert= true $ result:ok? strings-result
              assert= 2 $ count strings
              assert= 2 $ &map:get strings "|two words"
              assert= -3 $ &map:get strings |tail
              assert= 1 $ count without-two
              assert= false $ &map:contains? without-two "|two words"
              assert= true $ result:err? $ edn-parse-tag-string-map "|{} (:a"
              assert= true $ result:err? $ edn-parse-large-fraction |9007199254740990.5
              println |WASI-typed-EDN-maps:-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-parse-map-over-limit-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-map-over-limit-main! ()
            let
                x0 "| (:a |one)"
                x1 $ &str:concat x0 x0
                x2 $ &str:concat x1 x1
                x3 $ &str:concat x2 x2
                x4 $ &str:concat x3 x3
                x5 $ &str:concat x4 x4
                x6 $ &str:concat x5 x5
                x7 $ &str:concat x6 x6
                x8 $ &str:concat x7 x7
                x9 $ &str:concat x8 x8
                x10 $ &str:concat x9 x9
                x11 $ &str:concat x10 x10
                input $ &str:concat (&str:concat |{} x11) x0
              assert= true $ result:err? $ edn-parse-tag-string-map input
              println |WASI-typed-EDN-map-limit:-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-parse-nested-int-lists $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-nested-int-lists (text)
            try-parse-cirru-edn-as text $ :: 'List $ :: 'List 'Int32
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result
              :: 'List $ :: 'List 'Int32
              , 'String
          :tests $ [] $ %{} 'TestEntry (:name |parses-and-formats-closed-nesting)
            :code $ quote $ do
              let
                  parsed $ edn-parse-nested-int-lists "|[] ([] 1 2) ([] -3)"
                match parsed
                  (:err message) (raise message)
                  (:ok value)
                    do
                      assert=
                        [] ([] 1 2) ([] -3)
                        , value
                      assert=
                        str (char-from-code 10) "|[] ([] 1 2) ([] -3)" $ char-from-code 10
                        format-cirru-edn value
            :tags $ #{} :core :edn :unit :wasi :wasm
        'edn-parse-nested-string-lists $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-nested-string-lists (text)
            try-parse-cirru-edn-as text $ :: 'List $ :: 'List 'String
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result
              :: 'List $ :: 'List 'String
              , 'String
          :tests $ [] $ %{} 'TestEntry (:name |ignores-parentheses-inside-quoted-strings)
            :code $ quote $ assert=
              %ok $ [] $ [] "|(" "|)"
              edn-parse-nested-string-lists "|[] ([] \"|(\" \"|)\")"
            :tags $ #{} :core :edn :unit :wasi :wasm
        'edn-parse-outcome $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-outcome (text) (try-parse-cirru-edn-as text 'EdnOutcome)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result 'app.main/EdnOutcome 'String
          :tests $ [] $ %{} 'TestEntry (:name |parses-nominal-variants)
            :code $ quote $ do
              assert=
                %ok $ EdnOutcome :ready $ assert-type ([] 1 -2) (:: 'List 'Int32)
                edn-parse-outcome "|%:: 'EdnOutcome 'ready ([] 1 -2)"
              assert=
                %ok $ EdnOutcome :idle
                edn-parse-outcome "|%:: :EdnOutcome :idle"
              assert= true $ result:err? $ edn-parse-outcome "|%:: 'EdnOutcome 'unknown"
              assert= true $ result:err? $ edn-parse-outcome "|%:: 'EdnOutcome 'ready |wrong"
            :tags $ #{} :core :edn :wasi :wasm
        'edn-parse-over-limit-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-over-limit-main! ()
            let
                x0 |12345678
                x1 $ &str:concat x0 x0
                x2 $ &str:concat x1 x1
                x3 $ &str:concat x2 x2
                x4 $ &str:concat x3 x3
                x5 $ &str:concat x4 x4
                x6 $ &str:concat x5 x5
                x7 $ &str:concat x6 x6
                x8 $ &str:concat x7 x7
                x9 $ &str:concat x8 x8
                x10 $ &str:concat x9 x9
                x11 $ &str:concat x10 x10
                x12 $ &str:concat x11 x11
                x13 $ &str:concat x12 x12
                x14 $ &str:concat x13 x13
              assert= true $ result:err? $ try-parse-cirru-edn-as x14 'String
              println |WASI-typed-EDN-limit:-ok
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-parse-result-outcome $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-result-outcome (text)
            try-parse-cirru-edn-as text $ :: 'Result 'app.main/EdnOutcome 'String
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result (:: 'Result 'app.main/EdnOutcome 'String) 'String
        'edn-parse-scalars $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-scalars ()
            [] (try-parse-cirru-edn-as "|do 42" 'Int32) (try-parse-cirru-edn-as "|do |hello" 'String) (try-parse-cirru-edn-as "|do :ready" 'Tag) (try-parse-cirru-edn-as "|do true" 'Bool) (try-parse-cirru-edn-as "|do nil" 'Nil) (try-parse-cirru-edn-as "|do 128" 'Int8) (try-parse-cirru-edn-as "|do 1.5" 'Int8) (try-parse-cirru-edn-as "|do |bad" 'Int32) (try-parse-cirru-edn-as "|do :wasi-typed-edn-unknown" 'Tag) (try-parse-cirru-edn-as "|do nil" 'Unit)
              try-parse-cirru-edn-as (format-cirru-edn "|hello world") 'String
              try-parse-cirru-edn-as
                format-cirru-edn $ str |line (char-from-code 10) |next
                , 'String
              try-parse-cirru-edn-as
                format-cirru-edn $ str |a (char-from-code 9) (char-from-code 34) (char-from-code 92) |z
                , 'String
              try-parse-cirru-edn-as
                str "|do " (char-from-code 34) ||bad (char-from-code 92) |q $ char-from-code 34
                , 'String
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |parses-bounded-scalars)
              :code $ quote $ assert=
                [] (%ok 42) (%ok |hello) (%ok :ready) (%ok true) (%ok nil) true true true true true
                let
                    results $ edn-parse-scalars
                  [] (&list:nth results 0) (&list:nth results 1) (&list:nth results 2) (&list:nth results 3) (&list:nth results 4)
                    result:err? $ &list:nth results 5
                    result:err? $ &list:nth results 6
                    result:err? $ &list:nth results 7
                    result:ok? $ &list:nth results 8
                    result:err? $ &list:nth results 9
            %{} 'TestEntry (:name |parses-quoted-escaped-strings)
              :code $ quote $ let
                  results $ edn-parse-scalars
                  escaped $ str |a (char-from-code 9) (char-from-code 34) (char-from-code 92) |z
                assert= (%ok "|hello world") (&list:nth results 10)
                assert=
                  %ok $ str |line (char-from-code 10) |next
                  &list:nth results 11
                assert= (%ok escaped) (&list:nth results 12)
            %{} 'TestEntry (:name |rejects-invalid-quoted-escape)
              :code $ quote $ assert= true
                result:err? $ &list:nth (edn-parse-scalars) 13
        'edn-parse-string-int-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-string-int-map (text)
            try-parse-cirru-edn-as text $ :: 'Map 'String 'Int32
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result (:: 'Map 'String 'Int32) 'String
          :tests $ [] $ %{} 'TestEntry (:name |preserves-quoted-map-key)
            :code $ quote $ assert=
              %ok $ {,} "|two words" 2 |tail -3
              edn-parse-string-int-map "|{} (\"|two words\" 2) (|tail -3)"
            :tags $ #{} :core :unit
        'edn-parse-string-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-string-list (text)
            try-parse-cirru-edn-as text $ :: 'List 'String
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result (:: 'List 'String) 'String
          :tests $ [] $ %{} 'TestEntry (:name |preserves-quoted-string-items)
            :code $ quote $ assert=
              %ok $ [] |hello "|two words" |tail
              edn-parse-string-list "|[] |hello \"|two words\" |tail"
            :tags $ #{} :core :unit
        'edn-parse-tag-string-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-parse-tag-string-map (text)
            try-parse-cirru-edn-as text $ :: 'Map 'Tag 'String
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'String
            :return $ :: 'Result (:: 'Map 'Tag 'String) 'String
          :tests $ [] $ %{} 'TestEntry (:name |parses-bounded-tag-string-map)
            :code $ quote $ do
              assert=
                %ok $ {,} :a |one :b "|two words"
                edn-parse-tag-string-map "|{} (:a |one) (:b \"|two words\")"
              assert=
                %ok $ {}
                edn-parse-tag-string-map |{}
              assert= true $ result:err? $ edn-parse-tag-string-map "|{} (:a |one) (:bad 2)"
              assert= true $ result:err? $ edn-parse-tag-string-map "|{} (:a"
            :tags $ #{} :core :unit
        'edn-struct-roundtrip-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn edn-struct-roundtrip-main! ()
            let
                parsed $ edn-parse-job "|%{} 'EdnJob (:ready true) (:count 3) (:name |Ada)"
              if
                and
                  result:err? $ edn-parse-job "|%{} 'Other (:count 3) (:name |Ada) (:ready true)"
                  result:err? $ edn-parse-job "|%{} 'EdnJob (:count 3) (:name |Ada)"
                  result:err? $ edn-parse-job "|%{} 'EdnJob (:count 3) (:name |Ada) (:ready true) (:extra |x)"
                  result:err? $ edn-parse-job "|%{}'EdnJob (:count 3) (:name |Ada) (:ready true)"
                match parsed
                  (:err _) (quit! 1)
                  (:ok value)
                    println $ format-cirru-edn value
                quit! 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'edn-transform-count-map $ %{} 'CodeEntry (:doc "|递增有类型计数并显式保留 Int32 越界错误。")
          :code $ quote $ defn edn-transform-count-map (data)
            let
                next-count $ &+ (&map:get data |count) 1
                refinement :int32
              if (&number:fits? next-count refinement)
                %ok $ &map:assoc data |processed $ assert-type next-count 'Int32
                %err |Int32-overflow
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'String 'Int32
            :return $ :: 'Result (:: 'Map 'String 'Int32) 'String
          :tags $ #{} :edn :wasi
          :tests $ []
            %{} 'TestEntry (:name |increments-count)
              :code $ quote $ assert=
                %ok $ {}
                  |count $ assert-type 2 'Int32
                  |processed $ assert-type 3 'Int32
                edn-transform-count-map $ {} $ |count (assert-type 2 'Int32)
              :tags $ #{} :wasi
            %{} 'TestEntry (:name |rejects-overflow)
              :code $ quote $ assert= (%err |Int32-overflow)
                edn-transform-count-map $ {} $ |count (assert-type 2147483647 'Int32)
              :tags $ #{} :wasi
            %{} 'TestEntry (:name |accepts-bound-refinement-tag)
              :code $ quote $ let
                  refinement :int32
                assert= true $ &number:fits? 3 refinement
              :tags $ #{} :wasi
        'exit-7! $ %{} 'CodeEntry (:doc "|以状态码 7 终止进程，用于验证 command 退出边界。")
          :code $ quote $ defn exit-7! () (quit! 7)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :control :wasi
        'exit-invalid! $ %{} 'CodeEntry (:doc "|以越界状态码终止进程，用于验证各 command backend 拒绝隐式取模。")
          :code $ quote $ defn exit-invalid! () (quit! 256)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :control :wasi
        'filesystem-absolute-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn filesystem-absolute-main! ()
            if (filesystem-read-error? |/workspace/input.txt) (println "|WASI-filesystem-absolute: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'filesystem-denied-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn filesystem-denied-main! ()
            if (filesystem-read-error? |workspace/input.txt) (println "|WASI-filesystem-denied: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'filesystem-invalid-utf8-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn filesystem-invalid-utf8-main! ()
            if (filesystem-read-error? |workspace/invalid.txt) (println "|WASI-filesystem-invalid-utf8: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'filesystem-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn filesystem-main! ()
            let
                input $ .read-text $ fs:path |workspace/input.txt
                output $ .write-text (fs:path |workspace/output.txt) "|WASI-written: 好"
                input-ok? $ match input
                  (:ok content) (= content "|WASI-file: 你好")
                  (:err _) false
                output-ok? $ match output
                  (:ok _) true
                  (:err _) false
              if (and input-ok? output-ok?) (println "|WASI-filesystem: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'filesystem-read-dir-error-main! $ %{} 'CodeEntry (:doc "|通过 WASI 假宿主验证畸形目录记录仍返回 Result :err。")
          :code $ quote $ defn filesystem-read-dir-error-main! ()
            if (filesystem-read-dir-error? |workspace/listing) (println "|WASI-read-dir-error: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :file :wasi
        'filesystem-read-dir-error? $ %{} 'CodeEntry (:doc "|验证目录枚举失败仍以 Result :err 表达。")
          :code $ quote $ defn filesystem-read-dir-error? (path)
            match
              .read-dir $ fs:path path
              (:ok _) false
              (:err _) true
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'String
          :tags $ #{} :file :unit :wasi
          :tests $ [] $ %{} 'TestEntry (:name |missing-directory)
            :code $ quote $ assert= true (filesystem-read-dir-error? |/calcit-wasi-filesystem-does-not-exist)
            :tags $ #{} :file :unit :wasi
        'filesystem-read-dir-main! $ %{} 'CodeEntry (:doc "|通过真实与假 WASI 宿主验证目录分页、UTF-8 名称和确定排序。")
          :code $ quote $ defn filesystem-read-dir-main! ()
            match
              .read-dir $ fs:path |workspace/listing
              (:ok paths)
                if
                  and
                    = 3 $ count paths
                    = |workspace/listing/a.txt $ &struct:nth (&list:nth paths 0) 0 :value
                    = |workspace/listing/b.txt $ &struct:nth (&list:nth paths 1) 0 :value
                    = "|workspace/listing/子.txt" $ &struct:nth (&list:nth paths 2) 0 :value
                  println "|WASI-read-dir: ok"
                  quit! 1
              (:err _) (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :file :wasi
        'filesystem-read-error? $ %{} 'CodeEntry
          :doc "|验证 FsPath 文本读取失败仍以 Result :err 表达，不把 host error 泄漏为异常。"
          :code $ quote $ defn filesystem-read-error? (path)
            match
              .read-text $ fs:path path
              (:ok _) false
              (:err _) true
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |missing-path)
            :code $ quote $ assert= true
              filesystem-read-error? |/calcit-wasi-filesystem-does-not-exist/input.txt
            :tags $ #{} :file :unit :wasi
        'filesystem-traversal-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn filesystem-traversal-main! ()
            if (filesystem-read-error? |workspace/../secret.txt) (println "|WASI-filesystem-traversal: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println |WASI-stdout: "|你好") (echo |WASI-echo) (eprintln |WASI-stderr: 42)
            println |WASI-env: $ option:unwrap-or (get-env |CALCIT_WASI_TEST_ENV) |missing
            each (get-args)
              fn (arg) (println |WASI-arg: arg)
            + 1 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |returns-three)
            :code $ quote $ assert= 3 (main!)
            :tags $ #{} :core :unit :wasi :wasm
        'needs-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn needs-arg (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |returns-input)
            :code $ quote $ assert= 3 (needs-arg 3)
            :tags $ #{} :core :unit :wasi :wasm
        'number-fits-unknown-main! $ %{} 'CodeEntry (:doc "|验证 WASM 中未知 refinement tag 返回 false 而不是 trap。")
          :code $ quote $ defn number-fits-unknown-main! ()
            let
                refinement :not-a-refinement
              if (&number:fits? 3 refinement) (quit! 1) (println |WASI-number-fits-unknown:-ok)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :wasi :wasm
        'random-error? $ %{} 'CodeEntry (:doc "|验证越界的安全随机请求返回 String 错误。")
          :code $ quote $ defn random-error? (size)
            match (secure-random-bytes size)
              (:ok _) false
              (:err message) (string? message)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Number
          :tags $ #{} :core :crypto :unit :wasi :wasm
          :tests $ []
            %{} 'TestEntry (:name |rejects-negative-length)
              :code $ quote $ assert= true (random-error? -1)
              :tags $ #{} :core :crypto :unit :wasi :wasm
            %{} 'TestEntry (:name |rejects-oversized-length)
              :code $ quote $ assert= true (random-error? 65537)
              :tags $ #{} :core :crypto :unit :wasi :wasm
            %{} 'TestEntry (:name |rejects-fractional-length)
              :code $ quote $ assert= true (random-error? 1.5)
              :tags $ #{} :core :crypto :unit :wasi :wasm
        'random-fixed-main! $ %{} 'CodeEntry (:doc "|由确定性 WASI 假宿主验证四字节随机请求。")
          :code $ quote $ defn random-fixed-main! ()
            if (random-success? 4) (println "|secure-random-fixed: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :crypto :wasi
        'random-main! $ %{} 'CodeEntry (:doc "|验证安全随机 API 的成功与越界 Result 语义。")
          :code $ quote $ defn random-main! ()
            if
              and (random-success? 16) (random-error? 65537)
              println "|secure-random: ok"
              quit! 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :crypto :wasi
        'random-success? $ %{} 'CodeEntry (:doc "|验证指定长度的安全随机请求返回 Buffer。")
          :code $ quote $ defn random-success? (size)
            match (secure-random-bytes size)
              (:ok bytes) (buffer? bytes)
              (:err _) false
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Number
          :tags $ #{} :core :crypto :unit :wasi :wasm
          :tests $ []
            %{} 'TestEntry (:name |returns-empty-buffer)
              :code $ quote $ assert= true (random-success? 0)
              :tags $ #{} :core :crypto :unit :wasi :wasm
            %{} 'TestEntry (:name |returns-buffer)
              :code $ quote $ assert= true (random-success? 16)
              :tags $ #{} :core :crypto :unit :wasi :wasm
        'read-args $ %{} 'CodeEntry (:doc "|读取当前宿主进程的完整参数列表。")
          :code $ quote $ defn read-args () (get-args)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :features $ #{} :env :io
            :return $ :: 'List 'String
          :tests $ [] $ %{} 'TestEntry (:name |returns-strings)
            :code $ quote $ assert= true
              every? (read-args) string?
            :tags $ #{} :core :env :unit :wasi :wasm
        'read-clocks $ %{} 'CodeEntry (:doc "|读取系统时钟与单调时钟的毫秒值。")
          :code $ quote $ defn read-clocks ()
            [] (unix-time-ms) (cpu-time)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ []
            :return $ :: 'List 'Number
          :tags $ #{} :time :wasi
          :tests $ [] $ %{} 'TestEntry (:name |returns-numbers)
            :code $ quote $ assert= true
              every? (read-clocks) number?
            :tags $ #{} :core :time :unit :wasi :wasm
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'wait-error? $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wait-error? (duration)
            match (wait-ms duration)
              (:ok _) false
              (:err message) (string? message)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Number
          :tags $ #{} :core :io :time :unit :wasi
          :tests $ []
            %{} 'TestEntry (:name |rejects-negative)
              :code $ quote $ assert= true (wait-error? -1)
              :tags $ #{} :core :time :unit :wasi
            %{} 'TestEntry (:name |rejects-fractional)
              :code $ quote $ assert= true (wait-error? 1.5)
              :tags $ #{} :core :time :unit :wasi
            %{} 'TestEntry (:name |rejects-overflow)
              :code $ quote $ assert= true (wait-error? 4294967296)
              :tags $ #{} :core :time :unit :wasi
        'wait-failure-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wait-failure-main! ()
            if (wait-error? 1) (println "|WASI-wait-error: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :io :time :wasi
        'wait-fixed-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wait-fixed-main! ()
            if
              and (wait-success? 0) (wait-success? 4294967295)
              println "|WASI-fixed-wait: ok"
              quit! 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :io :time :wasi
        'wait-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wait-main! ()
            if (wait-success? 1) (println "|WASI-wait: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tags $ #{} :io :time :wasi
        'wait-success? $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wait-success? (duration)
            match (wait-ms duration)
              (:ok _) true
              (:err _) false
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'Number
          :tags $ #{} :core :io :time :unit :wasi
          :tests $ []
            %{} 'TestEntry (:name |zero-succeeds)
              :code $ quote $ assert= true (wait-success? 0)
              :tags $ #{} :core :time :unit :wasi
            %{} 'TestEntry (:name |positive-succeeds)
              :code $ quote $ assert= true (wait-success? 1)
              :tags $ #{} :core :time :unit :wasi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
