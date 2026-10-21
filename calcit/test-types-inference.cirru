
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-types-inference
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-types-inference.main/main!) (:mode :native) (:reload-fn 'test-types-inference.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'test-types-inference.main $ %{} 'FileEntry
      :defs $ {}
        '*dynamic-ref $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defatom *dynamic-ref (make-dynamic-value)
          :examples $ []
          :schema $ :: 'Ref 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |dynamic-atom-deref)
            :code $ quote $ do
              assert-type *dynamic-ref $ :: 'Ref 'Dynamic
              assert= 1 $ deref *dynamic-ref
              assert= 1 @*dynamic-ref
        'Address $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Address (:city 'String)
          :examples $ []
          :schema $ :: 'StructDef
        'CallbackEnvelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct CallbackEnvelope ([] 'T)
            :holder $ :: 'test-types-inference.main/CallbackHolder 'T
          :examples $ []
          :schema $ :: 'StructDef
        'CallbackHolder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct CallbackHolder ([] 'T)
            :callback $ :: 'Fn $ {} (:return 'Number)
              :args $ [] $ :: 'Fn
                {} (:return 'Number)
                  :args $ [] 'T
          :examples $ []
          :schema $ :: 'StructDef
        'Job $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Job (:title 'String) (:status Status)
          :examples $ []
          :schema $ :: 'StructDef
        'NominalAbsenceFields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NominalAbsenceFields
            :number $ :: 'Option 'Number
            :text $ :: 'Option 'String
            :result $ :: 'Result 'Number 'String
          :examples $ []
          :schema $ :: 'StructDef
        'Outcome $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Outcome (:status Status) (:none)
          :examples $ []
          :schema $ :: 'EnumDef
        'Person $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Person (:name 'String) (:age 'Number)
            :address $ :: Address
          :examples $ []
          :schema $ :: 'StructDef
        'PersonWrap $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum PersonWrap
            :person $ :: Person
            :none
          :examples $ []
          :schema $ :: 'EnumDef
        'SliceReceiver $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct SliceReceiver (:n 'Number)
            :end $ :: 'Option 'Number
          :examples $ []
          :schema $ :: 'StructDef
        'SliceReceiverImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl SliceReceiverImpl Sliceable
            .slice $ fn (self start end)
              hint-fn $ {}
                :args $ [] 'test-types-inference.main/SliceReceiver 'Number $ :: 'Option 'Number
                :return 'test-types-inference.main/SliceReceiver
              struct-with self
                :n $ + (:n self) start
                :end end
          :examples $ []
          :schema $ :: 'Impl
        'SliceReceiverWithMethods $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def SliceReceiverWithMethods (impl-traits SliceReceiver SliceReceiverImpl)
          :examples $ []
          :schema $ :: 'StructDef
        'Status $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Status (:ok 'Number) (:err 'String)
          :examples $ []
          :schema $ :: 'EnumDef
        'bounded-forward $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bounded-forward (value) (bounded-identity value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'C)
            :args $ [] 'C
            :generics $ [] 'C
            :where $ {} $ 'C 'Sliceable
          :tests $ [] $ %{} 'TestEntry (:name |forwards-the-same-variable)
            :code $ quote $ assert= ([] |a |b)
              bounded-forward $ [] |a |b
            :tags $ #{} :bounded-binder :generic :lexical-binder :unit
        'bounded-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bounded-identity (value) value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'C)
            :args $ [] 'C
            :generics $ [] 'C
            :where $ {} $ 'C 'Sliceable
          :tests $ []
            %{} 'TestEntry (:name |preserves-list-payload)
              :code $ quote $ assert= ([] 1 2)
                bounded-identity $ [] 1 2
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
            %{} 'TestEntry (:name |preserves-string)
              :code $ quote $ assert= "|你好" (bounded-identity "|你好")
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
            %{} 'TestEntry (:name |preserves-empty-list)
              :code $ quote $ assert= ([])
                bounded-identity $ []
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
        'bounded-local-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bounded-local-map (value)
            let
                shorten $ fn (item)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return 'T
                    :generics $ [] 'T
                    :where $ {} $ 'T Sliceable
                  .slice item 0 $ Option :some 1
              shorten value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'C)
            :args $ [] 'C
            :generics $ [] 'C
            :where $ {} $ 'C 'Sliceable
          :tests $ []
            %{} 'TestEntry (:name |local-map-keeps-list-payload)
              :code $ quote $ assert= ([] 1)
                bounded-local-map $ [] 1 2
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
            %{} 'TestEntry (:name |local-map-keeps-string)
              :code $ quote $ assert= |a (bounded-local-map |ab)
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
        'bounded-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bounded-map (value)
            map ([] value)
              fn (item)
                hint-fn $ {}
                  :args $ [] 'T
                  :return 'T
                  :generics $ [] 'T
                  :where $ {} $ 'T Sliceable
                , item
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'C
            :generics $ [] 'C
            :return $ :: 'List 'C
            :where $ {} $ 'C 'Sliceable
          :tests $ []
            %{} 'TestEntry (:name |bounded-callback-keeps-list-payload)
              :code $ quote $ assert=
                [] $ [] 1 2
                bounded-map $ [] 1 2
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
            %{} 'TestEntry (:name |bounded-callback-keeps-string)
              :code $ quote $ assert= ([] |ab) (bounded-map |ab)
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
        'bounded-multiple $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bounded-multiple (value)
            assert-type (.debug value) 'String
            .slice value 0 $ Option :some 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'C)
            :args $ [] 'C
            :generics $ [] 'C
            :where $ {} $ 'C ([] 'Debug 'Sliceable)
          :tests $ [] $ %{} 'TestEntry (:name |both-capabilities-keep-one-identity)
            :code $ quote $ assert= ([] 1)
              bounded-multiple $ [] 1 2
            :tags $ #{} :bounded-binder :generic :lexical-binder :unit
        'bounded-shadow-capture $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bounded-shadow-capture (value)
            let
                describe $ fn (other)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return 'String
                    :generics $ [] 'T
                  .debug value
              assert-type (describe 1) 'String
              , value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] 'T
            :generics $ [] 'T
            :where $ {} $ 'T 'Debug
          :tests $ []
            %{} 'TestEntry (:name |shadowing-retains-captured-number-bound)
              :code $ quote $ assert= 1 (bounded-shadow-capture 1)
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
            %{} 'TestEntry (:name |shadowing-retains-captured-string-bound)
              :code $ quote $ assert= |outer (bounded-shadow-capture |outer)
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
        'bounded-slice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bounded-slice (value)
            value .slice 0 $ Option :some 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'C)
            :args $ [] 'C
            :generics $ [] 'C
            :where $ {} $ 'C 'Sliceable
          :tests $ []
            %{} 'TestEntry (:name |method-keeps-list-payload)
              :code $ quote $ assert= ([] 1)
                bounded-slice $ [] 1 2
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
            %{} 'TestEntry (:name |method-keeps-unicode-string)
              :code $ quote $ assert= "|你" (bounded-slice "|你好")
              :tags $ #{} :bounded-binder :generic :lexical-binder :unit
        'call-callback-envelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-callback-envelope (dispatch! envelope)
            call-callback-holder dispatch! $ :holder envelope
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
              :: 'Fn $ {} (:return 'Number)
                :args $ [] 'T
              :: 'test-types-inference.main/CallbackEnvelope 'T
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |nested-nominal-callback-keeps-number-input)
            :code $ quote $ assert= 8
              call-callback-envelope
                fn (n)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Number
                  &+ n 1
                CallbackEnvelope :holder $ CallbackHolder :callback $ fn (dispatch!) (dispatch! 7)
            :tags $ #{} :nominal-callback-context :unit
        'call-callback-holder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-callback-holder (dispatch! holder)
            (:callback holder) dispatch!
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
              :: 'Fn $ {} (:return 'Number)
                :args $ [] 'T
              :: 'test-types-inference.main/CallbackHolder 'T
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |inline-nominal-callback-keeps-number-input)
            :code $ quote $ assert= 8
              call-callback-holder
                fn (n)
                  hint-fn $ {}
                    :args $ [] 'Number
                    :return 'Number
                  &+ n 1
                CallbackHolder :callback $ fn (dispatch!) (dispatch! 7)
            :tags $ #{} :nominal-callback-context :unit
        'consume-number-callback-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn consume-number-callback-map (callbacks)
            match (get callbacks :click)
              (:none) 0
              (:some callback) (callback 7)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'Map 'Tag
              :: 'Fn $ {} (:return 'Number)
                :args $ [] 'Number
          :tests $ [] $ %{} 'TestEntry (:name |inline-map-callback-preserves-number-input)
            :code $ quote $ assert= 8
              consume-number-callback-map $ &{} :click $ fn (n) (&+ n 1)
            :tags $ #{} :map-callback-context :unit
        'dynamic-last-compat $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn dynamic-last-compat (value) (.last value)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Dynamic
            :return $ :: 'Option 'Dynamic
        'first-rest-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn first-rest-value (label & values) (first values)
          :examples $ []
          :schema $ :: 'Fn $ {} (:rest 'String)
            :args $ [] 'String
            :return $ :: 'Option 'String
          :tests $ []
            %{} 'TestEntry (:name |no-rest)
              :code $ quote $ assert= (Option :none) (first-rest-value |prefix)
              :tags $ #{} :unit
            %{} 'TestEntry (:name |direct-rest)
              :code $ quote $ assert= (Option :some |one) (first-rest-value |prefix |one |two)
              :tags $ #{} :unit
            %{} 'TestEntry (:name |list-rest)
              :code $ quote $ assert= (Option :some |one)
                spread-first-rest $ [] |one |two
              :tags $ #{} :unit
            %{} 'TestEntry (:name |empty-list)
              :code $ quote $ assert= (Option :none)
                spread-first-rest $ []
              :tags $ #{} :unit
            %{} 'TestEntry (:name |literal-rest)
              :code $ quote $ assert= (Option :some |one)
                first-rest-value |prefix & $ [] |one |two
              :tags $ #{} :unit
            %{} 'TestEntry (:name |explicit-before-list)
              :code $ quote $ assert= (Option :some |one)
                first-rest-value |prefix |one & $ [] |two |three
              :tags $ #{} :unit
            %{} 'TestEntry (:name |single-evaluation)
              :code $ quote $ let
                  calls $ ref 0
                  supply $ fn ()
                    hint-fn $ {}
                      :args $ []
                      :return $ :: 'List 'String
                    swap! calls inc
                    [] |one
                assert= (Option :some |one)
                  first-rest-value |prefix & $ supply
                assert= 1 $ deref calls
              :tags $ #{} :unit
        'infer-later-dynamic-generic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-later-dynamic-generic (value) (same-type-second 7 value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |dynamic-second-is-returned)
            :code $ quote $ do
              assert= |hello $ infer-later-dynamic-generic |hello
              assert= 4 $ infer-later-dynamic-generic 4
            :tags $ #{} :unit
        'infer-match-raise $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-match-raise (enabled)
            let
                parsed $ if enabled (%ok 1) (%err |missing)
                value $ match parsed
                  (:ok n) (%some n)
                  (:err error) (raise error)
              value .unwrap
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |preserves-live-option-and-error)
            :code $ quote $ do
              assert= 1 $ infer-match-raise true
              assert= true $ try (infer-match-raise false)
                fn (error) (= error |missing)
            :tags $ #{} :unit
        'infer-named-none-fallback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-named-none-fallback ()
            option:unwrap-or (Option :none) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |named-none-fallback)
            :code $ quote $ assert= 7 (infer-named-none-fallback)
            :tags $ #{} :unit
        'infer-named-result-err-fallback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-named-result-err-fallback ()
            result:unwrap-or (Result :err 3) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |named-err-fallback-number)
            :code $ quote $ assert= 7 (infer-named-result-err-fallback)
            :tags $ #{} :unit
        'infer-named-result-ok-concrete $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-named-result-ok-concrete ()
            result:unwrap-or (Result :ok 9) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |retains-named-concrete-success)
            :code $ quote $ assert= 9 (infer-named-result-ok-concrete)
            :tags $ #{} :unit
        'infer-named-result-ok-open $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-named-result-ok-open (value)
            result:unwrap-or (Result :ok value) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |preserves-named-success-payload)
            :code $ quote $ do
              assert= |hello $ infer-named-result-ok-open |hello
              assert= 4 $ infer-named-result-ok-open 4
            :tags $ #{} :unit
        'infer-none-fallback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-none-fallback ()
            option:unwrap-or (%none) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |none-fallback-number)
            :code $ quote $ assert= 7 (infer-none-fallback)
            :tags $ #{} :unit
        'infer-option-none-predicate $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-option-none-predicate ()
            option:none? $ %some 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |predicate-contract)
            :code $ quote $ assert= false (infer-option-none-predicate)
            :tags $ #{} :unit
        'infer-option-presence-without-nil $ %{} 'CodeEntry
          :doc "|Option presence is checked with methods or match, not by comparing with nil."
          :code $ quote $ defn infer-option-presence-without-nil (enabled)
            let
                value $ if enabled (Option :some 1) (Option :none)
              match value
                (:some n) n
                (:none) 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |option-presence-without-nil)
            :code $ quote $ do
              assert= 1 $ infer-option-presence-without-nil true
              assert= 0 $ infer-option-presence-without-nil false
              assert= true $ .some? $ Option :some 1
              assert= true $ .none? $ Option :none
              assert= 7 $ .unwrap-or (Option :none) 7
              assert= 1 $ .unwrap-or (Option :some 1) 7
            :tags $ #{} :unit
        'infer-option-some-predicate $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-option-some-predicate ()
            option:some? $ %some 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |predicate-contract)
            :code $ quote $ assert= true (infer-option-some-predicate)
            :tags $ #{} :unit
        'infer-raise-left $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-raise-left (enabled)
            let
                value $ if enabled (raise |missing-left) (%some 2)
              value .unwrap
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |retains-option-and-raise)
            :code $ quote $ do
              assert= 2 $ infer-raise-left false
              assert= true $ try (infer-raise-left true)
                fn (error) true
            :tags $ #{} :unit
        'infer-raise-right $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-raise-right (enabled)
            let
                value $ if enabled (%some 1) (raise |missing-right)
              value .unwrap
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Bool
          :tests $ [] $ %{} 'TestEntry (:name |retains-option-and-raise)
            :code $ quote $ do
              assert= 1 $ infer-raise-right true
              assert= true $ try (infer-raise-right false)
                fn (error) true
            :tags $ #{} :unit
        'infer-result-err-fallback $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-result-err-fallback ()
            result:unwrap-or (%err 3) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |err-fallback-number)
            :code $ quote $ assert= 7 (infer-result-err-fallback)
            :tags $ #{} :unit
        'infer-result-err-predicate $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-result-err-predicate ()
            result:err? $ %ok 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |predicate-contract)
            :code $ quote $ assert= false (infer-result-err-predicate)
            :tags $ #{} :unit
        'infer-result-ok-concrete $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-result-ok-concrete ()
            result:unwrap-or (%ok 9) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |retains-concrete-success)
            :code $ quote $ assert= 9 (infer-result-ok-concrete)
            :tags $ #{} :unit
        'infer-result-ok-open $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-result-ok-open (value)
            result:unwrap-or (%ok value) 7
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
          :tests $ [] $ %{} 'TestEntry (:name |preserves-success-payload)
            :code $ quote $ do
              assert= |hello $ infer-result-ok-open |hello
              assert= 4 $ infer-result-ok-open 4
            :tags $ #{} :unit
        'infer-result-ok-predicate $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn infer-result-ok-predicate ()
            result:ok? $ %ok 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |predicate-contract)
            :code $ quote $ assert= true (infer-result-ok-predicate)
            :tags $ #{} :unit
        'lexical-generic-capture $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-generic-capture (value)
            let
                callback $ fn ()
                  hint-fn $ {}
                    :args $ []
                    :return 'T
                  , value
              callback
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] 'T
            :generics $ [] 'T
          :tests $ []
            %{} 'TestEntry (:name |preserves-outer-number)
              :code $ quote $ assert= 1 (lexical-generic-capture 1)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |preserves-outer-string)
              :code $ quote $ assert= |outer (lexical-generic-capture |outer)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |preserves-outer-list)
              :code $ quote $ assert= ([] |outer)
                lexical-generic-capture $ [] |outer
              :tags $ #{} :generic :lexical-binder :unit
        'lexical-generic-empty-collections $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-generic-empty-collections (value)
            let
                empty-list $ fn (item)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return $ :: 'List 'T
                    :generics $ [] 'T
                  []
                empty-map $ fn (item)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return $ :: 'Map 'Tag 'T
                    :generics $ [] 'T
                  if (= item item) ({}) ({})
                empty-set $ fn (item)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return $ :: 'Set 'T
                    :generics $ [] 'T
                  #{}
              assert= ({}) (empty-map value)
              assert= (#{}) (empty-set value)
              empty-list value
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'T
            :generics $ [] 'T
            :return $ :: 'List 'T
          :tests $ []
            %{} 'TestEntry (:name |empty-collections-number)
              :code $ quote $ assert= ([]) (lexical-generic-empty-collections 1)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |empty-collections-string)
              :code $ quote $ assert= ([]) (lexical-generic-empty-collections |outer)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |empty-collections-list)
              :code $ quote $ assert= ([])
                lexical-generic-empty-collections $ [] |outer
              :tags $ #{} :generic :lexical-binder :unit
        'lexical-generic-grid $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-generic-grid (x y f)
            -> (range x)
              mapcat $ fn (xi)
                -> (range y)
                  map $ fn (yi) (f xi yi)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] 'Number 'Number $ :: 'Fn
              {} (:return 'T)
                :args $ [] 'Number 'Number
            :generics $ [] 'T
            :return $ :: 'List 'T
          :tests $ []
            %{} 'TestEntry (:name |preserves-number-order)
              :code $ quote $ assert= ([] 0 1 10 11)
                lexical-generic-grid 2 2 $ fn (x y)
                  + (* x 10) y
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |preserves-string-result)
              :code $ quote $ assert= ([] |0:0 |0:1 |1:0 |1:1)
                lexical-generic-grid 2 2 $ fn (x y) (str x |: y)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |preserves-empty-dimensions)
              :code $ quote $ do
                assert= ([]) (lexical-generic-grid 0 2 +)
                assert= ([]) (lexical-generic-grid 2 0 +)
              :tags $ #{} :generic :lexical-binder :unit
        'lexical-generic-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-generic-identity (value)
            let
                callback $ fn (other)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return 'T
                    :generics $ [] 'T
                  , other
              &+ (callback 1) 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'T
            :generics $ [] 'T
          :tests $ []
            %{} 'TestEntry (:name |independent-string-capture)
              :code $ quote $ assert= 2 (lexical-generic-identity |outer)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |independent-list-capture)
              :code $ quote $ assert= 2
                lexical-generic-identity $ [] |outer
              :tags $ #{} :generic :lexical-binder :unit
        'lexical-generic-identity-renamed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-generic-identity-renamed (value)
            let
                callback $ fn (other)
                  hint-fn $ {}
                    :args $ [] 'U
                    :return 'U
                    :generics $ [] 'U
                  , other
              &+ (callback 1) 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'T
            :generics $ [] 'T
          :tests $ [] $ %{} 'TestEntry (:name |independent-renamed-binder)
            :code $ quote $ assert= 2 (lexical-generic-identity-renamed |outer)
            :tags $ #{} :generic :lexical-binder :unit
        'lexical-generic-open-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-generic-open-identity (value)
            let
                callback $ fn (item)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return 'T
                    :generics $ [] 'T
                  , item
              callback value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |generic-instantiation-preserves-open-number)
              :code $ quote $ assert= 1 (lexical-generic-open-identity 1)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |generic-instantiation-preserves-open-string)
              :code $ quote $ assert= |outer (lexical-generic-open-identity |outer)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |generic-instantiation-preserves-open-list)
              :code $ quote $ assert= ([] |outer)
                lexical-generic-open-identity $ [] |outer
              :tags $ #{} :generic :lexical-binder :unit
        'lexical-generic-shadow-capture $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lexical-generic-shadow-capture (outer)
            hint-fn $ {}
              :args $ [] 'T
              :return 'T
            let
                inner $ fn (value)
                  hint-fn $ {}
                    :args $ [] 'T
                    :return 'T
                    :generics $ [] 'T
                  let
                      capture $ fn ()
                        hint-fn $ {}
                          :args $ []
                          :return 'T
                        , value
                    capture
                outer-capture $ fn ()
                  hint-fn $ {}
                    :args $ []
                    :return 'T
                  , outer
              assert= 1 $ inner 1
              outer-capture
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] 'T
            :generics $ [] 'T
          :tests $ []
            %{} 'TestEntry (:name |nearest-binder-and-restored-outer-string)
              :code $ quote $ assert= |outer (lexical-generic-shadow-capture |outer)
              :tags $ #{} :generic :lexical-binder :unit
            %{} 'TestEntry (:name |nearest-binder-and-restored-outer-list)
              :code $ quote $ assert= ([] |outer)
                lexical-generic-shadow-capture $ [] |outer
              :tags $ #{} :generic :lexical-binder :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println "|Testing type inference...") (test-list-inference) (test-legacy-optional-inference) (test-count-inference) (test-fn-inference) (test-map-inference) (test-filter-map-kv-inference) (test-set-inference) (test-ref-inference) (test-struct-inference) (test-type-ref-combos) (test-generics-identity) (infer-raise-right true) (infer-raise-left false)
            assert= 2 $ recur-types-safe
              Person :name |Ada :age 3 :address $ Address :city |X
              , 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'make-dynamic-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-dynamic-value () 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'option-recur-sum $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn option-recur-sum (n acc)
            if (&<= n 0) (acc.unwrap-or 0)
              recur (dec n)
                Option :some $ &+ (acc.unwrap-or 0) n
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number $ :: 'Option 'Number
          :tests $ []
            %{} 'TestEntry (:name |omitted-option-preserves-recursion)
              :code $ quote $ do
                assert= 15 $ option-recur-sum 5
                assert= 25 $ option-recur-sum 5 $ Option :some 10
                assert= 0 $ option-recur-sum 0
                , &unit
              :tags $ #{} :option-call-arity :unit
            %{} 'TestEntry (:name |immediate-option-completion)
              :code $ quote $ do
                assert= 7 $
                  fn (end)
                    hint-fn $ {}
                      :args $ [] $ :: 'Option 'Number
                      :return 'Number
                    end.unwrap-or 7
                assert= 11 $
                  fn (end)
                    hint-fn $ {}
                      :args $ [] $ :: 'Option 'Number
                      :return 'Number
                    end.unwrap-or 7
                  Option :some 11
                assert= 7 $
                  fn (end)
                    hint-fn $ {}
                      :args $ [] $ :: 'Option 'Number
                      :return 'Number
                    end.unwrap-or 7
                  Option :none
                assert= 6 $
                  fn (n acc)
                    hint-fn $ {}
                      :args $ [] 'Number $ :: 'Option 'Number
                      :return 'Number
                    if (&<= n 0) (acc.unwrap-or 0)
                      recur (dec n)
                        Option :some $ &+ (acc.unwrap-or 0) n
                  , 3
                assert= 16 $
                  fn (n acc)
                    hint-fn $ {}
                      :args $ [] 'Number $ :: 'Option 'Number
                      :return 'Number
                    if (&<= n 0) (acc.unwrap-or 0)
                      recur (dec n)
                        Option :some $ &+ (acc.unwrap-or 0) n
                  , 3 $ Option :some 10
                , &unit
              :tags $ #{} :option-call-arity :unit
        'option-recursive-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn option-recursive-count (n acc)
            if (&<= n 0) (acc.unwrap-or 7)
              &+ 1 $ option-recursive-count $ dec n
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number $ :: 'Option 'Number
          :tests $ [] $ %{} 'TestEntry (:name |omitted-option-preserves-recursion)
            :code $ quote $ do
              assert= 9 $ option-recursive-count 2
              assert= 99 $ option-recursive-count 0 $ Option :some 99
              assert= 9 $ option-recursive-count 2 $ Option :some 99
              , &unit
            :tags $ #{} :option-call-arity :unit
        'read-state-metadata $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-state-metadata (states)
            get-in states $ [] :data
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'Map 'K 'V
            :generics $ [] 'K 'V
            :return $ :: 'Result 'V 'PathError
          :tests $ []
            %{} 'TestEntry (:name |present-payloads-preserve-values)
              :code $ quote $ do
                assert= (Result :ok false)
                  read-state-metadata $ {} $ :data false
                assert= (Result :ok 0)
                  read-state-metadata $ {} $ :data 0
                assert= (Result :ok |)
                  read-state-metadata $ {} $ :data |
                assert=
                  Result :ok $ [] :root |board 7
                  read-state-metadata $ {} $ :data ([] :root |board 7)
                , &unit
              :tags $ #{} :generic-metadata-path :unit
            %{} 'TestEntry (:name |unrelated-keys-are-absent)
              :code $ quote $ do
                assert=
                  Result :err $ PathError :missing
                  read-state-metadata $ {} $ 1 |value
                assert=
                  Result :err $ PathError :missing
                  read-state-metadata $ {} $ |data |value
                assert=
                  Result :err $ PathError :missing
                  read-state-metadata $ {}
                , &unit
              :tags $ #{} :generic-metadata-path :unit
            %{} 'TestEntry (:name |nil-path-payload-stays-absent)
              :code $ quote $ assert=
                Result :err $ PathError :missing
                read-state-metadata $ {} $ :data nil
              :tags $ #{} :generic-metadata-path :unit
            %{} 'TestEntry (:name |generic-payload-remains-precise)
              :code $ quote $ let
                  value $ read-state-metadata $ {} (:data 7)
                assert-type value $ :: 'Result 'Number 'PathError
                assert= (Result :ok 7) value
              :tags $ #{} :generic-metadata-path :unit
            %{} 'TestEntry (:name |concrete-mismatching-path-is-absent)
              :code $ quote $ assert=
                Result :err $ PathError :missing
                get-in
                  {} $ 1 |value
                  [] :data
              :tags $ #{} :generic-metadata-path :unit
            %{} 'TestEntry (:name |path-inputs-evaluate-once)
              :code $ quote $ let
                  calls $ atom 0
                  base $ fn () (swap! calls inc)
                    {} $ :data 7
                  key $ fn () (swap! calls inc) :data
                assert= (Result :ok 7)
                  get-in (base)
                    [] $ key
                assert= 2 @calls
              :tags $ #{} :generic-metadata-path :unit
        'recur-types-safe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn recur-types-safe (person index)
            if (>= index 2) index $ recur person $ inc index
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Person 'Number
          :tests $ [] $ %{} 'TestEntry (:name |keeps-typed-recur-parameters)
            :code $ quote $ assert= 2
              recur-types-safe
                Person :name |Ada :age 3 :address $ Address :city |X
                , 0
            :tags $ #{} :type-inference :unit
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'same-nominal-absence-fields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn same-nominal-absence-fields (fields)
            let
                expected-number $ Option :none
                expected-text $ Option :none
                expected-result $ Result :ok 1
              assert-type expected-number $ :: 'Option 'Number
              assert-type expected-text $ :: 'Option 'String
              assert-type expected-result $ :: 'Result 'Number 'String
              and
                = expected-number $ :number fields
                = expected-text $ :text fields
                = expected-result $ :result fields
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'test-types-inference.main/NominalAbsenceFields
          :tests $ []
            %{} 'TestEntry (:name |same-typed-nominal-fields)
              :code $ quote $ assert= true
                same-nominal-absence-fields $ NominalAbsenceFields :number (Option :none) :text (Option :none) :result $ Result :ok 1
              :tags $ #{} :unit
            %{} 'TestEntry (:name |number-payload-is-different)
              :code $ quote $ assert= false
                same-nominal-absence-fields $ NominalAbsenceFields :number (Option :some 2) :text (Option :none) :result $ Result :ok 1
              :tags $ #{} :unit
            %{} 'TestEntry (:name |text-payload-is-different)
              :code $ quote $ assert= false
                same-nominal-absence-fields $ NominalAbsenceFields :number (Option :none) :text (Option :some |x) :result $ Result :ok 1
              :tags $ #{} :unit
            %{} 'TestEntry (:name |result-payload-is-different)
              :code $ quote $ assert= false
                same-nominal-absence-fields $ NominalAbsenceFields :number (Option :none) :text (Option :none) :result $ Result :ok 2
              :tags $ #{} :unit
            %{} 'TestEntry (:name |result-error-is-different)
              :code $ quote $ assert= false
                same-nominal-absence-fields $ NominalAbsenceFields :number (Option :none) :text (Option :none) :result $ Result :err |error
              :tags $ #{} :unit
        'same-type-second $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn same-type-second (left right) right
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'T)
            :args $ [] 'T 'T
            :generics $ [] 'T
          :tests $ []
            %{} 'TestEntry (:name |empty-option-before-payload)
              :code $ quote $ assert= |x
                .unwrap $ same-type-second (Option :none) (Option :some |x)
              :tags $ #{} :bottom :generic :unit
            %{} 'TestEntry (:name |empty-option-after-payload)
              :code $ quote $ assert= true
                .none? $ same-type-second (Option :some |x) (Option :none)
              :tags $ #{} :bottom :generic :unit
            %{} 'TestEntry (:name |empty-list-before-elements)
              :code $ quote $ let
                  selected $ same-type-second ([]) ([] 1)
                assert-type selected $ :: 'List 'Number
                assert= ([] 1) selected
              :tags $ #{} :bottom :generic :unit
            %{} 'TestEntry (:name |local-empty-option-before-payload)
              :code $ quote $ let
                  choose $ fn (left right)
                    hint-fn $ {}
                      :generics $ [] 'T
                      :args $ [] 'T 'T
                      :return 'T
                    , right
                assert= |x $ .unwrap $ choose (Option :none) (Option :some |x)
              :tags $ #{} :bottom :generic :unit
            %{} 'TestEntry (:name |variadic-equality-bottom-refinement)
              :code $ quote $ do
                assert= false $ = (Option :none) (Option :some 1)
                assert= false $ = (Option :some 1) (Option :none)
                assert= false $ = (Option :none) (Option :none) (Option :some 1)
                assert= false $ = ([]) ([] 1)
              :tags $ #{} :bottom :generic :unit
            %{} 'TestEntry (:name |local-variadic-bottom-refinement)
              :code $ quote $ let
                  check $ fn (left & right)
                    hint-fn $ {}
                      :generics $ [] 'T
                      :args $ [] 'T
                      :rest 'T
                      :return 'Unit
                    , &unit
                assert= &unit $ check (Option :none) (Option :some |x) (Option :none)
              :tags $ #{} :bottom :generic :unit
            %{} 'TestEntry (:name |nil-remains-a-concrete-generic)
              :code $ quote $ assert= nil (same-type-second nil nil)
              :tags $ #{} :generic :unit
            %{} 'TestEntry (:name |variadic-nil-remains-homogeneous)
              :code $ quote $ let
                  check $ fn (left & right)
                    hint-fn $ {}
                      :generics $ [] 'T
                      :args $ [] 'T
                      :rest 'T
                      :return 'Unit
                    , &unit
                assert= true $ = nil nil nil
                assert= &unit $ check nil nil nil
              :tags $ #{} :generic :unit
        'slice-with-contract $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn slice-with-contract (value start end) (slice value start end)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'C)
            :args $ [] 'C 'Number $ :: 'Option 'Number
            :generics $ [] 'C
            :where $ {} $ 'C 'Sliceable
          :tests $ []
            %{} 'TestEntry (:name |custom-omitted-end)
              :code $ quote $ let
                  source $ SliceReceiverWithMethods :n 7 :end $ Option :some 9
                  result $ slice-with-contract source 2
                assert= 9 $ :n result
                assert= (Option :none) (:end result)
              :tags $ #{} :sliceable-option :unit
            %{} 'TestEntry (:name |custom-specified-end)
              :code $ quote $ let
                  source $ SliceReceiverWithMethods :n 7 :end $ Option :none
                  result $ slice-with-contract source 2 $ Option :some 5
                assert= 9 $ :n result
                assert= (Option :some 5) (:end result)
              :tags $ #{} :sliceable-option :unit
            %{} 'TestEntry (:name |custom-method-and-trait)
              :code $ quote $ let
                  source $ SliceReceiverWithMethods :n 7 :end $ Option :some 9
                  method-result $ .slice source 2
                  trait-result $ &trait-call Sliceable :slice source 2 $ Option :some 5
                  trait-omitted $ &trait-call Sliceable :slice source 2
                assert= 9 $ :n method-result
                assert= (Option :none) (:end method-result)
                assert= (Option :some 5) (:end trait-result)
                assert= (Option :none) (:end trait-omitted)
              :tags $ #{} :sliceable-option :unit
            %{} 'TestEntry (:name |custom-function-reference)
              :code $ quote $ let
                  source $ SliceReceiverWithMethods :n 7 :end $ Option :some 9
                  forward slice-with-contract
                  result $ forward source 2
                assert= 9 $ :n result
                assert= (Option :none) (:end result)
              :tags $ #{} :sliceable-option :unit
        'spread-first-rest $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn spread-first-rest (items) (first-rest-value |prefix & items)
          :examples $ []
          :schema $ :: 'Fn $ {}
            :args $ [] $ :: 'List 'String
            :return $ :: 'Option 'String
        'test-count-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-count-inference ()
            assert-type
              count $ [] 1 2 3
              , 'Number
            assert-type (count |abc) 'Number
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'test-filter-map-kv-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-filter-map-kv-inference ()
            let
                output $ filter-map-kv
                  {} (|a 1) (|b 2) (|c 3)
                  fn (k v)
                    if (> v 1)
                      %:: MapEntryDecision :keep (str k |!) (* v 10)
                      %:: MapEntryDecision :drop
              assert-type output $ :: 'Map 'String 'Number
              &inspect-type output
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-fn-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-fn-inference ()
            let
                f $ fn (x) (+ x 1)
              hint-fn f $ {}
                :args $ [] 'Number
                :return 'Number
              assert-type (f 1) 'Number
              assert-type
                apply f $ [] 1
                , 'Number
              &inspect-type f
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-generics-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-generics-identity ()
            let
                n $ identity 42
                s $ identity |hello
                applied-number $ apply identity $ [] 42
                applied-string $ apply identity $ [] |hello
              assert-type n 'Number
              assert-type s 'String
              assert-type applied-number 'Number
              assert-type applied-string 'String
              &inspect-type n
              &inspect-type s
              match (parse-float |41)
                (:ok value)
                  assert= 42 $ + value 1
                (:err error) (raise error)
              match (parse-float |bad)
                (:ok _) (raise |Expected-parse-error)
                (:err error)
                  assert= true $ starts-with? error |b
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-legacy-optional-inference $ %{} 'CodeEntry
          :doc "|Legacy-contract test: pins assert-type against Optional; retire with Optional (0.32.0). The Option forms are asserted first."
          :code $ quote $ defn test-legacy-optional-inference ()
            let
                opt-some $ Option :some 10
                opt-none $ Option :none
              assert-type opt-some $ :: 'Option 'Number
              assert-type opt-none $ :: 'Option 'Number
              &inspect-type opt-some
              &inspect-type opt-none
            let
                opt 10
              assert-type opt $ :: 'Optional 'Number
              &inspect-type opt
            let
                opt-nil nil
              assert-type opt-nil $ :: 'Optional 'String
              &inspect-type opt-nil
            let
                xs $ [] 1 2
                empty-xs $ empty xs
              assert-type empty-xs $ :: 'List 'Number
              &inspect-type empty-xs
            let
                tail $ rest |abc
              assert-type tail 'String
              &inspect-type tail
            let
                first-char $ &str:first |abc
                missing-first $ &str:first |
              assert-type first-char $ :: 'Option 'String
              assert-type missing-first $ :: 'Option 'String
              &inspect-type first-char
              &inspect-type missing-first
            let
                list-first $ first $ [] 1 2 3
                string-first $ first |abc
                empty-list-first $ first $ []
                empty-str-first $ first |
              assert-type list-first $ :: 'Option 'Number
              assert-type string-first $ :: 'Option 'String
              assert-type empty-list-first $ :: 'Option 'Dynamic
              assert-type empty-str-first $ :: 'Option 'String
              &inspect-type list-first
              &inspect-type string-first
              &inspect-type empty-list-first
              &inspect-type empty-str-first
            let
                list-last $ last $ [] 1 2 3
                empty-list-last $ last $ []
              assert-type list-last $ :: 'Option 'Number
              assert-type empty-list-last $ :: 'Option 'Dynamic
              &inspect-type list-last
              &inspect-type empty-list-last
            let
                nth-char $ &str:nth |abc 1
                missing-char $ &str:nth |abc 9
              assert-type nth-char $ :: 'Option 'String
              assert-type missing-char $ :: 'Option 'String
              &inspect-type nth-char
              &inspect-type missing-char
            let
                raw-hit-index $ &str:find-index |abc |b
                raw-miss-index $ &str:find-index |abc |z
                hit-index $ str-find-index |abc |b
                miss-index $ str-find-index |abc |z
              assert= 1 raw-hit-index
              assert= -1 raw-miss-index
              assert= (%some 1) hit-index
              assert= (%none) miss-index
              assert-type raw-hit-index 'Number
              assert-type raw-miss-index 'Number
              assert-type hit-index $ :: 'Option 'Number
              assert-type miss-index $ :: 'Option 'Number
              &inspect-type raw-hit-index
              &inspect-type raw-miss-index
              &inspect-type hit-index
              &inspect-type miss-index
            let
                parsed-ok $ parse-float |1.5
                parsed-bad $ parse-float |oops
              assert= (%ok 1.5) parsed-ok
              assert= (%err |oops) parsed-bad
              assert-type parsed-ok $ :: 'Result 'Number 'String
              assert-type parsed-bad $ :: 'Result 'Number 'String
              &inspect-type parsed-ok
              &inspect-type parsed-bad
            let
                found $ find ([] 1 2 3)
                  fn (x) (> x 1)
                missing $ find ([] 1 2 3)
                  fn (x) (> x 9)
                found-index $ find-index ([] 1 2 3)
                  fn (x) (> x 1)
                missing-index $ index-of ([] 1 2 3) 9
              assert-type found $ :: 'Option 'Number
              assert-type missing $ :: 'Option 'Number
              assert-type found-index $ :: 'Option 'Number
              assert-type missing-index $ :: 'Option 'Number
              &inspect-type found
              &inspect-type missing
              &inspect-type found-index
              &inspect-type missing-index
            let
                last-hit $ .find-last ([] 1 2 3)
                  fn (x) (> x 1)
                last-index $ .find-last-index ([] 1 2 3)
                  fn (x) (> x 1)
                last-position $ .last-index-of ([] 1 2 1) 1
                list-max $ .max $ [] 1 2 3
                set-min $ .min $ #{} 1 2 3
                string-index $ .find-index |abc |b
              assert-type last-hit $ :: 'Option 'Number
              assert-type last-index $ :: 'Option 'Number
              assert-type last-position $ :: 'Option 'Number
              assert-type list-max $ :: 'Option 'Number
              assert-type set-min $ :: 'Option 'Number
              assert-type string-index $ :: 'Option 'Number
              &inspect-type last-hit
              &inspect-type last-index
              &inspect-type last-position
              &inspect-type list-max
              &inspect-type set-min
              &inspect-type string-index
            let
                list-hit $ get ([] 1 2 3) 1
                list-miss $ get ([] 1 2 3) 9
                string-hit $ get |abc 1
                map-hit $ get
                  {} $ :a 1
                  , :a
                map-miss $ get
                  {} $ :a 1
                  , :b
              assert-type list-hit $ :: 'Option 'Number
              assert-type list-miss $ :: 'Option 'Number
              assert-type string-hit $ :: 'Option 'String
              assert-type map-hit $ :: 'Option 'Number
              assert-type map-miss $ :: 'Option 'Number
              &inspect-type list-hit
              &inspect-type list-miss
              &inspect-type string-hit
              &inspect-type map-hit
              &inspect-type map-miss
            let
                nested-hit $ get-in
                  [] $ {} $ :a 1
                  [] 0 :a
                nested-miss $ get-in
                  {} $ :p $ {} (:name |n)
                  [] :p :age
                nil-nested $ get-in nil $ [] :a
              assert= (Result :ok 1) nested-hit
              assert=
                Result :err $ PathError :missing
                , nested-miss
              assert=
                Result :err $ PathError :missing
                , nil-nested
              assert-type nested-hit $ :: 'Result 'Number 'PathError
              assert-type nested-miss $ :: 'Result 'Dynamic 'PathError
              assert-type nil-nested $ :: 'Result 'Dynamic 'PathError
              &inspect-type nested-hit
              &inspect-type nested-miss
              &inspect-type nil-nested
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-list-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-list-inference ()
            let
                nested $ [] ([] 1 2) ([] 3)
              assert-type nested $ :: 'List $ :: 'List 'Number
              &inspect-type nested
              let
                  inner $ &list:nth nested 0
                assert-type inner $ :: 'List 'Number
                &inspect-type inner
              let
                  val $ &list:nth ([] 1 2 3) 0
                assert-type val 'Number
                &inspect-type val
              let
                  xs $ [] 1 2 3
                  rest-xs $ rest xs
                assert-type rest-xs $ :: 'List 'Number
                &inspect-type rest-xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-map-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-map-inference ()
            let
                m $ {}
              assert-type m $ :: 'Map 'String 'Number
              let
                  m2 $ &map:assoc m |b 2
                  m3 $ &map:dissoc m2 |a
                  m4 $ &map:get m2 |b
                  m5 $ merge m2 $ {} (|c 3)
                &inspect-type m2
                &inspect-type m3
                &inspect-type m4
                assert-type m5 $ :: 'Map 'String 'Number
                &inspect-type m5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-ref-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-ref-inference ()
            let
                r $ atom 1
              assert-type r $ :: 'Ref 'Number
              let
                  x $ &atom:deref r
                assert-type x 'Number
                &inspect-type r
                &inspect-type x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-set-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-set-inference ()
            let
                s $ #{}
              assert-type s $ :: 'Set 'Number
              let
                  xs $ &set:to-list s
                &inspect-type s
                &inspect-type xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-struct-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-struct-inference ()
            let
                addr $ %{} Address $ :city |sh
                p $ %{} Person (:name |n) (:age 20) (:address addr)
              assert-type p $ :: Person
              &inspect-type p
              let
                  name-v $ :name p
                assert-type name-v String
                &inspect-type name-v
              let
                  top-name-v $ :name p
                  city-v $ :city $ :address p
                assert-type top-name-v String
                assert-type city-v String
                &inspect-type top-name-v
                &inspect-type city-v
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'test-type-ref-combos $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-type-ref-combos ()
            let
                addr $ %{} Address $ :city |sh
                person $ %{} Person (:name |n) (:age 20) (:address addr)
                job $ %{} Job (:title |dev)
                  :status $ %:: Status :ok 1
              assert-type person $ :: Person
              assert-type job $ :: Job
              &inspect-type person
              &inspect-type job
              let
                  wrapped $ %:: PersonWrap :person person
                  outcome $ %:: Outcome :status $ %:: Status :ok 2
                assert-type wrapped PersonWrap
                assert-type outcome Outcome
                &inspect-type wrapped
                &inspect-type outcome
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Nil)
            :args $ []
        'unit-value-as-annotation $ %{} 'CodeEntry
          :doc "||Calcit::Unit used as a type annotation resolves to Unit, not an opaque Custom value (#1553, #1554)."
          :code $ quote $ defn unit-value-as-annotation ()
            let
                u &unit
              assert-type u &unit
              , u
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |unit-value-annotation-proves-unit)
            :code $ quote $ assert= &unit (unit-value-as-annotation)
            :tags $ #{} :unit
        'write-nominal-absence-fields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn write-nominal-absence-fields (fields number text result)
            -> fields
              assoc :number $ Option :some number
              assoc :text $ Option :some text
              assoc :result result
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'test-types-inference.main/NominalAbsenceFields)
            :args $ [] 'test-types-inference.main/NominalAbsenceFields 'Number 'String $ :: 'Result 'Number 'String
          :tests $ []
            %{} 'TestEntry (:name |option-and-result-fields)
              :code $ quote $ let
                  original $ NominalAbsenceFields :number (Option :none) :text (Option :none) :result $ Result :ok 0
                  changed $ write-nominal-absence-fields original 7 |ready $ Result :err |failed
                assert= 7 $ .unwrap $ :number changed
                assert= |ready $ .unwrap $ :text changed
                assert= |failed $ match (:result changed)
                  (:err message) message
                  (:ok _) |unexpected
              :tags $ #{} :unit
            %{} 'TestEntry (:name |method-field-write)
              :code $ quote $ let
                  original $ NominalAbsenceFields :number (Option :none) :text (Option :none) :result $ Result :ok 0
                  changed $ .assoc original :text $ Option :some |method
                assert= |method $ .unwrap $ :text changed
              :tags $ #{} :unit
            %{} 'TestEntry (:name |collection-values)
              :code $ quote $ let
                  value $ Option :some 3
                  values $ [] $ Option :some 1
                  mapping $ {} $ :number (Option :some 1)
                  changed-list $ assoc values 0 value
                  changed-map $ assoc mapping :number value
                assert= ([] value) changed-list
                assert=
                  {} $ :number value
                  , changed-map
              :tags $ #{} :unit
            %{} 'TestEntry (:name |nested-collection-value)
              :code $ quote $ let
                  value $ Option :some 3
                  mapping $ {} $ :number (Option :some 1)
                  changed $ assoc-in
                    {} $ :branch mapping
                    [] :branch :number
                    , value
                assert=
                  {} $ :branch $ {} (:number value)
                  , changed
              :tags $ #{} :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-types-inference.main
    'test-types-inference.shadow $ %{} 'FileEntry
      :defs $ {}
        'Option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Option ([] 'T) (:none) (:some 'T)
          :examples $ []
          :schema $ :: 'EnumDef
          :tests $ [] $ %{} 'TestEntry (:name |same-name-is-not-core-option)
            :code $ quote $ assert= false
              test-types-inference.shadow/foreign-option-is-nil $ test-types-inference.shadow/Option :none
            :tags $ #{} :unit
        'Result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Result (:ok 'Number) (:err 'String)
          :examples $ []
          :schema $ :: 'EnumDef
          :tests $ [] $ %{} 'TestEntry (:name |same-name-is-not-core-result)
            :code $ quote $ assert= false
              test-types-inference.shadow/foreign-result-is-nil $ test-types-inference.shadow/Result :ok 1
            :tags $ #{} :unit
        'ShadowFields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ShadowFields
            :option $ :: 'test-types-inference.shadow/Option 'Number
            :other-option $ :: 'test-types-inference.shadow/Option 'Number
            :result 'test-types-inference.shadow/Result
            :other-result 'test-types-inference.shadow/Result
          :examples $ []
          :schema $ :: 'StructDef
        'foreign-option-is-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn foreign-option-is-nil (value)
            assert-type value $ :: 'Option 'Number
            nil? value
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] $ :: 'test-types-inference.shadow/Option 'Number
        'foreign-result-is-nil $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn foreign-result-is-nil (value) (assert-type value 'Result) (nil? value)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'test-types-inference.shadow/Result
        'same-shadow-fields $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn same-shadow-fields (fields)
            let
                expected-option $ :other-option fields
                expected-result $ :other-result fields
              assert-type expected-option $ :: 'Option 'Number
              assert-type expected-result 'Result
              and
                = expected-option $ :option fields
                = expected-result $ :result fields
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'test-types-inference.shadow/ShadowFields
          :tests $ []
            %{} 'TestEntry (:name |same-shadow-nominal-fields)
              :code $ quote $ assert= true
                test-types-inference.shadow/same-shadow-fields $ test-types-inference.shadow/ShadowFields :option (test-types-inference.shadow/Option :some 7) :other-option (test-types-inference.shadow/Option :some 7) :result (test-types-inference.shadow/Result :ok 1) :other-result $ test-types-inference.shadow/Result :ok 1
              :tags $ #{} :unit
            %{} 'TestEntry (:name |different-shadow-payload)
              :code $ quote $ assert= false
                test-types-inference.shadow/same-shadow-fields $ test-types-inference.shadow/ShadowFields :option (test-types-inference.shadow/Option :some 8) :other-option (test-types-inference.shadow/Option :some 7) :result (test-types-inference.shadow/Result :ok 1) :other-result $ test-types-inference.shadow/Result :ok 1
              :tags $ #{} :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-types-inference.shadow
