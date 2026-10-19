
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
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
