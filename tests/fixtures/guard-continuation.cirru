
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
          :schema $ quote $ Fn Dynamic (List Dynamic) (-> Dynamic)
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
          :schema $ quote $ Fn Dynamic (List Dynamic) (-> Dynamic)
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
          :schema $ quote $ Fn Dynamic (List Dynamic) (-> Dynamic)
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
          :schema $ quote $ Fn Dynamic (List Dynamic) (-> Dynamic)
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
          :schema $ quote $ Fn Dynamic (List Dynamic) (-> Dynamic)
          :tests $ []
            %{} 'TestEntry (:name |argument-order-and-call-count)
              :code $ quote $ let
                  order $ ref |
                  callback $ fn (a b)
                    hint-fn $ quote $ Fn Number Number (-> Number)
                    swap! order $ fn (text)
                      hint-fn $ quote $ Fn String (-> String)
                      str text |C
                    &+ a b
                  result $ call-value callback $ []
                    do
                      swap! order $ fn (text)
                        hint-fn $ quote $ Fn String (-> String)
                        str text |L
                      , 3
                    do
                      swap! order $ fn (text)
                        hint-fn $ quote $ Fn String (-> String)
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
                            hint-fn $ quote $ Fn String (-> String)
                            str text |args
                          []
                      , |missing-error
                    fn (message)
                      hint-fn $ quote $ Fn String (-> String)
                      , message
                assert= |expected-function message
                assert= |headargs $ deref order
              :tags $ #{} :guard-continuation :unit
            %{} 'TestEntry (:name |preserves-none-and-some-nil)
              :code $ quote $ let
                  present $ Option :some nil
                  absent $ Option :none
                  forward $ fn (value)
                    hint-fn $ quote $ Fn (Option Nil)
                      -> $ Option Nil
                    , value
                assert= true $ identical? present $ call-value forward ([] present)
                assert= true $ identical? absent $ call-value forward ([] absent)
              :tags $ #{} :guard-continuation :unit
        'check-loop-each-iteration! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-loop-each-iteration! (value)
            loop
                i 0
              if (< i 2)
                do
                  when
                    not $ number? value
                    raise |expected-number
                  &+ value 1
                  set! value |changed
                  recur $ inc i
                , &unit
            , &unit
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Unit) (:features :js-ffi)
          :tests $ [] $ %{} 'TestEntry (:name |guard-runs-again-after-capture-mutation)
            :code $ quote $ let
                message $ try
                  do (check-loop-each-iteration! 1) |missing-error
                  fn (message)
                    hint-fn $ quote $ Fn String (-> String)
                    , message
              assert= |expected-number message
            :tags $ #{} :guard-continuation :js :unit
        'count-checked-loop $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-checked-loop (value)
            when
              not $ list? value
              raise |expected-list
            loop
                idx 0
              if
                < idx $ .len value
                let
                    entry $ .get value idx
                  recur $ .inc idx
                , idx
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |immediate-loop-keeps-captured-list-evidence)
            :code $ quote $ assert= 2
              count-checked-loop $ [] nil false
            :tags $ #{} :guard-continuation :unit
        'count-items $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-items (items) (.len items)
          :examples $ []
          :schema $ quote $ Fn (List Dynamic) (-> Number)
        'inc-checked $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn inc-checked (value)
            when
              not $ number? value
              raise |expected-number
            .inc value
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Number)
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
                  hint-fn $ quote $ Fn (-> Fn)
                  , checked
              set! f 1
              get-value
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Fn) (:features :js-ffi)
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
          :schema $ quote $ Fn
            Fn Number $ -> Number
            -> $ Fn Number $ -> Number
          :tests $ []
            %{} 'TestEntry (:name |guard-keeps-known-signature)
              :code $ quote $ let
                  callback $ keep-signature $ fn (value)
                    hint-fn $ quote $ Fn Number (-> Number)
                    .inc value
                assert= 8 $ callback 7
              :tags $ #{} :guard-continuation :unit
            %{} 'TestEntry
              :name |declared-contract-survives-annotation-and-effects
              :code $ quote $ let
                  mark $ ref 0
                  callback $ keep-signature $ fn (value)
                    hint-fn $ quote $ Fn Number (-> Number)
                    .inc value
                assert-type callback 'Fn
                reset! mark 1
                assert= 8 $ callback 7
                assert= 1 $ deref mark
              :tags $ #{} :guard-continuation :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'read-checked $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-checked (value)
            when
              not $ list? value
              raise |expected-list
            let
                entry $ .get value 0
              .len value
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |option-construction-keeps-list-evidence)
            :code $ quote $ do
              assert= 0 $ read-checked $ []
              assert= 2 $ read-checked $ [] nil false
            :tags $ #{} :guard-continuation :unit
        'read-list-slot $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-list-slot (value index)
            when
              not $ list? value
              raise |expected-list
            if (.contains-index? value index) (.get value index) (Option :none)
          :examples $ []
          :schema $ quote $ Fn Dynamic Number
            -> $ Option Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |readonly-index-query-preserves-list-guard)
            :code $ quote $ do
              assert= (Option :some nil)
                read-list-slot ([] nil false) 0
              assert= (Option :some false)
                read-list-slot ([] nil false) 1
              assert= (Option :none)
                read-list-slot ([] nil false) 2
              assert= (Option :none)
                read-list-slot ([]) 0
              assert= |expected-list $ try (read-list-slot 1 0)
                fn (error) error
            :tags $ #{} :guard-continuation :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
