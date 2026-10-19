
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'call-after-assert $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-after-assert (f args)
            assert |expected-function $ fn? f
            f & args
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'List 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |uses-expanded-core-not-macro-name)
            :code $ quote $ let
                value $ call-after-assert
                  fn () |ok
                  []
              assert |expected-string $ string? value
              assert= |ok value
            :tags $ #{} :guard-continuation :unit
        'call-after-block $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-after-block (f args)
            let
                items args
              when
                not $ fn? f
                raise |expected-function
              count items
            f & args
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'List 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |carries-common-normal-exit-evidence)
            :code $ quote $ let
                value $ call-after-block
                  fn () |ok
                  []
              assert |expected-string $ string? value
              assert= |ok value
            :tags $ #{} :guard-continuation :unit
        'call-after-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-after-read (f args)
            when
              not $ fn? f
              raise |expected-function
            count-items args
            f & args
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'List 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |proven-readonly-call-keeps-guard)
            :code $ quote $ let
                value $ call-after-read
                  fn () 7
                  []
              assert |expected-number $ number? value
              assert= 7 value
            :tags $ #{} :guard-continuation :unit
        'call-in-let $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-in-let (f args)
            let
                items args
              when
                not $ fn? f
                raise |expected-function
              f & items
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'List 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |keeps-guard-in-lexical-sequence)
            :code $ quote $ let
                value $ call-in-let
                  fn () |ok
                  []
              assert |expected-string $ string? value
              assert= |ok value
            :tags $ #{} :guard-continuation :unit
        'call-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-value (f args)
            when
              not $ fn? f
              raise |expected-function
            f & args
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'List 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |argument-order-and-call-count)
              :code $ quote $ let
                  order $ ref |
                  callback $ fn (a b)
                    hint-fn $ {}
                      :args $ [] 'Number 'Number
                      :return 'Number
                    swap! order $ fn (text)
                      hint-fn $ {}
                        :args $ [] 'String
                        :return 'String
                      str text |C
                    &+ a b
                  result $ call-value callback $ []
                    do
                      swap! order $ fn (text)
                        hint-fn $ {}
                          :args $ [] 'String
                          :return 'String
                        str text |L
                      , 3
                    do
                      swap! order $ fn (text)
                        hint-fn $ {}
                          :args $ [] 'String
                          :return 'String
                        str text |R
                      , 4
                assert |expected-numeric-result $ number? result
                assert= 7 result
                assert= |LRC $ deref order
              :tags $ #{} :guard-continuation :unit
            %{} 'TestEntry (:name |rejects-non-functions-with-original-error)
              :code $ quote $ let
                  order $ ref |
                  message $ try
                    do
                      call-value
                        do (reset! order |head) 7
                        do
                          swap! order $ fn (text)
                            hint-fn $ {}
                              :args $ [] 'String
                              :return 'String
                            str text |args
                          []
                      , |missing-error
                    fn (message)
                      hint-fn $ {}
                        :args $ [] 'String
                        :return 'String
                      , message
                assert= |expected-function message
                assert= |headargs $ deref order
              :tags $ #{} :guard-continuation :unit
            %{} 'TestEntry (:name |preserves-none-and-some-nil)
              :code $ quote $ let
                  present $ Option :some nil
                  absent $ Option :none
                  forward $ fn (value)
                    hint-fn $ {}
                      :args $ [] $ :: 'Option 'Nil
                      :return $ :: 'Option 'Nil
                    , value
                assert= true $ identical? present $ call-value forward ([] present)
                assert= true $ identical? absent $ call-value forward ([] absent)
              :tags $ #{} :guard-continuation :unit
        'count-checked-loop $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-checked-loop (value)
            when
              not $ list? value
              raise |expected-list
            loop
                idx 0
              if
                < idx $ .count value
                let
                    entry $ .get value idx
                  recur $ .inc idx
                , idx
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |immediate-loop-keeps-captured-list-evidence)
            :code $ quote $ assert= 2
              count-checked-loop $ [] nil false
            :tags $ #{} :guard-continuation :unit
        'count-items $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-items (items) (.count items)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Dynamic
        'inc-checked $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn inc-checked (value)
            when
              not $ number? value
              raise |expected-number
            .inc value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |uses-number-guard-for-method-lowering)
            :code $ quote $ assert= 8 (inc-checked 7)
            :tags $ #{} :guard-continuation :unit
        'keep-checked $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn keep-checked (f)
            when
              not $ fn? f
              raise |expected-function
            let
                checked f
                get-value $ fn ()
                  hint-fn $ {}
                    :args $ []
                    :return 'Fn
                  , checked
              set! f 1
              get-value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Fn)
            :args $ [] 'Dynamic
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry
            :name |checked-value-is-independent-of-original-binding
            :code $ quote $ let
                original $ fn () 7
              assert= true $ identical? original $ keep-checked original
            :tags $ #{} :guard-continuation :js :unit
        'keep-signature $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn keep-signature (f)
            when
              not $ fn? f
              raise |expected-function
            , f
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Fn
              {} (:return 'Number)
                :args $ [] 'Number
            :return $ :: 'Fn $ {} (:return 'Number)
              :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |guard-keeps-known-signature)
            :code $ quote $ let
                callback $ keep-signature $ fn (value)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Number
                  .inc value
              assert= 8 $ callback 7
            :tags $ #{} :guard-continuation :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'read-checked $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-checked (value)
            when
              not $ list? value
              raise |expected-list
            let
                entry $ .get value 0
              .count value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |option-construction-keeps-list-evidence)
            :code $ quote $ do
              assert= 0 $ read-checked $ []
              assert= 2 $ read-checked $ [] nil false
            :tags $ #{} :guard-continuation :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
